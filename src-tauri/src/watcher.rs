use crate::agent::ChatRuntime;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter};

const DEBOUNCE_MS: u64 = 300;
const MAX_EVENT_PATHS: usize = 64;

#[derive(Default)]
pub struct FileWatcherState {
    watcher: Mutex<Option<RecommendedWatcher>>,
    workspace: Mutex<Option<PathBuf>>,
}

#[derive(Debug, Clone, Serialize)]
struct FileChangedEvent {
    workspace: String,
    paths: Vec<String>,
}

pub fn start(app: AppHandle, chat: ChatRuntime, state: &FileWatcherState) -> Result<(), String> {
    let workspace = chat.workspace()?;
    if state
        .workspace
        .lock()
        .map_err(|_| "watcher lock poisoned".to_string())?
        .as_ref()
        == Some(&workspace)
    {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let root = workspace.clone();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = tx.send(event);
        },
        Config::default(),
    )
    .map_err(|error| format!("watcher create failed: {error}"))?;
    watcher
        .watch(&workspace, RecursiveMode::Recursive)
        .map_err(|error| format!("watcher start failed: {error}"))?;

    spawn_event_loop(app, root.clone(), rx);
    *state
        .watcher
        .lock()
        .map_err(|_| "watcher lock poisoned".to_string())? = Some(watcher);
    *state
        .workspace
        .lock()
        .map_err(|_| "watcher lock poisoned".to_string())? = Some(root);
    Ok(())
}

pub fn stop(state: &FileWatcherState) -> Result<(), String> {
    *state
        .watcher
        .lock()
        .map_err(|_| "watcher lock poisoned".to_string())? = None;
    *state
        .workspace
        .lock()
        .map_err(|_| "watcher lock poisoned".to_string())? = None;
    Ok(())
}

fn spawn_event_loop(app: AppHandle, root: PathBuf, rx: mpsc::Receiver<notify::Result<Event>>) {
    thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let mut paths = event_paths(&root, event);
            while let Ok(event) = rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS)) {
                paths.extend(event_paths(&root, event));
                if paths.len() >= MAX_EVENT_PATHS {
                    break;
                }
            }

            paths.sort();
            paths.dedup();
            paths.truncate(MAX_EVENT_PATHS);
            if paths.is_empty() {
                continue;
            }

            let _ = app.emit(
                "file_changed",
                FileChangedEvent {
                    workspace: root.display().to_string(),
                    paths,
                },
            );
        }
    });
}

fn event_paths(root: &Path, event: notify::Result<Event>) -> Vec<String> {
    event
        .ok()
        .into_iter()
        .flat_map(|event| event.paths)
        .filter(|path| !ignored_path(path))
        .filter_map(|path| path.strip_prefix(root).ok().map(Path::to_path_buf))
        .map(|path| path.display().to_string())
        .collect()
}

fn ignored_path(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(
            value.as_ref(),
            ".git" | "node_modules" | "target" | "dist" | ".svelte-kit"
        ) || value.ends_with('~')
    })
}
