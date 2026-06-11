use sandevistan_core::{
    context::DEFAULT_CONTEXT_CHARS, provider, AgentMods, ProviderConfig as CoreProviderConfig,
    ProviderKind, PromptConfig,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, path::PathBuf, sync::OnceLock};

const CONFIG_DIR_NAME: &str = ".sandevistan";
const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_PROVIDER: &str = "openai";
const DEFAULT_THINKING_LEVEL: &str = "auto";
const DEFAULT_PROMPT_INJECTION: &str = "be very concise and efficient, drop grammars and pleasantries extremely and to output really useful words only. explain in simple markdown. just respond important things and keep the respond short";

mod store;
mod types;
pub use types::{
    ActiveProfileUpdate, AgentOption, AgentUpdate, AiConfig, AiConfigUpdate, DeleteAgentRequest,
    DeleteModelRequest, DeleteProviderRequest, DeleteSubagentRequest, FeatureUpdate, ModelOption, ModsUpdate, ProfileOption, ProviderOption, ProviderUpdate, RuntimeConfig,
    SubagentOption, SubagentUpdate, UiScaleUpdate, ChatMessage, SubagentDef,
};
use store::ConfigStore;
use types::{AgentConfig, AgentsConfig, AppConfig, AuthConfig, ModelConfig, ModelsConfig, ProfileConfig, ProviderAuth, ProviderConfig, SubagentConfig};

pub fn config() -> AiConfig {
    let store = ConfigStore::load();
    let runtime = runtime_config_from_store(&store, None);
    AiConfig {
        config_dir: runtime.config_dir.display().to_string(),
        provider: runtime.provider,
        api_base: runtime.api_base,
        model: runtime.model,
        model_id: runtime.model_id,
        context_chars: runtime.context_chars,
        has_api_key: runtime.api_key.is_some(),
        providers: provider_options(&store.models, &store.auth),
        models: model_options(&store.models),
        features: app_features_from(&store.app),
        mods: runtime.mods,
        active_profile: runtime.active_profile,
        profiles: profile_options_from(&store.app),
        agents: agent_options(&store.agents),
        subagents_registry: subagent_options(&store.agents),
        rtk_available: rtk_available(),
        ui_scale: clean_ui_scale(store.app.ui_scale.unwrap_or(1.0)),
    }
}

pub fn set_ui_scale(update: UiScaleUpdate) -> Result<AiConfig, String> {
    let config_dir = config_dir();
    ensure_config_files(&config_dir);
    let path = config_dir.join("config.toml");
    let mut app = read_toml::<AppConfig>(path.clone());
    app.ui_scale = Some(clean_ui_scale(update.scale));
    write_toml(path, &app)?;
    Ok(config())
}

pub fn set_active_profile(update: ActiveProfileUpdate) -> Result<AiConfig, String> {
    let profile = clean_required(update.profile, "profile")?;
    let config_dir = config_dir();
    ensure_config_files(&config_dir);
    let path = config_dir.join("config.toml");
    let mut app = read_toml::<AppConfig>(path.clone());
    if !normalized_profiles(&app).contains_key(&profile) {
        return Err("profile not found".into());
    }
    app.active_profile = Some(profile);
    write_toml(path, &app)?;
    Ok(config())
}

pub fn set_mods(update: ModsUpdate) -> Result<AiConfig, String> {
    let config_dir = config_dir();
    ensure_config_files(&config_dir);

    let path = config_dir.join("config.toml");
    let mut app = read_toml::<AppConfig>(path.clone());
    let profile = clean_optional(update.profile.unwrap_or_else(|| active_profile_name(&app)))
        .unwrap_or_else(|| "default".into());
    let mut profiles = normalized_profiles(&app);
    profiles.insert(
        profile.clone(),
        ProfileConfig {
            main_model: update.main_model.and_then(clean_optional),
            main_agent: update.main_agent.and_then(clean_optional),
            subagents: update.subagents.map(clean_subagents),
            persona: None,
            thinking_level: None,
            prompt_injection: None,
            rtk_enabled: update.rtk_enabled,
            shell_enabled: update.shell_enabled,
            git_panel_enabled: update.git_panel_enabled,
            subagents_enabled: update.subagents_enabled,
            subagent_model: update.subagent_model.and_then(clean_optional),
            subagent_max_concurrency: update
                .subagent_max_concurrency
                .map(clean_subagent_concurrency),
            subagents_config: update.subagents_config.and_then(clean_optional),
            mcp_enabled: update.mcp_enabled,
            mcp_config: update.mcp_config.and_then(clean_optional),
        },
    );
    app.active_profile = Some(profile);
    app.profiles = Some(profiles);

    write_toml(path, &app)?;
    Ok(config())
}

