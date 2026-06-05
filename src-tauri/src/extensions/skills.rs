use std::{
    fs,
    path::{Path, PathBuf},
};

use super::config;

const MAX_SKILLS: usize = 50;
const MAX_SKILL_BYTES: usize = 64_000;

#[derive(Debug, Clone)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

pub fn enabled() -> bool {
    config::extension_enabled("skills", true)
}

pub fn system_prompt(workspace: &Path) -> Option<String> {
    if !enabled() {
        return None;
    }
    let skills = discover(workspace);
    if skills.is_empty() {
        return None;
    }
    let mut lines = vec![
        "Skills extension active. Available skills are listed as name: description. When a task matches a skill, call skill.load with the skill name before following it.".to_string(),
        "Available skills:".to_string(),
    ];
    lines.extend(
        skills
            .iter()
            .map(|skill| format!("- {}: {}", skill.name, skill.description)),
    );
    Some(lines.join("\n"))
}

pub fn list(workspace: &Path) -> String {
    if !enabled() {
        return "status: failed\nerror: skills extension disabled".into();
    }
    let skills = discover(workspace);
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

pub fn load(workspace: &Path, name: &str) -> String {
    if !enabled() {
        return "status: failed\nerror: skills extension disabled".into();
    }
    let name = name.trim();
    if name.is_empty() {
        return "status: failed\nerror: missing skill name".into();
    }
    let Some(skill) = discover(workspace)
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

pub fn discover(workspace: &Path) -> Vec<SkillSummary> {
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
        roots.push(home.join(".sandevistan").join("skills"));
        roots.push(home.join(".agents").join("skills"));
    }
    for ancestor in workspace.ancestors() {
        roots.push(ancestor.join(".sandevistan").join("skills"));
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
