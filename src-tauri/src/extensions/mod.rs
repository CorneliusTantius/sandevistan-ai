use serde::Serialize;
use std::{collections::HashMap, path::Path};

pub mod config;
pub mod external;
pub mod hooks;
pub mod manifest;
pub mod protocol;
pub mod scaffold;

#[derive(Debug, Serialize)]
pub struct ExtensionsInfo {
    pub config_path: String,
    pub extensions: Vec<ExtensionInfo>,
}

#[derive(Debug, Serialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub removable: bool,
    pub description: String,
    pub path: Option<String>,
    pub hooks: Vec<String>,
    pub tools: Vec<ExtensionToolInfo>,
}

#[derive(Debug, Serialize)]
pub struct ExtensionToolInfo {
    pub name: String,
    pub description: String,
}

pub fn info(workspace: &Path) -> ExtensionsInfo {
    let mut tools_by_extension: HashMap<String, Vec<ExtensionToolInfo>> = HashMap::new();
    for (extension_id, tool) in external::extension_tools(workspace) {
        tools_by_extension
            .entry(extension_id)
            .or_default()
            .push(ExtensionToolInfo {
                name: tool.name,
                description: tool.description,
            });
    }
    ExtensionsInfo {
        config_path: config::config_path().display().to_string(),
        extensions: manifest::discover(workspace)
            .into_iter()
            .map(|manifest| ExtensionInfo {
                id: manifest.id.clone(),
                name: manifest.name.clone().unwrap_or_else(|| manifest.id.clone()),
                enabled: manifest_enabled(&manifest),
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
                hooks: manifest.hooks,
                tools: tools_by_extension.remove(&manifest.id).unwrap_or_default(),
            })
            .collect(),
    }
}

pub fn hook_bus(workspace: &Path) -> hooks::HookBus<'_> {
    hooks::HookBus::new(workspace)
}

pub fn system_prompt(workspace: &Path) -> String {
    hook_bus(workspace).system_context()
}

pub fn is_extension_tool(name: &str) -> bool {
    parse_extension_tool_name(name).is_some()
}

pub fn parse_extension_tool_name(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix("ext.")?;
    let (extension_id, tool_name) = rest.split_once('.')?;
    if extension_id.is_empty() || tool_name.is_empty() {
        return None;
    }
    Some((extension_id.to_string(), tool_name.to_string()))
}

pub fn tool_specs(workspace: &Path) -> Vec<crate::runtime_wire::NativeToolSpec> {
    external::extension_tools(workspace)
        .into_iter()
        .map(|(extension_id, tool)| {
            let internal_name = format!("ext.{}.{}", extension_id, tool.name);
            crate::runtime_wire::NativeToolSpec {
                openai_name: openai_tool_name(&extension_id, &tool.name),
                name: internal_name,
                description: tool.description,
                parameters: tool.parameters,
            }
        })
        .collect()
}

pub fn execute_tool(workspace: &Path, name: &str, args: serde_json::Value) -> String {
    let Some((extension_id, tool_name)) = parse_extension_tool_name(name) else {
        return "status: failed\nerror: invalid extension tool name".into();
    };
    match external::execute_tool(workspace, &extension_id, &tool_name, args) {
        Ok(content) => content,
        Err(error) => format!("status: failed\nerror: {error}"),
    }
}

pub fn original_tool_name(openai_name: &str) -> Option<String> {
    let rest = openai_name.strip_prefix("ext__")?;
    let (extension_id, tool_name) = rest.split_once("__")?;
    Some(format!(
        "ext.{}.{}",
        decode_name(extension_id)?,
        decode_name(tool_name)?
    ))
}

fn manifest_enabled(manifest: &manifest::ExtensionManifest) -> bool {
    config::extension_enabled(&manifest.id, manifest.enabled.unwrap_or(false))
}

pub fn create_rust_extension(
    request: scaffold::CreateRustExtensionRequest,
) -> Result<String, String> {
    scaffold::create_rust_extension(request).map(|path| path.display().to_string())
}

pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("extension id is empty".into());
    }
    config::set_extension_enabled(id, enabled)
}

fn openai_tool_name(extension_id: &str, tool_name: &str) -> String {
    format!(
        "ext__{}__{}",
        encode_name(extension_id),
        encode_name(tool_name)
    )
}

fn encode_name(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            'a'..='z' | '0'..='9' => out.push(c),
            '_' => out.push_str("_u"),
            '-' => out.push_str("_d"),
            '.' => out.push_str("_p"),
            _ => out.push('_'),
        }
    }
    out
}

fn decode_name(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '_' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'u' => out.push('_'),
            'd' => out.push('-'),
            'p' => out.push('.'),
            _ => return None,
        }
    }
    Some(out)
}

pub(super) fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
        })
}
