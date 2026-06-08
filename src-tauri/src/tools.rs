use crate::{ai, command_utils, runtime_wire::NativeToolSpec};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CONFIG_DIR_NAME: &str = ".sandevistan";
const MAX_LIST_ENTRIES: usize = 200;
const MAX_READ_BYTES: u64 = 24_000;
const MAX_WRITE_BYTES: usize = 200_000;
const MAX_TOOL_OUTPUT_BYTES: usize = 16_000;
const MAX_SEARCH_RESULTS: usize = 200;
const MAX_DIFF_BYTES: usize = 50_000;
const TOOL_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(45);
const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const RTK_REWRITE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_SHELL_OUTPUT_BYTES: usize = 24_000;

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ToolOptions {
    pub rtk_enabled: bool,
    pub shell_enabled: bool,
    pub backup_session_id: Option<String>,
}

struct ToolRunResult {
    ok: bool,
    output: String,
}

type ToolExecutor = fn(&Path, &Value, ToolOptions) -> Result<String, String>;
type ToolValidator = fn(&Value, &ai::ModelMods) -> Result<(), String>;
type ToolParameters = fn(&[String]) -> Value;

#[derive(Debug, Clone, Copy)]
struct ToolEntry {
    name: &'static str,
    openai_name: &'static str,
    description: &'static str,
    mutating: bool,
    shell_only: bool,
    delegate_only: bool,
    parameters: ToolParameters,
    validate: ToolValidator,
    execute: ToolExecutor,
}

const TOOL_ENTRIES: &[ToolEntry] = &[
    ToolEntry { name: "fs.list", openai_name: "fs_list", description: "list directory entries", mutating: false, shell_only: false, delegate_only: false, parameters: params_fs_list, validate: validate_fs_list, execute: exec_fs_list },
    ToolEntry { name: "fs.read", openai_name: "fs_read", description: "read UTF-8 file, capped", mutating: false, shell_only: false, delegate_only: false, parameters: params_fs_read, validate: validate_fs_read, execute: exec_fs_read },
    ToolEntry { name: "fs.edit", openai_name: "fs_edit", description: "exact replace in existing file; creates backup", mutating: true, shell_only: false, delegate_only: false, parameters: params_fs_edit, validate: validate_fs_edit, execute: exec_fs_edit },
    ToolEntry { name: "fs.write", openai_name: "fs_write", description: "create/overwrite file; creates backup when replacing", mutating: true, shell_only: false, delegate_only: false, parameters: params_fs_write, validate: validate_fs_write, execute: exec_fs_write },
    ToolEntry { name: "search.rg", openai_name: "search_rg", description: "content search via ripgrep, respects ignore files, capped", mutating: false, shell_only: false, delegate_only: false, parameters: params_search_rg, validate: validate_search_rg, execute: exec_search_rg },
    ToolEntry { name: "git.status", openai_name: "git_status", description: "git branch + porcelain status", mutating: false, shell_only: false, delegate_only: false, parameters: params_empty, validate: validate_noop, execute: exec_git_status },
    ToolEntry { name: "git.diff", openai_name: "git_diff", description: "git diff, optional path, capped", mutating: false, shell_only: false, delegate_only: false, parameters: params_git_diff, validate: validate_git_diff, execute: exec_git_diff },
    ToolEntry { name: "shell.run", openai_name: "shell_run", description: "run shell command in workspace; timeout/capped output", mutating: true, shell_only: true, delegate_only: false, parameters: params_shell_run, validate: validate_shell_run, execute: exec_shell_run },
    ToolEntry { name: "agent.delegate", openai_name: "agent_delegate", description: "run selected subagents concurrently; provide many small specific independent tasks, not one broad task", mutating: false, shell_only: false, delegate_only: true, parameters: params_agent_delegate, validate: validate_agent_delegate, execute: exec_agent_delegate },
];

