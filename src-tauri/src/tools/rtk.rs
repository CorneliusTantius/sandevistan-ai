use crate::command_utils;
use std::{env, path::{Path, PathBuf}, process::Command, sync::OnceLock};

use super::{
    arg_bool, arg_string, arg_usize, clean_relative, resolve_existing, truncate_string,
    constants::{MAX_DIFF_BYTES, MAX_SEARCH_RESULTS, MAX_TOOL_OUTPUT_BYTES, TOOL_COMMAND_TIMEOUT},
};

pub(crate) fn rtk_list(workspace: &Path, relative: &str) -> Result<String, String> {
    let path = resolve_existing(workspace, relative)?;
    if !path.is_dir() {
        return Err("path is not a directory".into());
    }
    let mut command = rtk_command()?;
    command.current_dir(workspace).arg("ls").arg("--ultra-compact").arg(path);
    rtk_output("rtk-ls", &mut command, MAX_TOOL_OUTPUT_BYTES)
}

pub(crate) fn rtk_read(workspace: &Path, relative: &str) -> Result<String, String> {
    let path = resolve_existing(workspace, relative)?;
    if !path.is_file() {
        return Err("path is not a file".into());
    }
    let mut command = rtk_command()?;
    command
        .current_dir(workspace)
        .arg("read")
        .arg("--ultra-compact")
        .arg("--level")
        .arg("minimal")
        .arg("--max-lines")
        .arg("800")
        .arg(clean_relative(relative)?);
    rtk_output("rtk-read", &mut command, MAX_TOOL_OUTPUT_BYTES)
}

pub(crate) fn rtk_grep(workspace: &Path, args: &serde_json::Value) -> Result<String, String> {
    let query = arg_string(args, "query").ok_or_else(|| "missing query".to_string())?;
    if query.trim().is_empty() {
        return Err("query is empty".into());
    }
    let max_results = arg_usize(args, "max_results").unwrap_or(MAX_SEARCH_RESULTS).clamp(1, MAX_SEARCH_RESULTS);
    let root = arg_string(args, "path").unwrap_or_else(|| ".".into());
    resolve_existing(workspace, &root)?;

    let mut command = rtk_command()?;
    command
        .current_dir(workspace)
        .arg("grep")
        .arg("--ultra-compact")
        .arg("--max")
        .arg(max_results.to_string())
        .arg(&query)
        .arg(clean_relative(&root)?);
    if arg_bool(args, "case_sensitive").unwrap_or(false) {
        command.arg("--case-sensitive");
    } else {
        command.arg("--smart-case");
    }
    rtk_output("rtk-grep", &mut command, MAX_TOOL_OUTPUT_BYTES)
}

pub(crate) fn rtk_git_status(workspace: &Path) -> Result<String, String> {
    let mut command = rtk_command()?;
    command.current_dir(workspace).arg("git").arg("--ultra-compact").arg("status").arg("--porcelain=v1").arg("--branch");
    rtk_output("rtk-git", &mut command, MAX_TOOL_OUTPUT_BYTES)
}

pub(crate) fn rtk_git_diff(workspace: &Path, args: &serde_json::Value) -> Result<String, String> {
    let mut command = rtk_command()?;
    command.current_dir(workspace).arg("git").arg("--ultra-compact").arg("diff");
    if let Some(relative) = arg_string(args, "path") {
        clean_relative(&relative)?;
        command.arg("--").arg(relative);
    }
    let output = rtk_output("rtk-git", &mut command, MAX_DIFF_BYTES)?;
    if output.trim().is_empty() { Ok("no diff".into()) } else { Ok(output) }
}

pub(crate) fn rtk_output(label: &str, command: &mut Command, max_bytes: usize) -> Result<String, String> {
    let output = command_utils::output_with_timeout(label, command, TOOL_COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(truncate_string(String::from_utf8_lossy(&output.stdout).to_string(), max_bytes))
}

pub(crate) fn rtk_available() -> bool {
    rtk_path().is_some()
}

pub(crate) fn rtk_command() -> Result<Command, String> {
    rtk_path().map(Command::new).ok_or_else(|| "rtk enabled but unavailable on PATH".into())
}

fn rtk_path() -> Option<PathBuf> {
    static RTK_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    RTK_PATH
        .get_or_init(|| {
            env::var_os("PATH")
                .into_iter()
                .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
                .chain(rtk_fallback_dirs())
                .flat_map(|dir| rtk_executable_names().iter().map(move |name| dir.join(name)))
                .find(|path| path.is_file())
        })
        .clone()
}

fn rtk_executable_names() -> &'static [&'static str] {
    if cfg!(windows) { &["rtk.exe", "rtk.cmd", "rtk.bat", "rtk"] } else { &["rtk"] }
}

fn rtk_fallback_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".cargo/bin"));
    }
    if cfg!(windows) {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local_app_data).join("Programs").join("rtk"));
        }
    }
    dirs
}