pub fn save_agent(update: AgentUpdate) -> Result<AiConfig, String> {
    let name = clean_required(update.name, "agent")?;
    let original = update.original_name.and_then(clean_optional);
    let config_dir = config_dir();
    ensure_config_files(&config_dir);
    let path = config_dir.join("agents.toml");
    let mut agents = read_toml::<AgentsConfig>(path.clone());
    if let Some(original) = original.filter(|original| original != &name) {
        agents.agents.remove(&original);
        rename_profile_agent(&config_dir, &original, &name)?;
    }
    agents.agents.insert(
        name,
        AgentConfig {
            description: update.description.and_then(clean_optional),
            persona: update.persona.and_then(clean_optional),
            thinking_level: Some(clean_thinking_level(
                &update
                    .thinking_level
                    .unwrap_or_else(|| DEFAULT_THINKING_LEVEL.into()),
            )?),
            prompt_injection: update.prompt_injection.and_then(clean_optional),
        },
    );
    write_toml(path, &agents)?;
    Ok(config())
}

pub fn delete_agent(request: DeleteAgentRequest) -> Result<AiConfig, String> {
    let name = clean_required(request.name, "agent")?;
    if name == "custom" {
        return Err("custom agent cannot be deleted".into());
    }
    let config_dir = config_dir();
    ensure_config_files(&config_dir);
    let path = config_dir.join("agents.toml");
    let mut agents = read_toml::<AgentsConfig>(path.clone());
    agents.agents.remove(&name);
    write_toml(path, &agents)?;
    rename_profile_agent(&config_dir, &name, "custom")?;
    Ok(config())
}

pub fn save_subagent(update: SubagentUpdate) -> Result<AiConfig, String> {
    let name = clean_required(update.name, "subagent")?;
    let system = clean_required(update.system, "subagent system")?;
    let original = update.original_name.and_then(clean_optional);
    let config_dir = config_dir();
    ensure_config_files(&config_dir);
    let path = config_dir.join("agents.toml");
    let mut agents = read_toml::<AgentsConfig>(path.clone());
    if let Some(original) = original.filter(|original| original != &name) {
        agents.subagents.remove(&original);
        rename_profile_subagent(&config_dir, &original, &name)?;
    }
    agents.subagents.insert(
        name,
        SubagentConfig {
            description: update.description.and_then(clean_optional),
            system,
            model: update.model.and_then(clean_optional),
            max_result_chars: update
                .max_result_chars
                .map(|value| value.clamp(500, 20_000)),
        },
    );
    write_toml(path, &agents)?;
    Ok(config())
}

pub fn delete_subagent(request: DeleteSubagentRequest) -> Result<AiConfig, String> {
    let name = clean_required(request.name, "subagent")?;
    let config_dir = config_dir();
    ensure_config_files(&config_dir);
    let path = config_dir.join("agents.toml");
    let mut agents = read_toml::<AgentsConfig>(path.clone());
    agents.subagents.remove(&name);
    write_toml(path, &agents)?;
    remove_profile_subagent(&config_dir, &name)?;
    Ok(config())
}

