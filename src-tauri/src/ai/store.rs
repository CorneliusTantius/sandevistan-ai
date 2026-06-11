use super::*;

pub(super) struct ConfigStore {
    pub config_dir: PathBuf,
    pub app: AppConfig,
    pub models: ModelsConfig,
    pub agents: AgentsConfig,
    pub auth: AuthConfig,
}

impl ConfigStore {
    pub fn load() -> Self {
        let config_dir = config_dir();
        ensure_config_files(&config_dir);
        Self {
            app: read_toml(config_dir.join("config.toml")),
            models: read_toml(config_dir.join("models.toml")),
            agents: read_toml(config_dir.join("agents.toml")),
            auth: read_toml(config_dir.join("auth.toml")),
            config_dir,
        }
    }
}
