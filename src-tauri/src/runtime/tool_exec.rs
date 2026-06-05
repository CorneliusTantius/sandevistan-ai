use crate::{
    ai,
    extensions::hooks::{HookDecision, HookEvent},
    subagent, tools,
};
use std::{path::PathBuf, time::Duration};

use super::{budget::AgentBudgets, tool_validation::validate_tool_call};

pub async fn run_streamed_tool_call(
    workspace: PathBuf,
    call: tools::ToolCall,
    mods: ai::ModelMods,
    budgets: &AgentBudgets,
    read_only: bool,
    delegate_depth_remaining: usize,
) -> String {
    let name = call.name.clone();

    let mut call = match validate_tool_call(call, &mods, read_only) {
        Ok(validated) => validated.call,
        Err(error) => return failed_tool_content(&name, &format!("invalid tool call: {error}")),
    };

    let mut modified = false;
    for decision in crate::extensions::hook_bus(&workspace).emit(HookEvent::BeforeToolCall {
        tool: call.name.clone(),
        args: call.args.clone(),
    }) {
        match decision {
            HookDecision::Block { reason } => return failed_tool_content(&name, &reason),
            HookDecision::ModifyToolArgs { args } => {
                call.args = args;
                modified = true;
            }
            HookDecision::Continue | HookDecision::AppendSystemContext { .. } => {}
        }
    }
    if modified {
        call = match validate_tool_call(call, &mods, read_only) {
            Ok(validated) => validated.call,
            Err(error) => {
                return failed_tool_content(&name, &format!("modified tool call invalid: {error}"))
            }
        };
    }

    let output = run_tool_call(
        workspace,
        call,
        mods,
        budgets.tool_timeout,
        delegate_depth_remaining,
    )
    .await;
    format!("{name}\n{output}")
}

fn failed_tool_content(name: &str, error: &str) -> String {
    format!(
        "{name}\nstatus: failed\nerror: {error}\nnote: tool failed; do not repeat identical call. answer with current evidence or try a different tool/query."
    )
}

async fn run_tool_call(
    workspace: PathBuf,
    call: tools::ToolCall,
    mods: ai::ModelMods,
    timeout: Duration,
    delegate_depth_remaining: usize,
) -> String {
    if subagent::is_delegate(&call) {
        if delegate_depth_remaining == 0 {
            return "status: failed\nerror: delegate depth exhausted".into();
        }
        return subagent::run_delegate_depth(
            workspace,
            call.args,
            mods.clone(),
            mods.rtk_enabled,
            delegate_depth_remaining - 1,
        )
        .await;
    }
    run_tool_blocking(
        workspace,
        call,
        mods.rtk_enabled,
        mods.shell_enabled,
        timeout,
    )
    .await
}

async fn run_tool_blocking(
    workspace: PathBuf,
    call: tools::ToolCall,
    rtk_enabled: bool,
    shell_enabled: bool,
    timeout: Duration,
) -> String {
    match tokio::time::timeout(
        timeout,
        tauri::async_runtime::spawn_blocking(move || {
            tools::run_with_options(
                &workspace,
                &call,
                tools::ToolOptions {
                    rtk_enabled,
                    shell_enabled,
                },
            )
        }),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => format!("status: failed\nerror: tool task failed: {error}"),
        Err(_) => format!(
            "status: failed\nerror: tool execution timed out after {}s",
            timeout.as_secs()
        ),
    }
}