pub fn set_feature(update: FeatureUpdate) -> Result<AiConfig, String> {
    let name = clean_required(update.name, "feature")?;
    let config_dir = config_dir();
    ensure_config_files(&config_dir);

    let path = config_dir.join("config.toml");
    let mut app = read_toml::<AppConfig>(path.clone());
    let mut features = app.features.unwrap_or_else(default_features);
    if !default_features().contains_key(&name) {
        return Err(format!("unknown feature: {name}"));
    }
    features.insert(name, update.enabled);
    app.features = Some(features);
    write_toml(path, &app)?;
    Ok(config())
}

pub fn delete_model(request: DeleteModelRequest) -> Result<AiConfig, String> {
    let model = clean_required(request.model, "model")?;
    let config_dir = config_dir();
    ensure_config_files(&config_dir);

    let mut app = read_toml::<AppConfig>(config_dir.join("config.toml"));
    let mut models = read_toml::<ModelsConfig>(config_dir.join("models.toml"));
    models.models.remove(&model);

    if app.default_model.as_deref() == Some(model.as_str()) {
        app.default_model = models.models.keys().next().cloned();
        app.default_provider = app
            .default_model
            .as_ref()
            .and_then(|next| models.models.get(next))
            .and_then(|next| next.provider.clone());
    }
    let mut profiles = normalized_profiles(&app);
    for profile in profiles.values_mut() {
        if profile.main_model.as_deref() == Some(model.as_str()) {
            profile.main_model = app.default_model.clone();
        }
    }
    app.profiles = Some(profiles);

    write_toml(config_dir.join("config.toml"), &app)?;
    write_toml(config_dir.join("models.toml"), &models)?;

    Ok(config())
}

pub fn save_config(update: AiConfigUpdate) -> Result<AiConfig, String> {
    let provider = clean_required(update.provider, "provider")?;
    let model = clean_required(update.model, "model")?;
    let config_dir = config_dir();
    ensure_config_files(&config_dir);

    let mut app = read_toml::<AppConfig>(config_dir.join("config.toml"));
    app.default_provider = Some(provider.clone());
    app.default_model = Some(model.clone());
    let active_profile = active_profile_name(&app);
    let mut profiles = normalized_profiles(&app);
    if let Some(profile) = profiles.get_mut(&active_profile) {
        profile.main_model = Some(model.clone());
    }
    app.active_profile = Some(active_profile);
    app.profiles = Some(profiles);

    let mut models = read_toml::<ModelsConfig>(config_dir.join("models.toml"));
    if !models.providers.contains_key(&provider) {
        return Err(format!("provider not found: {provider}"));
    }
    let original_model = update.original_model.and_then(clean_optional);
    let previous = original_model
        .as_ref()
        .filter(|original| *original != &model)
        .and_then(|original| models.models.remove(original))
        .or_else(|| models.models.remove(&model));
    models.models.insert(
        model.clone(),
        ModelConfig {
            provider: Some(provider.clone()),
            id: Some(model),
            context_chars: Some(clean_context_chars(
                update
                    .context_chars
                    .or_else(|| previous.as_ref().and_then(|entry| entry.context_chars))
                    .unwrap_or(DEFAULT_CONTEXT_CHARS),
            )),
        },
    );

    write_toml(config_dir.join("config.toml"), &app)?;
    write_toml(config_dir.join("models.toml"), &models)?;

    Ok(config())
}

