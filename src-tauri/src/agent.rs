use crate::{
    ai::{self, ChatMessage},
    context, subagent, tools,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

const CONFIG_DIR_NAME: &str = ".sandevistan";
const MAX_REPEAT_TOOL_CALLS: usize = 3;
const TOOL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(25);
const SUMMARY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize)]
pub struct WorkspaceRequest {
    path: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionRequest {
    id: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameSessionRequest {
    id: String,
    title: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchSessionsRequest {
    query: String,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceOption {
    path: String,
    name: String,
    deletable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOption {
    id: String,
    title: String,
    preview: String,
    message_count: usize,
    updated_at: u128,
    running: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    workspace: String,
    active_session_id: String,
    messages: Vec<ChatMessage>,
    sessions: Vec<SessionOption>,
    workspaces: Vec<WorkspaceOption>,
}

#[derive(Debug, Serialize)]
pub struct TaskInfo {
    id: String,
    session_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct StreamEvent {
    session_id: String,
    kind: String,
    role: Option<String>,
    text: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    workspace: PathBuf,
    active_session_id: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct SessionIndex {
    active_session_id: Option<String>,
    sessions: Vec<SessionOption>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct SessionFile {
    id: String,
    workspace: String,
    title: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct SessionSummary {
    summary: String,
    last_message_count: usize,
    updated_at: u128,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct AppConfig {
    default_provider: Option<String>,
    default_model: Option<String>,
    active_workspace: Option<String>,
    workspaces: Option<Vec<String>>,
    features: Option<HashMap<String, bool>>,
    active_profile: Option<String>,
    profiles: Option<HashMap<String, toml::Value>>,
    persona: Option<String>,
    thinking_level: Option<String>,
    prompt_injection: Option<String>,
    rtk_enabled: Option<bool>,
}

#[derive(Clone)]
pub struct ChatRuntime {
    state: Arc<Mutex<RuntimeState>>,
    tasks: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
}

impl Default for ChatRuntime {
    fn default() -> Self {
        let workspace = startup_workspace();
        add_workspace(&workspace).ok();
        save_active_workspace(&workspace).ok();
        let active_session_id = ensure_index(&workspace)
            .active_session_id
            .unwrap_or_else(|| create_session(&workspace).id);
        Self {
            state: Arc::new(Mutex::new(RuntimeState {
                workspace,
                active_session_id,
            })),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ChatRuntime {
    pub fn info(&self) -> Result<SessionInfo, String> {
        let state = self.snapshot()?;
        session_info(&state.workspace, &state.active_session_id)
    }

    pub fn workspace(&self) -> Result<PathBuf, String> {
        self.snapshot().map(|state| state.workspace)
    }

    pub fn set_workspace(&self, request: WorkspaceRequest) -> Result<SessionInfo, String> {
        let workspace = normalize_workspace(request.path)?;
        add_workspace(&workspace)?;
        let index = ensure_index(&workspace);
        let active_session_id = index
            .active_session_id
            .unwrap_or_else(|| create_session(&workspace).id);
        self.replace(RuntimeState {
            workspace: workspace.clone(),
            active_session_id: active_session_id.clone(),
        })?;
        save_active_workspace(&workspace)?;
        session_info(&workspace, &active_session_id)
    }

    pub fn delete_workspace(&self, request: WorkspaceRequest) -> Result<SessionInfo, String> {
        let workspace = normalize_workspace(request.path)?;
        if workspace == default_workspace() {
            return Err("home workspace cannot be deleted".into());
        }
        remove_workspace(&workspace)?;
        let _ = fs::remove_dir_all(workspace_sessions_dir(&workspace));
        let current = self.snapshot()?.workspace;
        if current == workspace {
            let fallback = load_workspaces()
                .into_iter()
                .next()
                .unwrap_or_else(default_workspace);
            let index = ensure_index(&fallback);
            let active_session_id = index
                .active_session_id
                .unwrap_or_else(|| create_session(&fallback).id);
            self.replace(RuntimeState {
                workspace: fallback.clone(),
                active_session_id: active_session_id.clone(),
            })?;
            save_active_workspace(&fallback)?;
            return session_info(&fallback, &active_session_id);
        }
        self.info()
    }

    pub fn new_session(&self) -> Result<SessionInfo, String> {
        let workspace = self.snapshot()?.workspace;
        let session = create_session(&workspace);
        let mut index = ensure_index(&workspace);
        index.active_session_id = Some(session.id.clone());
        index.sessions.push(meta_from_file(&session, false));
        save_index(&workspace, &index)?;
        self.replace(RuntimeState {
            workspace: workspace.clone(),
            active_session_id: session.id.clone(),
        })?;
        session_info(&workspace, &session.id)
    }

    pub fn select_session(&self, request: SessionRequest) -> Result<SessionInfo, String> {
        let workspace = self.snapshot()?.workspace;
        let mut index = ensure_index(&workspace);
        if !index
            .sessions
            .iter()
            .any(|session| session.id == request.id)
        {
            return Err("session not found".into());
        }
        index.active_session_id = Some(request.id.clone());
        save_index(&workspace, &index)?;
        self.replace(RuntimeState {
            workspace: workspace.clone(),
            active_session_id: request.id.clone(),
        })?;
        session_info(&workspace, &request.id)
    }

    pub fn delete_session(&self, request: SessionRequest) -> Result<SessionInfo, String> {
        let state = self.snapshot()?;
        let mut index = ensure_index(&state.workspace);
        index.sessions.retain(|session| session.id != request.id);
        let _ = fs::remove_file(session_file_path(&state.workspace, &request.id));

        if index.sessions.is_empty() {
            let session = create_session(&state.workspace);
            index.sessions.push(meta_from_file(&session, false));
            index.active_session_id = Some(session.id.clone());
        } else if index.active_session_id.as_deref() == Some(request.id.as_str()) {
            index.active_session_id = index.sessions.first().map(|session| session.id.clone());
        }

        let active_session_id = index
            .active_session_id
            .clone()
            .ok_or_else(|| "active session missing".to_string())?;
        save_index(&state.workspace, &index)?;
        self.replace(RuntimeState {
            workspace: state.workspace.clone(),
            active_session_id: active_session_id.clone(),
        })?;
        session_info(&state.workspace, &active_session_id)
    }

    pub fn rename_session(&self, request: RenameSessionRequest) -> Result<SessionInfo, String> {
        let title = clean_title(request.title)?;
        let state = self.snapshot()?;
        let mut index = ensure_index(&state.workspace);
        let meta = index
            .sessions
            .iter_mut()
            .find(|session| session.id == request.id)
            .ok_or_else(|| "session not found".to_string())?;
        meta.title = title.clone();
        meta.updated_at = now_ms();

        let mut file = load_session_file(&state.workspace, &request.id)?;
        file.title = title;
        save_session_file(&state.workspace, &file)?;
        save_index(&state.workspace, &index)?;
        session_info(&state.workspace, &state.active_session_id)
    }

    pub fn search_sessions(
        &self,
        request: SearchSessionsRequest,
    ) -> Result<Vec<SessionOption>, String> {
        let state = self.snapshot()?;
        let query = request.query.trim().to_lowercase();
        let index = ensure_index(&state.workspace);
        if query.is_empty() {
            return Ok(index.sessions);
        }

        Ok(index
            .sessions
            .into_iter()
            .filter(|session| {
                session.title.to_lowercase().contains(&query)
                    || session.preview.to_lowercase().contains(&query)
                    || load_session_file(&state.workspace, &session.id)
                        .map(|file| {
                            file.messages
                                .iter()
                                .any(|message| message.content.to_lowercase().contains(&query))
                        })
                        .unwrap_or(false)
            })
            .collect())
    }

    pub fn send(&self, app: AppHandle, input: String) -> Result<TaskInfo, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("message is empty".into());
        }

        let state = self.snapshot()?;
        let mut index = ensure_index(&state.workspace);
        let meta = index
            .sessions
            .iter_mut()
            .find(|session| session.id == state.active_session_id)
            .ok_or_else(|| "active session not found".to_string())?;
        if meta.running {
            return Err("session is running".into());
        }

        let mut file = load_session_file(&state.workspace, &state.active_session_id)?;
        file.messages.push(ChatMessage {
            role: "user".into(),
            content: input.into(),
        });
        if file.title == "untitled" {
            file.title = title_from_messages(&file.messages);
            meta.title = file.title.clone();
        }
        meta.preview = preview_from_messages(&file.messages);
        meta.message_count = file.messages.len();
        meta.updated_at = now_ms();
        meta.running = true;
        save_session_file(&state.workspace, &file)?;
        save_index(&state.workspace, &index)?;

        let workspace = state.workspace.clone();
        let session_id = state.active_session_id.clone();
        let task_session_id = session_id.clone();
        let messages = file.messages;
        let task_id = new_id();
        let tasks = self.tasks.clone();
        let mods = ai::active_mods();
        let prompt_config = ai::prompt_config();
        let handle = tauri::async_runtime::spawn(async move {
            let result = run_agent_loop(
                &app,
                &workspace,
                &task_session_id,
                messages,
                mods,
                prompt_config,
            )
            .await;
            if let Err(error) = &result {
                emit_stream_error(&app, &task_session_id, error);
            }
            finish_task(&workspace, &task_session_id, result).await.ok();
            emit_stream_done(&app, &task_session_id);
            if let Ok(mut tasks) = tasks.lock() {
                tasks.remove(&task_session_id);
            }
        });
        self.tasks
            .lock()
            .map_err(|_| "task lock poisoned".to_string())?
            .insert(session_id.clone(), handle);

        Ok(TaskInfo {
            id: task_id,
            session_id,
        })
    }

    pub fn cancel_active(&self) -> Result<SessionInfo, String> {
        let state = self.snapshot()?;
        let handle = self
            .tasks
            .lock()
            .map_err(|_| "task lock poisoned".to_string())?
            .remove(&state.active_session_id);

        if let Some(handle) = handle {
            handle.abort();
            cancel_session(&state.workspace, &state.active_session_id)?;
        }

        session_info(&state.workspace, &state.active_session_id)
    }

    fn snapshot(&self) -> Result<RuntimeState, String> {
        self.state
            .lock()
            .map_err(|_| "chat lock poisoned".to_string())
            .map(|state| state.clone())
    }

    fn replace(&self, state: RuntimeState) -> Result<(), String> {
        *self
            .state
            .lock()
            .map_err(|_| "chat lock poisoned".to_string())? = state;
        Ok(())
    }
}

fn cancel_session(workspace: &PathBuf, session_id: &str) -> Result<(), String> {
    let mut index = ensure_index(workspace);
    let mut file = load_session_file(workspace, session_id)?;
    file.messages.push(ChatMessage {
        role: "error".into(),
        content: "cancelled".into(),
    });

    if let Some(meta) = index
        .sessions
        .iter_mut()
        .find(|session| session.id == session_id)
    {
        meta.preview = preview_from_messages(&file.messages);
        meta.message_count = file.messages.len();
        meta.updated_at = now_ms();
        meta.running = false;
    }
    save_session_file(workspace, &file)?;
    save_index(workspace, &index)
}

async fn run_agent_loop(
    app: &AppHandle,
    workspace: &PathBuf,
    session_id: &str,
    mut messages: Vec<ChatMessage>,
    mods: ai::ModelMods,
    prompt_config: context::PromptConfig,
) -> Result<Vec<ChatMessage>, String> {
    let mods_prompt = ai::mods_prompt();
    let base_prompt = tools::prompt_with_subagents(
        mods.subagents_enabled && !mods.subagents.is_empty(),
        &mods.subagents,
        mods.shell_enabled,
    );
    let system_prompt = if mods_prompt.is_empty() {
        base_prompt
    } else {
        format!("{}\n\n{}", base_prompt, mods_prompt)
    };
    let summary = tokio::time::timeout(
        SUMMARY_TIMEOUT,
        update_session_summary(workspace, session_id, &messages),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_else(|| load_session_summary(workspace, session_id).unwrap_or_default())
    .summary;
    if should_ping_subagents(&messages, &mods) {
        let calls = vec![tools::ToolCall {
            name: "agent.delegate".into(),
            args: serde_json::json!({
                "tasks": mods.subagents.iter().map(|agent| serde_json::json!({
                    "agent": agent,
                    "task": "Ping back with your subagent name and one concise status line. Do not use tools unless needed."
                })).collect::<Vec<_>>()
            }),
        }];
        let tool_contents = run_tool_calls(workspace, calls, mods.clone()).await;
        for tool_content in tool_contents {
            emit_stream_tool(app, session_id, &tool_content);
            messages.push(ChatMessage {
                role: "tool".into(),
                content: tool_content,
            });
        }
    }

    let mut tool_call_counts: HashMap<String, usize> = HashMap::new();
    loop {
        let prompt =
            context::build_prompt(&system_prompt, Some(&summary), &messages, &prompt_config);

        let mut streamed = false;
        let mut pending = String::new();
        let content =
            ai::complete_chat_stream_model(prompt, Some(mods.main_model.clone()), |delta| {
                if streamed {
                    emit_stream_delta(app, session_id, &delta);
                    return;
                }

                pending.push_str(&delta);
                let trimmed = pending.trim_start();
                if "<tool_call>".starts_with(trimmed) || trimmed.starts_with("<tool_call>") {
                    return;
                }

                streamed = true;
                emit_stream_start(app, session_id);
                emit_stream_delta(app, session_id, &pending);
                pending.clear();
            })
            .await?;
        let calls = tools::parse_tool_calls(&content);
        if !calls.is_empty() {
            for call in &calls {
                let signature = tool_signature(call);
                let count = tool_call_counts.entry(signature.clone()).or_default();
                *count += 1;
                if *count > MAX_REPEAT_TOOL_CALLS {
                    return Err(format!("repeated tool call loop: {signature}"));
                }
            }

            let tool_contents = run_tool_calls(workspace, calls, mods.clone()).await;
            for tool_content in tool_contents {
                emit_stream_tool(app, session_id, &tool_content);
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: tool_content,
                });
            }
            continue;
        }

        if !streamed {
            emit_stream_start(app, session_id);
            emit_stream_delta(app, session_id, &content);
        }
        messages.push(ChatMessage {
            role: "assistant".into(),
            content,
        });
        return Ok(messages);
    }
}

fn should_ping_subagents(messages: &[ChatMessage], mods: &ai::ModelMods) -> bool {
    if !mods.subagents_enabled || mods.subagents.is_empty() {
        return false;
    }
    let Some(message) = messages.last() else {
        return false;
    };
    if message.role != "user" {
        return false;
    }
    let content = message.content.to_lowercase();
    content.contains("ping") && content.contains("subagent")
}

async fn run_tool_calls(
    workspace: &PathBuf,
    calls: Vec<tools::ToolCall>,
    mods: ai::ModelMods,
) -> Vec<String> {
    if calls.iter().any(tools::is_mutating) {
        let mut results = Vec::new();
        for call in calls {
            let name = call.name.clone();
            let output = run_tool_call(workspace.clone(), call, mods.clone()).await;
            results.push(format!("{name}\n{output}"));
        }
        return results;
    }

    let mut results = Vec::new();
    let mut parallel = Vec::new();
    for (index, call) in calls.into_iter().enumerate() {
        let workspace = workspace.clone();
        let mods = mods.clone();
        parallel.push(tauri::async_runtime::spawn(async move {
            let name = call.name.clone();
            let output = run_tool_call(workspace, call, mods).await;
            (index, format!("{name}\n{output}"))
        }));
    }

    for handle in parallel {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, content)| content).collect()
}

async fn run_tool_call(workspace: PathBuf, call: tools::ToolCall, mods: ai::ModelMods) -> String {
    if subagent::is_delegate(&call) {
        return subagent::run_delegate(workspace, call.args, mods.clone(), mods.rtk_enabled).await;
    }
    run_tool_blocking(workspace, call, mods.rtk_enabled, mods.shell_enabled).await
}

async fn run_tool_blocking(
    workspace: PathBuf,
    call: tools::ToolCall,
    rtk_enabled: bool,
    shell_enabled: bool,
) -> String {
    match tokio::time::timeout(
        TOOL_EXECUTION_TIMEOUT,
        tauri::async_runtime::spawn_blocking(move || {
            tools::run_with_options(
                &workspace,
                &call,
                tools::ToolOptions {
                    rtk_enabled,
                    shell_enabled,
                },
            )
        }),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => format!("status: failed\nerror: tool task failed: {error}"),
        Err(_) => format!(
            "status: failed\nerror: tool execution timed out after {}s",
            TOOL_EXECUTION_TIMEOUT.as_secs()
        ),
    }
}

fn tool_signature(call: &tools::ToolCall) -> String {
    let args = serde_json::to_string(&call.args).unwrap_or_else(|_| "{}".into());
    format!("{} {args}", call.name)
}

fn emit_stream_start(app: &AppHandle, session_id: &str) {
    emit_stream(
        app,
        StreamEvent {
            session_id: session_id.into(),
            kind: "start".into(),
            role: Some("assistant".into()),
            text: None,
            content: None,
        },
    );
}

fn emit_stream_delta(app: &AppHandle, session_id: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    emit_stream(
        app,
        StreamEvent {
            session_id: session_id.into(),
            kind: "delta".into(),
            role: None,
            text: Some(text.into()),
            content: None,
        },
    );
}

fn emit_stream_tool(app: &AppHandle, session_id: &str, content: &str) {
    emit_stream(
        app,
        StreamEvent {
            session_id: session_id.into(),
            kind: "tool".into(),
            role: None,
            text: None,
            content: Some(content.into()),
        },
    );
}

fn emit_stream_error(app: &AppHandle, session_id: &str, content: &str) {
    emit_stream(
        app,
        StreamEvent {
            session_id: session_id.into(),
            kind: "error".into(),
            role: None,
            text: None,
            content: Some(content.into()),
        },
    );
}

fn emit_stream_done(app: &AppHandle, session_id: &str) {
    emit_stream(
        app,
        StreamEvent {
            session_id: session_id.into(),
            kind: "done".into(),
            role: None,
            text: None,
            content: None,
        },
    );
}

fn emit_stream(app: &AppHandle, event: StreamEvent) {
    let _ = app.emit("chat_stream", event);
}

async fn finish_task(
    workspace: &PathBuf,
    session_id: &str,
    result: Result<Vec<ChatMessage>, String>,
) -> Result<(), String> {
    let mut index = ensure_index(workspace);
    let mut file = load_session_file(workspace, session_id)?;
    match result {
        Ok(messages) => {
            file.messages = messages;
        }
        Err(error) => file.messages.push(ChatMessage {
            role: "error".into(),
            content: error,
        }),
    }

    if let Some(meta) = index
        .sessions
        .iter_mut()
        .find(|session| session.id == session_id)
    {
        meta.title = file.title.clone();
        meta.preview = preview_from_messages(&file.messages);
        meta.message_count = file.messages.len();
        meta.updated_at = now_ms();
        meta.running = false;
    }
    save_session_file(workspace, &file)?;
    save_index(workspace, &index)
}

fn session_info(workspace: &PathBuf, active_session_id: &str) -> Result<SessionInfo, String> {
    let index = ensure_index(workspace);
    let file = load_session_file(workspace, active_session_id)?;
    Ok(SessionInfo {
        workspace: workspace.display().to_string(),
        active_session_id: active_session_id.into(),
        messages: file.messages,
        sessions: index.sessions,
        workspaces: workspace_options(workspace),
    })
}

fn ensure_index(workspace: &PathBuf) -> SessionIndex {
    let mut index = read_json::<SessionIndex>(index_path(workspace));
    if index.sessions.is_empty() {
        let file = create_session(workspace);
        index.active_session_id = Some(file.id.clone());
        index.sessions.push(meta_from_file(&file, false));
        save_index(workspace, &index).ok();
        return index;
    }

    if index.active_session_id.is_none() {
        index.active_session_id = index.sessions.first().map(|session| session.id.clone());
        save_index(workspace, &index).ok();
    }
    index
}

fn create_session(workspace: &PathBuf) -> SessionFile {
    let file = SessionFile {
        id: new_id(),
        workspace: workspace.display().to_string(),
        title: "untitled".into(),
        messages: Vec::new(),
    };
    save_session_file(workspace, &file).ok();
    file
}

fn load_session_file(workspace: &PathBuf, session_id: &str) -> Result<SessionFile, String> {
    fs::read_to_string(session_file_path(workspace, session_id))
        .map_err(|error| format!("session read failed: {error}"))
        .and_then(|content| {
            serde_json::from_str(&content).map_err(|error| format!("session parse failed: {error}"))
        })
}

fn save_session_file(workspace: &PathBuf, file: &SessionFile) -> Result<(), String> {
    let path = session_file_path(workspace, &file.id);
    write_json(path, file)
}

fn load_session_summary(workspace: &PathBuf, session_id: &str) -> Result<SessionSummary, String> {
    let path = session_summary_path(workspace, session_id);
    if !path.exists() {
        return Ok(SessionSummary::default());
    }
    fs::read_to_string(path)
        .map_err(|error| format!("summary read failed: {error}"))
        .and_then(|content| {
            serde_json::from_str(&content).map_err(|error| format!("summary parse failed: {error}"))
        })
}

fn save_session_summary(
    workspace: &PathBuf,
    session_id: &str,
    summary: &SessionSummary,
) -> Result<(), String> {
    write_json(session_summary_path(workspace, session_id), summary)
}

fn save_index(workspace: &PathBuf, index: &SessionIndex) -> Result<(), String> {
    write_json(index_path(workspace), index)
}

fn meta_from_file(file: &SessionFile, running: bool) -> SessionOption {
    SessionOption {
        id: file.id.clone(),
        title: file.title.clone(),
        preview: preview_from_messages(&file.messages),
        message_count: file.messages.len(),
        updated_at: now_ms(),
        running,
    }
}

async fn update_session_summary(
    workspace: &PathBuf,
    session_id: &str,
    messages: &[ChatMessage],
) -> Result<SessionSummary, String> {
    const MIN_MESSAGES: usize = 18;
    const KEEP_RECENT: usize = 8;
    const MIN_NEW_MESSAGES: usize = 8;
    const MAX_SOURCE_CHARS: usize = 18_000;

    let current = load_session_summary(workspace, session_id)?;
    if messages.len() < MIN_MESSAGES
        || messages.len().saturating_sub(current.last_message_count) < MIN_NEW_MESSAGES
    {
        return Ok(current);
    }

    let cutoff = messages.len().saturating_sub(KEEP_RECENT);
    if cutoff <= current.last_message_count {
        return Ok(current);
    }

    let source = messages[current.last_message_count..cutoff]
        .iter()
        .map(|message| {
            format!(
                "{}: {}",
                message.role,
                truncate_summary_source(&message.content, 1_200)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let source = truncate_summary_source(&source, MAX_SOURCE_CHARS);
    let prompt = vec![
        ChatMessage {
            role: "system".into(),
            content: "Update compact coding-session memory. Preserve goals, decisions, file paths, commands, errors, pending work. Be terse. Max 1800 chars.".into(),
        },
        ChatMessage {
            role: "user".into(),
            content: format!(
                "Previous summary:\n{}\n\nNew messages:\n{}",
                current.summary, source
            ),
        },
    ];

    let summary = ai::complete_chat(prompt)
        .await
        .map(|value| truncate_summary_source(value.trim(), 1_800))?;
    let next = SessionSummary {
        summary,
        last_message_count: cutoff,
        updated_at: now_ms(),
    };
    save_session_summary(workspace, session_id, &next)?;
    Ok(next)
}

fn truncate_summary_source(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.into();
    }
    value.chars().take(max).collect::<String>()
}

fn title_from_messages(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .find(|message| message.role == "user")
        .map(|message| truncate(&message.content, 48))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "untitled".into())
}

fn preview_from_messages(messages: &[ChatMessage]) -> String {
    messages
        .last()
        .map(|message| truncate(&message.content, 72))
        .unwrap_or_default()
}

fn truncate(value: &str, max: usize) -> String {
    let value = value.trim().replace('\n', " ");
    if value.chars().count() <= max {
        return value;
    }
    value.chars().take(max).collect::<String>()
}

fn add_workspace(workspace: &PathBuf) -> Result<(), String> {
    let mut workspaces = load_workspaces();
    if !workspaces.iter().any(|entry| entry == workspace) {
        workspaces.push(workspace.clone());
    }
    save_workspaces(workspaces)
}

fn remove_workspace(workspace: &PathBuf) -> Result<(), String> {
    let workspaces = load_workspaces()
        .into_iter()
        .filter(|entry| entry != workspace)
        .collect::<Vec<_>>();
    save_workspaces(workspaces)
}

fn workspace_options(current: &PathBuf) -> Vec<WorkspaceOption> {
    let mut workspaces = load_workspaces();
    if !workspaces.iter().any(|entry| entry == current) {
        workspaces.push(current.clone());
    }
    workspaces.sort();
    let home = default_workspace();
    workspaces
        .into_iter()
        .map(|path| WorkspaceOption {
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| path.to_str().unwrap_or("/"))
                .to_string(),
            deletable: path != home,
            path: path.display().to_string(),
        })
        .collect()
}

fn startup_workspace() -> PathBuf {
    let config = read_toml::<AppConfig>(config_dir().join("config.toml"));
    config
        .active_workspace
        .and_then(|path| normalize_workspace(path).ok())
        .or_else(|| load_workspaces().into_iter().next())
        .unwrap_or_else(default_workspace)
}

fn load_workspaces() -> Vec<PathBuf> {
    read_toml::<AppConfig>(config_dir().join("config.toml"))
        .workspaces
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| normalize_workspace(path).ok())
        .collect::<Vec<_>>()
}

fn save_workspaces(workspaces: Vec<PathBuf>) -> Result<(), String> {
    let path = config_dir().join("config.toml");
    let mut config = read_toml::<AppConfig>(path.clone());
    let mut values = workspaces
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    config.workspaces = Some(values);
    write_toml(path, &config)
}

fn save_active_workspace(workspace: &PathBuf) -> Result<(), String> {
    let path = config_dir().join("config.toml");
    let mut config = read_toml::<AppConfig>(path.clone());
    config.active_workspace = Some(workspace.display().to_string());
    write_toml(path, &config)
}

fn workspace_sessions_dir(workspace: &PathBuf) -> PathBuf {
    config_dir()
        .join("sessions")
        .join(hash_workspace(workspace))
}

fn index_path(workspace: &PathBuf) -> PathBuf {
    workspace_sessions_dir(workspace).join("index.json")
}

fn session_file_path(workspace: &PathBuf, session_id: &str) -> PathBuf {
    workspace_sessions_dir(workspace).join(format!("{session_id}.json"))
}

fn session_summary_path(workspace: &PathBuf, session_id: &str) -> PathBuf {
    workspace_sessions_dir(workspace).join(format!("{session_id}.summary.json"))
}

fn normalize_workspace(path: String) -> Result<PathBuf, String> {
    let path = clean_path(path)?;
    if !path.is_dir() {
        return Err("workspace is not a directory".into());
    }
    path.canonicalize()
        .map_err(|error| format!("workspace canonicalize failed: {error}"))
}

fn clean_title(title: String) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("title is empty".into());
    }
    Ok(truncate(title, 80))
}

fn clean_path(path: String) -> Result<PathBuf, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("workspace path is empty".into());
    }

    if let Some(rest) = path.strip_prefix("~/") {
        return dirs::home_dir()
            .map(|home| home.join(rest))
            .ok_or_else(|| "home directory unavailable".into());
    }

    Ok(PathBuf::from(path))
}

fn default_workspace() -> PathBuf {
    dirs::home_dir()
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(CONFIG_DIR_NAME)
}

fn read_json<T>(path: PathBuf) -> T
where
    T: for<'de> Deserialize<'de> + Default,
{
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<T>(&content).ok())
        .unwrap_or_default()
}

fn write_json<T>(path: PathBuf, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("dir create failed: {error}"))?;
    }
    let content =
        serde_json::to_string(value).map_err(|error| format!("json serialize failed: {error}"))?;
    fs::write(path, content).map_err(|error| format!("json write failed: {error}"))
}

fn read_toml<T>(path: PathBuf) -> T
where
    T: for<'de> Deserialize<'de> + Default,
{
    fs::read_to_string(path)
        .ok()
        .and_then(|content| toml::from_str::<T>(&content).ok())
        .unwrap_or_default()
}

fn write_toml<T>(path: PathBuf, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("config dir create failed: {error}"))?;
    }

    let content = toml::to_string_pretty(value)
        .map_err(|error| format!("config serialize failed: {error}"))?;
    fs::write(path, content).map_err(|error| format!("config write failed: {error}"))
}

fn new_id() -> String {
    format!("{:x}", now_ms())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn hash_workspace(path: &PathBuf) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.display().to_string().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