fn find_tool_entry(name: &str) -> Option<&'static ToolEntry> {
    match name {
        "fs.list" => Some(&TOOL_ENTRIES[0]),
        "fs.read" => Some(&TOOL_ENTRIES[1]),
        "fs.edit" => Some(&TOOL_ENTRIES[2]),
        "fs.write" => Some(&TOOL_ENTRIES[3]),
        "search.rg" => Some(&TOOL_ENTRIES[4]),
        "git.status" => Some(&TOOL_ENTRIES[5]),
        "git.diff" => Some(&TOOL_ENTRIES[6]),
        "shell.run" => Some(&TOOL_ENTRIES[7]),
        "agent.delegate" => Some(&TOOL_ENTRIES[8]),
        _ => None,
    }
}

pub fn run_with_options(workspace: &Path, call: &ToolCall, options: ToolOptions) -> String {
    let result = run_result(workspace, call, options);
    let status = if result.ok { "ok" } else { "failed" };
    let output = truncate_string(result.output, MAX_TOOL_OUTPUT_BYTES);
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

fn shell_run(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    if !options.shell_enabled {
        return Err("shell tools disabled for this profile".into());
    }
    let command = arg_string(args, "command").ok_or_else(|| "missing command".to_string())?;
    let command = command.trim();
    if command.is_empty() {
        return Err("command is empty".into());
    }
    let timeout = args
        .get("timeout_secs")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, 300))
        .map(Duration::from_secs)
        .unwrap_or(SHELL_COMMAND_TIMEOUT);

    let output = if options.rtk_enabled {
        let rewritten = rtk_rewrite_shell_command(workspace, command)?;
        let mut cmd = if let Some(rewritten) = rewritten {
            let mut cmd = Command::new("sh");
            cmd.current_dir(workspace).arg("-lc").arg(rewritten);
            cmd
        } else {
            let mut cmd = rtk_command()?;
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

    let mut rewrite = rtk_command()?;
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

fn fs_write(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    let relative = arg_string(args, "path").ok_or_else(|| "missing path".to_string())?;
    let content = arg_string(args, "content").ok_or_else(|| "missing content".to_string())?;
    if content.len() > MAX_WRITE_BYTES {
        return Err(format!("content too large: {} bytes", content.len()));
    }

    let path = resolve_for_write(workspace, &relative)?;
    let backup = if path.exists() {
        Some(backup_file(workspace, &path, options.backup_session_id.as_deref())?)
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

fn fs_edit(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
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

    let backup = backup_file(workspace, &path, options.backup_session_id.as_deref())?;
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
                diff.push_str("\n");
            }
            diff.push_str(&untracked);
        }
        diff
    };
    output = truncate_string(output, MAX_DIFF_BYTES);
    if output.trim().is_empty() {
        Ok("no diff".into())
    } else {
        Ok(output)
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
    let mut command = rtk_command()?;
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

fn rtk_git_status(workspace: &Path) -> Result<String, String> {
    let mut command = rtk_command()?;
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
    let mut command = rtk_command()?;
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

fn rtk_available() -> bool {
    rtk_path().is_some()
}

fn rtk_command() -> Result<Command, String> {
    rtk_path()
        .map(Command::new)
        .ok_or_else(|| "rtk enabled but unavailable on PATH".into())
}

fn rtk_path() -> Option<PathBuf> {
    static RTK_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    RTK_PATH
        .get_or_init(|| {
            env::var_os("PATH")
                .into_iter()
                .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
                .chain(rtk_fallback_dirs())
                .flat_map(|dir| {
                    rtk_executable_names()
                        .iter()
                        .map(move |name| dir.join(name))
                })
                .find(|path| path.is_file())
        })
        .clone()
}

fn rtk_executable_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["rtk.exe", "rtk.cmd", "rtk.bat", "rtk"]
    } else {
        &["rtk"]
    }
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

fn read_text(path: &Path) -> Result<String, String> {
    let size = fs::metadata(path)
        .map_err(|error| format!("file metadata failed: {error}"))?
        .len();
    if size > MAX_READ_BYTES {
        return Err(format!("file too large: {size} bytes"));
    }

    fs::read_to_string(path).map_err(|error| format!("file read failed: {error}"))
}

fn backup_file(workspace: &Path, path: &Path, session_id: Option<&str>) -> Result<PathBuf, String> {
    let relative = path
        .strip_prefix(workspace)
        .map_err(|_| "path outside workspace")?;
    let backup = config_dir()
        .join("backups")
        .join(session_id.unwrap_or("unknown-session"))
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

pub fn native_system_prompt(
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

fn spec(entry: &ToolEntry, parameters: Value) -> NativeToolSpec {
    NativeToolSpec {
        name: entry.name.into(),
        openai_name: entry.openai_name.into(),
        description: entry.description.into(),
        parameters,
    }
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

    fn entry_enabled(&self, entry: &ToolEntry) -> bool {
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

pub fn validate_tool_call(
    call: ToolCall,
    mods: &ai::ModelMods,
    read_only: bool,
) -> Result<ValidatedToolCall, String> {
    let entry =
        find_tool_entry(&call.name).ok_or_else(|| format!("unknown tool: {}", call.name))?;
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

fn params_empty(_: &[String]) -> Value {
    json!({"type":"object","properties":{},"required":[],"additionalProperties":false})
}

fn params_fs_list(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string","default":"."}},"required":[],"additionalProperties":false})
}

fn params_fs_read(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false})
}

fn params_fs_edit(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"}},"required":["path","old","new"],"additionalProperties":false})
}

fn params_fs_write(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false})
}

fn params_search_rg(_: &[String]) -> Value {
    json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string","default":"."},"case_sensitive":{"type":"boolean","default":false},"max_results":{"type":"integer","minimum":1,"maximum":200,"default":200}},"required":["query"],"additionalProperties":false})
}

fn params_git_diff(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"}},"required":[],"additionalProperties":false})
}

fn params_shell_run(_: &[String]) -> Value {
    json!({"type":"object","properties":{"command":{"type":"string"},"timeout_secs":{"type":"integer","minimum":1,"maximum":300,"default":120}},"required":["command"],"additionalProperties":false})
}

fn params_agent_delegate(subagents: &[String]) -> Value {
    json!({"type":"object","properties":{"tasks":{"type":"array","minItems":1,"maxItems":8,"items":{"type":"object","properties":{"agent":{"type":"string","enum":subagents},"task":{"type":"string","maxLength":1000}},"required":["agent","task"],"additionalProperties":false}}},"required":["tasks"],"additionalProperties":false})
}

fn validate_noop(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &[])
}

fn validate_fs_list(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path"])?;
    optional_string(args, "path")
}

fn validate_fs_read(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path"])?;
    required_string(args, "path")
}

fn validate_fs_edit(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path", "old", "new"])?;
    required_string(args, "path")?;
    required_string(args, "old")?;
    required_string(args, "new")
}

fn validate_fs_write(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path", "content"])?;
    required_string(args, "path")?;
    required_string(args, "content")
}

fn validate_search_rg(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["query", "path", "case_sensitive", "max_results"])?;
    required_string(args, "query")?;
    optional_string(args, "path")?;
    optional_bool(args, "case_sensitive")?;
    optional_integer(args, "max_results")
}

fn validate_git_diff(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path"])?;
    optional_string(args, "path")
}

fn validate_shell_run(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["command", "timeout_secs"])?;
    required_string(args, "command")?;
    optional_integer(args, "timeout_secs")
}

fn validate_agent_delegate(args: &Value, mods: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["tasks"])?;
    validate_delegate_tasks(args, mods)
}

