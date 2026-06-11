use crate::ai;
use sandevistan_core::NativeToolSpec;
use std::path::Path;

use super::{
    constants::MAX_TOOL_OUTPUT_BYTES,
    entries::{find_tool_entry, spec, TOOL_ENTRIES},
    types::{ToolCall, ToolOptions, ToolRunResult},
};

pub fn native_system_prompt(
    subagents_enabled: bool,
    subagents: &[String],
    shell_enabled: bool,
) -> String {
    sandevistan_core::tools::default_system_prompt(subagents_enabled, subagents, shell_enabled)
}

pub fn original_tool_name(name: &str) -> Option<&'static str> {
    match name {
        "fs_list" => Some("fs.list"),
        "fs_read" => Some("fs.read"),
        "fs_edit" => Some("fs.edit"),
        "fs_write" => Some("fs.write"),
        "search_rg" => Some("search.rg"),
        "git_status" => Some("git.status"),
        "git_diff" => Some("git.diff"),
        "shell_run" => Some("shell.run"),
        "agent_delegate" => Some("agent.delegate"),
        _ => None,
    }
}

pub struct ToolRegistry {
    subagents_enabled: bool,
    subagents: Vec<String>,
    shell_enabled: bool,
    read_only: bool,
}

#[derive(Debug, Clone)]
pub struct ValidatedToolCall {
    pub call: ToolCall,
}

impl ToolRegistry {
    pub fn new(
        subagents_enabled: bool,
        subagents: &[String],
        shell_enabled: bool,
        read_only: bool,
    ) -> Self {
        Self {
            subagents_enabled,
            subagents: subagents.to_vec(),
            shell_enabled,
            read_only,
        }
    }

    pub fn specs(&self) -> Vec<NativeToolSpec> {
        TOOL_ENTRIES
            .iter()
            .filter(|entry| self.entry_enabled(entry))
            .map(|entry| spec(entry, (entry.parameters)(&self.subagents)))
            .collect()
    }

    pub fn validate(
        &self,
        call: ToolCall,
        mods: &ai::ModelMods,
    ) -> Result<ValidatedToolCall, String> {
        validate_tool_call(call, mods, self.read_only)
    }

    fn entry_enabled(&self, entry: &super::entries::ToolEntry) -> bool {
        if self.read_only && entry.mutating {
            return false;
        }
        if entry.shell_only && !self.shell_enabled {
            return false;
        }
        if entry.delegate_only && (!self.subagents_enabled || self.subagents.is_empty()) {
            return false;
        }
        true
    }
}

pub fn run_with_options(workspace: &Path, call: &ToolCall, options: ToolOptions) -> String {
    let result = run_result(workspace, call, options);
    let status = if result.ok { "ok" } else { "failed" };
    let output = super::truncate_string(result.output, MAX_TOOL_OUTPUT_BYTES);
    format!("status: {status}\n{output}")
}

fn run_result(workspace: &Path, call: &ToolCall, options: ToolOptions) -> ToolRunResult {
    let result = find_tool_entry(&call.name)
        .map(|entry| (entry.execute)(workspace, &call.args, options))
        .unwrap_or_else(|| Err(format!("unknown tool: {}", call.name)));

    match result {
        Ok(output) => ToolRunResult { ok: true, output },
        Err(error) => ToolRunResult {
            ok: false,
            output: format!("error: {error}"),
        },
    }
}

pub fn validate_tool_call(
    call: ToolCall,
    mods: &ai::ModelMods,
    read_only: bool,
) -> Result<ValidatedToolCall, String> {
    let entry = find_tool_entry(&call.name).ok_or_else(|| format!("unknown tool: {}", call.name))?;
    if !call.args.is_object() {
        return Err("args must be a JSON object".into());
    }
    if read_only && entry.mutating {
        return Err("mutating tools disabled in read-only runtime".into());
    }
    if entry.shell_only && !mods.shell_enabled {
        return Err("shell.run disabled for this profile".into());
    }
    if entry.delegate_only && (!mods.subagents_enabled || mods.subagents.is_empty()) {
        return Err("agent.delegate disabled for this profile".into());
    }
    (entry.validate)(&call.args, mods)?;
    Ok(ValidatedToolCall { call })
}
