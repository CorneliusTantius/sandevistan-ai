use crate::runtime_wire::{NativeMessage, NativeToolCall};

#[derive(Debug, Clone)]
pub enum AgentRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone)]
pub enum ContentPart {
    Text(String),
    ToolCall(NativeToolCall),
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        content: String,
    },
}

#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub role: AgentRole,
    pub parts: Vec<ContentPart>,
}

impl AgentMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: AgentRole::System,
            parts: vec![ContentPart::Text(content.into())],
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: AgentRole::User,
            parts: vec![ContentPart::Text(content.into())],
        }
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Vec<NativeToolCall>) -> Self {
        let mut parts = Vec::new();
        let content = content.into();
        if !content.is_empty() {
            parts.push(ContentPart::Text(content));
        }
        parts.extend(tool_calls.into_iter().map(ContentPart::ToolCall));
        Self {
            role: AgentRole::Assistant,
            parts,
        }
    }

    pub fn tool_result(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: AgentRole::Tool,
            parts: vec![ContentPart::ToolResult {
                tool_call_id: tool_call_id.into(),
                tool_name: tool_name.into(),
                content: content.into(),
            }],
        }
    }

    pub fn text_content(&self) -> String {
        self.parts
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn estimated_chars(&self) -> usize {
        let role_cost = 16;
        role_cost
            + self
                .parts
                .iter()
                .map(|part| match part {
                    ContentPart::Text(text) => text.len(),
                    ContentPart::ToolCall(call) => {
                        call.id.len()
                            + call.name.len()
                            + call.openai_name.len()
                            + call.args.to_string().len()
                            + 32
                    }
                    ContentPart::ToolResult {
                        tool_call_id,
                        tool_name,
                        content,
                    } => tool_call_id.len() + tool_name.len() + content.len() + 32,
                })
                .sum::<usize>()
    }

    pub fn to_native(&self) -> NativeMessage {
        match self.role {
            AgentRole::System => NativeMessage::System {
                content: self.text_content(),
            },
            AgentRole::User => NativeMessage::User {
                content: self.text_content(),
            },
            AgentRole::Assistant => {
                let mut content = String::new();
                let mut tool_calls = Vec::new();
                for part in &self.parts {
                    match part {
                        ContentPart::Text(text) => {
                            if !content.is_empty() {
                                content.push('\n');
                            }
                            content.push_str(text);
                        }
                        ContentPart::ToolCall(call) => tool_calls.push(call.clone()),
                        ContentPart::ToolResult { .. } => {}
                    }
                }
                NativeMessage::Assistant {
                    content,
                    tool_calls,
                }
            }
            AgentRole::Tool => {
                let Some(ContentPart::ToolResult {
                    tool_call_id,
                    tool_name,
                    content,
                }) = self
                    .parts
                    .iter()
                    .find(|part| matches!(part, ContentPart::ToolResult { .. }))
                else {
                    return NativeMessage::User {
                        content: self.text_content(),
                    };
                };
                NativeMessage::ToolResult {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    content: content.clone(),
                }
            }
        }
    }
}

pub fn to_native_messages(messages: &[AgentMessage]) -> Vec<NativeMessage> {
    messages.iter().map(AgentMessage::to_native).collect()
}
