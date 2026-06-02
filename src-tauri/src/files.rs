use crate::agent::ChatRuntime;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_ENTRIES: usize = 600;
const MAX_INDEX_ENTRIES: usize = 5_000;
const MAX_FILE_BYTES: u64 = 10_000_000;

#[derive(Debug, Deserialize)]
pub struct FileRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveFileRequest {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
pub struct TreeRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    query: String,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    name: String,
    path: String,
    kind: String,
    depth: usize,
}

#[derive(Debug, Serialize)]
pub struct FileData {
    path: String,
    content: String,
}

pub fn tree(chat: &ChatRuntime) -> Result<Vec<FileEntry>, String> {
    let workspace = chat.workspace()?;
    tree_at(&workspace, &workspace, 0)
}

pub fn children(chat: &ChatRuntime, request: TreeRequest) -> Result<Vec<FileEntry>, String> {
    let workspace = chat.workspace()?;
    let dir = resolve_existing(&workspace, &request.path)?;
    if !dir.is_dir() {
        return Err("path is not a directory".into());
    }

    let depth = clean_relative(&request.path)?.components().count();
    tree_at(&workspace, &dir, depth)
}

pub fn index(chat: &ChatRuntime) -> Result<Vec<FileEntry>, String> {
    let workspace = chat.workspace()?;
    let mut entries = walk_paths(&workspace, &workspace, None)?;
    entries.sort_by(sort_path);
    Ok(entries
        .into_iter()
        .take(MAX_INDEX_ENTRIES)
        .map(|path| path_entry(&workspace, path))
        .collect())
}

pub fn search(chat: &ChatRuntime, request: SearchRequest) -> Result<Vec<FileEntry>, String> {
    let query = request.query.trim().to_lowercase();
    if query.is_empty() {
        return tree(chat);
    }

    let workspace = chat.workspace()?;
    let paths = walk_paths(&workspace, &workspace, None)?;
    Ok(paths
        .into_iter()
        .filter(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let relative = path
                .strip_prefix(&workspace)
                .unwrap_or(path)
                .display()
                .to_string()
                .to_lowercase();
            name.contains(&query) || relative.contains(&query)
        })
        .take(MAX_ENTRIES)
        .map(|path| path_entry(&workspace, path))
        .collect())
}

fn tree_at(workspace: &Path, dir: &Path, depth: usize) -> Result<Vec<FileEntry>, String> {
    let mut paths = walk_paths(workspace, dir, Some(1))?;
    paths.sort_by(sort_path);
    Ok(paths
        .into_iter()
        .take(MAX_ENTRIES)
        .map(|path| file_entry(workspace, path, depth))
        .collect())
}

pub fn read(chat: &ChatRuntime, request: FileRequest) -> Result<FileData, String> {
    let workspace = chat.workspace()?;
    let path = resolve_existing(&workspace, &request.path)?;
    if !path.is_file() {
        return Err("path is not a file".into());
    }

    let size = fs::metadata(&path)
        .map_err(|error| format!("file metadata failed: {error}"))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err("file too large".into());
    }

    let content =
        fs::read_to_string(&path).map_err(|error| format!("file read failed: {error}"))?;
    Ok(FileData {
        path: request.path,
        content,
    })
}

pub fn save(chat: &ChatRuntime, request: SaveFileRequest) -> Result<FileData, String> {
    if request.content.len() as u64 > MAX_FILE_BYTES {
        return Err("file too large".into());
    }

    let workspace = chat.workspace()?;
    let path = resolve_for_write(&workspace, &request.path)?;
    fs::write(&path, request.content.as_bytes())
        .map_err(|error| format!("file write failed: {error}"))?;
    Ok(FileData {
        path: request.path,
        content: request.content,
    })
}

fn walk_paths(
    workspace: &Path,
    root: &Path,
    max_depth: Option<usize>,
) -> Result<Vec<PathBuf>, String> {
    let mut builder = WalkBuilder::new(root);
    builder
        .standard_filters(true)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            name != ".git" && !name.ends_with('~')
        });
    if let Some(depth) = max_depth {
        builder.max_depth(Some(depth));
    }

    let mut paths = Vec::new();
    for entry in builder.build() {
        let entry = entry.map_err(|error| format!("walk failed: {error}"))?;
        let path = entry.path();
        if path == root || !path.starts_with(workspace) {
            continue;
        }
        paths.push(path.to_path_buf());
    }
    Ok(paths)
}

fn sort_path(a: &PathBuf, b: &PathBuf) -> std::cmp::Ordering {
    let a_dir = a.is_dir();
    let b_dir = b.is_dir();
    b_dir
        .cmp(&a_dir)
        .then_with(|| a.file_name().cmp(&b.file_name()))
}

fn file_entry(workspace: &Path, path: PathBuf, depth: usize) -> FileEntry {
    let is_dir = path.is_dir();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let relative = path.strip_prefix(workspace).unwrap_or(&path);
    FileEntry {
        name,
        path: relative.display().to_string(),
        kind: if is_dir { "dir" } else { "file" }.into(),
        depth,
    }
}

fn path_entry(workspace: &Path, path: PathBuf) -> FileEntry {
    let relative = path.strip_prefix(workspace).unwrap_or(&path);
    let depth = relative.components().count().saturating_sub(1);
    file_entry(workspace, path, depth)
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

fn resolve_for_write(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    let clean = clean_relative(relative)?;
    let path = workspace.join(&clean);
    let parent = path.parent().ok_or_else(|| "invalid path".to_string())?;
    let parent = parent
        .canonicalize()
        .map_err(|error| format!("parent canonicalize failed: {error}"))?;
    if !parent.starts_with(workspace) {
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
