use crate::{ai, context, tools};

pub fn system_prompt(mods: &ai::ModelMods) -> String {
    let mods_prompt = ai::mods_prompt();
    let base_prompt = tools::native_system_prompt(
        mods.subagents_enabled && !mods.subagents.is_empty(),
        &mods.subagents,
        mods.shell_enabled,
    );
    if mods_prompt.is_empty() {
        base_prompt
    } else {
        format!("{}\n\n{}", base_prompt, mods_prompt)
    }
}

pub fn build_prompt(
    system_prompt: &str,
    summary: Option<&str>,
    messages: &[ai::ChatMessage],
    prompt_config: &context::PromptConfig,
) -> Vec<ai::ChatMessage> {
    context::build_prompt(system_prompt, summary, messages, prompt_config)
}
