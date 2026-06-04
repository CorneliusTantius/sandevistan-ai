use crate::{
    ai::{self, ChatMessage, ModelMods, SubagentDef},
    tools::{self, ToolCall, ToolOptions},
};
use serde::Deserialize;
use serde_json::Value;
use std::{future::Future, path::PathBuf, pin::Pin, time::Duration};

const MAX_TASKS: usize = 4;
const MAX_TASK_CHARS: usize = 4_000;
const MAX_RESULT_CHARS: usize = 6_000;
const MAX_TOOL_LOOPS: usize = 3;
const DEFAULT_MAX_DELEGATE_DEPTH: usize = 1;
const SUBAGENT_TIMEOUT: Duration = Duration::from_secs(180);
const SUBAGENT_RETRIES: usize = 3;
const SUBAGENT_RETRY_DELAY: Duration = Duration::from_secs(1);
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

pub async fn run_delegate(
    workspace: PathBuf,
    args: Value,
    mods: ModelMods,
    rtk_enabled: bool,
) -> String {
    run_delegate_depth(
        workspace,
        args,
        mods,
        rtk_enabled,
        DEFAULT_MAX_DELEGATE_DEPTH,
    )
    .await
}

fn run_delegate_depth(
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
                handles.push(tauri::async_runtime::spawn(async move {
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

    for retry in 0..=SUBAGENT_RETRIES {
        let future = run_one_inner(
            workspace.clone(),
            &def,
            &task,
            &mods,
            rtk_enabled,
            depth_remaining,
        );
        match tokio::time::timeout(SUBAGENT_TIMEOUT, future).await {
            Ok(Ok(output)) => {
                return format!(
                    "[subagent {}]\nstatus: ok\n{}",
                    def.name,
                    truncate(&output, def.max_result_chars.unwrap_or(MAX_RESULT_CHARS))
                )
            }
            Ok(Err(error)) => {
                return format!("[subagent {}]\nstatus: failed\nerror: {error}", def.name)
            }
            Err(_) if retry < SUBAGENT_RETRIES => {
                tokio::time::sleep(SUBAGENT_RETRY_DELAY).await;
            }
            Err(_) => {
                return format!(
                    "[subagent {}]\nstatus: failed\nerror: timed out after {}s after {} retries",
                    def.name,
                    SUBAGENT_TIMEOUT.as_secs(),
                    SUBAGENT_RETRIES
                )
            }
        }
    }

    format!("[subagent {}]\nstatus: failed\nerror: timed out", def.name)
}

async fn run_one_inner(
    workspace: PathBuf,
    def: &SubagentDef,
    task: &DelegateTask,
    mods: &ModelMods,
    rtk_enabled: bool,
    depth_remaining: usize,
) -> Result<String, String> {
    let task_text = truncate(&task.task, MAX_TASK_CHARS);
    let tools_prompt = tools::prompt_with_subagents(
        depth_remaining > 0 && mods.subagents_enabled && !mods.subagents.is_empty(),
        &mods.subagents,
        false,
    );
    let delegation_rule = if depth_remaining > 0 {
        "- may delegate to subagents when useful; remaining delegate depth: 1"
    } else {
        "- do not delegate; delegate depth exhausted"
    };
    let system = format!(
        "{}\n\nSubagent role: {}\n{}\nRules:\n- concise findings only\n- use read/search/git tools when needed\n- do not edit files\n{}\n- return final answer only",
        tools_prompt,
        def.name,
        def.system.trim(),
        delegation_rule
    );
    let mut messages = vec![
        ChatMessage {
            role: "system".into(),
            content: system,
        },
        ChatMessage {
            role: "user".into(),
            content: format!("Task:\n{task_text}"),
        },
    ];
    let model = def
        .model
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| (!mods.subagent_model.trim().is_empty()).then(|| mods.subagent_model.clone()))
        .or_else(|| Some(mods.main_model.clone()));

    for _ in 0..MAX_TOOL_LOOPS {
        let content = ai::complete_chat_model(messages.clone(), model.clone()).await?;
        let calls = tools::parse_tool_calls(&content);
        if calls.is_empty() {
            return Ok(content.trim().to_string());
        }

        for call in calls {
            let name = call.name.clone();
            let output = if is_delegate(&call) {
                if depth_remaining == 0 {
                    "status: failed\nerror: delegate depth exhausted".into()
                } else {
                    Box::pin(run_delegate_depth(
                        workspace.clone(),
                        call.args,
                        mods.clone(),
                        rtk_enabled,
                        depth_remaining - 1,
                    ))
                    .await
                }
            } else {
                run_read_tool(workspace.clone(), call, rtk_enabled).await
            };
            messages.push(ChatMessage {
                role: "tool".into(),
                content: format!("{name}\n{output}"),
            });
        }
    }

    Err("tool loop limit reached".into())
}

async fn run_read_tool(workspace: PathBuf, call: ToolCall, rtk_enabled: bool) -> String {
    if tools::is_mutating(&call) {
        return "status: failed\nerror: subagents are read-only".into();
    }

    match tokio::time::timeout(
        TOOL_TIMEOUT,
        tauri::async_runtime::spawn_blocking(move || {
            tools::run_with_options(
                &workspace,
                &call,
                ToolOptions {
                    rtk_enabled,
                    shell_enabled: false,
                },
            )
        }),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => format!("status: failed\nerror: subagent tool task failed: {error}"),
        Err(_) => format!(
            "status: failed\nerror: subagent tool timed out after {}s",
            TOOL_TIMEOUT.as_secs()
        ),
    }
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
