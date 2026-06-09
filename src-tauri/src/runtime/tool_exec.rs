use crate::{ai, extensions, mcp, runtime_wire::NativeToolSpec, subagent, tools};
use sandevistan_core::{
    tools::{ToolFuture, ToolHost, ToolRequest},
    AgentMods,
};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct AppToolHost {
    workspace: PathBuf,
    mods: AgentMods,
}

impl AppToolHost {
    pub fn new(workspace: PathBuf, mods: AgentMods) -> Arc<Self> {
        Arc::new(Self { workspace, mods })
    }
}

impl ToolHost for AppToolHost {
    fn system_prompt(&self) -> String {
        let mut prompt = tools::native_system_prompt(
            self.mods.subagents_enabled && !self.mods.subagents.is_empty(),
            &self.mods.subagents,
            self.mods.shell_enabled,
        );
        let extension_prompt = extensions::system_prompt(&self.workspace);
        if !extension_prompt.trim().is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&extension_prompt);
        }
        prompt
    }

    fn specs(&self) -> Vec<NativeToolSpec> {
        let mut specs = tools::ToolRegistry::new(
            self.mods.subagents_enabled && !self.mods.subagents.is_empty(),
            &self.mods.subagents,
            self.mods.shell_enabled,
            false,
        )
        .specs();
        if self.mods.mcp_enabled {
            specs.extend(mcp::tool_specs());
        }
        specs.extend(extensions::tool_specs(&self.workspace));
        specs
    }

    fn run<'a>(&'a self, request: ToolRequest) -> ToolFuture<'a> {
        Box::pin(async move { self.run_inner(request).await })
    }
}

impl AppToolHost {
    async fn run_inner(&self, request: ToolRequest) -> String {
        let call = tools::ToolCall {
            name: request.name,
            args: request.args,
        };
        run_streamed_tool_call(
            self.workspace.clone(),
            request.session_id,
            call,
            self.mods.clone(),
            request.read_only,
            request.delegate_depth_remaining,
        )
        .await
    }
}

pub async fn run_streamed_tool_call(
    workspace: PathBuf,
    session_id: String,
    call: tools::ToolCall,
    mods: ai::ModelMods,
    read_only: bool,
    delegate_depth_remaining: usize,
) -> String {
    let name = call.name.clone();
    let call = normalize_tool_call(call);

    let mut call = if extensions::is_extension_tool(&call.name) {
        if !call.args.is_object() {
            return failed_tool_content(&name, "invalid tool call: args must be a JSON object");
        }
        call
    } else {
        match validate_tool_call(call, &mods, read_only) {
            Ok(validated) => validated.call,
            Err(error) => return failed_tool_content(&name, &format!("invalid tool call: {error}")),
        }
    };

    let mut modified = false;
    for decision in extensions::hook_bus(&workspace).emit(extensions::hooks::HookEvent::BeforeToolCall {
        tool: call.name.clone(),
        args: call.args.clone(),
    }) {
        match decision {
            extensions::hooks::HookDecision::Block { reason } => return failed_tool_content(&name, &reason),
            extensions::hooks::HookDecision::ModifyToolArgs { args } => {
                call.args = args;
                modified = true;
            }
            extensions::hooks::HookDecision::Continue | extensions::hooks::HookDecision::AppendSystemContext { .. } => {}
        }
    }
    if modified && !extensions::is_extension_tool(&call.name) {
        call = match validate_tool_call(call, &mods, read_only) {
            Ok(validated) => validated.call,
            Err(error) => return failed_tool_content(&name, &format!("modified tool call invalid: {error}")),
        };
    }

        let output = if mcp::is_mcp_tool(&call.name) {
        mcp::run(&workspace, call, &mods.mcp_config).await
    } else if extensions::is_extension_tool(&call.name) {
        extensions::execute_tool(&workspace, &call.name, call.args)
    } else {
        run_builtin_tool(workspace.clone(), session_id, call, mods, delegate_depth_remaining).await
    };
    extensions::hook_bus(&workspace).emit(extensions::hooks::HookEvent::AfterToolResult {
        tool: name.clone(),
        content: output.clone(),
    });
    format!("{name}\n{output}")
}

fn normalize_tool_call(mut call: tools::ToolCall) -> tools::ToolCall {
    if let Some(name) = tools::original_tool_name(&call.name)
        .map(str::to_string)
        .or_else(|| mcp::original_tool_name(&call.name).map(str::to_string))
        .or_else(|| extensions::original_tool_name(&call.name))
    {
        call.name = name;
    }
    call
}

fn validate_tool_call(
    call: tools::ToolCall,
    mods: &ai::ModelMods,
    read_only: bool,
) -> Result<tools::ValidatedToolCall, String> {
    if mcp::is_mcp_tool(&call.name) {
        mcp::validate_tool_call(&call, mods)?;
        return Ok(tools::ValidatedToolCall { call });
    }
    tools::ToolRegistry::new(
        mods.subagents_enabled && !mods.subagents.is_empty(),
        &mods.subagents,
        mods.shell_enabled,
        read_only,
    )
    .validate(call, mods)
}

fn failed_tool_content(name: &str, error: &str) -> String {
    format!(
        "{name}\nstatus: failed\nerror: {error}\nnote: tool failed; do not repeat identical call. answer with current evidence or try a different tool/query."
    )
}

async fn run_builtin_tool(
    workspace: PathBuf,
    session_id: String,
    call: tools::ToolCall,
    mods: ai::ModelMods,
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
    tokio::task::spawn_blocking(move || {
        tools::run_with_options(
            &workspace,
            &call,
            tools::ToolOptions {
                rtk_enabled: mods.rtk_enabled,
                shell_enabled: mods.shell_enabled,
                backup_session_id: Some(session_id),
            },
        )
    })
    .await
    .unwrap_or_else(|error| format!("status: failed\nerror: tool task failed: {error}"))
}
