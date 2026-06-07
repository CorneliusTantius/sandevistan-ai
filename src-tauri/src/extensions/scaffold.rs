use serde::Deserialize;
use std::{fs, path::PathBuf};

use super::config;

#[derive(Debug, Deserialize)]
pub struct CreateRustExtensionRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub fn create_rust_extension(request: CreateRustExtensionRequest) -> Result<PathBuf, String> {
    let id = clean_id(&request.id)?;
    let name = request
        .name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let description = request
        .description
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Sandevistan native Rust extension".into());
    let dir = config::config_dir().join("extensions").join(&id);
    if dir.exists() {
        return Err(format!("extension already exists: {}", dir.display()));
    }
    fs::create_dir_all(dir.join("src"))
        .map_err(|error| format!("extension dir create failed: {error}"))?;
    fs::write(
        dir.join("extension.toml"),
        manifest_template(&id, &name, &description),
    )
    .map_err(|error| format!("manifest write failed: {error}"))?;
    fs::write(dir.join("Cargo.toml"), cargo_template(&id))
        .map_err(|error| format!("Cargo.toml write failed: {error}"))?;
    fs::write(dir.join("src").join("main.rs"), main_template())
        .map_err(|error| format!("main.rs write failed: {error}"))?;
    Ok(dir)
}

fn clean_id(value: &str) -> Result<String, String> {
    let id = value.trim().to_lowercase();
    if id.is_empty()
        || id.len() > 64
        || id.starts_with('-')
        || id.ends_with('-')
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("invalid extension id; use lowercase letters, numbers, hyphens".into());
    }
    Ok(id)
}

fn manifest_template(id: &str, name: &str, description: &str) -> String {
    format!(
        r#"id = "{id}"
name = "{name}"
description = "{description}"
enabled = false
hooks = ["before_model_call", "before_tool_call"]
timeout_ms = 1500

[commands]
default = "target/release/{id}"
windows = "target/release/{id}.exe"
"#
    )
}

fn cargo_template(id: &str) -> String {
    format!(
        r#"[package]
name = "{id}"
version = "0.1.0"
edition = "2021"

[dependencies]
serde_json = "1"
"#
    )
}

fn main_template() -> &'static str {
    r#"use serde_json::{json, Value};
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let request: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));

    match request.get("method").and_then(Value::as_str).unwrap_or_default() {
        "initialize" => initialize(),
        "tool.execute" => execute_tool(&request),
        "hook" => handle_hook(&request),
        _ => println!("{}", json!({ "decisions": [] })),
    }
}

fn initialize() {
    println!("{}", json!({
        "tools": [{
            "name": "greet",
            "description": "Greet someone by name",
            "parameters": {
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"],
                "additionalProperties": false
            }
        }]
    }));
}

fn execute_tool(request: &Value) {
    let name = request
        .pointer("/tool_call/args/name")
        .and_then(Value::as_str)
        .unwrap_or("world");
    println!("{}", json!({ "content": format!("status: ok\nhello {name}") }));
}

fn handle_hook(request: &Value) {
    if request.pointer("/event/type").and_then(Value::as_str) == Some("before_model_call") {
        println!("{}", json!({
            "decisions": [{ "action": "append_system_context", "content": "Rust extension active." }]
        }));
        return;
    }
    println!("{}", json!({ "decisions": [] }));
}
"#
}
