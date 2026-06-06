use super::{
    hooks::{HookDecision, HookEvent},
    manifest, protocol, valid_tool_name,
};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PROTOCOL: &str = "sandevistan.extension.v1";
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1500);
const MAX_STDOUT_BYTES: usize = 256_000;

pub fn emit(workspace: &Path, event: &HookEvent) -> Vec<HookDecision> {
    let hook = hook_name(event);
    let mut decisions = Vec::new();
    for manifest in manifest::discover(workspace) {
        if manifest.enabled != Some(true)
            || manifest.command.as_deref().unwrap_or_default().is_empty()
        {
            continue;
        }
        if !manifest.hooks.iter().any(|entry| entry == hook) {
            continue;
        }
        let request = protocol::ExtensionRequest {
            protocol: PROTOCOL.into(),
            request_id: request_id(),
            extension_id: manifest.id.clone(),
            workspace: workspace.display().to_string(),
            method: "hook".into(),
            event: Some(protocol_event(event)),
            tool_call: None,
        };
        let timeout = manifest
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_TIMEOUT);
        match call_extension(&manifest, &request, timeout) {
            Ok(response) => decisions.extend(response.decisions.into_iter().map(map_decision)),
            Err(error) => eprintln!("extension {} hook {hook} failed: {error}", manifest.id),
        }
    }
    decisions
}

pub fn extension_tools(workspace: &Path) -> Vec<(String, protocol::ExtensionToolSpec)> {
    let mut tools = Vec::new();
    for manifest in manifest::discover(workspace) {
        if manifest.enabled != Some(true)
            || manifest.command.as_deref().unwrap_or_default().is_empty()
        {
            continue;
        }
        let request = protocol::ExtensionRequest {
            protocol: PROTOCOL.into(),
            request_id: request_id(),
            extension_id: manifest.id.clone(),
            workspace: workspace.display().to_string(),
            method: "initialize".into(),
            event: None,
            tool_call: None,
        };
        let timeout = manifest
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_TIMEOUT);
        match call_extension(&manifest, &request, timeout) {
            Ok(response) => tools.extend(
                response
                    .tools
                    .into_iter()
                    .filter(|tool| valid_tool_name(&tool.name))
                    .map(|tool| (manifest.id.clone(), tool)),
            ),
            Err(error) => eprintln!("extension {} initialize failed: {error}", manifest.id),
        }
    }
    tools
}

pub fn execute_tool(
    workspace: &Path,
    extension_id: &str,
    tool_name: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    let manifest = manifest::discover(workspace)
        .into_iter()
        .find(|manifest| manifest.id == extension_id && manifest.enabled == Some(true))
        .ok_or_else(|| format!("extension not found or disabled: {extension_id}"))?;
    if manifest.command.as_deref().unwrap_or_default().is_empty() {
        return Err(format!("extension command missing: {extension_id}"));
    }
    let request = protocol::ExtensionRequest {
        protocol: PROTOCOL.into(),
        request_id: request_id(),
        extension_id: manifest.id.clone(),
        workspace: workspace.display().to_string(),
        method: "tool.execute".into(),
        event: None,
        tool_call: Some(protocol::ExtensionToolCall {
            name: tool_name.into(),
            args,
        }),
    };
    let timeout = manifest
        .timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_TIMEOUT);
    let response = call_extension(&manifest, &request, timeout)?;
    Ok(response.content.unwrap_or_else(|| "status: ok".into()))
}

fn call_extension(
    manifest: &manifest::ExtensionManifest,
    request: &protocol::ExtensionRequest,
    timeout: Duration,
) -> Result<protocol::ExtensionResponse, String> {
    let command = manifest
        .command
        .as_deref()
        .ok_or_else(|| "missing command".to_string())?;
    let mut child = Command::new(command)
        .args(&manifest.args)
        .current_dir(manifest.path.parent().unwrap_or_else(|| Path::new(".")))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn failed: {error}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        let input = serde_json::to_vec(request)
            .map_err(|error| format!("request encode failed: {error}"))?;
        stdin
            .write_all(&input)
            .map_err(|error| format!("stdin write failed: {error}"))?;
        stdin
            .write_all(b"\n")
            .map_err(|error| format!("stdin newline failed: {error}"))?;
    }
    drop(child.stdin.take());

    let start = Instant::now();
    loop {
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("timed out after {}ms", timeout.as_millis()));
        }
        if child
            .try_wait()
            .map_err(|error| format!("wait failed: {error}"))?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .map_err(|error| format!("output read failed: {error}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("exit {}: {}", output.status, stderr.trim()));
            }
            if output.stdout.len() > MAX_STDOUT_BYTES {
                return Err("stdout too large".into());
            }
            let response = serde_json::from_slice::<protocol::ExtensionResponse>(&output.stdout)
                .map_err(|error| format!("response parse failed: {error}"))?;
            return Ok(response);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn hook_name(event: &HookEvent) -> &'static str {
    match event {
        HookEvent::AgentStart => "agent_start",
        HookEvent::BeforeModelCall => "before_model_call",
        HookEvent::BeforeToolCall { .. } => "before_tool_call",
        HookEvent::AfterToolResult { .. } => "after_tool_result",
        HookEvent::AgentEnd => "agent_end",
        HookEvent::Error { .. } => "error",
    }
}

fn protocol_event(event: &HookEvent) -> protocol::ExtensionHookEvent {
    match event {
        HookEvent::AgentStart => protocol::ExtensionHookEvent::AgentStart,
        HookEvent::BeforeModelCall => protocol::ExtensionHookEvent::BeforeModelCall,
        HookEvent::BeforeToolCall { tool, args } => protocol::ExtensionHookEvent::BeforeToolCall {
            tool: tool.clone(),
            args: args.clone(),
        },
        HookEvent::AfterToolResult { tool, content } => {
            protocol::ExtensionHookEvent::AfterToolResult {
                tool: tool.clone(),
                content: content.clone(),
            }
        }
        HookEvent::AgentEnd => protocol::ExtensionHookEvent::AgentEnd,
        HookEvent::Error { message } => protocol::ExtensionHookEvent::Error {
            message: message.clone(),
        },
    }
}

fn map_decision(decision: protocol::ExtensionDecision) -> HookDecision {
    match decision {
        protocol::ExtensionDecision::Continue => HookDecision::Continue,
        protocol::ExtensionDecision::Block { reason } => HookDecision::Block { reason },
        protocol::ExtensionDecision::ModifyToolArgs { args } => {
            HookDecision::ModifyToolArgs { args }
        }
        protocol::ExtensionDecision::AppendSystemContext { content } => {
            HookDecision::AppendSystemContext { content }
        }
    }
}

fn request_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{}-{millis}", std::process::id())
}
