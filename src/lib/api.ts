import { invoke } from "@tauri-apps/api/core";
import type { AiConfig, ExtensionsInfo, FileChangedEvent, GitStatus, Message, SearchHit, SessionInfo } from "../types";
import type { FileEntry } from "../types";

export const api = {
  aiConfig: () => invoke<AiConfig>("ai_config"),
  chatSession: () => invoke<SessionInfo>("chat_session"),
  chatCompact: () => invoke<SessionInfo>("chat_compact"),
  chatCancel: () => invoke<SessionInfo>("chat_cancel"),
  chatNewSession: () => invoke<SessionInfo>("chat_new_session"),
  workspaceTree: () => invoke<FileEntry[]>("workspace_tree"),
  extensionsInfo: () => invoke<ExtensionsInfo>("extensions_info"),
  gitStatus: () => invoke<GitStatus>("git_status"),
  fileWatchStart: () => invoke("file_watch_start"),
  fileWatchStop: () => invoke("file_watch_stop"),
};

export type { AiConfig, ExtensionsInfo, FileChangedEvent, FileEntry, GitStatus, Message, SearchHit, SessionInfo };
