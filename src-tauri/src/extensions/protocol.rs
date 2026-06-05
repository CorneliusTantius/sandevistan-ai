use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct ExtensionRequest {
    pub protocol: String,
    pub request_id: String,
    pub extension_id: String,
    pub workspace: String,
    pub event: ExtensionHookEvent,
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

#[derive(Debug, Deserialize)]
pub struct ExtensionResponse {
    #[serde(default)]
    pub decisions: Vec<ExtensionDecision>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ExtensionDecision {
    Continue,
    Block { reason: String },
    ModifyToolArgs { args: Value },
    AppendSystemContext { content: String },
}
