use serde::Serialize;
use std::path::Path;

pub mod config;
pub mod external;
pub mod hooks;
pub mod manifest;
pub mod mcp;
pub mod protocol;
pub mod skills;

#[derive(Debug, Serialize)]
pub struct ExtensionsInfo {
    pub config_path: String,
    pub extensions: Vec<ExtensionInfo>,
    pub skills: Vec<SkillInfo>,
}

#[derive(Debug, Serialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub removable: bool,
    pub description: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: String,
}

pub fn skills_enabled() -> bool {
    skills::enabled()
}

pub fn mcp_enabled() -> bool {
    mcp::enabled()
}

pub fn info(workspace: &Path) -> ExtensionsInfo {
    let skills_enabled = skills::enabled();
    ExtensionsInfo {
        config_path: config::config_path().display().to_string(),
        extensions: vec![
            ExtensionInfo {
                id: "skills".into(),
                name: "Skills".into(),
                enabled: skills_enabled,
                removable: true,
                description: "Agent Skills discovery + skill.list/skill.load tools".into(),
                path: None,
            },
            ExtensionInfo {
                id: "mcp".into(),
                name: "MCP".into(),
                enabled: mcp::enabled(),
                removable: true,
                description: "MCP extension slot; protocol client not configured yet".into(),
                path: None,
            },
        ]
        .into_iter()
        .chain(
            manifest::discover(workspace)
                .into_iter()
                .map(|manifest| ExtensionInfo {
                    id: manifest.id.clone(),
                    name: manifest.name.unwrap_or(manifest.id),
                    enabled: manifest.enabled.unwrap_or(false),
                    removable: true,
                    description: manifest.description.unwrap_or_else(|| {
                        let command = manifest.command.unwrap_or_else(|| "not configured".into());
                        let hooks = if manifest.hooks.is_empty() {
                            "no hooks".into()
                        } else {
                            format!("hooks: {}", manifest.hooks.join(", "))
                        };
                        format!("External extension manifest · {command} · {hooks}")
                    }),
                    path: Some(manifest.path.display().to_string()),
                }),
        )
        .collect(),
        skills: if skills_enabled {
            skills::discover(workspace)
                .into_iter()
                .map(|skill| SkillInfo {
                    name: skill.name,
                    description: skill.description,
                    path: skill.path.display().to_string(),
                })
                .collect()
        } else {
            Vec::new()
        },
    }
}

pub fn hook_bus(workspace: &Path) -> hooks::HookBus<'_> {
    hooks::HookBus::new(workspace)
}

pub fn system_prompt(workspace: &Path) -> String {
    hook_bus(workspace).system_context()
}

pub fn list_skills(workspace: &Path) -> String {
    skills::list(workspace)
}

pub fn load_skill(workspace: &Path, name: &str) -> String {
    skills::load(workspace, name)
}

pub fn list_mcp_servers() -> String {
    mcp::list_servers()
}
