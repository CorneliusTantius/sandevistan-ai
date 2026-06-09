use crate::wire::NativeToolSpec;
use serde_json::Value;
use std::{future::Future, pin::Pin};

pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = String> + Send + 'a>>;

pub trait ToolHost: Send + Sync {
    fn system_prompt(&self) -> String;
    fn specs(&self) -> Vec<NativeToolSpec>;
    fn run<'a>(&'a self, request: ToolRequest) -> ToolFuture<'a>;
}

#[derive(Debug, Clone)]
pub struct ToolRequest {
    pub session_id: String,
    pub name: String,
    pub args: Value,
    pub read_only: bool,
    pub delegate_depth_remaining: usize,
}

pub fn default_system_prompt(
    subagents_enabled: bool,
    subagents: &[String],
    shell_enabled: bool,
) -> String {
    let mut lines = vec![
        "You are Sandevistan, a concise coding agent.".to_string(),
        "Use tools when workspace context is needed; otherwise answer directly.".to_string(),
        "Prefer targeted reads/searches over broad exploration.".to_string(),
        "Strictly use targeted commands: inspect specific files, paths, symbols, line ranges, or narrow search patterns. Do not run broad/noisy commands like full-tree cat/ls/find/rg, huge diffs, or verbose builds unless explicitly needed.".to_string(),
        "After tool results, answer briefly with final useful result.".to_string(),
        "Do not call tools after task is complete.".to_string(),
        "Prefer fs.edit for existing files. Use fs.write for new files.".to_string(),
    ];
    if shell_enabled {
        lines.push("Use shell.run only when shell/CLI execution is clearly useful. Prefer read-only commands unless user asks for mutation.".into());
    }
    if subagents_enabled && !subagents.is_empty() {
        lines.push(format!("Subagents are available via agent.delegate: {}. Split delegation into multiple small, specific, independent tasks so subagents run concurrently; avoid one broad subagent task.", subagents.join(", ")));
    }
    lines.join("\n")
}
