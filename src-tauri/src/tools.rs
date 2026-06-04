use crate::command_utils;
use serde::Deserialize;
use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CONFIG_DIR_NAME: &str = ".sandevistan";
const MAX_LIST_ENTRIES: usize = 200;
const MAX_READ_BYTES: u64 = 50_000;
const MAX_WRITE_BYTES: usize = 200_000;
const MAX_TOOL_OUTPUT_BYTES: usize = 64_000;
const MAX_SEARCH_RESULTS: usize = 50;
const MAX_DIFF_BYTES: usize = 200_000;
const TOOL_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const RTK_REWRITE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_SHELL_OUTPUT_BYTES: usize = 120_000;

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ToolOptions {
    pub rtk_enabled: bool,
    pub shell_enabled: bool,
}

#[derive(Debug, Clone, Copy)]
struct ToolDef {
    name: &'static str,
    description: &'static str,
    args: &'static str,
}

struct ToolRunResult {
    ok: bool,
    output: String,
}

const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "fs.list",
        description: "list directory entries",
        args: r#"{"path":"."}"#,
    },
    ToolDef {
        name: "fs.read",
        description: "read UTF-8 file, capped",
        args: r#"{"path":"relative/file"}"#,
    },
    ToolDef {
        name: "fs.edit",
        description: "exact replace in existing file; creates backup",
        args: r#"{"path":"relative/file","old":"exact text","new":"replacement"}"#,
    },
    ToolDef {
        name: "fs.write",
        description: "create/overwrite file; creates backup when replacing",
        args: r#"{"path":"relative/file","content":"text"}"#,
    },
    ToolDef {
        name: "search.rg",
        description: "content search via ripgrep, respects ignore files, capped",
        args: r#"{"query":"text","path":".","case_sensitive":false,"max_results":50}"#,
    },
    ToolDef {
        name: "git.status",
        description: "git branch + porcelain status",
        args: r#"{}"#,
    },
    ToolDef {
        name: "git.diff",
        description: "git diff, optional path, capped",
        args: r#"{"path":"relative/file"}"#,
    },
    ToolDef {
        name: "shell.run",
        description:
            "run shell command in workspace when shell tools are enabled; timeout/capped output",
        args: r#"{"command":"npm test","timeout_secs":30}"#,
    },
];

pub fn prompt_with_subagents(
    subagents_enabled: bool,
    subagents: &[String],
    shell_enabled: bool,
) -> String {
    let mut lines = vec![
        "You are Sandevistan, a concise coding agent.".to_string(),
        "Use tools when workspace context is needed.".to_string(),
        "Available tools:".to_string(),
    ];
    lines.extend(
        TOOLS
            .iter()
            .filter(|tool| tool.name != "shell.run")
            .map(|tool| format!("- {}: {}. args: {}", tool.name, tool.description, tool.args)),
    );
    if shell_enabled {
        lines.push("- shell.run: run shell command in workspace when needed. args: {\"command\":\"npm test\",\"timeout_secs\":30}".to_string());
    }
    if subagents_enabled {
        let names = if subagents.is_empty() {
            "none".into()
        } else {
            subagents.join(", ")
        };
        lines.push(format!("- agent.delegate: run selected subagents concurrently. available: {names}. args: {{\"tasks\":[{{\"agent\":\"scout\",\"task\":\"find relevant files\"}},{{\"agent\":\"reviewer\",\"task\":\"review risks\"}}]}}"));
    }
    lines.extend([
        "Prefer fs.edit for existing files. Use fs.write for new files.".to_string(),
        "Use search.rg before broad file reads. Use git.status/git.diff for repo state."
            .to_string(),
    ]);
    if shell_enabled {
        lines.push("Use shell.run for explicit shell/CLI requests. Prefer read-only commands unless user clearly asks for mutations. If RTK is enabled, shell.run executes through rtk.".to_string());
    }
    if subagents_enabled {
        lines.push("When user asks to ping/run/call subagents, use agent.delegate; never claim no subagents spawned before trying it. Use agent.delegate for parallel research/review/implementation planning; max 4 subagents; you own final answer.".to_string());
    }
    lines.extend([
        "Tool call format; emit one or more blocks when independent reads/searches can run together:".to_string(),
        r#"<tool_call>{"name":"fs.read","args":{"path":"src/main.ts"}}</tool_call>"#.to_string(),
        "After tool results, answer briefly.".to_string(),
    ]);
    lines.join("\n")
}