fn ensure_no_extra_args(args: &Value, allowed: &[&str]) -> Result<(), String> {
    let Some(object) = args.as_object() else {
        return Err("args must be a JSON object".into());
    };
    if let Some(extra) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unexpected arg: {extra}"));
    }
    Ok(())
}

fn required_string(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key).and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => Ok(()),
        Some(_) => Err(format!("{key} must not be empty")),
        None => Err(format!("missing required arg: {key}")),
    }
}

fn optional_string(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key) {
        Some(value) if !value.is_string() => Err(format!("{key} must be a string")),
        _ => Ok(()),
    }
}

fn optional_bool(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key) {
        Some(value) if !value.is_boolean() => Err(format!("{key} must be a boolean")),
        _ => Ok(()),
    }
}

fn optional_integer(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key) {
        Some(value) if value.as_u64().is_none() => Err(format!("{key} must be an integer")),
        _ => Ok(()),
    }
}

fn validate_delegate_tasks(args: &Value, mods: &ai::ModelMods) -> Result<(), String> {
    let tasks = args
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing required arg: tasks".to_string())?;
    if tasks.is_empty() {
        return Err("tasks must not be empty".into());
    }
    if tasks.len() > 8 {
        return Err("tasks exceeds maxItems 8".into());
    }
    if tasks.len() == 1 {
        if let Some(task_text) = tasks[0].get("task").and_then(Value::as_str) {
            if task_text.chars().count() > 350 {
                return Err(
                    "single delegate task too broad; split into multiple small concurrent tasks"
                        .into(),
                );
            }
        }
    }
    for (index, task) in tasks.iter().enumerate() {
        let agent = task
            .get("agent")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("tasks[{index}].agent must be a string"))?;
        if !mods.subagents.iter().any(|name| name == agent) {
            return Err(format!("tasks[{index}].agent unknown or disabled: {agent}"));
        }
        let task_text = task
            .get("task")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("tasks[{index}].task must be a string"))?;
        if task_text.trim().is_empty() {
            return Err(format!("tasks[{index}].task must not be empty"));
        }
        if task_text.chars().count() > 1_000 {
            return Err(format!(
                "tasks[{index}].task too broad; split into smaller specific tasks under 1000 chars"
            ));
        }
    }
    Ok(())
}