pub fn save_provider(update: ProviderUpdate) -> Result<AiConfig, String> {
    let name = clean_required(update.name, "provider")?;
    let api_base = clean_required(update.api_base, "api base")?;
    let kind = clean_provider_kind(&update.kind)?;
    let api_key_header = update
        .api_key_header
        .and_then(clean_optional)
        .map(|value| value.to_ascii_lowercase());
    let original = update.original_name.and_then(clean_optional);
    let config_dir = config_dir();
    ensure_config_files(&config_dir);

    let mut models = read_toml::<ModelsConfig>(config_dir.join("models.toml"));
    if let Some(original) = original.filter(|original| original != &name) {
        if let Some(previous) = models.providers.remove(&original) {
            models.providers.insert(name.clone(), previous);
        }
        for model in models.models.values_mut() {
            if model.provider.as_deref() == Some(original.as_str()) {
                model.provider = Some(name.clone());
            }
        }
        let mut auth = read_toml::<AuthConfig>(config_dir.join("auth.toml"));
        if let Some(previous_auth) = auth.providers.remove(&original) {
            auth.providers.insert(name.clone(), previous_auth);
            write_toml(config_dir.join("auth.toml"), &auth)?;
            set_owner_only_permissions(&config_dir.join("auth.toml"));
        }
    }

    models.providers.insert(
        name.clone(),
        ProviderConfig {
            kind: Some(kind),
            base_url: Some(api_base),
            api_key_header,
        },
    );

    if let Some(api_key) = update.api_key.and_then(clean_optional) {
        let mut auth = read_toml::<AuthConfig>(config_dir.join("auth.toml"));
        auth.providers.insert(name, ProviderAuth { api_key: Some(api_key) });
        write_toml(config_dir.join("auth.toml"), &auth)?;
        set_owner_only_permissions(&config_dir.join("auth.toml"));
    }

    write_toml(config_dir.join("models.toml"), &models)?;
    Ok(config())
}

pub fn delete_provider(request: DeleteProviderRequest) -> Result<AiConfig, String> {
    let provider = clean_required(request.provider, "provider")?;
    let config_dir = config_dir();
    ensure_config_files(&config_dir);
    let mut models = read_toml::<ModelsConfig>(config_dir.join("models.toml"));
    if models
        .models
        .values()
        .any(|model| model.provider.as_deref() == Some(provider.as_str()))
    {
        return Err("provider is used by models".into());
    }
    models.providers.remove(&provider);
    let mut auth = read_toml::<AuthConfig>(config_dir.join("auth.toml"));
    auth.providers.remove(&provider);
    write_toml(config_dir.join("models.toml"), &models)?;
    write_toml(config_dir.join("auth.toml"), &auth)?;
    set_owner_only_permissions(&config_dir.join("auth.toml"));
    Ok(config())
}

pub async fn complete_chat(messages: Vec<ChatMessage>) -> Result<String, String> {
    let (runtime, messages) = runtime_request(messages)?;
    provider::complete(runtime, messages)
        .await
        .map_err(|error| error.to_string())
}

pub fn provider_config_for_model(model: Option<String>) -> Result<CoreProviderConfig, String> {
    runtime_request_for_model(
        vec![ChatMessage {
            role: "user".into(),
            content: "native request".into(),
        }],
        model,
    )
    .map(|(runtime, _)| runtime)
}

pub fn prompt_config() -> PromptConfig {
    PromptConfig::from_context_chars(runtime_config().context_chars)
}

pub fn active_mods() -> AgentMods {
    runtime_config().mods
}

pub fn stream_error_debug(content: &str) -> String {
    let config = config();
    let kind = config
        .providers
        .iter()
        .find(|provider| provider.name == config.provider)
        .map(|provider| provider.kind.as_str())
        .unwrap_or("unknown");
    format!(
        "provider: {}\napi_base: {}\nmodel: {}\nkind: {}\nerror:\n{}",
        config.provider, config.api_base, config.model_id, kind, content
    )
}

fn runtime_request(
    messages: Vec<ChatMessage>,
) -> Result<(CoreProviderConfig, Vec<ChatMessage>), String> {
    runtime_request_for_model(messages, None)
}

fn runtime_request_for_model(
    messages: Vec<ChatMessage>,
    model_override: Option<String>,
) -> Result<(CoreProviderConfig, Vec<ChatMessage>), String> {
    if messages.is_empty() {
        return Err("messages are empty".into());
    }

    let config = runtime_config_for_model(model_override);
    if config.api_key.is_none() && config.api_base == DEFAULT_API_BASE {
        return Err(format!(
            "missing api key: set SANDEVISTAN_API_KEY or ~/.sandevistan/auth.toml for provider '{}'",
            config.provider
        ));
    }

    Ok((
        CoreProviderConfig {
            kind: provider_kind(&config.kind),
            api_base: config.api_base,
            api_key: config.api_key,
            api_key_header: config.api_key_header,
            model_id: config.model_id,
        },
        normalize_messages(messages),
    ))
}

