use crate::{ai, tools};

pub type ValidatedToolCall = tools::ValidatedToolCall;

pub fn validate_tool_call(
    call: tools::ToolCall,
    mods: &ai::ModelMods,
    read_only: bool,
) -> Result<ValidatedToolCall, String> {
    tools::ToolRegistry::new(
        mods.subagents_enabled && !mods.subagents.is_empty(),
        &mods.subagents,
        mods.shell_enabled,
        read_only,
    )
    .validate(call, mods)
}
