use serde::Deserialize;
use std::{fs, path::PathBuf};

const CONFIG_DIR_NAME: &str = ".sandevistan";
pub const EXTENSIONS_FILE: &str = "extensions.toml";

#[derive(Debug, Default, Deserialize)]
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
