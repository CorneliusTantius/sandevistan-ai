use crate::ai;
use sandevistan_core::NativeToolSpec;
use serde_json::Value;
use std::path::Path;

use super::{executors, schema, validation, ToolOptions};

type ToolExecutor = fn(&Path, &Value, ToolOptions) -> Result<String, String>;
type ToolValidator = fn(&Value, &ai::ModelMods) -> Result<(), String>;
type ToolParameters = fn(&[String]) -> Value;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ToolEntry {
    pub name: &'static str,
    pub openai_name: &'static str,
    pub description: &'static str,
    pub mutating: bool,
    pub shell_only: bool,
    pub delegate_only: bool,
    pub parameters: ToolParameters,
    pub validate: ToolValidator,
    pub execute: ToolExecutor,
}

pub(crate) const TOOL_ENTRIES: &[ToolEntry] = &[
    ToolEntry { name: "fs.list", openai_name: "fs_list", description: "list directory entries", mutating: false, shell_only: false, delegate_only: false, parameters: schema::params_fs_list, validate: validation::validate_fs_list, execute: executors::exec_fs_list },
    ToolEntry { name: "fs.read", openai_name: "fs_read", description: "read UTF-8 file, capped", mutating: false, shell_only: false, delegate_only: false, parameters: schema::params_fs_read, validate: validation::validate_fs_read, execute: executors::exec_fs_read },
    ToolEntry { name: "fs.edit", openai_name: "fs_edit", description: "exact replace in existing file; creates backup", mutating: true, shell_only: false, delegate_only: false, parameters: schema::params_fs_edit, validate: validation::validate_fs_edit, execute: executors::exec_fs_edit },
    ToolEntry { name: "fs.write", openai_name: "fs_write", description: "create/overwrite file; creates backup when replacing", mutating: true, shell_only: false, delegate_only: false, parameters: schema::params_fs_write, validate: validation::validate_fs_write, execute: executors::exec_fs_write },
    ToolEntry { name: "search.rg", openai_name: "search_rg", description: "content search via ripgrep, respects ignore files, capped", mutating: false, shell_only: false, delegate_only: false, parameters: schema::params_search_rg, validate: validation::validate_search_rg, execute: executors::exec_search_rg },
    ToolEntry { name: "git.status", openai_name: "git_status", description: "git branch + porcelain status", mutating: false, shell_only: false, delegate_only: false, parameters: schema::params_empty, validate: validation::validate_noop, execute: executors::exec_git_status },
    ToolEntry { name: "git.diff", openai_name: "git_diff", description: "git diff, optional path, capped", mutating: false, shell_only: false, delegate_only: false, parameters: schema::params_git_diff, validate: validation::validate_git_diff, execute: executors::exec_git_diff },
    ToolEntry { name: "shell.run", openai_name: "shell_run", description: "run shell command in workspace; timeout/capped output", mutating: true, shell_only: true, delegate_only: false, parameters: schema::params_shell_run, validate: validation::validate_shell_run, execute: executors::exec_shell_run },
    ToolEntry { name: "agent.delegate", openai_name: "agent_delegate", description: "run selected subagents concurrently; provide many small specific independent tasks, not one broad task", mutating: false, shell_only: false, delegate_only: true, parameters: schema::params_agent_delegate, validate: validation::validate_agent_delegate, execute: executors::exec_agent_delegate },
];

pub(crate) fn find_tool_entry(name: &str) -> Option<&'static ToolEntry> {
    TOOL_ENTRIES.iter().find(|entry| entry.name == name)
}

pub(crate) fn spec(entry: &ToolEntry, parameters: Value) -> NativeToolSpec {
    NativeToolSpec {
        name: entry.name.into(),
        openai_name: entry.openai_name.into(),
        description: entry.description.into(),
        parameters,
    }
}
