use crate::agent::ChatRuntime;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
pub struct TerminalState {
    terminals: Mutex<HashMap<String, TerminalHandle>>,
}

struct TerminalHandle {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalStartRequest {
    id: String,
    cols: u16,
    rows: u16,
}

#[derive(Debug, Deserialize)]
pub struct TerminalWriteRequest {
    id: String,
    data: String,
}

#[derive(Debug, Deserialize)]
pub struct TerminalResizeRequest {
    id: String,
    cols: u16,
    rows: u16,
}

#[derive(Debug, Deserialize)]
pub struct TerminalStopRequest {
    id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalOutput {
    id: String,
    data: String,
}

pub fn start(
    app: AppHandle,
    chat: ChatRuntime,
    state: tauri::State<'_, TerminalState>,
    request: TerminalStartRequest,
) -> Result<(), String> {
    let cwd = chat.workspace()?;
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: request.rows,
            cols: request.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("pty open failed: {error}"))?;

    let mut command = CommandBuilder::new(default_shell());
    command.cwd(cwd);
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("shell spawn failed: {error}"))?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("pty reader failed: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("pty writer failed: {error}"))?;

    let id = request.id;
    state
        .terminals
        .lock()
        .map_err(|_| "terminal lock poisoned".to_string())?
        .insert(
            id.clone(),
            TerminalHandle {
                master: pair.master,
                writer,
                child,
            },
        );

    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        while let Ok(size) = reader.read(&mut buffer) {
            if size == 0 {
                break;
            }
            let data = String::from_utf8_lossy(&buffer[..size]).to_string();
            let _ = app.emit(
                "terminal-output",
                TerminalOutput {
                    id: id.clone(),
                    data,
                },
            );
        }
    });

    Ok(())
}

pub fn write(
    state: tauri::State<'_, TerminalState>,
    request: TerminalWriteRequest,
) -> Result<(), String> {
    let mut terminals = state
        .terminals
        .lock()
        .map_err(|_| "terminal lock poisoned".to_string())?;
    let terminal = terminals
        .get_mut(&request.id)
        .ok_or_else(|| "terminal not found".to_string())?;
    terminal
        .writer
        .write_all(request.data.as_bytes())
        .map_err(|error| format!("terminal write failed: {error}"))
}

pub fn resize(
    state: tauri::State<'_, TerminalState>,
    request: TerminalResizeRequest,
) -> Result<(), String> {
    let terminals = state
        .terminals
        .lock()
        .map_err(|_| "terminal lock poisoned".to_string())?;
    let terminal = terminals
        .get(&request.id)
        .ok_or_else(|| "terminal not found".to_string())?;
    terminal
        .master
        .resize(PtySize {
            rows: request.rows,
            cols: request.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("terminal resize failed: {error}"))
}

pub fn stop(
    state: tauri::State<'_, TerminalState>,
    request: TerminalStopRequest,
) -> Result<(), String> {
    if let Some(mut terminal) = state
        .terminals
        .lock()
        .map_err(|_| "terminal lock poisoned".to_string())?
        .remove(&request.id)
    {
        let _ = terminal.child.kill();
    }
    Ok(())
}

#[cfg(windows)]
fn default_shell() -> String {
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
}

#[cfg(not(windows))]
fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
}