fn exec_fs_list(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    let path = arg_string(args, "path").unwrap_or_else(|| ".".into());
    if options.rtk_enabled && rtk_available() {
        rtk_list(workspace, &path).or_else(|_| fs_list(workspace, path))
    } else {
        fs_list(workspace, path)
    }
}

fn exec_fs_read(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    let path = arg_string(args, "path").ok_or_else(|| "missing path".to_string())?;
    if options.rtk_enabled && rtk_available() {
        rtk_read(workspace, &path).or_else(|_| fs_read(workspace, path))
    } else {
        fs_read(workspace, path)
    }
}

fn exec_fs_edit(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    fs_edit(workspace, args, options)
}

fn exec_fs_write(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    fs_write(workspace, args, options)
}

fn exec_search_rg(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    if options.rtk_enabled && rtk_available() {
        rtk_grep(workspace, args).or_else(|_| search_rg(workspace, args))
    } else {
        search_rg(workspace, args)
    }
}

fn exec_git_status(workspace: &Path, _: &Value, options: ToolOptions) -> Result<String, String> {
    if options.rtk_enabled && rtk_available() {
        rtk_git_status(workspace).or_else(|_| git_status(workspace))
    } else {
        git_status(workspace)
    }
}

fn exec_git_diff(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    if options.rtk_enabled && rtk_available() {
        rtk_git_diff(workspace, args).or_else(|_| git_diff(workspace, args))
    } else {
        git_diff(workspace, args)
    }
}

fn exec_shell_run(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    shell_run(workspace, args, options)
}

fn exec_agent_delegate(_: &Path, _: &Value, _: ToolOptions) -> Result<String, String> {
    Err("agent.delegate must be executed by agent runtime".into())
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