fn provider_kind(kind: &str) -> ProviderKind {
    match kind {
        "openai-responses" => ProviderKind::OpenAiResponses,
        _ => ProviderKind::OpenAiChat,
    }
}

fn normalize_messages(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    messages
        .into_iter()
        .map(|message| match message.role.as_str() {
            "tool" => ChatMessage {
                role: "user".into(),
                content: format!("Tool result:\n{}", message.content),
            },
            "error" => ChatMessage {
                role: "user".into(),
                content: format!("Runtime error:\n{}", message.content),
            },
            _ => message,
        })
        .collect()
}

mod runtime;
use runtime::{app_features_from, default_features, runtime_config, runtime_config_for_model, runtime_config_from_store};

fn provider_options(models: &ModelsConfig, auth: &AuthConfig) -> Vec<ProviderOption> {
    let mut providers = models
        .providers
        .iter()
        .map(|(name, provider)| ProviderOption {
            name: name.clone(),
            kind: provider
                .kind
                .clone()
                .unwrap_or_else(|| "openai-compatible".into()),
            api_base: provider.base_url.clone().unwrap_or_default(),
            api_key_header: provider
                .api_key_header
                .clone()
                .unwrap_or_else(|| "authorization".into()),
            has_api_key: auth
                .providers
                .get(name)
                .and_then(|provider| provider.api_key.as_ref())
                .is_some_and(|key| !key.trim().is_empty()),
        })
        .collect::<Vec<_>>();
    providers.sort_by(|a, b| a.name.cmp(&b.name));
    providers
}

