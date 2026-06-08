use crate::{ai, mcp, tools};

pub type ValidatedToolCall = tools::ValidatedToolCall;

pub fn validate_tool_call(
    call: tools::ToolCall,
    mods: &ai::ModelMods,
    read_only: bool,
) -> Result<ValidatedToolCall, String> {
    if mcp::is_mcp_tool(&call.name) {
        mcp::validate_tool_call(&call, mods)?;
        return Ok(ValidatedToolCall { call });
    }
    tools::ToolRegistry::new(
        mods.subagents_enabled && !mods.subagents.is_empty(),
        &mods.subagents,
        mods.shell_enabled,
        read_only,
    )
    .validate(call, mods)
}
