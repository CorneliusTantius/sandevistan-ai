use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubagentDef {
    pub name: String,
    pub description: Option<String>,
    pub system: String,
    pub model: Option<String>,
    pub max_result_chars: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentMods {
    pub main_model: String,
    pub main_agent: String,
    pub subagents: Vec<String>,
    pub persona: String,
    pub thinking_level: String,
    pub prompt_injection: String,
    pub rtk_enabled: bool,
    pub shell_enabled: bool,
    pub git_panel_enabled: bool,
    pub subagents_enabled: bool,
    pub subagent_model: String,
    pub subagent_max_concurrency: usize,
    pub subagents_config: String,
    pub mcp_enabled: bool,
    pub mcp_config: String,
    pub subagents_registry: Vec<SubagentDef>,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: String,
    pub api_base: String,
    pub api_key: Option<String>,
    pub api_key_header: String,
    pub model_id: String,
}
