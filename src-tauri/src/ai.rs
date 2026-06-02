use crate::{
    context,
    provider::{self, ProviderRuntime},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, path::PathBuf};

const CONFIG_DIR_NAME: &str = ".sandevistan";
const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_PROVIDER: &str = "openai";
const DEFAULT_THINKING_LEVEL: &str = "auto";
const DEFAULT_PROMPT_INJECTION: &str = "be very concise and efficient, drop grammars and pleasantries extremely and to output really useful words only. explain in simple markdown. just respond important things and keep the respond short";

#[derive(Debug, Deserialize)]
pub struct AiConfigUpdate {
    provider: String,
    api_base: String,
    model: String,
    original_model: Option<String>,
    api_key: Option<String>,
    context_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteModelRequest {
    model: String,
}

#[derive(Debug, Deserialize)]
pub struct FeatureUpdate {
    name: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ModsUpdate {
    profile: Option<String>,
    persona: Option<String>,
    thinking_level: Option<String>,
    prompt_injection: Option<String>,
    rtk_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AiConfig {
    config_dir: String,
    provider: String,
    api_base: String,
    model: String,
    model_id: String,
    context_chars: usize,
    has_api_key: bool,
    providers: Vec<ProviderOption>,
    models: Vec<ModelOption>,
    features: HashMap<String, bool>,
    mods: ModelMods,
    active_profile: String,
    profiles: Vec<ProfileOption>,
    rtk_available: bool,
}

#[derive(Debug, Serialize)]
pub struct ProviderOption {
    name: String,
    api_base: String,
}

#[derive(Debug, Serialize)]
pub struct ModelOption {
    name: String,
    provider: String,
    id: String,
    context_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelMods {
    pub persona: String,
    pub thinking_level: String,
    pub prompt_injection: String,
    pub rtk_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileOption {
    name: String,
    persona: String,
    thinking_level: String,
    prompt_injection: String,
    rtk_enabled: bool,
}

#[derive(Debug)]
struct RuntimeConfig {
    config_dir: PathBuf,
    provider: String,
    api_base: String,
    model: String,
    model_id: String,
    context_chars: usize,
    api_key: Option<String>,
    api_key_header: String,
    kind: String,
    mods: ModelMods,
    active_profile: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct AppConfig {
    default_provider: Option<String>,
    default_model: Option<String>,
    active_workspace: Option<String>,
    workspaces: Option<Vec<String>>,
    features: Option<HashMap<String, bool>>,
    active_profile: Option<String>,
    profiles: Option<HashMap<String, ProfileConfig>>,
    persona: Option<String>,
    thinking_level: Option<String>,
    prompt_injection: Option<String>,
    rtk_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ProfileConfig {
    persona: Option<String>,
    thinking_level: Option<String>,
    prompt_injection: Option<String>,
    rtk_enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ModelsConfig {
    providers: HashMap<String, ProviderConfig>,
    models: HashMap<String, ModelConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ProviderConfig {
    kind: Option<String>,
    base_url: Option<String>,
    api_key_header: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModelConfig {
    provider: Option<String>,
    id: Option<String>,
    context_chars: Option<usize>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct AuthConfig {
    providers: HashMap<String, ProviderAuth>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ProviderAuth {
    api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub fn config() -> AiConfig {
    let runtime = runtime_config();
    let models = read_toml::<ModelsConfig>(runtime.config_dir.join("models.toml"));
    AiConfig {
        config_dir: runtime.config_dir.display().to_string(),
        provider: runtime.provider,
        api_base: runtime.api_base,
        model: runtime.model,
        model_id: runtime.model_id,
        context_chars: runtime.context_chars,
        has_api_key: runtime.api_key.is_some(),
        providers: provider_options(&models),
        models: model_options(&models),
        features: app_features(),
        mods: runtime.mods,
        active_profile: runtime.active_profile,
        profiles: profile_options(),
        rtk_available: rtk_available(),
    }
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
            persona: update.persona.and_then(clean_optional),
            thinking_level: Some(clean_thinking_level(
                &update
                    .thinking_level
                    .unwrap_or_else(|| DEFAULT_THINKING_LEVEL.into()),
            )?),
            prompt_injection: update.prompt_injection.and_then(clean_optional),
            rtk_enabled: update.rtk_enabled,
        },
    );
    app.active_profile = Some(profile);
    app.profiles = Some(profiles);

    write_toml(path, &app)?;
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

    write_toml(config_dir.join("config.toml"), &app)?;
    write_toml(config_dir.join("models.toml"), &models)?;

    Ok(config())
}

pub fn save_config(update: AiConfigUpdate) -> Result<AiConfig, String> {
    let provider = clean_required(update.provider, "provider")?;
    let api_base = clean_required(update.api_base, "api base")?;
    let model = clean_required(update.model, "model")?;
    let config_dir = config_dir();
    ensure_config_files(&config_dir);

    let mut app = read_toml::<AppConfig>(config_dir.join("config.toml"));
    app.default_provider = Some(provider.clone());
    app.default_model = Some(model.clone());

    let mut models = read_toml::<ModelsConfig>(config_dir.join("models.toml"));
    models.providers.insert(
        provider.clone(),
        ProviderConfig {
            kind: Some("openai-compatible".into()),
            base_url: Some(api_base),
            api_key_header: None,
        },
    );
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
                    .unwrap_or(context::DEFAULT_CONTEXT_CHARS),
            )),
        },
    );

    let mut auth = read_toml::<AuthConfig>(config_dir.join("auth.toml"));
    if let Some(api_key) = update.api_key.and_then(clean_optional) {
        auth.providers.insert(
            provider,
            ProviderAuth {
                api_key: Some(api_key),
            },
        );
    }

    write_toml(config_dir.join("config.toml"), &app)?;
    write_toml(config_dir.join("models.toml"), &models)?;
    write_toml(config_dir.join("auth.toml"), &auth)?;
    set_owner_only_permissions(&config_dir.join("auth.toml"));

    Ok(config())
}

pub async fn complete_chat(messages: Vec<ChatMessage>) -> Result<String, String> {
    let (runtime, messages) = runtime_request(messages)?;
    provider::complete(runtime, messages).await
}

pub async fn complete_chat_stream<F>(
    messages: Vec<ChatMessage>,
    on_delta: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    let (runtime, messages) = runtime_request(messages)?;
    provider::complete_stream(runtime, messages, on_delta).await
}

pub fn prompt_config() -> context::PromptConfig {
    context::PromptConfig::from_context_chars(runtime_config().context_chars)
}

pub fn active_mods() -> ModelMods {
    runtime_config().mods
}

pub fn mods_prompt() -> String {
    let mods = active_mods();
    let mut lines = Vec::new();
    if !mods.persona.trim().is_empty() {
        lines.push(format!(
            "Persona override: {}",
            compact_line(&mods.persona, 1_200)
        ));
    }
    if mods.thinking_level != DEFAULT_THINKING_LEVEL {
        lines.push(format!(
            "Thinking level: {}. Keep reasoning internal; answer concise.",
            mods.thinking_level
        ));
    }
    if !mods.prompt_injection.trim().is_empty() {
        lines.push(format!(
            "User instruction: {}",
            compact_line(&mods.prompt_injection, 2_000)
        ));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("Model mods:\n{}", lines.join("\n"))
    }
}

fn runtime_request(
    messages: Vec<ChatMessage>,
) -> Result<(ProviderRuntime, Vec<ChatMessage>), String> {
    if messages.is_empty() {
        return Err("messages are empty".into());
    }

    let config = runtime_config();
    if config.api_key.is_none() && config.api_base == DEFAULT_API_BASE {
        return Err(format!(
            "missing api key: set SANDEVISTAN_API_KEY or ~/.sandevistan/auth.toml for provider '{}'",
            config.provider
        ));
    }

    Ok((
        ProviderRuntime {
            kind: config.kind,
            api_base: config.api_base,
            api_key: config.api_key,
            api_key_header: config.api_key_header,
            model_id: config.model_id,
        },
        normalize_messages(messages),
    ))
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

fn app_features() -> HashMap<String, bool> {
    let mut features = default_features();
    if let Some(saved) = read_toml::<AppConfig>(config_dir().join("config.toml")).features {
        features.extend(saved);
    }
    features
}

fn default_features() -> HashMap<String, bool> {
    HashMap::from([
        ("content_search".into(), true),
        ("git".into(), true),
        ("file_watcher".into(), false),
    ])
}

fn runtime_config() -> RuntimeConfig {
    let config_dir = config_dir();
    ensure_config_files(&config_dir);

    let app = read_toml::<AppConfig>(config_dir.join("config.toml"));
    let models = read_toml::<ModelsConfig>(config_dir.join("models.toml"));
    let auth = read_toml::<AuthConfig>(config_dir.join("auth.toml"));

    let model = env_value("SANDEVISTAN_MODEL")
        .or_else(|| app.default_model.clone())
        .unwrap_or_else(|| DEFAULT_MODEL.into());
    let model_entry = models.models.get(&model);

    let provider = env_value("SANDEVISTAN_PROVIDER")
        .or_else(|| model_entry.and_then(|model| model.provider.clone()))
        .or_else(|| app.default_provider.clone())
        .unwrap_or_else(|| DEFAULT_PROVIDER.into());

    let model_id = model_entry
        .and_then(|model| model.id.clone())
        .unwrap_or_else(|| model.clone());

    let api_base = env_value("SANDEVISTAN_API_BASE")
        .or_else(|| {
            models
                .providers
                .get(&provider)
                .and_then(|provider| provider.base_url.clone())
        })
        .unwrap_or_else(|| DEFAULT_API_BASE.into());

    let api_key = env_value("SANDEVISTAN_API_KEY")
        .or_else(|| env_value("OPENAI_API_KEY"))
        .or_else(|| {
            auth.providers
                .get(&provider)
                .and_then(|provider| provider.api_key.clone())
                .filter(|value| !value.trim().is_empty())
        });

    let provider_config = models.providers.get(&provider);
    let api_key_header = provider_config
        .and_then(|provider| provider.api_key_header.clone())
        .unwrap_or_else(|| "authorization".into());
    let kind = provider_config
        .and_then(|provider| provider.kind.clone())
        .unwrap_or_else(|| "openai-compatible".into());

    let context_chars = clean_context_chars(
        model_entry
            .and_then(|model| model.context_chars)
            .unwrap_or(context::DEFAULT_CONTEXT_CHARS),
    );
    let active_profile = active_profile_name(&app);
    let mods = model_mods(&app, &active_profile);

    RuntimeConfig {
        config_dir,
        provider,
        api_base,
        model,
        model_id,
        context_chars,
        api_key,
        api_key_header,
        kind,
        mods,
        active_profile,
    }
}

fn provider_options(models: &ModelsConfig) -> Vec<ProviderOption> {
    let mut providers = models
        .providers
        .iter()
        .map(|(name, provider)| ProviderOption {
            name: name.clone(),
            api_base: provider.base_url.clone().unwrap_or_default(),
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
                    .unwrap_or(context::DEFAULT_CONTEXT_CHARS),
            ),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
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

fn clean_context_chars(value: usize) -> usize {
    value.clamp(4_000, 1_000_000)
}

fn clean_thinking_level(value: &str) -> Result<String, String> {
    let value = value.trim().to_lowercase();
    match value.as_str() {
        "auto" | "low" | "medium" | "high" => Ok(value),
        _ => Err("thinking level must be auto, low, medium, or high".into()),
    }
}

fn model_mods(app: &AppConfig, name: &str) -> ModelMods {
    let profiles = normalized_profiles(app);
    let profile = profiles.get(name);
    ModelMods {
        persona: profile
            .and_then(|value| value.persona.clone())
            .unwrap_or_default(),
        thinking_level: profile
            .and_then(|value| value.thinking_level.clone())
            .unwrap_or_else(|| DEFAULT_THINKING_LEVEL.into()),
        prompt_injection: profile
            .and_then(|value| value.prompt_injection.clone())
            .unwrap_or_default(),
        rtk_enabled: profile
            .and_then(|value| value.rtk_enabled)
            .unwrap_or_else(rtk_available),
    }
}

fn profile_options() -> Vec<ProfileOption> {
    let app = read_toml::<AppConfig>(config_dir().join("config.toml"));
    let mut entries = normalized_profiles(&app)
        .into_iter()
        .map(|(name, profile)| ProfileOption {
            name,
            persona: profile.persona.unwrap_or_default(),
            thinking_level: profile
                .thinking_level
                .unwrap_or_else(|| DEFAULT_THINKING_LEVEL.into()),
            prompt_injection: profile.prompt_injection.unwrap_or_default(),
            rtk_enabled: profile.rtk_enabled.unwrap_or_else(rtk_available),
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
        persona: app.persona.clone(),
        thinking_level: app.thinking_level.clone(),
        prompt_injection: app
            .prompt_injection
            .clone()
            .or_else(|| Some(DEFAULT_PROMPT_INJECTION.into())),
        rtk_enabled: app.rtk_enabled,
    });
    profiles
}

fn compact_line(value: &str, max: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max {
        return compact;
    }
    compact.chars().take(max).collect()
}

fn rtk_available() -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|path| path.join("rtk").is_file())
}

fn env_value(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
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
