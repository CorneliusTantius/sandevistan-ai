use crate::ChatMessage;

pub const DEFAULT_CONTEXT_CHARS: usize = 80_000;
const DEFAULT_TOOL_CHARS: usize = 8_000;
const DEFAULT_MESSAGE_CHARS: usize = 24_000;
const MIN_CONTEXT_CHARS: usize = 4_000;
const MAX_CONTEXT_CHARS: usize = 1_000_000;
const MIN_RECENT_MESSAGES: usize = 4;

#[derive(Debug, Clone)]
pub struct PromptConfig {
    pub max_prompt_chars: usize,
    pub max_tool_chars: usize,
    pub max_message_chars: usize,
    pub min_recent_messages: usize,
}

impl PromptConfig {
    pub fn from_context_chars(value: usize) -> Self {
        let max_prompt_chars = value.clamp(MIN_CONTEXT_CHARS, MAX_CONTEXT_CHARS);
        Self {
            max_prompt_chars,
            max_tool_chars: DEFAULT_TOOL_CHARS.min(max_prompt_chars / 4).max(1_000),
            max_message_chars: DEFAULT_MESSAGE_CHARS.min(max_prompt_chars / 2).max(2_000),
            min_recent_messages: MIN_RECENT_MESSAGES,
        }
    }
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self::from_context_chars(DEFAULT_CONTEXT_CHARS)
    }
}

pub fn build_prompt(
    system_prompt: &str,
    summary: Option<&str>,
    messages: &[ChatMessage],
    config: &PromptConfig,
) -> Vec<ChatMessage> {
    let mut selected = Vec::new();
    let summary = summary.map(str::trim).filter(|value| !value.is_empty());
    let mut used = system_prompt.len() + summary.map(str::len).unwrap_or_default();
    let mut omitted = 0usize;

    for (index, message) in messages.iter().enumerate().rev() {
        let must_keep = messages.len().saturating_sub(index) <= config.min_recent_messages;
        let next = budget_message(message, config);
        let cost = next.role.len() + next.content.len() + 8;
        if !must_keep && used + cost > config.max_prompt_chars {
            omitted = index + 1;
            break;
        }
        used += cost;
        selected.push(next);
    }

    selected.reverse();

    let mut prompt = vec![ChatMessage {
        role: "system".into(),
        content: system_prompt.into(),
    }];
    if let Some(summary) = summary {
        prompt.push(ChatMessage {
            role: "system".into(),
            content: format!("Session summary:\n{summary}"),
        });
    }
    if omitted > 0 {
        prompt.push(ChatMessage {
            role: "system".into(),
            content: format!(
                "Context budget active. Earlier conversation omitted: {omitted} messages. Ask user or inspect files if missing details matter."
            ),
        });
    }
    prompt.extend(selected);
    prompt
}

fn budget_message(message: &ChatMessage, config: &PromptConfig) -> ChatMessage {
    let limit = if message.role == "tool" {
        config.max_tool_chars
    } else {
        config.max_message_chars
    };
    ChatMessage {
        role: message.role.clone(),
        content: truncate(&message.content, limit),
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
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
