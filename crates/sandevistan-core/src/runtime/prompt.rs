use crate::{
    runtime::types::{AgentMessage, AgentRole},
    AgentMods, PromptConfig,
};

pub(super) fn mods_prompt(mods: &AgentMods) -> String {
    let mut lines = Vec::new();
    if !mods.persona.trim().is_empty() {
        lines.push(format!(
            "Persona override: {}",
            compact_line(&mods.persona, 1_200)
        ));
    }
    if mods.thinking_level != "auto" {
        lines.push(format!(
            "Thinking level: {}. Keep reasoning internal; answer concise.",
            mods.thinking_level
        ));
    }
    if !mods.prompt_injection.trim().is_empty() {
        lines.push(format!(
            "User instruction: {}",
            compact_line(&mods.prompt_injection, 2_000)
        ));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("Model mods:\n{}", lines.join("\n"))
    }
}

fn compact_line(value: &str, max: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max {
        return compact;
    }
    compact.chars().take(max).collect()
}

pub(super) fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.into();
    }
    let mut end = 0;
    for (count, (index, ch)) in value.char_indices().enumerate() {
        if count >= max_chars {
            break;
        }
        end = index + ch.len_utf8();
    }
    format!("{}\n... truncated to {max_chars} chars", &value[..end])
}

pub(super) fn context_window<'a>(
    messages: &'a [AgentMessage],
    prompt_config: &PromptConfig,
) -> Vec<&'a AgentMessage> {
    let mut system = Vec::new();
    let mut rest = Vec::new();
    for message in messages {
        if matches!(message.role, AgentRole::System) {
            system.push(message);
        } else {
            rest.push(message);
        }
    }

    let mut used = system
        .iter()
        .map(|message| message.estimated_chars())
        .sum::<usize>();
    let mut selected = Vec::new();
    for message in rest.into_iter().rev() {
        let cost = message.estimated_chars();
        if !selected.is_empty() && used + cost > prompt_config.max_prompt_chars {
            break;
        }
        used += cost;
        selected.push(message);
    }
    selected.reverse();
    while matches!(
        selected.first().map(|message| &message.role),
        Some(AgentRole::Tool)
    ) {
        selected.remove(0);
    }

    system.extend(selected);
    system
}
