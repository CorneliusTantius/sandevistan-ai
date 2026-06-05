use serde::Serialize;
use std::path::Path;

pub mod config;
pub mod mcp;
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
            },
            ExtensionInfo {
                id: "mcp".into(),
                name: "MCP".into(),
                enabled: mcp::enabled(),
                removable: true,
                description: "MCP extension slot; protocol client not configured yet".into(),
            },
        ],
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

pub fn system_prompt(workspace: &Path) -> String {
    [skills::system_prompt(workspace), mcp::system_prompt()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n")
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
