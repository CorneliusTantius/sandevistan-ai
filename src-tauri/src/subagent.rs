use crate::{
    ai::{ChatMessage, ModelMods, SubagentDef},
    tools::{self, ToolCall},
};
use sandevistan_core::PromptConfig;
use serde::Deserialize;
use serde_json::Value;
use std::{future::Future, path::PathBuf, pin::Pin, time::Duration};

const MAX_TASKS: usize = 8;
const MAX_TASK_CHARS: usize = 1_000;
const MAX_RESULT_CHARS: usize = 6_000;
const SUBAGENT_TIMEOUT: Duration = Duration::from_secs(180);
const SUBAGENT_STAGGER: Duration = Duration::from_millis(500);
const TOOL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize)]
struct DelegateArgs {
    #[serde(default, alias = "agents")]
    tasks: Vec<DelegateTask>,
}

#[derive(Debug, Clone, Deserialize)]
struct DelegateTask {
    agent: String,
    task: String,
}

pub fn is_delegate(call: &ToolCall) -> bool {
    call.name == "agent.delegate"
}

pub(crate) fn run_delegate_depth(
    workspace: PathBuf,
    args: Value,
    mods: ModelMods,
    rtk_enabled: bool,
    depth_remaining: usize,
) -> Pin<Box<dyn Future<Output = String> + Send>> {
    Box::pin(async move {
        if !mods.subagents_enabled {
            return "status: failed\nerror: subagents disabled in mods".into();
        }

        let request = match serde_json::from_value::<DelegateArgs>(args) {
            Ok(value) => value,
            Err(error) => return format!("status: failed\nerror: invalid delegate args: {error}"),
        };

        let tasks = request
            .tasks
            .into_iter()
            .take(MAX_TASKS)
            .collect::<Vec<_>>();
        if tasks.is_empty() {
            return "status: failed\nerror: no subagent tasks".into();
        }

        let (defs, config_note) = configured_subagent_defs(&mods);
        let concurrency = mods.subagent_max_concurrency.clamp(1, MAX_TASKS);
        let mut results = Vec::new();

        for batch in tasks.chunks(concurrency) {
            let mut handles = Vec::new();
            for (batch_index, task) in batch.iter().cloned().enumerate() {
                let workspace = workspace.clone();
                let defs = defs.clone();
                let mods = mods.clone();
                handles.push(tokio::spawn(async move {
                    if batch_index > 0 {
                        tokio::time::sleep(SUBAGENT_STAGGER * batch_index as u32).await;
                    }
                    let output =
                        run_one(workspace, defs, task, mods, rtk_enabled, depth_remaining).await;
                    (batch_index, output)
                }));
            }

            let mut batch_results = Vec::new();
            for handle in handles {
                if let Ok(result) = handle.await {
                    batch_results.push(result);
                }
            }
            batch_results.sort_by_key(|(index, _)| *index);
            results.extend(batch_results.into_iter().map(|(_, output)| output));
        }

        let mut output = String::from("status: ok");
        if let Some(note) = config_note {
            output.push_str(&format!("\nconfig: {note}"));
        }
        output.push_str("\n");
        output.push_str(&results.join("\n\n"));
        output
    })
}

