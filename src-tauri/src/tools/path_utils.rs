use serde_json::Value;
use std::{env, fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

use super::constants::{CONFIG_DIR_NAME, MAX_READ_BYTES};

pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    let size = fs::metadata(path)
        .map_err(|error| format!("file metadata failed: {error}"))?
        .len();
    if size > MAX_READ_BYTES {
        return Err(format!("file too large: {size} bytes"));
    }
    fs::read_to_string(path).map_err(|error| format!("file read failed: {error}"))
}

pub(crate) fn backup_file(workspace: &Path, path: &Path, session_id: Option<&str>) -> Result<PathBuf, String> {
    let relative = path.strip_prefix(workspace).map_err(|_| "path outside workspace")?;
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

pub(crate) fn arg_string(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

pub(crate) fn arg_usize(args: &Value, key: &str) -> Option<usize> {
    args.get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

pub(crate) fn resolve_for_write(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = workspace.join(clean_relative(relative)?);
    let parent = path.parent().ok_or_else(|| "invalid path".to_string())?;
    let parent = if parent.exists() {
        parent.canonicalize().map_err(|error| format!("parent canonicalize failed: {error}"))?
    } else {
        nearest_existing_parent(parent)?.canonicalize().map_err(|error| format!("parent canonicalize failed: {error}"))?
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
        current = current.parent().ok_or_else(|| "invalid parent".to_string())?;
    }
}

pub(crate) fn resolve_existing(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = workspace.join(clean_relative(relative)?);
    let path = path.canonicalize().map_err(|error| format!("path canonicalize failed: {error}"))?;
    if !path.starts_with(workspace) {
        return Err("path outside workspace".into());
    }
    Ok(path)
}

pub(crate) fn clean_relative(path: &str) -> Result<PathBuf, String> {
    let path = path.trim();
    if path.is_empty() || path == "." {
        return Ok(PathBuf::from("."));
    }
    if path.starts_with('/') || path.contains("..") {
        return Err("invalid relative path".into());
    }
    Ok(PathBuf::from(path))
}

pub(crate) fn ignored_name(name: &str) -> bool {
    matches!(name, ".git" | "node_modules" | "target" | "dist" | ".svelte-kit") || name.ends_with('~')
}

pub(crate) fn truncate_string(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... truncated to {max_bytes} bytes", &value[..end])
}

pub(crate) fn config_dir() -> PathBuf {
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
