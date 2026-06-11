use serde::{Deserialize, Serialize};
use sandevistan_core::{AgentMods, ChatMessage as CoreChatMessage, SubagentDef as CoreSubagentDef};
use std::{collections::HashMap, path::PathBuf};

pub type ChatMessage = CoreChatMessage;
pub type SubagentDef = CoreSubagentDef;

#[derive(Debug, Deserialize)]
pub struct AiConfigUpdate {
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) original_model: Option<String>,
    pub(super) context_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderUpdate {
    pub(super) name: String,
    pub(super) original_name: Option<String>,
    pub(super) kind: String,
    pub(super) api_base: String,
    pub(super) api_key_header: Option<String>,
    pub(super) api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteProviderRequest {
    pub(super) provider: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteModelRequest {
    pub(super) model: String,
}

#[derive(Debug, Deserialize)]
pub struct FeatureUpdate {
    pub(super) name: String,
    pub(super) enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UiScaleUpdate {
    pub(super) scale: f32,
}

#[derive(Debug, Deserialize)]
pub struct ActiveProfileUpdate {
    pub(super) profile: String,
}

#[derive(Debug, Deserialize)]
pub struct ModsUpdate {
    pub(super) profile: Option<String>,
    pub(super) main_model: Option<String>,
    pub(super) main_agent: Option<String>,
    pub(super) subagents: Option<Vec<String>>,
    pub(super) rtk_enabled: Option<bool>,
    pub(super) shell_enabled: Option<bool>,
    pub(super) git_panel_enabled: Option<bool>,
    pub(super) subagents_enabled: Option<bool>,
    pub(super) subagent_model: Option<String>,
    pub(super) subagent_max_concurrency: Option<usize>,
    pub(super) subagents_config: Option<String>,
    pub(super) mcp_enabled: Option<bool>,
    pub(super) mcp_config: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiConfig {
    pub config_dir: String,
    pub provider: String,
    pub api_base: String,
    pub model: String,
    pub model_id: String,
    pub context_chars: usize,
    pub has_api_key: bool,
    pub providers: Vec<ProviderOption>,
    pub models: Vec<ModelOption>,
    pub features: HashMap<String, bool>,
    pub mods: AgentMods,
    pub active_profile: String,
    pub profiles: Vec<ProfileOption>,
    pub agents: Vec<AgentOption>,
    pub subagents_registry: Vec<SubagentOption>,
    pub rtk_available: bool,
    pub ui_scale: f32,
}

#[derive(Debug, Serialize)]
pub struct ProviderOption {
    pub name: String,
    pub kind: String,
    pub api_base: String,
    pub api_key_header: String,
    pub has_api_key: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelOption {
    pub(super) name: String,
    pub(super) provider: String,
    pub(super) id: String,
    pub(super) context_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentOption {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) persona: String,
    pub(super) thinking_level: String,
    pub(super) prompt_injection: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentOption {
    pub(super) name: String,
    pub(super) description: String,
    pub(super) system: String,
    pub(super) model: String,
    pub(super) max_result_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileOption {
    pub(super) name: String,
    pub(super) main_model: String,
    pub(super) main_agent: String,
    pub(super) subagents: Vec<String>,
    pub(super) persona: String,
    pub(super) thinking_level: String,
    pub(super) prompt_injection: String,
    pub(super) rtk_enabled: bool,
    pub(super) shell_enabled: bool,
    pub(super) git_panel_enabled: bool,
    pub(super) subagents_enabled: bool,
    pub(super) subagent_model: String,
    pub(super) subagent_max_concurrency: usize,
    pub(super) subagents_config: String,
    pub(super) mcp_enabled: bool,
    pub(super) mcp_config: String,
}

#[derive(Debug)]
pub struct RuntimeConfig {
    pub(super) config_dir: PathBuf,
    pub(super) provider: String,
    pub(super) api_base: String,
    pub(super) model: String,
    pub(super) model_id: String,
    pub(super) context_chars: usize,
    pub(super) api_key: Option<String>,
    pub(super) api_key_header: String,
    pub(super) kind: String,
    pub(super) mods: AgentMods,
    pub(super) active_profile: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct AppConfig {
    pub(super) default_provider: Option<String>,
    pub(super) default_model: Option<String>,
    pub(super) active_workspace: Option<String>,
    pub(super) workspaces: Option<Vec<String>>,
    pub(super) features: Option<HashMap<String, bool>>,
    pub(super) active_profile: Option<String>,
    pub(super) profiles: Option<HashMap<String, ProfileConfig>>,
    pub(super) persona: Option<String>,
    pub(super) thinking_level: Option<String>,
    pub(super) prompt_injection: Option<String>,
    pub(super) rtk_enabled: Option<bool>,
    pub(super) ui_scale: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct ProfileConfig {
    pub(super) main_model: Option<String>,
    pub(super) main_agent: Option<String>,
    pub(super) subagents: Option<Vec<String>>,
    pub(super) persona: Option<String>,
    pub(super) thinking_level: Option<String>,
    pub(super) prompt_injection: Option<String>,
    pub(super) rtk_enabled: Option<bool>,
    pub(super) shell_enabled: Option<bool>,
    pub(super) git_panel_enabled: Option<bool>,
    pub(super) subagents_enabled: Option<bool>,
    pub(super) subagent_model: Option<String>,
    pub(super) subagent_max_concurrency: Option<usize>,
    pub(super) subagents_config: Option<String>,
    pub(super) mcp_enabled: Option<bool>,
    pub(super) mcp_config: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct AgentsConfig {
    pub(super) agents: HashMap<String, AgentConfig>,
    pub(super) subagents: HashMap<String, SubagentConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct AgentConfig {
    pub(super) description: Option<String>,
    pub(super) persona: Option<String>,
    pub(super) thinking_level: Option<String>,
    pub(super) prompt_injection: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct SubagentConfig {
    pub(super) description: Option<String>,
    pub(super) system: String,
    pub(super) model: Option<String>,
    pub(super) max_result_chars: Option<usize>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct ModelsConfig {
    pub(super) providers: HashMap<String, ProviderConfig>,
    pub(super) models: HashMap<String, ModelConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ProviderConfig {
    pub(super) kind: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) api_key_header: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ModelConfig {
    pub(super) provider: Option<String>,
    pub(super) id: Option<String>,
    pub(super) context_chars: Option<usize>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct AuthConfig {
    pub(super) providers: HashMap<String, ProviderAuth>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ProviderAuth {
    pub(super) api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentUpdate {
    pub(super) name: String,
    pub(super) original_name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) persona: Option<String>,
    pub(super) thinking_level: Option<String>,
    pub(super) prompt_injection: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAgentRequest {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub struct SubagentUpdate {
    pub(super) name: String,
    pub(super) original_name: Option<String>,
    pub(super) description: Option<String>,
    pub(super) system: String,
    pub(super) model: Option<String>,
    pub(super) max_result_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteSubagentRequest {
    pub(super) name: String,
}