async fn run_one(
    workspace: PathBuf,
    defs: Vec<SubagentDef>,
    task: DelegateTask,
    mods: ModelMods,
    rtk_enabled: bool,
    depth_remaining: usize,
) -> String {
    let agent_name = compact(&task.agent, 64);
    let Some(def) = defs.iter().find(|def| def.name == task.agent).cloned() else {
        return format!(
            "[subagent {agent_name}]\nstatus: failed\nerror: unknown subagent; available: {}",
            defs.iter()
                .map(|def| def.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };

    let future = run_one_inner(
        workspace.clone(),
        &def,
        &task,
        &mods,
        rtk_enabled,
        depth_remaining,
    );
    match tokio::time::timeout(SUBAGENT_TIMEOUT, future).await {
        Ok(Ok(output)) => format!(
            "[subagent {}]\nstatus: ok\n{}",
            def.name,
            truncate(&output, def.max_result_chars.unwrap_or(MAX_RESULT_CHARS))
        ),
        Ok(Err(error)) => format!("[subagent {}]\nstatus: failed\nerror: {error}", def.name),
        Err(_) => format!(
            "[subagent {}]\nstatus: failed\nerror: timed out after {}s",
            def.name,
            SUBAGENT_TIMEOUT.as_secs()
        ),
    }
}

async fn run_one_inner(
    workspace: PathBuf,
    def: &SubagentDef,
    task: &DelegateTask,
    mods: &ModelMods,
    _rtk_enabled: bool,
    depth_remaining: usize,
) -> Result<String, String> {
    let task_text = truncate(&task.task, MAX_TASK_CHARS);
    let subagents_available =
        depth_remaining > 0 && mods.subagents_enabled && !mods.subagents.is_empty();
    let tools_prompt =
        tools::native_system_prompt(subagents_available, &mods.subagents, mods.shell_enabled);
    let delegation_rule = if depth_remaining > 0 {
        "- may delegate to subagents when useful; remaining delegate depth: 1"
    } else {
        "- do not delegate; delegate depth exhausted"
    };
    let system = format!(
        "{}\n\nSubagent role: {}\n{}\nRules:\n- concise findings only\n- use tools when needed\n- strictly use targeted commands; inspect specific files/paths/symbols/line ranges or narrow searches, never broad/noisy commands unless explicitly necessary\n{}\n- return final answer only",
        tools_prompt,
        def.name,
        def.system.trim(),
        delegation_rule
    );
    let model = def
        .model
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| (!mods.subagent_model.trim().is_empty()).then(|| mods.subagent_model.clone()))
        .or_else(|| Some(mods.main_model.clone()));

    let mut subagent_mods = mods.clone();
    subagent_mods.subagents_enabled = subagents_available;

    let provider = crate::ai::provider_config_for_model(model)?;
    let runtime = crate::runtime::AgentRuntime::new();
    let result = runtime
        .run(
            crate::runtime::AgentRuntimeConfig::builder()
                .session_id(format!("subagent-{}", def.name))
                .messages(vec![ChatMessage {
                    role: "user".into(),
                    content: format!("Task:\n{task_text}"),
                }])
                .mods(subagent_mods.clone())
                .prompt_config(PromptConfig::from_context_chars(40_000))
                .system_prompt(system)
                .provider(provider)
                .delegate_depth_remaining(depth_remaining)
                .budgets(crate::runtime::AgentBudgets {
                    tool_timeout: TOOL_TIMEOUT,
                })
                .cancellation_token(crate::runtime::CancellationToken::new())
                .shared_tool_host(crate::runtime::AppToolHost::new(workspace, subagent_mods))
                .build()
                .map_err(|error| error.to_string())?,
        )
        .await
        .map_err(|error| error.message)?;

    result
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .map(|message| message.content.trim().to_string())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| "subagent returned no assistant content".into())
}

fn configured_subagent_defs(mods: &ModelMods) -> (Vec<SubagentDef>, Option<String>) {
    let defs = filter_defs(mods.subagents_registry.clone(), &mods.subagents);
    if !defs.is_empty() || mods.subagents.is_empty() {
        return (defs, None);
    }
    subagent_defs(&mods.subagents_config, &mods.subagents)
}

fn subagent_defs(config: &str, selected: &[String]) -> (Vec<SubagentDef>, Option<String>) {
    let config = config.trim();
    if config.is_empty() {
        return (filter_defs(default_defs(), selected), None);
    }

    match serde_json::from_str::<Vec<SubagentDef>>(config) {
        Ok(defs) => {
            let defs = defs
                .into_iter()
                .filter(|def| !def.name.trim().is_empty() && !def.system.trim().is_empty())
                .take(MAX_TASKS)
                .collect::<Vec<_>>();
            if defs.is_empty() {
                (
                    filter_defs(default_defs(), selected),
                    Some("custom config empty; using defaults".into()),
                )
            } else {
                (filter_defs(defs, selected), None)
            }
        }
        Err(error) => (
            filter_defs(default_defs(), selected),
            Some(format!(
                "custom config parse failed; using defaults: {error}"
            )),
        ),
    }
}

fn filter_defs(defs: Vec<SubagentDef>, selected: &[String]) -> Vec<SubagentDef> {
    if selected.is_empty() {
        return Vec::new();
    }
    defs.into_iter()
        .filter(|def| selected.iter().any(|name| name == &def.name))
        .collect()
}

fn default_defs() -> Vec<SubagentDef> {
    vec![
        SubagentDef {
            name: "scout".into(),
            description: Some("Find relevant files, symbols, and facts. No edits.".into()),
            system: "Fast codebase scout. Search first, read only relevant files, return paths and findings.".into(),
            model: None,
            max_result_chars: Some(4_000),
        },
        SubagentDef {
            name: "reviewer".into(),
            description: Some("Review risks, bugs, and missing tests. No edits.".into()),
            system: "Strict senior reviewer. Return only concrete risks, bugs, and fixes.".into(),
            model: None,
            max_result_chars: Some(4_000),
        },
        SubagentDef {
            name: "planner".into(),
            description: Some("Create small safe implementation steps. No edits.".into()),
            system: "Careful planner. Return current state, target state, steps, validation, rollback.".into(),
            model: None,
            max_result_chars: Some(4_000),
        },
        SubagentDef {
            name: "worker".into(),
            description: Some("Bounded implementation analysis. Read-only in MVP.".into()),
            system: "Implementation worker. Inspect code and propose exact minimal changes. Do not edit files.".into(),
            model: None,
            max_result_chars: Some(6_000),
        },
    ]
}

fn compact(value: &str, max: usize) -> String {
    truncate(&value.split_whitespace().collect::<Vec<_>>().join(" "), max)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut output = value.chars().take(max).collect::<String>();
    output.push_str("…");
    output
}
