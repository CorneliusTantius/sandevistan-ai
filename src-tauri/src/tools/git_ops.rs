use crate::command_utils;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

use super::{
    arg_string, clean_relative, resolve_existing, truncate_string,
    constants::{GIT_COMMAND_TIMEOUT, MAX_DIFF_BYTES, MAX_TOOL_OUTPUT_BYTES},
};

pub(crate) fn status(workspace: &Path) -> Result<String, String> {
    git_output(workspace, &["status", "--porcelain=v1", "--branch"], MAX_TOOL_OUTPUT_BYTES)
}

pub(crate) fn diff(workspace: &Path, args: &Value) -> Result<String, String> {
    let mut output = if let Some(relative) = arg_string(args, "path") {
        clean_relative(&relative)?;
        let diff = git_output(workspace, &["diff", "--", &relative], MAX_DIFF_BYTES)?;
        if diff.trim().is_empty() {
            untracked_diff(workspace, Some(&relative))?
        } else {
            diff
        }
    } else {
        let mut diff = git_output(workspace, &["diff"], MAX_DIFF_BYTES)?;
        let untracked = untracked_diff(workspace, None)?;
        if !untracked.trim().is_empty() {
            if !diff.trim().is_empty() {
                diff.push('\n');
            }
            diff.push_str(&untracked);
        }
        diff
    };
    output = truncate_string(output, MAX_DIFF_BYTES);
    if output.trim().is_empty() { Ok("no diff".into()) } else { Ok(output) }
}

fn untracked_diff(workspace: &Path, relative: Option<&str>) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(workspace).args(["ls-files", "--others", "--exclude-standard", "--"]);
    if let Some(path) = relative {
        command.arg(path);
    }
    let output = command_utils::output_with_timeout("git", &mut command, GIT_COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut diff = String::new();
    for path in String::from_utf8_lossy(&output.stdout).lines() {
        let clean = clean_relative(path)?;
        let full_path = resolve_existing(workspace, path)?;
        if !full_path.is_file() {
            continue;
        }
        let content = match fs::read_to_string(&full_path) {
            Ok(content) => content,
            Err(_) => {
                diff.push_str(&format!("diff --git a/{path} b/{path}\nnew file mode 100644\nBinary file {path} differs\n"));
                continue;
            }
        };
        diff.push_str(&format!(
            "diff --git a/{0} b/{0}\nnew file mode 100644\n--- /dev/null\n+++ b/{0}\n@@ -0,0 +1,{1} @@\n",
            clean.display(),
            content.lines().count()
        ));
        for line in content.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
        if diff.len() >= MAX_DIFF_BYTES {
            break;
        }
    }
    Ok(diff)
}

fn git_output(workspace: &Path, args: &[&str], max_bytes: usize) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(workspace).args(args);
    let output = command_utils::output_with_timeout("git", &mut command, GIT_COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(truncate_string(String::from_utf8_lossy(&output.stdout).to_string(), max_bytes))
}
