use super::*;

pub(super) fn app_features_from(app: &AppConfig) -> HashMap<String, bool> {
    let mut features = default_features();
    if let Some(saved) = &app.features {
        features.extend(saved.clone());
    }
    features
}

pub(super) fn default_features() -> HashMap<String, bool> {
    HashMap::from([
        ("content_search".into(), true),
        ("git".into(), true),
        ("file_watcher".into(), true),
    ])
}

pub(super) fn runtime_config() -> RuntimeConfig {
    runtime_config_for_model(None)
}

pub(super) fn runtime_config_for_model(model_override: Option<String>) -> RuntimeConfig {
    let store = ConfigStore::load();
    runtime_config_from_store(&store, model_override)
}

pub(super) fn runtime_config_from_store(
    store: &ConfigStore,
    model_override: Option<String>,
) -> RuntimeConfig {
    let config_dir = store.config_dir.clone();
    let app = &store.app;
    let models = &store.models;
    let auth = &store.auth;

    let active_profile = active_profile_name(app);
    let profile_model = normalized_profiles(app)
        .get(&active_profile)
        .and_then(|profile| profile.main_model.clone());
    let model = model_override
        .and_then(clean_optional)
        .or_else(|| env_value("SANDEVISTAN_MODEL"))
        .or(profile_model)
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
            .unwrap_or(DEFAULT_CONTEXT_CHARS),
    );
    let mods = model_mods(app, &store.agents, &active_profile);

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
