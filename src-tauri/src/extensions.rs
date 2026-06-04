use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CONFIG_DIR_NAME: &str = ".sandevistan";
const EXTENSIONS_FILE: &str = "extensions.toml";
const MAX_SKILLS: usize = 50;
const MAX_SKILL_BYTES: usize = 64_000;

#[derive(Debug, Default, Deserialize)]
struct ExtensionsConfig {
    enabled: Option<Vec<String>>,
}

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

#[derive(Debug, Clone)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

pub fn info(workspace: &Path) -> ExtensionsInfo {
    let skills_enabled = skills_enabled();
    ExtensionsInfo {
        config_path: config_dir().join(EXTENSIONS_FILE).display().to_string(),
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
                enabled: mcp_enabled(),
                removable: true,
                description: "MCP extension slot; protocol client not configured yet".into(),
            },
        ],
        skills: if skills_enabled {
            discover_skills(workspace)
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

pub fn skills_enabled() -> bool {
    extension_enabled("skills", true)
}

pub fn mcp_enabled() -> bool {
    extension_enabled("mcp", false)
}

fn extension_enabled(id: &str, default_enabled: bool) -> bool {
    let Some(config) = read_extensions_config() else {
        return default_enabled;
    };
    let Some(enabled) = config.enabled else {
        return default_enabled;
    };
    enabled.iter().any(|entry| entry == id)
}

fn read_extensions_config() -> Option<ExtensionsConfig> {
    let path = config_dir().join(EXTENSIONS_FILE);
    let content = fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

pub fn system_prompt(workspace: &Path) -> String {
    let mut sections = Vec::new();
    if skills_enabled() {
        let skills = discover_skills(workspace);
        if !skills.is_empty() {
            let mut lines = vec![
                "Skills extension active. Available skills are listed as name: description. When a task matches a skill, call skill.load with the skill name before following it.".to_string(),
                "Available skills:".to_string(),
            ];
            lines.extend(
                skills
                    .iter()
                    .map(|skill| format!("- {}: {}", skill.name, skill.description)),
            );
            sections.push(lines.join("\n"));
        }
    }
    if mcp_enabled() {
        sections.push(
            "MCP extension configured, but MCP runtime tools are not loaded in this build.".into(),
        );
    }
    sections.join("\n\n")
}

pub fn list_skills(workspace: &Path) -> String {
    if !skills_enabled() {
        return "status: failed\nerror: skills extension disabled".into();
    }
    let skills = discover_skills(workspace);
    if skills.is_empty() {
        return "status: ok\nskills: none".into();
    }
    let mut output = String::from("status: ok\nskills:\n");
    output.push_str(
        &skills
            .iter()
            .map(|skill| format!("- {}: {}", skill.name, skill.description))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    output
}

pub fn load_skill(workspace: &Path, name: &str) -> String {
    if !skills_enabled() {
        return "status: failed\nerror: skills extension disabled".into();
    }
    let name = name.trim();
    if name.is_empty() {
        return "status: failed\nerror: missing skill name".into();
    }
    let Some(skill) = discover_skills(workspace)
        .into_iter()
        .find(|skill| skill.name == name)
    else {
        return format!("status: failed\nerror: skill not found: {name}");
    };
    let Ok(content) = fs::read_to_string(&skill.path) else {
        return format!(
            "status: failed\nerror: skill read failed: {}",
            skill.path.display()
        );
    };
    let content = truncate_utf8(content, MAX_SKILL_BYTES);
    format!(
        "status: ok\nname: {}\npath: {}\n\n{}",
        skill.name,
        skill.path.display(),
        content
    )
}

pub fn discover_skills(workspace: &Path) -> Vec<SkillSummary> {
    let mut roots = skill_roots(workspace);
    roots.dedup();
    let mut skills = Vec::new();
    for root in roots {
        collect_skills_from_root(&root, &mut skills);
        if skills.len() >= MAX_SKILLS {
            break;
        }
    }
    dedupe_skills(skills)
}

fn skill_roots(workspace: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(CONFIG_DIR_NAME).join("skills"));
        roots.push(home.join(".agents").join("skills"));
    }
    for ancestor in workspace.ancestors() {
        roots.push(ancestor.join(CONFIG_DIR_NAME).join("skills"));
        roots.push(ancestor.join(".agents").join("skills"));
        if ancestor.join(".git").is_dir() {
            break;
        }
    }
    roots
}

fn collect_skills_from_root(root: &Path, skills: &mut Vec<SkillSummary>) {
    if !root.is_dir() {
        return;
    }
    if root.file_name().and_then(|name| name.to_str()) == Some("skills") {
        collect_root_markdown(root, skills);
    }
    collect_skill_dirs(root, skills, 0);
}

fn collect_root_markdown(root: &Path, skills: &mut Vec<SkillSummary>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        if let Some(skill) = parse_skill_file(&path) {
            skills.push(skill);
        }
        if skills.len() >= MAX_SKILLS {
            return;
        }
    }
}

fn collect_skill_dirs(root: &Path, skills: &mut Vec<SkillSummary>, depth: usize) {
    if depth > 4 || skills.len() >= MAX_SKILLS {
        return;
    }
    let skill_file = root.join("SKILL.md");
    if skill_file.is_file() {
        if let Some(skill) = parse_skill_file(&skill_file) {
            skills.push(skill);
        }
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_skill_dirs(&path, skills, depth + 1);
        }
        if skills.len() >= MAX_SKILLS {
            return;
        }
    }
}

fn parse_skill_file(path: &Path) -> Option<SkillSummary> {
    let content = fs::read_to_string(path).ok()?;
    let frontmatter = parse_frontmatter(&content)?;
    let name = frontmatter.get("name")?.trim().to_string();
    let description = frontmatter.get("description")?.trim().to_string();
    if name.is_empty() || description.is_empty() || !valid_skill_name(&name) {
        return None;
    }
    Some(SkillSummary {
        name,
        description,
        path: path.to_path_buf(),
    })
}

fn parse_frontmatter(content: &str) -> Option<std::collections::HashMap<String, String>> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut map = std::collections::HashMap::new();
    for line in lines {
        let line = line.trim();
        if line == "---" {
            return Some(map);
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        map.insert(
            key.trim().to_string(),
            value.trim().trim_matches('"').to_string(),
        );
    }
    None
}

fn valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn dedupe_skills(skills: Vec<SkillSummary>) -> Vec<SkillSummary> {
    let mut seen = std::collections::HashSet::new();
    skills
        .into_iter()
        .filter(|skill| seen.insert(skill.name.clone()))
        .collect()
}

fn truncate_utf8(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... truncated to {max_bytes} bytes", &value[..end])
}

fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(CONFIG_DIR_NAME)
}