pub fn parse_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let mut remaining = content;
    while let Some(start_index) = remaining.find("<tool_call>") {
        let start = start_index + "<tool_call>".len();
        let Some(end_index) = remaining[start..].find("</tool_call>") else {
            break;
        };
        let end = start + end_index;
        if let Ok(call) = serde_json::from_str(remaining[start..end].trim()) {
            calls.push(call);
        }
        remaining = &remaining[end + "</tool_call>".len()..];
    }
    calls
}

pub fn run_with_options(workspace: &Path, call: &ToolCall, options: ToolOptions) -> String {
    let result = run_result(workspace, call, options);
    let status = if result.ok { "ok" } else { "failed" };
    let output = truncate_string(result.output, MAX_TOOL_OUTPUT_BYTES);
    format!("status: {status}\n{output}")
}

fn run_result(workspace: &Path, call: &ToolCall, options: ToolOptions) -> ToolRunResult {
    let result = match call.name.as_str() {
        "fs.list" => {
            let path = arg_string(&call.args, "path").unwrap_or_else(|| ".".into());
            if options.rtk_enabled && rtk_available() {
                rtk_list(workspace, &path).or_else(|_| fs_list(workspace, path))
            } else {
                fs_list(workspace, path)
            }
        }
        "fs.read" => match arg_string(&call.args, "path") {
            Some(path) => {
                if options.rtk_enabled && rtk_available() {
                    rtk_read(workspace, &path).or_else(|_| fs_read(workspace, path))
                } else {
                    fs_read(workspace, path)
                }
            }
            None => Err("missing path".into()),
        },
        "fs.edit" => fs_edit(workspace, &call.args),
        "fs.write" => fs_write(workspace, &call.args),
        "search.rg" => {
            if options.rtk_enabled && rtk_available() {
                rtk_grep(workspace, &call.args).or_else(|_| search_rg(workspace, &call.args))
            } else {
                search_rg(workspace, &call.args)
            }
        }
        "git.status" => {
            if options.rtk_enabled && rtk_available() {
                rtk_git_status(workspace).or_else(|_| git_status(workspace))
            } else {
                git_status(workspace)
            }
        }
        "git.diff" => {
            if options.rtk_enabled && rtk_available() {
                rtk_git_diff(workspace, &call.args).or_else(|_| git_diff(workspace, &call.args))
            } else {
                git_diff(workspace, &call.args)
            }
        }
        "shell.run" => shell_run(workspace, &call.args, options),
        _ => Err(format!("unknown tool: {}", call.name)),
    };

    match result {
        Ok(output) => ToolRunResult { ok: true, output },
        Err(error) => ToolRunResult {
            ok: false,
            output: format!("error: {error}"),
        },
    }
}

