use crate::{agent::ChatRuntime, command_utils};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const MAX_SEARCH_RESULTS: usize = 200;
const MAX_DIFF_BYTES: usize = 200_000;
const SEARCH_TIMEOUT: Duration = Duration::from_secs(60);
const GIT_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Deserialize)]
pub struct ContentSearchRequest {
    query: String,
    path: Option<String>,
    max_results: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    path: String,
    line: u64,
    column: u64,
    text: String,
}

#[derive(Debug, Deserialize)]
pub struct GitDiffRequest {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitStatus {
    branch: String,
    entries: Vec<GitStatusEntry>,
}

#[derive(Debug, Serialize)]
pub struct GitStatusEntry {
    path: String,
    status: String,
    raw: String,
}

pub fn content_search(
    chat: &ChatRuntime,
    request: ContentSearchRequest,
) -> Result<Vec<SearchHit>, String> {
    let query = request.query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let workspace = chat.workspace()?;
    let root = match request.path {
        Some(path) => resolve_existing(&workspace, &path)?,
        None => workspace.clone(),
    };
    let max_results = request
        .max_results
        .unwrap_or(200)
        .clamp(1, MAX_SEARCH_RESULTS);

    let mut command = Command::new("rg");
    command
        .current_dir(&workspace)
        .arg("--json")
        .arg("--color")
        .arg("never")
        .arg("--smart-case")
        .arg("--max-filesize")
        .arg("1M")
        .arg("--")
        .arg(query)
        .arg(root);
    let output = command_utils::output_with_timeout("rg", &mut command, SEARCH_TIMEOUT).map_err(
        |error| {
            if error.contains("No such file") {
                "ripgrep not found: install rg".into()
            } else {
                error
            }
        },
    )?;

    if !output.status.success() && output.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut hits = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if hits.len() >= max_results {
            break;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event.get("type").and_then(Value::as_str) != Some("match") {
            continue;
        }
        let data = &event["data"];
        let Some(path_text) = data["path"]["text"].as_str() else {
            continue;
        };
        let path = PathBuf::from(path_text);
        let relative = path.strip_prefix(&workspace).unwrap_or(&path);
        let text = data["lines"]["text"]
            .as_str()
            .unwrap_or_default()
            .trim_end()
            .to_string();
        let column = data["submatches"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item["start"].as_u64())
            .unwrap_or(0)
            + 1;
        hits.push(SearchHit {
            path: relative.display().to_string(),
            line: data["line_number"].as_u64().unwrap_or(0),
            column,
            text,
        });
    }

    Ok(hits)
}

pub fn git_status(chat: &ChatRuntime) -> Result<GitStatus, String> {
    let workspace = chat.workspace()?;
    ensure_git_repo(&workspace)?;

    let branch = git_text(&workspace, &["branch", "--show-current"])
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            git_text(&workspace, &["rev-parse", "--short", "HEAD"])
                .ok()
                .map(|value| format!("detached {}", value.trim()))
        })
        .unwrap_or_else(|| "unknown".into());

    let status = git_text(&workspace, &["status", "--porcelain=v1"])?;
    let entries = status
        .lines()
        .map(|line| GitStatusEntry {
            status: line.chars().take(2).collect::<String>(),
            path: line.get(3..).unwrap_or_default().to_string(),
            raw: line.to_string(),
        })
        .collect();

    Ok(GitStatus { branch, entries })
}

pub fn git_diff(chat: &ChatRuntime, request: GitDiffRequest) -> Result<String, String> {
    let workspace = chat.workspace()?;
    ensure_git_repo(&workspace)?;

    let mut diff = if let Some(path) = request.path {
        clean_relative(&path)?;
        let mut command = Command::new("git");
        command.current_dir(&workspace).args(["diff", "--", &path]);
        let output = command_utils::output_with_timeout("git", &mut command, GIT_TIMEOUT)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let diff = String::from_utf8_lossy(&output.stdout).to_string();
        if diff.trim().is_empty() {
            untracked_diff(&workspace, Some(&path))?
        } else {
            diff
        }
    } else {
        let mut command = Command::new("git");
        command.current_dir(&workspace).arg("diff");
        let output = command_utils::output_with_timeout("git", &mut command, GIT_TIMEOUT)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let mut diff = String::from_utf8_lossy(&output.stdout).to_string();
        let untracked = untracked_diff(&workspace, None)?;
        if !untracked.trim().is_empty() {
            if !diff.trim().is_empty() {
                diff.push_str("\n");
            }
            diff.push_str(&untracked);
        }
        diff
    };

    diff = truncate(diff, MAX_DIFF_BYTES);
    if diff.trim().is_empty() {
        Ok("no diff".into())
    } else {
        Ok(diff)
    }
}

fn untracked_diff(workspace: &Path, relative: Option<&str>) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .current_dir(workspace)
        .args(["ls-files", "--others", "--exclude-standard", "--"]);
    if let Some(path) = relative {
        command.arg(path);
    }
    let output = command_utils::output_with_timeout("git", &mut command, GIT_TIMEOUT)?;
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
        let content = match std::fs::read_to_string(&full_path) {
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

fn ensure_git_repo(workspace: &Path) -> Result<(), String> {
    git_text(workspace, &["rev-parse", "--is-inside-work-tree"])
        .map(|_| ())
        .map_err(|_| "not a git repository".into())
}

fn git_text(workspace: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(workspace).args(args);
    let output = command_utils::output_with_timeout("git", &mut command, GIT_TIMEOUT)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn resolve_existing(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = workspace.join(clean_relative(relative)?);
    let path = path
        .canonicalize()
        .map_err(|error| format!("path canonicalize failed: {error}"))?;
    if !path.starts_with(workspace) {
        return Err("path outside workspace".into());
    }
    Ok(path)
}

fn clean_relative(path: &str) -> Result<PathBuf, String> {
    let path = path.trim();
    if path.is_empty() || path == "." {
        return Ok(PathBuf::from("."));
    }
    if path.starts_with('/') || path.contains("..") {
        return Err("invalid relative path".into());
    }
    Ok(PathBuf::from(path))
}

fn truncate(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... truncated to {max_bytes} bytes", &value[..end])
}
