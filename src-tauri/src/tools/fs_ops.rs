use serde_json::Value;
use std::{fs, path::Path};

use super::{
    arg_string, backup_file, read_text, resolve_existing, resolve_for_write, ToolOptions,
    constants::{MAX_LIST_ENTRIES, MAX_WRITE_BYTES},
};

pub(crate) fn list_dir(workspace: &Path, relative: String) -> Result<String, String> {
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
        b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
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

pub(crate) fn read_file(workspace: &Path, relative: String) -> Result<String, String> {
    let path = resolve_existing(workspace, &relative)?;
    read_text(&path)
}

pub(crate) fn write_file(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
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
        backup.map(|path| path.display().to_string()).unwrap_or_else(|| "none".into())
    ))
}

pub(crate) fn edit_file(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
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
