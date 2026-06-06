use super::config;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MCP_FILE: &str = "mcp.json";
const MCP_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Default, Deserialize)]
struct McpConfig {
    #[serde(default)]
    servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

pub fn enabled() -> bool {
    config::extension_enabled("mcp", false)
}

pub fn system_prompt() -> Option<String> {
    enabled().then(|| {
        "MCP extension active. Use mcp.list to inspect configured servers and mcp.call to call server tools.".into()
    })
}

pub fn list_servers() -> String {
    if !enabled() {
        return "status: failed\nerror: mcp extension disabled".into();
    }
    let config = read_config();
    if config.servers.is_empty() {
        return format!(
            "status: ok\nservers: none\nconfig: {}",
            config_path().display()
        );
    }
    let mut output = format!("status: ok\nconfig: {}\nservers:", config_path().display());
    for (name, server) in config.servers {
        output.push_str(&format!(
            "\n- {name}: {} {}",
            server.command,
            server.args.join(" ")
        ));
        match request(&server, "tools/list", json!({}), Path::new(".")) {
            Ok(value) => {
                let tools = value
                    .get("tools")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                if !tools.is_empty() {
                    output.push_str(&format!("\n  tools: {tools}"));
                }
            }
            Err(error) => output.push_str(&format!("\n  error: {error}")),
        }
    }
    output
}

pub fn call_tool(workspace: &Path, server: &str, tool: &str, arguments: Value) -> String {
    if !enabled() {
        return "status: failed\nerror: mcp extension disabled".into();
    }
    let config = read_config();
    let Some(server_config) = config.servers.get(server) else {
        return format!("status: failed\nerror: mcp server not found: {server}");
    };
    let params = json!({ "name": tool, "arguments": arguments });
    match request(server_config, "tools/call", params, workspace) {
        Ok(value) => format!("status: ok\n{}", format_mcp_result(&value)),
        Err(error) => format!("status: failed\nerror: {error}"),
    }
}

fn request(
    server: &McpServerConfig,
    method: &str,
    params: Value,
    workspace: &Path,
) -> Result<Value, String> {
    let mut child = Command::new(&server.command)
        .args(&server.args)
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("mcp server spawn failed: {error}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "mcp stdin unavailable".to_string())?;
    write_request(
        &mut stdin,
        1,
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "sandevistan", "version": env!("CARGO_PKG_VERSION") }
        }),
    )?;
    write_notification(&mut stdin, "notifications/initialized", json!({}))?;
    write_request(&mut stdin, 2, method, params)?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "mcp stdout unavailable".to_string())?;
    let mut reader = BufReader::new(stdout);
    let start = Instant::now();
    let mut line = String::new();
    while start.elapsed() < MCP_TIMEOUT {
        line.clear();
        let count = reader
            .read_line(&mut line)
            .map_err(|error| format!("mcp stdout read failed: {error}"))?;
        if count == 0 {
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("id").and_then(Value::as_i64) != Some(2) {
            continue;
        }
        let _ = child.kill();
        let _ = child.wait();
        if let Some(error) = value.get("error") {
            return Err(error.to_string());
        }
        return Ok(value.get("result").cloned().unwrap_or_else(|| json!({})));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(format!(
        "mcp request timed out after {}s",
        MCP_TIMEOUT.as_secs()
    ))
}

fn write_request(
    stdin: &mut impl Write,
    id: i64,
    method: &str,
    params: Value,
) -> Result<(), String> {
    let value = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    writeln!(stdin, "{value}").map_err(|error| format!("mcp stdin write failed: {error}"))
}

fn write_notification(stdin: &mut impl Write, method: &str, params: Value) -> Result<(), String> {
    let value = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    writeln!(stdin, "{value}").map_err(|error| format!("mcp stdin write failed: {error}"))
}

fn format_mcp_result(value: &Value) -> String {
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        return content
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| item.get("resource").map(Value::to_string))
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    value.to_string()
}

fn read_config() -> McpConfig {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn config_path() -> PathBuf {
    config::config_dir().join(MCP_FILE)
}
