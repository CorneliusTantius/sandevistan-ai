mod agent;
mod ai;
mod command_utils;
mod context;
mod files;
mod provider;
mod shell;
mod subagent;
mod tools;
mod watcher;
mod workspace_tools;

#[tauri::command]
fn ai_config() -> ai::AiConfig {
    ai::config()
}

#[tauri::command]
fn ai_save_config(update: ai::AiConfigUpdate) -> Result<ai::AiConfig, String> {
    ai::save_config(update)
}

#[tauri::command]
fn ai_delete_model(request: ai::DeleteModelRequest) -> Result<ai::AiConfig, String> {
    ai::delete_model(request)
}

#[tauri::command]
fn ai_set_feature(update: ai::FeatureUpdate) -> Result<ai::AiConfig, String> {
    ai::set_feature(update)
}

#[tauri::command]
fn ai_set_mods(update: ai::ModsUpdate) -> Result<ai::AiConfig, String> {
    ai::set_mods(update)
}

#[tauri::command]
fn ai_set_active_profile(update: ai::ActiveProfileUpdate) -> Result<ai::AiConfig, String> {
    ai::set_active_profile(update)
}

#[tauri::command]
fn ai_save_agent(update: ai::AgentUpdate) -> Result<ai::AiConfig, String> {
    ai::save_agent(update)
}

#[tauri::command]
fn ai_delete_agent(request: ai::DeleteAgentRequest) -> Result<ai::AiConfig, String> {
    ai::delete_agent(request)
}

#[tauri::command]
fn ai_save_subagent(update: ai::SubagentUpdate) -> Result<ai::AiConfig, String> {
    ai::save_subagent(update)
}

#[tauri::command]
fn ai_delete_subagent(request: ai::DeleteSubagentRequest) -> Result<ai::AiConfig, String> {
    ai::delete_subagent(request)
}

#[tauri::command]
fn chat_session(chat: tauri::State<'_, agent::ChatRuntime>) -> Result<agent::SessionInfo, String> {
    chat.info()
}

#[tauri::command]
fn chat_set_workspace(
    request: agent::WorkspaceRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<agent::SessionInfo, String> {
    chat.set_workspace(request)
}

#[tauri::command]
fn chat_delete_workspace(
    request: agent::WorkspaceRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<agent::SessionInfo, String> {
    chat.delete_workspace(request)
}

#[tauri::command]
fn chat_new_session(
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<agent::SessionInfo, String> {
    chat.new_session()
}

#[tauri::command]
fn chat_select_session(
    request: agent::SessionRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<agent::SessionInfo, String> {
    chat.select_session(request)
}

#[tauri::command]
fn chat_delete_session(
    request: agent::SessionRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<agent::SessionInfo, String> {
    chat.delete_session(request)
}

#[tauri::command]
fn chat_rename_session(
    request: agent::RenameSessionRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<agent::SessionInfo, String> {
    chat.rename_session(request)
}

#[tauri::command]
fn chat_search_sessions(
    request: agent::SearchSessionsRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<Vec<agent::SessionOption>, String> {
    chat.search_sessions(request)
}

#[tauri::command]
fn chat_send(
    app: tauri::AppHandle,
    prompt: String,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<agent::TaskInfo, String> {
    chat.send(app, prompt)
}

#[tauri::command]
fn chat_cancel(chat: tauri::State<'_, agent::ChatRuntime>) -> Result<agent::SessionInfo, String> {
    chat.cancel_active()
}

#[tauri::command]
fn workspace_tree(
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<Vec<files::FileEntry>, String> {
    files::tree(&chat)
}

#[tauri::command]
fn workspace_children(
    request: files::TreeRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<Vec<files::FileEntry>, String> {
    files::children(&chat, request)
}

#[tauri::command]
fn workspace_index(
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<Vec<files::FileEntry>, String> {
    files::index(&chat)
}

#[tauri::command]
fn workspace_search(
    request: files::SearchRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<Vec<files::FileEntry>, String> {
    files::search(&chat, request)
}

#[tauri::command]
fn content_search(
    request: workspace_tools::ContentSearchRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<Vec<workspace_tools::SearchHit>, String> {
    workspace_tools::content_search(&chat, request)
}

#[tauri::command]
fn git_status(
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<workspace_tools::GitStatus, String> {
    workspace_tools::git_status(&chat)
}

#[tauri::command]
fn git_diff(
    request: workspace_tools::GitDiffRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<String, String> {
    workspace_tools::git_diff(&chat, request)
}

#[tauri::command]
fn file_read(
    request: files::FileRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<files::FileData, String> {
    files::read(&chat, request)
}

#[tauri::command]
fn file_save(
    request: files::SaveFileRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
) -> Result<files::FileData, String> {
    files::save(&chat, request)
}

#[tauri::command]
fn file_watch_start(
    app: tauri::AppHandle,
    chat: tauri::State<'_, agent::ChatRuntime>,
    watcher: tauri::State<'_, watcher::FileWatcherState>,
) -> Result<(), String> {
    watcher::start(app, chat.inner().clone(), &watcher)
}

#[tauri::command]
fn file_watch_stop(watcher: tauri::State<'_, watcher::FileWatcherState>) -> Result<(), String> {
    watcher::stop(&watcher)
}

#[tauri::command]
fn terminal_start(
    app: tauri::AppHandle,
    request: shell::TerminalStartRequest,
    chat: tauri::State<'_, agent::ChatRuntime>,
    terminals: tauri::State<'_, shell::TerminalState>,
) -> Result<(), String> {
    shell::start(app, chat.inner().clone(), terminals, request)
}

#[tauri::command]
fn terminal_write(
    request: shell::TerminalWriteRequest,
    terminals: tauri::State<'_, shell::TerminalState>,
) -> Result<(), String> {
    shell::write(terminals, request)
}

#[tauri::command]
fn terminal_resize(
    request: shell::TerminalResizeRequest,
    terminals: tauri::State<'_, shell::TerminalState>,
) -> Result<(), String> {
    shell::resize(terminals, request)
}

#[tauri::command]
fn terminal_stop(
    request: shell::TerminalStopRequest,
    terminals: tauri::State<'_, shell::TerminalState>,
) -> Result<(), String> {
    shell::stop(terminals, request)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(agent::ChatRuntime::default())
        .manage(shell::TerminalState::default())
        .manage(watcher::FileWatcherState::default())
        .invoke_handler(tauri::generate_handler![
            ai_config,
            ai_save_config,
            ai_delete_model,
            ai_set_feature,
            ai_set_mods,
            ai_set_active_profile,
            ai_save_agent,
            ai_delete_agent,
            ai_save_subagent,
            ai_delete_subagent,
            chat_session,
            chat_set_workspace,
            chat_delete_workspace,
            chat_new_session,
            chat_select_session,
            chat_delete_session,
            chat_rename_session,
            chat_search_sessions,
            chat_send,
            chat_cancel,
            workspace_tree,
            workspace_children,
            workspace_index,
            workspace_search,
            content_search,
            git_status,
            git_diff,
            file_read,
            file_save,
            file_watch_start,
            file_watch_stop,
            terminal_start,
            terminal_write,
            terminal_resize,
            terminal_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
