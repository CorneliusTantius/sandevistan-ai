import type { AiConfig, AiMods, ExtensionInfo, ExtensionsInfo, McpServerDraft, ThinkingLevel } from "../types";

export const baseMods: AiMods = {
  main_model: "gpt-4o-mini",
  main_agent: "custom",
  subagents: ["scout", "reviewer", "planner"],
  persona: "",
  thinking_level: "auto",
  prompt_injection: "",
  rtk_enabled: true,
  shell_enabled: false,
  git_panel_enabled: true,
  subagents_enabled: true,
  subagent_model: "",
  subagent_max_concurrency: 3,
  subagents_config: "",
  mcp_enabled: false,
  mcp_config: "",
};

export const emptyConfig: AiConfig = {
  config_dir: "",
  provider: "openai",
  api_base: "https://api.openai.com/v1",
  model: "gpt-4o-mini",
  model_id: "gpt-4o-mini",
  context_chars: 80000,
  has_api_key: false,
  providers: [],
  models: [],
  features: { content_search: true, git: true, file_watcher: true },
  mods: baseMods,
  active_profile: "default",
  profiles: [{ name: "default", ...baseMods }],
  agents: [{ name: "custom", description: "Default main agent", persona: "", thinking_level: "auto", prompt_injection: "" }],
  subagents_registry: [],
  rtk_available: false,
  ui_scale: 1,
};

export const defaultProviderDraft = {
  name: "openai",
  original_name: "",
  kind: "openai-compatible",
  api_base: "https://api.openai.com/v1",
  api_key_header: "authorization",
  api_key: "",
};

export const defaultModelDraft = { provider: "openai", model: "gpt-4o-mini", original_model: "", context_chars: 80000 };
export const defaultMcpDraft: McpServerDraft = { name: "", original_name: "", command: "", args: "", timeout_ms: 8000, env: "" };
export const defaultAgentDraft = { name: "", original_name: "", description: "", persona: "", thinking_level: "auto" as ThinkingLevel, prompt_injection: "" };
export const defaultSubagentDraft = { name: "", original_name: "", description: "", system: "", model: "", max_result_chars: 4000 };
export const defaultExtensionDraft: ExtensionInfo = { id: "", name: "", enabled: false, removable: true, description: "", hooks: [], tools: [] };
export const defaultExtensionsInfo: ExtensionsInfo = { config_path: "", extensions: [] };
