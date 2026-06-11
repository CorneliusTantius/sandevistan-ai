use crate::ai::ChatMessage;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct WorkspaceRequest {
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionRequest {
    pub(super) id: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameSessionRequest {
    pub(super) id: String,
    pub(super) title: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchSessionsRequest {
    pub(super) query: String,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceOption {
    pub(super) path: String,
    pub(super) name: String,
    pub(super) deletable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOption {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) preview: String,
    pub(super) message_count: usize,
    pub(super) updated_at: u128,
    pub(super) running: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub(super) workspace: String,
    pub(super) active_session_id: String,
    pub(super) messages: Vec<ChatMessage>,
    pub(super) sessions: Vec<SessionOption>,
    pub(super) workspaces: Vec<WorkspaceOption>,
}

#[derive(Debug, Serialize)]
pub struct TaskInfo {
    pub(super) id: String,
    pub(super) session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct StreamEvent {
    pub(super) id: String,
    pub(super) session_id: String,
    pub(super) kind: String,
    pub(super) role: Option<String>,
    pub(super) text: Option<String>,
    pub(super) content: Option<String>,
    pub(super) debug: Option<String>,
    pub(super) input_tokens: Option<usize>,
    pub(super) output_tokens: Option<usize>,
    pub(super) total_tokens: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeState {
    pub(super) workspace: PathBuf,
    pub(super) active_session_id: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct SessionIndex {
    pub(super) active_session_id: Option<String>,
    pub(super) sessions: Vec<SessionOption>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct SessionFile {
    pub(super) id: String,
    pub(super) workspace: String,
    pub(super) title: String,
    pub(super) messages: Vec<ChatMessage>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct SessionSummary {
    pub(super) summary: String,
    pub(super) last_message_count: usize,
    pub(super) updated_at: u128,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct AppConfig {
    pub(super) default_provider: Option<String>,
    pub(super) default_model: Option<String>,
    pub(super) active_workspace: Option<String>,
    pub(super) workspaces: Option<Vec<String>>,
    pub(super) features: Option<HashMap<String, bool>>,
    pub(super) active_profile: Option<String>,
    pub(super) profiles: Option<HashMap<String, toml::Value>>,
    pub(super) persona: Option<String>,
    pub(super) thinking_level: Option<String>,
    pub(super) prompt_injection: Option<String>,
    pub(super) rtk_enabled: Option<bool>,
}

pub(super) struct RunningTask {
    pub(super) handle: tauri::async_runtime::JoinHandle<()>,
    pub(super) cancellation_token: crate::runtime::CancellationToken,
}
