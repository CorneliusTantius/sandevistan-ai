use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ToolOptions {
    pub rtk_enabled: bool,
    pub shell_enabled: bool,
    pub backup_session_id: Option<String>,
}

pub(crate) struct ToolRunResult {
    pub ok: bool,
    pub output: String,
}