fn model_options(models: &ModelsConfig) -> Vec<ModelOption> {
    let mut entries = models
        .models
        .iter()
        .map(|(name, model)| ModelOption {
            name: name.clone(),
            provider: model
                .provider
                .clone()
                .unwrap_or_else(|| DEFAULT_PROVIDER.into()),
            id: model.id.clone().unwrap_or_else(|| name.clone()),
            context_chars: clean_context_chars(
                model
                    .context_chars
                    .unwrap_or(DEFAULT_CONTEXT_CHARS),
            ),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn agent_options(agents: &AgentsConfig) -> Vec<AgentOption> {
    let mut entries = agents
        .agents
        .iter()
        .map(|(name, agent)| AgentOption {
            name: name.clone(),
            description: agent.description.clone().unwrap_or_default(),
            persona: agent.persona.clone().unwrap_or_default(),
            thinking_level: agent
                .thinking_level
                .clone()
                .unwrap_or_else(|| DEFAULT_THINKING_LEVEL.into()),
            prompt_injection: agent.prompt_injection.clone().unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn subagent_options(agents: &AgentsConfig) -> Vec<SubagentOption> {
    let mut entries = agents
        .subagents
        .iter()
        .map(|(name, subagent)| SubagentOption {
            name: name.clone(),
            description: subagent.description.clone().unwrap_or_default(),
            system: subagent.system.clone(),
            model: subagent.model.clone().unwrap_or_default(),
            max_result_chars: subagent.max_result_chars.unwrap_or(4_000),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn subagent_defs(agents: &AgentsConfig) -> Vec<SubagentDef> {
    subagent_options(agents)
        .into_iter()
        .map(|entry| SubagentDef {
            name: entry.name,
            description: (!entry.description.is_empty()).then_some(entry.description),
            system: entry.system,
            model: (!entry.model.is_empty()).then_some(entry.model),
            max_result_chars: Some(entry.max_result_chars),
        })
        .collect()
}

fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(CONFIG_DIR_NAME)
}

fn ensure_config_files(config_dir: &PathBuf) {
    let _ = fs::create_dir_all(config_dir);

    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        let _ = fs::write(
            &config_path,
            "default_provider = \"openai\"\ndefault_model = \"gpt-4o-mini\"\nactive_profile = \"default\"\n\n[profiles.default]\nthinking_level = \"auto\"\nprompt_injection = \"be very concise and efficient, drop grammars and pleasantries extremely and to output really useful words only. explain in simple markdown. just respond important things and keep the respond short\"\n",
        );
    }

    let models_path = config_dir.join("models.toml");
    if !models_path.exists() {
        let _ = fs::write(
            &models_path,
            "[providers.openai]\nkind = \"openai-compatible\"\nbase_url = \"https://api.openai.com/v1\"\n\n[providers.ollama]\nkind = \"openai-compatible\"\nbase_url = \"http://localhost:11434/v1\"\n\n[models.gpt-4o-mini]\nprovider = \"openai\"\nid = \"gpt-4o-mini\"\n\n[models.llama3_2]\nprovider = \"ollama\"\nid = \"llama3.2\"\n",
        );
    }

    let agents_path = config_dir.join("agents.toml");
    if !agents_path.exists() {
        let _ = fs::write(
            &agents_path,
            "[agents.custom]\ndescription = \"Default main agent\"\nthinking_level = \"auto\"\nprompt_injection = \"be very concise and efficient, drop grammars and pleasantries extremely and to output really useful words only. explain in simple markdown. just respond important things and keep the respond short\"\n\n[subagents.scout]\ndescription = \"Find relevant files, symbols, and facts. No edits.\"\nsystem = \"Fast codebase scout. Search first, read only relevant files, return paths and findings.\"\nmax_result_chars = 4000\n\n[subagents.reviewer]\ndescription = \"Review risks, bugs, and missing tests. No edits.\"\nsystem = \"Strict senior reviewer. Return only concrete risks, bugs, and fixes.\"\nmax_result_chars = 4000\n\n[subagents.planner]\ndescription = \"Create small safe implementation steps. No edits.\"\nsystem = \"Careful planner. Return current state, target state, steps, validation, rollback.\"\nmax_result_chars = 4000\n\n[subagents.worker]\ndescription = \"Bounded implementation analysis. Read-only in MVP.\"\nsystem = \"Implementation worker. Inspect code and propose exact minimal changes. Do not edit files.\"\nmax_result_chars = 6000\n",
        );
    }

    let auth_path = config_dir.join("auth.toml");
    if !auth_path.exists() {
        let _ = fs::write(
            &auth_path,
            "# Local secrets. Do not share.\n[providers.openai]\n# api_key = \"sk-...\"\n",
        );
        set_owner_only_permissions(&auth_path);
    }
}

fn read_toml<T>(path: PathBuf) -> T
where
    T: for<'de> Deserialize<'de> + Default,
{
    fs::read_to_string(path)
        .ok()
        .and_then(|content| toml::from_str::<T>(&content).ok())
        .unwrap_or_default()
}

fn write_toml<T>(path: PathBuf, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let content = toml::to_string_pretty(value)
        .map_err(|error| format!("config serialize failed: {error}"))?;
    fs::write(path, content).map_err(|error| format!("config write failed: {error}"))
}

fn clean_required(value: String, label: &str) -> Result<String, String> {
    clean_optional(value).ok_or_else(|| format!("{label} is empty"))
}

fn clean_optional(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn clean_provider_kind(value: &str) -> Result<String, String> {
    match value.trim() {
        "openai-compatible" | "openai-responses" => Ok(value.trim().to_string()),
        _ => Err("provider kind must be openai-compatible or openai-responses".into()),
    }
}

fn clean_context_chars(value: usize) -> usize {
    value.clamp(4_000, 1_000_000)
}

fn clean_ui_scale(value: f32) -> f32 {
    if !value.is_finite() {
        return 1.0;
    }
    (value * 100.0).round().clamp(70.0, 160.0) / 100.0
}

fn clean_subagent_concurrency(value: usize) -> usize {
    value.clamp(1, 8)
}

fn clean_subagents(values: Vec<String>) -> Vec<String> {
    let mut output = Vec::new();
    for value in values.into_iter().filter_map(clean_optional) {
        if !output.contains(&value) {
            output.push(value);
        }
    }
    output
}

fn clean_thinking_level(value: &str) -> Result<String, String> {
    let value = value.trim().to_lowercase();
    match value.as_str() {
        "auto" | "low" | "medium" | "high" => Ok(value),
        _ => Err("thinking level must be auto, low, medium, or high".into()),
    }
}

fn model_mods(app: &AppConfig, agents: &AgentsConfig, name: &str) -> AgentMods {
    let profiles = normalized_profiles(app);
    let profile = profiles.get(name);
    let default_model = app
        .default_model
        .clone()
        .unwrap_or_else(|| DEFAULT_MODEL.into());
    let agent_name = profile
        .and_then(|value| value.main_agent.clone())
        .unwrap_or_else(|| "custom".into());
    let agent = agents.agents.get(&agent_name);
    AgentMods {
        main_model: profile
            .and_then(|value| value.main_model.clone())
            .unwrap_or(default_model),
        main_agent: agent_name,
        subagents: profile
            .and_then(|value| value.subagents.clone())
            .unwrap_or_else(|| vec!["scout".into(), "reviewer".into(), "planner".into()]),
        persona: agent
            .and_then(|value| value.persona.clone())
            .or_else(|| profile.and_then(|value| value.persona.clone()))
            .unwrap_or_default(),
        thinking_level: agent
            .and_then(|value| value.thinking_level.clone())
            .or_else(|| profile.and_then(|value| value.thinking_level.clone()))
            .unwrap_or_else(|| DEFAULT_THINKING_LEVEL.into()),
        prompt_injection: agent
            .and_then(|value| value.prompt_injection.clone())
            .or_else(|| profile.and_then(|value| value.prompt_injection.clone()))
            .unwrap_or_default(),
        rtk_enabled: profile
            .and_then(|value| value.rtk_enabled)
            .unwrap_or_else(rtk_available),
        shell_enabled: profile
            .and_then(|value| value.shell_enabled)
            .unwrap_or(false),
        git_panel_enabled: profile
            .and_then(|value| value.git_panel_enabled)
            .unwrap_or(true),
        subagents_enabled: profile
            .and_then(|value| value.subagents_enabled)
            .unwrap_or(true),
        subagent_model: profile
            .and_then(|value| value.subagent_model.clone())
            .unwrap_or_default(),
        subagent_max_concurrency: clean_subagent_concurrency(
            profile
                .and_then(|value| value.subagent_max_concurrency)
                .unwrap_or(3),
        ),
        subagents_config: profile
            .and_then(|value| value.subagents_config.clone())
            .unwrap_or_default(),
        mcp_enabled: profile.and_then(|value| value.mcp_enabled).unwrap_or(false),
        mcp_config: profile
            .and_then(|value| value.mcp_config.clone())
            .unwrap_or_default(),
        subagents_registry: subagent_defs(agents),
    }
}

fn profile_options_from(app: &AppConfig) -> Vec<ProfileOption> {
    let default_model = app
        .default_model
        .clone()
        .unwrap_or_else(|| DEFAULT_MODEL.into());
    let mut entries = normalized_profiles(app)
        .into_iter()
        .map(|(name, profile)| ProfileOption {
            name,
            main_model: profile.main_model.unwrap_or_else(|| default_model.clone()),
            main_agent: profile.main_agent.unwrap_or_else(|| "custom".into()),
            subagents: profile
                .subagents
                .unwrap_or_else(|| vec!["scout".into(), "reviewer".into(), "planner".into()]),
            persona: profile.persona.unwrap_or_default(),
            thinking_level: profile
                .thinking_level
                .unwrap_or_else(|| DEFAULT_THINKING_LEVEL.into()),
            prompt_injection: profile.prompt_injection.unwrap_or_default(),
            rtk_enabled: profile.rtk_enabled.unwrap_or_else(rtk_available),
            shell_enabled: profile.shell_enabled.unwrap_or(false),
            git_panel_enabled: profile.git_panel_enabled.unwrap_or(true),
            subagents_enabled: profile.subagents_enabled.unwrap_or(true),
            subagent_model: profile.subagent_model.unwrap_or_default(),
            subagent_max_concurrency: clean_subagent_concurrency(
                profile.subagent_max_concurrency.unwrap_or(8),
            ),
            subagents_config: profile.subagents_config.unwrap_or_default(),
            mcp_enabled: profile.mcp_enabled.unwrap_or(false),
            mcp_config: profile.mcp_config.unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn active_profile_name(app: &AppConfig) -> String {
    app.active_profile
        .clone()
        .and_then(clean_optional)
        .unwrap_or_else(|| "default".into())
}

fn normalized_profiles(app: &AppConfig) -> HashMap<String, ProfileConfig> {
    let mut profiles = app.profiles.clone().unwrap_or_default();
    profiles.entry("default".into()).or_insert(ProfileConfig {
        main_model: app.default_model.clone(),
        main_agent: Some("custom".into()),
        subagents: None,
        persona: app.persona.clone(),
        thinking_level: app.thinking_level.clone(),
        prompt_injection: app
            .prompt_injection
            .clone()
            .or_else(|| Some(DEFAULT_PROMPT_INJECTION.into())),
        rtk_enabled: app.rtk_enabled,
        shell_enabled: None,
        git_panel_enabled: None,
        subagents_enabled: None,
        subagent_model: None,
        subagent_max_concurrency: None,
        subagents_config: None,
        mcp_enabled: None,
        mcp_config: None,
    });
    profiles
}

fn rtk_available() -> bool {
    rtk_path().is_some()
}

fn rtk_path() -> Option<PathBuf> {
    static RTK_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    RTK_PATH
        .get_or_init(|| {
            env::var_os("PATH")
                .into_iter()
                .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
                .chain(rtk_fallback_dirs())
                .flat_map(|dir| {
                    rtk_executable_names()
                        .iter()
                        .map(move |name| dir.join(name))
                })
                .find(|path| path.is_file())
        })
        .clone()
}

fn rtk_executable_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["rtk.exe", "rtk.cmd", "rtk.bat", "rtk"]
    } else {
        &["rtk"]
    }
}

fn rtk_fallback_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".cargo/bin"));
    }
    if cfg!(windows) {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local_app_data).join("Programs").join("rtk"));
        }
    }
    dirs
}

fn env_value(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn rename_profile_agent(config_dir: &PathBuf, from: &str, to: &str) -> Result<(), String> {
    let path = config_dir.join("config.toml");
    let mut app = read_toml::<AppConfig>(path.clone());
    let mut changed = false;
    for profile in app.profiles.get_or_insert_with(HashMap::new).values_mut() {
        if profile.main_agent.as_deref() == Some(from) {
            profile.main_agent = Some(to.into());
            changed = true;
        }
    }
    if changed {
        write_toml(path, &app)?;
    }
    Ok(())
}

fn rename_profile_subagent(config_dir: &PathBuf, from: &str, to: &str) -> Result<(), String> {
    let path = config_dir.join("config.toml");
    let mut app = read_toml::<AppConfig>(path.clone());
    let mut changed = false;
    for profile in app.profiles.get_or_insert_with(HashMap::new).values_mut() {
        if let Some(subagents) = &mut profile.subagents {
            for name in subagents.iter_mut() {
                if name == from {
                    *name = to.into();
                    changed = true;
                }
            }
            *subagents = clean_subagents(subagents.clone());
        }
    }
    if changed {
        write_toml(path, &app)?;
    }
    Ok(())
}

fn remove_profile_subagent(config_dir: &PathBuf, name: &str) -> Result<(), String> {
    let path = config_dir.join("config.toml");
    let mut app = read_toml::<AppConfig>(path.clone());
    let mut changed = false;
    for profile in app.profiles.get_or_insert_with(HashMap::new).values_mut() {
        if let Some(subagents) = &mut profile.subagents {
            let len = subagents.len();
            subagents.retain(|entry| entry != name);
            changed |= subagents.len() != len;
        }
    }
    if changed {
        write_toml(path, &app)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        let _ = fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &PathBuf) {}
