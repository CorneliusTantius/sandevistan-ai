use crate::{ai, runtime_wire::NativeToolSpec, tools::ToolCall};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const MAX_FRAME_BYTES: usize = 2_000_000;
const MAX_OUTPUT_CHARS: usize = 64_000;

#[derive(Debug, Default, Deserialize)]
struct McpConfig {
    #[serde(default)]
    servers: Vec<McpServer>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpServer {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    timeout_ms: Option<u64>,
}

pub fn is_mcp_tool(name: &str) -> bool {
    matches!(name, "mcp.list" | "mcp.call")
}

pub fn original_tool_name(name: &str) -> Option<&'static str> {
    match name {
        "mcp_list" => Some("mcp.list"),
        "mcp_call" => Some("mcp.call"),
        _ => None,
    }
}

pub fn tool_specs() -> Vec<NativeToolSpec> {
    vec![
        NativeToolSpec {
            name: "mcp.list".into(),
            openai_name: "mcp_list".into(),
            description: "list tools exposed by configured MCP servers".into(),
            parameters: json!({"type":"object","properties":{"server":{"type":"string","description":"optional MCP server name"}},"required":[],"additionalProperties":false}),
        },
        NativeToolSpec {
            name: "mcp.call".into(),
            openai_name: "mcp_call".into(),
            description: "call a tool on a configured MCP server".into(),
            parameters: json!({"type":"object","properties":{"server":{"type":"string"},"tool":{"type":"string"},"args":{"type":"object","default":{}}},"required":["server","tool"],"additionalProperties":false}),
        },
    ]
}

pub fn validate_tool_call(call: &ToolCall, mods: &ai::ModelMods) -> Result<(), String> {
    if !mods.mcp_enabled {
        return Err("MCP disabled for this profile".into());
    }
    if !call.args.is_object() {
        return Err("args must be a JSON object".into());
    }
    match call.name.as_str() {
        "mcp.list" => ensure_args(&call.args, &["server"]),
        "mcp.call" => {
            ensure_args(&call.args, &["server", "tool", "args"])?;
            required_string(&call.args, "server")?;
            required_string(&call.args, "tool")?;
            if let Some(args) = call.args.get("args") {
                if !args.is_object() {
                    return Err("args must be an object".into());
                }
            }
            Ok(())
        }
        _ => Err(format!("unknown MCP tool: {}", call.name)),
    }
}

pub async fn run(workspace: &Path, call: ToolCall, config: &str) -> String {
    match run_inner(workspace, call, config).await {
        Ok(output) => format!("status: ok\n{}", truncate(output)),
        Err(error) => format!("status: failed\nerror: {error}"),
    }
}

async fn run_inner(workspace: &Path, call: ToolCall, config: &str) -> Result<String, String> {
    let parsed = parse_config(config)?;
    if parsed.servers.is_empty() {
        return Err("no MCP servers configured".into());
    }
    match call.name.as_str() {
        "mcp.list" => {
            let filter = call
                .args
                .get("server")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty());
            let mut out = Vec::new();
            for server in parsed
                .servers
                .iter()
                .filter(|s| filter.map_or(true, |name| s.name == name))
            {
                let tools = with_client(workspace, server, |client| async move {
                    client.list_tools().await
                })
                .await?;
                out.push(format!("server: {}", server.name));
                if tools.is_empty() {
                    out.push("tools: none".into());
                } else {
                    for tool in tools {
                        out.push(format!(
                            "- {}: {}",
                            tool.name,
                            tool.description.unwrap_or_default()
                        ));
                    }
                }
            }
            if out.is_empty() {
                Err("MCP server not found".into())
            } else {
                Ok(out.join("\n"))
            }
        }
        "mcp.call" => {
            let server_name = call
                .args
                .get("server")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let tool_name = call
                .args
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = call.args.get("args").cloned().unwrap_or_else(|| json!({}));
            let server = parsed
                .servers
                .iter()
                .find(|server| server.name == server_name)
                .ok_or_else(|| format!("MCP server not found: {server_name}"))?;
            with_client(workspace, server, |client| async move {
                client.call_tool(tool_name, args).await
            })
            .await
        }
        _ => Err("invalid MCP tool".into()),
    }
}