fn shell_run(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    if !options.shell_enabled {
        return Err("shell tools disabled for this profile".into());
    }
    let command = arg_string(args, "command").ok_or_else(|| "missing command".to_string())?;
    let command = command.trim();
    if command.is_empty() {
        return Err("command is empty".into());
    }
    if command.chars().count() > 2_000 {
        return Err("command too long".into());
    }
    let timeout = args
        .get("timeout_secs")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, 60))
        .map(Duration::from_secs)
        .unwrap_or(SHELL_COMMAND_TIMEOUT);

    let output = if options.rtk_enabled {
        let rewritten = rtk_rewrite_shell_command(workspace, command)?;
        let mut cmd = if let Some(rewritten) = rewritten {
            let mut cmd = Command::new("sh");
            cmd.current_dir(workspace).arg("-lc").arg(rewritten);
            cmd
        } else {
            let mut cmd = Command::new("rtk");
            cmd.current_dir(workspace).arg("run").arg("-c").arg(command);
            cmd
        };
        command_utils::output_with_timeout("rtk-shell", &mut cmd, timeout)?
    } else {
        let mut cmd = Command::new("sh");
        cmd.current_dir(workspace).arg("-lc").arg(command);
        command_utils::output_with_timeout("shell", &mut cmd, timeout)?
    };

    let status = output
        .status
        .code()
        .map_or_else(|| "signal".into(), |code| code.to_string());
    let stdout = truncate_string(
        String::from_utf8_lossy(&output.stdout).to_string(),
        MAX_SHELL_OUTPUT_BYTES / 2,
    );
    let stderr = truncate_string(
        String::from_utf8_lossy(&output.stderr).to_string(),
        MAX_SHELL_OUTPUT_BYTES / 2,
    );
    let mut parts = vec![format!("exit: {status}")];
    if !stdout.trim().is_empty() {
        parts.push(format!("stdout:\n{}", stdout.trim_end()));
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    Ok(parts.join("\n"))
}

fn rtk_rewrite_shell_command(workspace: &Path, command: &str) -> Result<Option<String>, String> {
    if !rtk_available() {
        return Err("rtk enabled but unavailable on PATH".into());
    }

    let mut rewrite = Command::new("rtk");
    rewrite.current_dir(workspace).arg("rewrite").arg(command);
    let output =
        command_utils::output_with_timeout("rtk-rewrite", &mut rewrite, RTK_REWRITE_TIMEOUT)?;
    let rewritten = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!rewritten.is_empty()).then_some(rewritten))
}

