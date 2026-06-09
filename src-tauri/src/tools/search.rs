use crate::command_utils;
use serde_json::Value;
use std::{fs, path::Path, process::Command, time::Instant};

use super::{
    arg_bool, arg_string, arg_usize, ignored_name, resolve_existing,
    constants::{MAX_READ_BYTES, MAX_SEARCH_RESULTS, TOOL_COMMAND_TIMEOUT},
};

pub(crate) fn ripgrep(workspace: &Path, args: &Value) -> Result<String, String> {
    let query = arg_string(args, "query").ok_or_else(|| "missing query".to_string())?;
    if query.trim().is_empty() {
        return Err("query is empty".into());
    }
    let max_results = arg_usize(args, "max_results").unwrap_or(MAX_SEARCH_RESULTS).clamp(1, MAX_SEARCH_RESULTS);
    let root = match arg_string(args, "path") {
        Some(path) => resolve_existing(workspace, &path)?,
        None => workspace.to_path_buf(),
    };

    let mut command = Command::new("rg");
    command
        .current_dir(workspace)
        .arg("--line-number")
        .arg("--column")
        .arg("--no-heading")
        .arg("--color")
        .arg("never")
        .arg("--max-filesize")
        .arg("1M");
    if arg_bool(args, "case_sensitive").unwrap_or(false) {
        command.arg("--case-sensitive");
    } else {
        command.arg("--smart-case");
    }
    command.arg(&query).arg(&root);

    match command_utils::output_with_timeout("rg", &mut command, TOOL_COMMAND_TIMEOUT) {
        Ok(output) if output.status.success() || output.status.code() == Some(1) => {
            let text = String::from_utf8_lossy(&output.stdout);
            let lines = text.lines().take(max_results).collect::<Vec<_>>();
            if lines.is_empty() {
                Ok("no matches".into())
            } else {
                let suffix = if text.lines().count() > lines.len() {
                    format!("\n... truncated to {max_results} results")
                } else {
                    String::new()
                };
                Ok(format!("{}{}", lines.join("\n"), suffix))
            }
        }
        Ok(output) => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Err(error) if error.contains("No such file") => fallback_search(&root, &query, max_results),
        Err(error) => Err(error),
    }
}

fn fallback_search(root: &Path, query: &str, max_results: usize) -> Result<String, String> {
    let mut results = Vec::new();
    let deadline = Instant::now() + TOOL_COMMAND_TIMEOUT;
    fallback_search_dir(root, root, &query.to_lowercase(), max_results, deadline, &mut results)?;
    if results.is_empty() { Ok("no matches".into()) } else { Ok(results.join("\n")) }
}

fn fallback_search_dir(
    root: &Path,
    dir: &Path,
    query: &str,
    max_results: usize,
    deadline: Instant,
    results: &mut Vec<String>,
) -> Result<(), String> {
    if Instant::now() >= deadline {
        return Err(format!("search fallback timed out after {}s", TOOL_COMMAND_TIMEOUT.as_secs()));
    }
    if results.len() >= max_results {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|error| format!("read dir failed: {error}"))? {
        let entry = entry.map_err(|error| format!("read dir entry failed: {error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if ignored_name(&name) {
            continue;
        }
        if path.is_dir() {
            fallback_search_dir(root, &path, query, max_results, deadline, results)?;
            continue;
        }
        if fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0) > MAX_READ_BYTES {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else { continue; };
        for (index, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(query) {
                let relative = path.strip_prefix(root).unwrap_or(&path).display();
                results.push(format!("{relative}:{}:{}", index + 1, line.trim()));
                if results.len() >= max_results {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}
