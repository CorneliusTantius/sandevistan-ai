use crate::{runtime::types::AgentMessage, ChatMessage};

pub fn chat_to_agent_message(message: ChatMessage) -> AgentMessage {
    match message.role.as_str() {
        "system" => AgentMessage::system(message.content),
        "assistant" => AgentMessage::assistant(message.content, Vec::new()),
        "tool" => AgentMessage::user(format!(
            "Tool result from previous turn:\n{}",
            message.content
        )),
        "error" => AgentMessage::user(format!(
            "Runtime error from previous turn:\n{}",
            message.content
        )),
        _ => AgentMessage::user(message.content),
    }
}

pub fn chat_to_agent_messages(messages: Vec<ChatMessage>) -> Vec<AgentMessage> {
    messages.into_iter().map(chat_to_agent_message).collect()
}