fn fs_list(workspace: &Path, relative: String) -> Result<String, String> {
    let dir = resolve_existing(workspace, &relative)?;
    if !dir.is_dir() {
        return Err("path is not a directory".into());
    }

    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("read dir failed: {error}"))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    entries.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });

    Ok(entries
        .into_iter()
        .take(MAX_LIST_ENTRIES)
        .map(|entry| {
            let kind = if entry.path().is_dir() { "dir" } else { "file" };
            format!("{kind}\t{}", entry.file_name().to_string_lossy())
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn fs_read(workspace: &Path, relative: String) -> Result<String, String> {
    let path = resolve_existing(workspace, &relative)?;
    read_text(&path)
}

fn fs_write(workspace: &Path, args: &Value) -> Result<String, String> {
    let relative = arg_string(args, "path").ok_or_else(|| "missing path".to_string())?;
    let content = arg_string(args, "content").ok_or_else(|| "missing content".to_string())?;
    if content.len() > MAX_WRITE_BYTES {
        return Err(format!("content too large: {} bytes", content.len()));
    }

    let path = resolve_for_write(workspace, &relative)?;
    let backup = if path.exists() {
        Some(backup_file(workspace, &path)?)
    } else {
        None
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("parent dir create failed: {error}"))?;
    }
    fs::write(&path, content.as_bytes()).map_err(|error| format!("file write failed: {error}"))?;

    Ok(format!(
        "wrote {relative}\nbytes {}\nbackup {}",
        content.len(),
        backup
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".into())
    ))
}

fn fs_edit(workspace: &Path, args: &Value) -> Result<String, String> {
    let relative = arg_string(args, "path").ok_or_else(|| "missing path".to_string())?;
    let old = arg_string(args, "old").ok_or_else(|| "missing old".to_string())?;
    let new = arg_string(args, "new").ok_or_else(|| "missing new".to_string())?;
    if old.is_empty() {
        return Err("old is empty".into());
    }

    let path = resolve_existing(workspace, &relative)?;
    if !path.is_file() {
        return Err("path is not a file".into());
    }

    let content = read_text(&path)?;
    let matches = content.matches(&old).count();
    if matches != 1 {
        return Err(format!("old text match count is {matches}, expected 1"));
    }

    let backup = backup_file(workspace, &path)?;
    let next = content.replacen(&old, &new, 1);
    fs::write(&path, next.as_bytes()).map_err(|error| format!("file write failed: {error}"))?;

    Ok(format!(
        "edited {relative}\n-{} chars\n+{} chars\nbackup {}",
        old.len(),
        new.len(),
        backup.display()
    ))
}

fn search_rg(workspace: &Path, args: &Value) -> Result<String, String> {
    let query = arg_string(args, "query").ok_or_else(|| "missing query".to_string())?;
    if query.trim().is_empty() {
        return Err("query is empty".into());
    }
    let max_results = arg_usize(args, "max_results")
        .unwrap_or(MAX_SEARCH_RESULTS)
        .clamp(1, MAX_SEARCH_RESULTS);
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
        Err(error) if error.contains("No such file") => search_fallback(&root, &query, max_results),
        Err(error) => Err(error),
    }
}

fn search_fallback(root: &Path, query: &str, max_results: usize) -> Result<String, String> {
    let mut results = Vec::new();
    let deadline = Instant::now() + TOOL_COMMAND_TIMEOUT;
    search_fallback_dir(
        root,
        root,
        &query.to_lowercase(),
        max_results,
        deadline,
        &mut results,
    )?;
    if results.is_empty() {
        Ok("no matches".into())
    } else {
        Ok(results.join("\n"))
    }
}

fn search_fallback_dir(
    root: &Path,
    dir: &Path,
    query: &str,
    max_results: usize,
    deadline: Instant,
    results: &mut Vec<String>,
) -> Result<(), String> {
    if Instant::now() >= deadline {
        return Err(format!(
            "search fallback timed out after {}s",
            TOOL_COMMAND_TIMEOUT.as_secs()
        ));
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
            search_fallback_dir(root, &path, query, max_results, deadline, results)?;
            continue;
        }
        if fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0) > MAX_READ_BYTES {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
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

fn git_status(workspace: &Path) -> Result<String, String> {
    git_output(
        workspace,
        &["status", "--porcelain=v1", "--branch"],
        MAX_TOOL_OUTPUT_BYTES,
    )
}

fn git_diff(workspace: &Path, args: &Value) -> Result<String, String> {
    let owned;
    let git_args: Vec<&str> = if let Some(relative) = arg_string(args, "path") {
        clean_relative(&relative)?;
        owned = vec!["diff".to_string(), "--".to_string(), relative];
        owned.iter().map(String::as_str).collect()
    } else {
        vec!["diff"]
    };
    let output = git_output(workspace, &git_args, MAX_DIFF_BYTES)?;
    if output.trim().is_empty() {
        Ok("no diff".into())
    } else {
        Ok(output)
    }
}

fn git_output(workspace: &Path, args: &[&str], max_bytes: usize) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(workspace).args(args);
    let output = command_utils::output_with_timeout("git", &mut command, GIT_COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(truncate_string(
        String::from_utf8_lossy(&output.stdout).to_string(),
        max_bytes,
    ))
}

fn rtk_list(workspace: &Path, relative: &str) -> Result<String, String> {
    let path = resolve_existing(workspace, relative)?;
    if !path.is_dir() {
        return Err("path is not a directory".into());
    }
    let mut command = Command::new("rtk");
    command
        .current_dir(workspace)
        .arg("ls")
        .arg("--ultra-compact")
        .arg(path);
    rtk_output("rtk-ls", &mut command, MAX_TOOL_OUTPUT_BYTES)
}

fn rtk_read(workspace: &Path, relative: &str) -> Result<String, String> {
    let path = resolve_existing(workspace, relative)?;
    if !path.is_file() {
        return Err("path is not a file".into());
    }
    let mut command = Command::new("rtk");
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

fn rtk_grep(workspace: &Path, args: &Value) -> Result<String, String> {
    let query = arg_string(args, "query").ok_or_else(|| "missing query".to_string())?;
    if query.trim().is_empty() {
        return Err("query is empty".into());
    }
    let max_results = arg_usize(args, "max_results")
        .unwrap_or(MAX_SEARCH_RESULTS)
        .clamp(1, MAX_SEARCH_RESULTS);
    let root = arg_string(args, "path").unwrap_or_else(|| ".".into());
    resolve_existing(workspace, &root)?;

    let mut command = Command::new("rtk");
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

fn rtk_git_status(workspace: &Path) -> Result<String, String> {
    let mut command = Command::new("rtk");
    command
        .current_dir(workspace)
        .arg("git")
        .arg("--ultra-compact")
        .arg("status")
        .arg("--porcelain=v1")
        .arg("--branch");
    rtk_output("rtk-git", &mut command, MAX_TOOL_OUTPUT_BYTES)
}

fn rtk_git_diff(workspace: &Path, args: &Value) -> Result<String, String> {
    let mut command = Command::new("rtk");
    command
        .current_dir(workspace)
        .arg("git")
        .arg("--ultra-compact")
        .arg("diff");
    if let Some(relative) = arg_string(args, "path") {
        clean_relative(&relative)?;
        command.arg("--").arg(relative);
    }
    let output = rtk_output("rtk-git", &mut command, MAX_DIFF_BYTES)?;
    if output.trim().is_empty() {
        Ok("no diff".into())
    } else {
        Ok(output)
    }
}

fn rtk_output(label: &str, command: &mut Command, max_bytes: usize) -> Result<String, String> {
    let output = command_utils::output_with_timeout(label, command, TOOL_COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(truncate_string(
        String::from_utf8_lossy(&output.stdout).to_string(),
        max_bytes,
    ))
}

pub fn is_mutating(call: &ToolCall) -> bool {
    matches!(call.name.as_str(), "fs.edit" | "fs.write" | "shell.run")
}

fn rtk_available() -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|path| path.join("rtk").is_file())
}

fn read_text(path: &Path) -> Result<String, String> {
    let size = fs::metadata(path)
        .map_err(|error| format!("file metadata failed: {error}"))?
        .len();
    if size > MAX_READ_BYTES {
        return Err(format!("file too large: {size} bytes"));
    }

    fs::read_to_string(path).map_err(|error| format!("file read failed: {error}"))
}

fn backup_file(workspace: &Path, path: &Path) -> Result<PathBuf, String> {
    let relative = path
        .strip_prefix(workspace)
        .map_err(|_| "path outside workspace")?;
    let backup = config_dir()
        .join("backups")
        .join(now_ms().to_string())
        .join(hash_path(workspace))
        .join(relative);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("backup dir create failed: {error}"))?;
    }
    fs::copy(path, &backup).map_err(|error| format!("backup failed: {error}"))?;
    Ok(backup)
}

fn arg_string(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

fn arg_usize(args: &Value, key: &str) -> Option<usize> {
    args.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn resolve_for_write(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = workspace.join(clean_relative(relative)?);
    let parent = path.parent().ok_or_else(|| "invalid path".to_string())?;
    let parent = if parent.exists() {
        parent
            .canonicalize()
            .map_err(|error| format!("parent canonicalize failed: {error}"))?
    } else {
        let base = nearest_existing_parent(parent)?;
        base.canonicalize()
            .map_err(|error| format!("parent canonicalize failed: {error}"))?
    };
    if !parent.starts_with(workspace) {
        return Err("path outside workspace".into());
    }
    Ok(path)
}

fn nearest_existing_parent(path: &Path) -> Result<PathBuf, String> {
    let mut current = path;
    loop {
        if current.exists() {
            return Ok(current.to_path_buf());
        }
        current = current
            .parent()
            .ok_or_else(|| "invalid parent".to_string())?;
    }
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

fn ignored_name(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | ".svelte-kit"
    ) || name.ends_with('~')
}

fn truncate_string(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... truncated to {max_bytes} bytes", &value[..end])
}

fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(CONFIG_DIR_NAME)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn hash_path(path: &Path) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.display().to_string().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
