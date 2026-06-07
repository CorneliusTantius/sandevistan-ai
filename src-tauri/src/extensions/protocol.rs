use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct ExtensionRequest {
    pub protocol: String,
    pub request_id: String,
    pub extension_id: String,
    pub workspace: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<ExtensionHookEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<ExtensionToolCall>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtensionHookEvent {
    AgentStart,
    BeforeModelCall,
    BeforeToolCall { tool: String, args: Value },
    AfterToolResult { tool: String, content: String },
    AgentEnd,
    Error { message: String },
}

#[derive(Debug, Serialize)]
pub struct ExtensionToolCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Deserialize)]
pub struct ExtensionResponse {
    #[serde(default)]
    pub decisions: Vec<ExtensionDecision>,
    #[serde(default)]
    pub tools: Vec<ExtensionToolSpec>,
    pub content: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    #[serde(default)]
    pub mutating: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ExtensionDecision {
    Continue,
    Block { reason: String },
    ModifyToolArgs { args: Value },
    AppendSystemContext { content: String },
}