fn parse_config(config: &str) -> Result<McpConfig, String> {
    let trimmed = config.trim();
    if trimmed.is_empty() {
        return Ok(McpConfig::default());
    }
    toml::from_str::<McpConfig>(trimmed)
        .map_err(|error| format!("MCP config parse failed: {error}"))
}

async fn with_client<F, Fut, T>(workspace: &Path, server: &McpServer, run: F) -> Result<T, String>
where
    F: FnOnce(McpClient) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let timeout = Duration::from_millis(
        server
            .timeout_ms
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .clamp(1_000, 60_000),
    );
    let mut client = McpClient::start(workspace, server).await?;
    let result = tokio::time::timeout(timeout, async {
        client.initialize().await?;
        run(client).await
    })
    .await
    .map_err(|_| format!("MCP server timed out after {}ms", timeout.as_millis()))?;
    result
}

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

#[derive(Debug)]
struct ListedTool {
    name: String,
    description: Option<String>,
}

impl McpClient {
    async fn start(workspace: &Path, server: &McpServer) -> Result<Self, String> {
        let mut cmd = Command::new(&server.command);
        cmd.current_dir(workspace)
            .args(&server.args)
            .envs(&server.env);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|error| format!("MCP server start failed: {error}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "MCP stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "MCP stdout unavailable".to_string())?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.request("initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"sandevistan","version":"1.1"}})).await?;
        self.notify("notifications/initialized", json!({})).await
    }

    async fn list_tools(mut self) -> Result<Vec<ListedTool>, String> {
        let value = self.request("tools/list", json!({})).await?;
        let _ = self.child.kill().await;
        let tools = value
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(tools
            .into_iter()
            .filter_map(|tool| {
                Some(ListedTool {
                    name: tool.get("name")?.as_str()?.to_string(),
                    description: tool
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect())
    }

    async fn call_tool(mut self, name: &str, args: Value) -> Result<String, String> {
        let value = self
            .request("tools/call", json!({"name": name, "arguments": args}))
            .await?;
        let _ = self.child.kill().await;
        Ok(extract_tool_content(&value))
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.write(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        loop {
            let msg = self.read().await?;
            if msg.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = msg.get("error") {
                return Err(error.to_string());
            }
            return Ok(msg.get("result").cloned().unwrap_or_else(|| json!({})));
        }
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.write(json!({"jsonrpc":"2.0","method":method,"params":params}))
            .await
    }

    async fn write(&mut self, value: Value) -> Result<(), String> {
        let body = value.to_string();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.stdin
            .write_all(header.as_bytes())
            .await
            .map_err(|error| format!("MCP write failed: {error}"))?;
        self.stdin
            .write_all(body.as_bytes())
            .await
            .map_err(|error| format!("MCP write failed: {error}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| format!("MCP flush failed: {error}"))
    }

    async fn read(&mut self) -> Result<Value, String> {
        let mut len = None;
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|error| format!("MCP read failed: {error}"))?;
            if read == 0 {
                return Err("MCP server closed stdout".into());
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                len = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "invalid MCP Content-Length".to_string())?,
                );
            }
        }
        let len = len.ok_or_else(|| "missing MCP Content-Length".to_string())?;
        if len > MAX_FRAME_BYTES {
            return Err("MCP response too large".into());
        }
        let mut body = vec![0; len];
        self.stdout
            .read_exact(&mut body)
            .await
            .map_err(|error| format!("MCP body read failed: {error}"))?;
        serde_json::from_slice(&body).map_err(|error| format!("MCP JSON parse failed: {error}"))
    }
}

fn extract_tool_content(value: &Value) -> String {
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        let parts = content
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| item.get("data").map(Value::to_string))
            })
            .collect::<Vec<_>>();
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    value.to_string()
}

fn ensure_args(args: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = args
        .as_object()
        .ok_or_else(|| "args must be an object".to_string())?;
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unexpected argument: {key}"));
        }
    }
    Ok(())
}

fn required_string(args: &Value, key: &str) -> Result<(), String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|_| ())
        .ok_or_else(|| format!("missing {key}"))
}

fn truncate(value: String) -> String {
    if value.len() <= MAX_OUTPUT_CHARS {
        return value;
    }
    let mut end = MAX_OUTPUT_CHARS;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... truncated", &value[..end])
}
