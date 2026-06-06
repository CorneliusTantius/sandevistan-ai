use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const CONFIG_DIR_NAME: &str = ".sandevistan";
pub const EXTENSIONS_FILE: &str = "extensions.toml";

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ExtensionsConfig {
    pub enabled: Option<Vec<String>>,
}

pub fn extension_enabled(id: &str, default_enabled: bool) -> bool {
    let Some(config) = read_extensions_config() else {
        return default_enabled;
    };
    let Some(enabled) = config.enabled else {
        return default_enabled;
    };
    enabled.iter().any(|entry| entry == id)
}

pub fn set_extension_enabled(id: &str, enabled: bool) -> Result<(), String> {
    let path = config_path();
    let mut config = read_extensions_config().unwrap_or_default();
    let mut enabled_list = config
        .enabled
        .take()
        .unwrap_or_else(|| vec!["skills".into()]);
    if enabled {
        if !enabled_list.iter().any(|entry| entry == id) {
            enabled_list.push(id.to_string());
        }
    } else {
        enabled_list.retain(|entry| entry != id);
    }
    enabled_list.sort();
    enabled_list.dedup();
    config.enabled = Some(enabled_list);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("extension config dir create failed: {error}"))?;
    }
    let content = toml::to_string_pretty(&config)
        .map_err(|error| format!("extension config serialize failed: {error}"))?;
    fs::write(path, content).map_err(|error| format!("extension config write failed: {error}"))
}

pub fn config_path() -> PathBuf {
    config_dir().join(EXTENSIONS_FILE)
}

fn read_extensions_config() -> Option<ExtensionsConfig> {
    let content = fs::read_to_string(config_path()).ok()?;
    toml::from_str(&content).ok()
}

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(CONFIG_DIR_NAME)
}
