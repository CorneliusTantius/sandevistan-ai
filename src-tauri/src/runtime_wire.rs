use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeToolSpec {
    pub name: String,
    pub openai_name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeToolCall {
    pub id: String,
    pub name: String,
    pub openai_name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NativeMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
        tool_calls: Vec<NativeToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        content: String,
    },
}

#[derive(Debug, Clone)]
pub enum NativeStreamEvent {
    TextDelta(String),
}

#[derive(Debug, Clone)]
pub struct NativeTurnResult {
    pub content: String,
    pub tool_calls: Vec<NativeToolCall>,
}
