<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { api } from "./lib/api";
  import { defaultAgentDraft, defaultExtensionDraft, defaultExtensionsInfo, defaultMcpDraft, defaultModelDraft, baseMods, defaultProviderDraft, defaultSubagentDraft, emptyConfig } from "./lib/defaults";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import Checkbox from "./components/ui/Checkbox.svelte";
  import AppHeader from "./panels/AppHeader.svelte";
  import AppSidebar from "./panels/AppSidebar.svelte";
  import ChatPanel from "./panels/ChatPanel.svelte";
  import type { DiffTab } from "./panels/DiffPane.svelte";
  import type { OpenFile } from "./panels/EditorPane.svelte";
  import ItemList, { type Item } from "./components/ui/ItemList.svelte";
  import Modal from "./components/ui/Modal.svelte";
  import RenameModal from "./panels/modals/RenameModal.svelte";
  import WorkspaceModal from "./panels/modals/WorkspaceModal.svelte";
  import SelectBox, { type SelectOption } from "./components/ui/SelectBox.svelte";
  import type {
    AgentOption,
    AiConfig,
    AiMods,
    ChatStreamEvent,
    ExtensionsInfo,
    ExtensionInfo,
    FileChangedEvent,
    FileEntry,
    GitStatus,
    McpServer,
    McpServerDraft,
    Message,
    MessageGroup,
    ModelOption,
    ProfileOption,
    ProviderOption,
    SearchHit,
    SessionInfo,
    SessionOption,
    SubagentOption,
    TextSnapshot,
    ThinkingLevel,
    WorkspaceOption,
  } from "./types";

  const appVersion = import.meta.env.PACKAGE_VERSION ?? "dev";

  let modelLabel = "model";
  let prompt = "";
  let promptEl: HTMLTextAreaElement;
  let mentionQuery = "";
  let mentionStart = -1;
  let mentionResults: FileEntry[] = [];
  let mentionIndex = 0;
  let mentionTimer = 0;
  let promptUndoStack: TextSnapshot[] = [];
  let promptRedoStack: TextSnapshot[] = [];
  let busy = false;
  let compacting = false;
  let showConfig = false;
  let showMods = false;
  let showWorkspace = false;
  let addingModel = false;
  let addingProvider = false;
  let EditorPaneComponent: any = null;
  let DiffPaneComponent: any = null;
  let TerminalPaneComponent: any = null;
  let messages: Message[] = [];
  let messageGroupLimit = 80;
  let messagesEl: HTMLDivElement;
  let streamBuffer = "";
  let streamFrame = 0;
  let fileChangeTimer = 0;
  let streamMessageOpen = false;
  let liveSessions: Record<string, { messages: Message[]; streamBuffer: string; streamMessageOpen: boolean }> = {};
  let seenStreamEvents = new Set<string>();
  let providerInputTokens = 0;
  let providerOutputTokens = 0;
  let providerTotalTokens = 0;
  let files: FileEntry[] = [];
  let fileSearchResults: FileEntry[] = [];
  let fileSearching = false;
  let fileSearchTimer = 0;
  let fileSearchTruncated = false;
  let expandedDirs = new Set<string>();
  let loadedDirs = new Set<string>();
  let fileTreeVersion = 0;
  let fileQuery = "";
  let contentQuery = "";
  let contentResults: SearchHit[] = [];
  let contentSearching = false;
  let gitStatus: GitStatus | null = null;
  let gitDiff = "";
  let gitDiffPath = "";
  let gitLoading = false;
  let diffTabs: DiffTab[] = [];
  let openFiles: OpenFile[] = [];
  let toolMutationCount = 0;
  let chatOpen = true;
  let terminalOpen = false;
  let terminalId = "main";
  let activeTab = "chat";
  let sideTab: "files" | "content" | "git" = "files";
  let workspace = "";
  let workspaces: WorkspaceOption[] = [];
  let addingWorkspace = false;
  let workspaceDraft = "";
  let activeSessionId = "";
  let sessionLabel = "session";
  let sessions: SessionOption[] = [];
  let sessionQuery = "";
  let renameSessionId = "";
  let renameDraft = "";
  let config: AiConfig = emptyConfig;
  let uiScale = 1;
  let providerChoice = "openai";
  let draft = { ...defaultModelDraft };
  let providerDraft = { ...defaultProviderDraft };
  let modsDraft: AiMods = { ...baseMods };
  let modsProfile = "default";
  let modsTab: "general" | "profile" | "providers" | "models" | "agents" | "subagents" | "mcp" | "extensions" = "profile";
  let addingAgent = false;
  let addingSubagent = false;
  let editingExtension = false;
  let editingMcp = false;
  let mcpDraft: McpServerDraft = { ...defaultMcpDraft };
  let agentDraft = { ...defaultAgentDraft };
  let subagentDraft = { ...defaultSubagentDraft };
  let extensionDraft: ExtensionInfo = { ...defaultExtensionDraft };
  let creatingExtension = false;
  let extensionsInfo: ExtensionsInfo = { ...defaultExtensionsInfo };
  $: providerOptions = [
    ...config.providers.map((provider): SelectOption => ({ value: provider.name, label: provider.name })),
  ];
  const providerKindOptions: SelectOption[] = [
    { value: "openai-compatible", label: "chat/completions" },
    { value: "openai-responses", label: "responses" },
  ];
  const apiKeyHeaderOptions: SelectOption[] = [
    { value: "authorization", label: "Authorization: Bearer" },
    { value: "api-key", label: "api-key" },
  ];
  const thinkingOptions: SelectOption[] = [
    { value: "auto", label: "auto" },
    { value: "low", label: "low" },
    { value: "medium", label: "medium" },
    { value: "high", label: "high" },
  ];
  $: topProfileOptions = config.profiles.map((profile): SelectOption => ({ value: profile.name, label: `${profile.name} · ${profile.main_model || config.model || "model"}` }));
  $: profileOptions = [
    ...topProfileOptions,
    { value: "__new__", label: "+ profile" },
  ];
  $: mainModelOptions = config.models.map((model): SelectOption => ({ value: model.name, label: model.name }));
  $: subagentModelOptions = [
    { value: "", label: "default (follow profile/fallback)" },
    ...config.models.map((model): SelectOption => ({ value: model.name, label: model.name })),
  ];
  $: subagentEntries = config.subagents_registry.map((entry) => entry.name);
  $: mainAgentOptions = config.agents.map((agent): SelectOption => ({ value: agent.name, label: agent.name }));
  $: agentItems = config.agents.map((agent): Item => ({
    key: agent.name,
    title: agent.name,
    subtitle: agent.description || agent.prompt_injection || "agent",
    active: false,
    onSelect: () => {},
    actions: [
      { label: "edit", onClick: () => editAgent(agent) },
      { label: "del", danger: true, onClick: () => void deleteAgent(agent.name) },
    ],
  }));
  $: subagentItems = config.subagents_registry.map((subagent): Item => ({
    key: subagent.name,
    title: subagent.name,
    subtitle: subagent.description || subagent.system,
    active: false,
    onSelect: () => {},
    actions: [
      { label: "edit", onClick: () => editSubagent(subagent) },
      { label: "del", danger: true, onClick: () => void deleteSubagent(subagent.name) },
    ],
  }));
  $: mcpServers = parseMcpConfig(modsDraft.mcp_config);
  $: mcpItems = mcpServers.map((server): Item => ({
    key: server.name,
    title: `${server.name}${modsDraft.mcp_enabled ? "" : " (off)"}`,
    subtitle: `${server.command} ${server.args.join(" ")}`.trim() || "MCP server",
    active: false,
    onSelect: () => editMcpServer(server),
    actions: [
      { label: "edit", onClick: () => editMcpServer(server) },
      { label: "del", danger: true, onClick: () => deleteMcpServer(server.name) },
    ],
  }));

  $: extensionItems = extensionsInfo.extensions.map((extension): Item => ({
    key: extension.id,
    title: `${extension.name}${extension.enabled ? "" : " (off)"}`,
    subtitle: extension.description || extension.path || "extension",
    active: false,
    onSelect: () => editExtension(extension),
    actions: [{ label: "edit", onClick: () => editExtension(extension) }],
  }));

  $: providerItems = config.providers.map((provider): Item => ({
    key: provider.name,
    title: provider.name,
    subtitle: `${provider.kind} · ${provider.api_base}${provider.has_api_key ? " · key saved" : ""}`,
    active: false,
    onSelect: () => editProvider(provider),
    actions: [
      { label: "edit", onClick: () => editProvider(provider) },
      { label: "del", danger: true, onClick: () => void deleteProvider(provider) },
    ],
  }));
  $: modelItems = config.models.map((model): Item => ({
    key: model.name,
    title: model.name,
    subtitle: `${model.provider} · ${model.id} · ctx ${formatContext(model.context_chars)}`,
    active: false,
    onSelect: () => void selectModel(model),
    actions: [
      { label: "edit", onClick: () => editModel(model) },
      { label: "del", danger: true, onClick: () => void deleteModel(model) },
    ],
  }));
  $: workspaceItems = workspaces.map((item): Item => ({
    key: item.path,
    title: item.name,
    subtitle: item.path,
    active: item.path === workspace,
    onSelect: () => void selectWorkspace(item.path),
    actions: item.deletable ? [{ label: "del", danger: true, onClick: () => void deleteWorkspace(item.path) }] : [],
  }));
  $: visibleSessions = sessions.filter((session) => {
    const query = sessionQuery.trim().toLowerCase();
    return !query || session.title.toLowerCase().includes(query) || session.preview.toLowerCase().includes(query);
  });
  $: searchingFiles = Boolean(fileQuery.trim());
  $: visibleFiles = searchingFiles ? fileSearchResults : filterFiles(files, expandedDirs, "");
  $: expandedFilePaths = searchingFiles ? visibleFiles.filter((file) => file.kind === "dir").map((file) => norm(file.path)) : [...expandedDirs];
  $: messageGroups = groupMessages(messages);
  $: hiddenMessageGroupCount = Math.max(0, messageGroups.length - messageGroupLimit);
  $: visibleMessageGroups = hiddenMessageGroupCount > 0 ? messageGroups.slice(hiddenMessageGroupCount) : messageGroups;
  $: activeSessionRunning = compacting || (sessions.find((session) => session.id === activeSessionId)?.running ?? false);
  $: featureContentSearch = config.features?.content_search ?? true;
  $: featureGit = config.features?.git ?? true;
  $: featureFileWatcher = config.features?.file_watcher ?? true;
  $: contextLimit = config.context_chars || 80000;
  $: estimatedContextTokens = estimateActiveContext(messages, prompt, contextLimit);
  $: contextUsed = providerTotalTokens || estimatedContextTokens;
  $: transcriptUsed = messages.reduce((total, message) => total + message.content.length, 0) + prompt.length;
  $: contextPercent = Math.min(100, Math.round((contextUsed / Math.max(contextLimit, 1)) * 100));
  $: inputTokens = providerInputTokens || estimateInputContext(messages, prompt, contextLimit);
  $: outputTokens = providerOutputTokens || estimateTokenCount(messages.filter((message) => message.role === "assistant").map((message) => message.content).join("\n"));
  $: if (sideTab === "content" && !featureContentSearch) sideTab = "files";
  $: if (sideTab === "git" && !featureGit) sideTab = "files";
  $: sessionItems = visibleSessions.map((session): Item => ({
    key: session.id,
    title: session.running ? `${session.title} *` : session.title,
    subtitle: `${session.message_count} msg · ${session.preview}`,
    active: session.id === activeSessionId,
    onSelect: () => void selectSession(session.id),
    actions: [
      { label: "rename", onClick: () => startRenameSession(session) },
      { label: "del", danger: true, onClick: () => void deleteSession(session.id) },
    ],
  }));

  function setConfigDraft(value: AiConfig) {
    providerChoice = value.provider;
    draft = { provider: value.provider, model: value.model, original_model: value.model, context_chars: value.context_chars || 80000 };
    modsDraft = { ...value.mods };
    modsProfile = value.active_profile || "default";
    applyUiScale(value.ui_scale || 1);
  }

  function applyUiScale(scale: number) {
    uiScale = clampScale(scale);
    document.body.style.zoom = String(uiScale);
  }

  function clampScale(scale: number) {
    return Math.round(Math.min(1.6, Math.max(0.7, scale)) * 100) / 100;
  }

  async function setUiScale(scale: number) {
    const next = clampScale(scale);
    applyUiScale(next);
    try {
      config = await invoke<AiConfig>("ai_set_ui_scale", { update: { scale: next } });
      applyUiScale(config.ui_scale || next);
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  async function ensureEditorPane() {
    EditorPaneComponent ??= (await import("./panels/EditorPane.svelte")).default;
  }

  async function ensureDiffPane() {
    DiffPaneComponent ??= (await import("./panels/DiffPane.svelte")).default;
  }

  async function ensureTerminalPane() {
    TerminalPaneComponent ??= (await import("./panels/TerminalPane.svelte")).default;
  }

  function estimateTokenCount(text: string) {
    const words = text.trim().match(/[\p{L}\p{N}_]+|[^\s]/gu)?.length ?? 0;
    return Math.max(Math.ceil(text.length / 4), words);
  }

  function estimateMessageCost(message: Message, limit: number) {
    const cap = message.role === "tool" ? Math.min(1200, Math.max(600, Math.floor(limit / 12))) : Math.min(24000, Math.max(2000, Math.floor(limit / 2)));
    return estimateTokenCount(message.role) + estimateTokenCount(message.content.slice(0, cap)) + 4;
  }

  function activeContextMessages(source: Message[], draftPrompt: string, limit: number) {
    const draftMessages = draftPrompt ? [...source, { role: "user" as Role, content: draftPrompt }] : source;
    const selected: Message[] = [];
    let used = 0;
    for (let index = draftMessages.length - 1; index >= 0; index -= 1) {
      const message = draftMessages[index];
      const mustKeep = draftMessages.length - index <= 4;
      const cost = estimateMessageCost(message, limit);
      if (!mustKeep && selected.length > 0 && used + cost > limit) break;
      used += cost;
      selected.push(message);
    }
    return selected.reverse();
  }

  function estimateActiveContext(source: Message[], draftPrompt: string, limit: number) {
    return activeContextMessages(source, draftPrompt, limit).reduce((total, message) => total + estimateMessageCost(message, limit), 0);
  }

  function estimateInputContext(source: Message[], draftPrompt: string, limit: number) {
    return activeContextMessages(source, draftPrompt, limit)
      .filter((message) => message.role !== "assistant")
      .reduce((total, message) => total + estimateMessageCost(message, limit), 0);
  }

  function formatContext(value: number) {
    return value >= 1000 ? `${Math.round(value / 1000)}k` : String(value);
  }

  function addMessage(role: Role, content: string) {
    messages = [...messages, { role, content }];
  }

  function ensureStreamMessage() {
    if (streamMessageOpen && messages.at(-1)?.role === "assistant") return;
    streamMessageOpen = true;
    messages = [...messages, { role: "assistant", content: "" }];
  }

  function scheduleStreamFlush() {
    if (streamFrame) return;
    streamFrame = requestAnimationFrame(() => {
      streamFrame = 0;
      flushStreamBuffer();
    });
  }

  function flushStreamBuffer() {
    if (!streamBuffer) return;
    const text = streamBuffer;
    streamBuffer = "";
    const next = [...messages];
    const last = next.at(-1);
    if (streamMessageOpen && last?.role === "assistant") {
      next[next.length - 1] = { ...last, content: last.content + text };
    } else {
      streamMessageOpen = true;
      next.push({ role: "assistant", content: text });
    }
    messages = next;
  }

  function flushStreamNow() {
    if (streamFrame) {
      cancelAnimationFrame(streamFrame);
      streamFrame = 0;
    }
    flushStreamBuffer();
  }

  function toolTitle(content: string) {
    return content.split("\n", 1)[0] || "tool";
  }

  function lastToolStatus(content: string) {
    const statuses = [...content.matchAll(/^status:\s*([^\n]+)/gm)];
    return statuses.at(-1)?.[1]?.trim().toLowerCase() ?? "";
  }

  function isRunningTool(content: string) {
    return lastToolStatus(content).startsWith("running");
  }

  function upsertToolMessage(content: string) {
    streamMessageOpen = false;
    const title = toolTitle(content);
    const next = [...messages];
    const matchIndex = next.findLastIndex(
      (message) => message.role === "tool" && toolTitle(message.content) === title && isRunningTool(message.content),
    );
    if (matchIndex !== -1) {
      next[matchIndex] = { role: "tool", content };
      messages = next;
      return;
    }
    if (next.at(-1)?.role === "tool" && next.at(-1)?.content === content) return;
    addMessage("tool", content);
  }

  function handleFileChanged(event: FileChangedEvent) {
    if (event.workspace !== workspace || !featureFileWatcher) return;
    if (fileChangeTimer) window.clearTimeout(fileChangeTimer);
    fileChangeTimer = window.setTimeout(() => {
      fileChangeTimer = 0;
      void handleToolFileMutation();
    }, 300);
  }

  function saveLiveSession(sessionId: string) {
    if (!sessionId) return;
    liveSessions = { ...liveSessions, [sessionId]: { messages, streamBuffer, streamMessageOpen } };
  }

  function loadLiveSession(sessionId: string) {
    const live = liveSessions[sessionId];
    if (!live) return false;
    messages = live.messages;
    streamBuffer = live.streamBuffer;
    streamMessageOpen = live.streamMessageOpen;
    return true;
  }

  function streamEventKey(event: ChatStreamEvent) {
    return event.id || `${event.session_id}:${event.kind}:${event.text ?? ""}:${event.content ?? ""}:${event.total_tokens ?? ""}`;
  }

  function rememberStreamEvent(event: ChatStreamEvent) {
    const key = streamEventKey(event);
    if (seenStreamEvents.has(key)) return false;
    seenStreamEvents.add(key);
    if (seenStreamEvents.size > 2000) seenStreamEvents = new Set([...seenStreamEvents].slice(-1000));
    return true;
  }

  function applyStreamEvent(event: ChatStreamEvent, visible: boolean) {
    if (event.kind === "usage") {
      providerInputTokens = event.input_tokens ?? providerInputTokens;
      providerOutputTokens = event.output_tokens ?? providerOutputTokens;
      providerTotalTokens = event.total_tokens ?? providerTotalTokens;
      return;
    }
    if (event.kind === "start") {
      streamBuffer = "";
      ensureStreamMessage();
    } else if (event.kind === "delta") {
      ensureStreamMessage();
      streamBuffer += event.text ?? "";
      visible ? scheduleStreamFlush() : flushStreamBuffer();
    } else if (event.kind === "tool") {
      flushStreamNow();
      upsertToolMessage(event.content ?? "");
    } else if (event.kind === "error") {
      flushStreamNow();
      streamMessageOpen = false;
      const content = event.debug ? `${event.content ?? "stream error"}\n\n\`\`\`text\n${event.debug}\n\`\`\`` : event.content ?? "stream error";
      addMessage("error", content);
    } else if (event.kind === "done") {
      flushStreamNow();
      streamMessageOpen = false;
      const { [event.session_id]: _done, ...rest } = liveSessions;
      liveSessions = rest;
      if (visible) void loadSession();
    }
  }

  function handleStream(event: ChatStreamEvent) {
    if (!rememberStreamEvent(event)) return;
    const visible = event.session_id === activeSessionId;
    if (visible) {
      applyStreamEvent(event, true);
      if (event.kind !== "done") saveLiveSession(event.session_id);
      return;
    }

    const current = { messages, streamBuffer, streamMessageOpen };
    loadLiveSession(event.session_id);
    applyStreamEvent(event, false);
    if (event.kind !== "done") saveLiveSession(event.session_id);
    messages = current.messages;
    streamBuffer = current.streamBuffer;
    streamMessageOpen = current.streamMessageOpen;
  }

  function textSnapshot(target: HTMLTextAreaElement): TextSnapshot {
    return {
      value: target.value,
      selectionStart: target.selectionStart,
      selectionEnd: target.selectionEnd,
    };
  }

  function sameTextSnapshot(left: TextSnapshot | undefined, right: TextSnapshot) {
    return Boolean(left && left.value === right.value && left.selectionStart === right.selectionStart && left.selectionEnd === right.selectionEnd);
  }

  function pushTextSnapshot(stack: TextSnapshot[], snapshot: TextSnapshot) {
    if (sameTextSnapshot(stack.at(-1), snapshot)) return;
    stack.push(snapshot);
    if (stack.length > 100) stack.shift();
  }

  function restorePromptSnapshot(target: HTMLTextAreaElement, snapshot: TextSnapshot) {
    prompt = snapshot.value;
    target.value = snapshot.value;
    requestAnimationFrame(() => target.setSelectionRange(snapshot.selectionStart, snapshot.selectionEnd));
  }

  function rememberPromptSnapshot(event: Event) {
    pushTextSnapshot(promptUndoStack, textSnapshot(event.currentTarget as HTMLTextAreaElement));
    promptRedoStack = [];
  }

  function inputPrompt(event: Event) {
    prompt = (event.currentTarget as HTMLTextAreaElement).value;
    updateMention(event.currentTarget as HTMLTextAreaElement);
  }

  function mentionToken(target: HTMLTextAreaElement) {
    const before = prompt.slice(0, target.selectionStart);
    const match = /(^|\s)@([^\s@]*)$/.exec(before);
    if (!match) return null;
    return { start: before.length - match[2].length - 1, query: match[2] };
  }

  function updateMention(target = promptEl) {
    if (!target) return closeMention();
    const token = mentionToken(target);
    if (!token) return closeMention();
    mentionStart = token.start;
    mentionQuery = token.query;
    mentionIndex = 0;
    if (mentionTimer) window.clearTimeout(mentionTimer);
    mentionTimer = window.setTimeout(() => {
      mentionTimer = 0;
      void runMentionSearch(token.query);
    }, 120);
  }

  function closeMention() {
    mentionQuery = "";
    mentionStart = -1;
    mentionResults = [];
    mentionIndex = 0;
    if (mentionTimer) {
      window.clearTimeout(mentionTimer);
      mentionTimer = 0;
    }
  }

  async function runMentionSearch(query: string) {
    try {
      const results = await invoke<FileEntry[]>("workspace_search", { request: { query } });
      if (query !== mentionQuery) return;
      mentionResults = results.filter((entry) => entry.kind === "file").slice(0, 8);
    } catch {
      mentionResults = [];
    }
  }

  function insertMention(entry: FileEntry) {
    if (!promptEl || mentionStart < 0) return;
    const start = mentionStart;
    const end = promptEl.selectionStart;
    const nextPrompt = `${prompt.slice(0, start)}@${entry.path} ${prompt.slice(end)}`;
    const pos = start + entry.path.length + 2;
    prompt = nextPrompt;
    closeMention();
    requestAnimationFrame(() => {
      promptEl.focus();
      promptEl.setSelectionRange(pos, pos);
    });
  }

  function clearPromptHistory() {
    promptUndoStack = [];
    promptRedoStack = [];
  }

  function handlePromptUndoRedo(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return false;

    const key = event.key.toLowerCase();
    const undo = key === "z" && !event.shiftKey;
    const redo = key === "y" || (key === "z" && event.shiftKey);
    if (!undo && !redo) return false;

    const target = event.currentTarget as HTMLTextAreaElement;
    const from = undo ? promptUndoStack : promptRedoStack;
    const to = undo ? promptRedoStack : promptUndoStack;
    const snapshot = from.pop();
    if (!snapshot) return true;

    event.preventDefault();
    pushTextSnapshot(to, textSnapshot(target));
    restorePromptSnapshot(target, snapshot);
    return true;
  }

  async function scrollChatToBottom() {
    await tick();
    if (messagesEl) messagesEl.scrollTop = messagesEl.scrollHeight;
  }

  function showEarlierMessages() {
    messageGroupLimit += 80;
  }

  function isStreamingMessage(group: MessageGroup) {
    return streamMessageOpen && activeSessionRunning && group.kind === "message" && group.key === `message-${messages.length - 1}` && group.message.role === "assistant";
  }

  function groupMessages(items: Message[]): MessageGroup[] {
    const groups: MessageGroup[] = [];
    let tools: Message[] = [];
    let toolStart = 0;

    items.forEach((message, index) => {
      if (message.role === "tool") {
        if (!tools.length) toolStart = index;
        tools.push(message);
        return;
      }

      if (tools.length) {
        groups.push({ key: `tools-${toolStart}`, kind: "tools", tools });
        tools = [];
      }
      groups.push({ key: `message-${index}`, kind: "message", message });
    });

    if (tools.length) groups.push({ key: `tools-${toolStart}`, kind: "tools", tools });
    return groups;
  }

  async function loadExtensionsInfo() {
    try {
      extensionsInfo = await api.extensionsInfo();
    } catch (error) {
      addMessage("error", String(error));
      extensionsInfo = { config_path: "", extensions: [] };
    }
  }

  async function setExtensionEnabled(id: string, enabled: boolean) {
    try {
      extensionsInfo = await invoke<ExtensionsInfo>("extensions_set_enabled", { request: { id, enabled } });
      extensionDraft = { ...extensionDraft, enabled };
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  function addExtension() {
    editingExtension = true;
    creatingExtension = true;
    extensionDraft = { id: "", name: "New Rust extension", enabled: false, removable: true, description: "Native Rust extension scaffold", hooks: [], tools: [] };
  }

  function editExtension(extension: ExtensionInfo) {
    editingExtension = true;
    creatingExtension = false;
    extensionDraft = { ...extension, hooks: [...extension.hooks], tools: [...extension.tools] };
  }

  async function createRustExtension() {
    try {
      extensionsInfo = await invoke<ExtensionsInfo>("extensions_create_rust", { request: { id: extensionDraft.id, name: extensionDraft.name, description: extensionDraft.description } });
      const created = extensionsInfo.extensions.find((extension) => extension.id === extensionDraft.id);
      if (created) editExtension(created);
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  function extensionManifestHint(id: string) {
    return `${extensionsInfo.config_path.replace(/extensions\.toml$/, "extensions") || "~/.sandevistan/extensions"}/${id || "my-extension"}/extension.toml`;
  }

  function openConfig() {
    addingModel = false;
    addingProvider = false;
    setConfigDraft(config);
    modsTab = "models";
    showMods = true;
  }

  async function selectModel(model: ModelOption) {
    draft = {
      provider: model.provider,
      model: model.name,
      original_model: model.name,
      context_chars: model.context_chars || 80000,
    };
    const nextMods = normalizeMods({ ...config.mods, main_model: model.name });
    config = await invoke<AiConfig>("ai_set_mods", { update: { profile: config.active_profile || modsProfile, ...nextMods } });
    setConfigDraft(config);
    modelLabel = `${config.active_profile} · ${config.model_id}`;
    showConfig = false;
    addingModel = false;
  }

  function startAddModel() {
    addingModel = true;
    providerChoice = draft.provider;
    draft = { ...draft, original_model: "", context_chars: config.context_chars || 80000 };
  }

  function editModel(model: ModelOption) {
    addingModel = true;
    providerChoice = model.provider;
    draft = {
      provider: model.provider,
      model: model.name,
      original_model: model.name,
      context_chars: model.context_chars || 80000,
    };
  }

  function chooseProvider(value: string) {
    providerChoice = value;
    draft = { ...draft, provider: value };
  }

  function addProvider() {
    addingProvider = true;
    providerDraft = { name: "", original_name: "", kind: "openai-compatible", api_base: "https://api.openai.com/v1", api_key_header: "authorization", api_key: "" };
  }

  function editProvider(provider: ProviderOption) {
    addingProvider = true;
    providerDraft = { name: provider.name, original_name: provider.name, kind: provider.kind, api_base: provider.api_base, api_key_header: provider.api_key_header || "authorization", api_key: "" };
  }

  function openMods() {
    modsProfile = config.active_profile || "default";
    modsDraft = normalizeMods(config.mods);
    modsTab = "general";
    addingAgent = false;
    addingSubagent = false;
    editingExtension = false;
    editingMcp = false;
    showMods = true;
    void loadExtensionsInfo();
  }

  function normalizeMods(value: AiMods): AiMods {
    return {
      ...defaultMods(),
      ...value,
      main_model: value.main_model || config.model || "gpt-4o-mini",
      main_agent: value.main_agent || "custom",
      subagents: Array.isArray(value.subagents) ? value.subagents : ["scout", "reviewer", "planner"],
      shell_enabled: value.shell_enabled ?? false,
      git_panel_enabled: value.git_panel_enabled ?? true,
      subagents_enabled: value.subagents_enabled ?? true,
      subagent_max_concurrency: Math.min(5, Math.max(1, Number(value.subagent_max_concurrency) || 3)),
      mcp_enabled: value.mcp_enabled ?? false,
      mcp_config: value.mcp_config || "",
    };
  }

  function defaultMods(): AiMods {
    return { main_model: config.model || "gpt-4o-mini", main_agent: "custom", subagents: ["scout", "reviewer", "planner"], persona: "", thinking_level: "auto", prompt_injection: "", rtk_enabled: config.rtk_available, shell_enabled: false, git_panel_enabled: true, subagents_enabled: true, subagent_model: "", subagent_max_concurrency: 3, subagents_config: "", mcp_enabled: false, mcp_config: "" };
  }

  function toggleSubagent(name: string) {
    const selected = new Set(modsDraft.subagents);
    if (selected.has(name)) selected.delete(name);
    else selected.add(name);
    modsDraft = { ...modsDraft, subagents: [...selected] };
  }

  function chooseProfile(value: string) {
    if (value === "__new__") {
      modsProfile = uniqueProfileName();
      modsDraft = defaultMods();
      return;
    }
    modsProfile = value;
    const profile = config.profiles.find((item) => item.name === value);
    if (profile) modsDraft = normalizeMods(profile);
  }

  function uniqueProfileName() {
    const taken = new Set(config.profiles.map((profile) => profile.name));
    let index = taken.size + 1;
    while (taken.has(`profile-${index}`)) index += 1;
    return `profile-${index}`;
  }

  function parseMcpConfig(configText: string): McpServer[] {
    const servers: McpServer[] = [];
    let current: McpServer | null = null;
    for (const rawLine of (configText || "").split("\n")) {
      const line = rawLine.trim();
      if (!line || line.startsWith("#")) continue;
      if (line === "[[servers]]") {
        current = { name: "", command: "", args: [], timeout_ms: 8000, env: {} };
        servers.push(current);
        continue;
      }
      if (!current) continue;
      const match = line.match(/^(\w+)\s*=\s*(.*)$/);
      if (!match) continue;
      const [, key, rawValue] = match;
      if (key === "name") current.name = unquoteToml(rawValue);
      else if (key === "command") current.command = unquoteToml(rawValue);
      else if (key === "timeout_ms") current.timeout_ms = Math.max(1, Number(rawValue) || 8000);
      else if (key === "args") current.args = parseTomlStringArray(rawValue);
      else if (key === "env") current.env = parseTomlInlineTable(rawValue);
    }
    return servers.filter((server) => server.name.trim());
  }

  function unquoteToml(value: string) {
    const trimmed = value.trim();
    if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
      try { return JSON.parse(trimmed); } catch { return trimmed.slice(1, -1); }
    }
    return trimmed;
  }

  function parseTomlStringArray(value: string) {
    try {
      const parsed = JSON.parse(value.trim());
      return Array.isArray(parsed) ? parsed.map(String) : [];
    } catch {
      return [];
    }
  }

  function parseTomlInlineTable(value: string) {
    const env: Record<string, string> = {};
    const body = value.trim().replace(/^\{/, "").replace(/\}$/, "");
    for (const part of body.split(",")) {
      const [key, ...rest] = part.split("=");
      if (!key || !rest.length) continue;
      env[key.trim()] = unquoteToml(rest.join("=").trim());
    }
    return env;
  }

  function tomlString(value: string) {
    return JSON.stringify(value || "");
  }

  function mcpConfigFromServers(servers: McpServer[]) {
    return servers.map((server) => {
      const lines = ["[[servers]]", `name = ${tomlString(server.name)}`, `command = ${tomlString(server.command)}`];
      if (server.args.length) lines.push(`args = [${server.args.map(tomlString).join(", ")}]`);
      lines.push(`timeout_ms = ${Math.max(1, Number(server.timeout_ms) || 8000)}`);
      const envEntries = Object.entries(server.env).filter(([key]) => key.trim());
      if (envEntries.length) lines.push(`env = { ${envEntries.map(([key, value]) => `${key.trim()} = ${tomlString(value)}`).join(", ")} }`);
      return lines.join("\n");
    }).join("\n\n");
  }

  function envText(env: Record<string, string>) {
    return Object.entries(env).map(([key, value]) => `${key}=${value}`).join("\n");
  }

  function parseEnvText(value: string) {
    const env: Record<string, string> = {};
    for (const line of value.split("\n")) {
      const [key, ...rest] = line.split("=");
      if (!key.trim() || !rest.length) continue;
      env[key.trim()] = rest.join("=").trim();
    }
    return env;
  }

  function addMcpServer() {
    editingMcp = true;
    mcpDraft = { name: "", original_name: "", command: "", args: "", timeout_ms: 8000, env: "" };
  }

  function editMcpServer(server: McpServer) {
    editingMcp = true;
    mcpDraft = { name: server.name, original_name: server.name, command: server.command, args: server.args.join("\n"), timeout_ms: server.timeout_ms, env: envText(server.env) };
  }

  function saveMcpServer() {
    const server: McpServer = {
      name: mcpDraft.name.trim(),
      command: mcpDraft.command.trim(),
      args: mcpDraft.args.split("\n").map((arg) => arg.trim()).filter(Boolean),
      timeout_ms: Math.max(1, Number(mcpDraft.timeout_ms) || 8000),
      env: parseEnvText(mcpDraft.env),
    };
    if (!server.name || !server.command) return;
    const servers = mcpServers.filter((item) => item.name !== (mcpDraft.original_name || server.name));
    modsDraft = { ...modsDraft, mcp_config: mcpConfigFromServers([...servers, server]) };
    editingMcp = false;
  }

  function deleteMcpServer(name: string) {
    modsDraft = { ...modsDraft, mcp_config: mcpConfigFromServers(mcpServers.filter((server) => server.name !== name)) };
    editingMcp = false;
  }

  function editAgent(agent: AgentOption) {
    addingAgent = true;
    agentDraft = { name: agent.name, original_name: agent.name, description: agent.description, persona: agent.persona, thinking_level: agent.thinking_level, prompt_injection: agent.prompt_injection };
  }

  function addAgent() {
    addingAgent = true;
    agentDraft = { name: "", original_name: "", description: "", persona: "", thinking_level: "auto", prompt_injection: "" };
  }

  async function saveAgent() {
    config = await invoke<AiConfig>("ai_save_agent", { update: agentDraft });
    setConfigDraft(config);
    agentDraft = { name: "", original_name: "", description: "", persona: "", thinking_level: "auto", prompt_injection: "" };
    addingAgent = false;
  }

  async function deleteAgent(name: string) {
    config = await invoke<AiConfig>("ai_delete_agent", { request: { name } });
    setConfigDraft(config);
  }

  function editSubagent(subagent: SubagentOption) {
    addingSubagent = true;
    subagentDraft = { name: subagent.name, original_name: subagent.name, description: subagent.description, system: subagent.system, model: subagent.model, max_result_chars: subagent.max_result_chars };
  }

  function addSubagent() {
    addingSubagent = true;
    subagentDraft = { name: "", original_name: "", description: "", system: "", model: "", max_result_chars: 4000 };
  }

  async function saveSubagent() {
    config = await invoke<AiConfig>("ai_save_subagent", { update: subagentDraft });
    setConfigDraft(config);
    subagentDraft = { name: "", original_name: "", description: "", system: "", model: "", max_result_chars: 4000 };
    addingSubagent = false;
  }

  async function deleteSubagent(name: string) {
    config = await invoke<AiConfig>("ai_delete_subagent", { request: { name } });
    setConfigDraft(config);
  }

  async function switchProfile(profile: string) {
    if (!profile || profile === config.active_profile) return;
    try {
      config = await invoke<AiConfig>("ai_set_active_profile", { update: { profile } });
      setConfigDraft(config);
      modelLabel = `${config.active_profile} · ${config.model_id}`;
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  async function saveMods() {
    busy = true;
    try {
      config = await invoke<AiConfig>("ai_set_mods", { update: { profile: modsProfile, ...normalizeMods(modsDraft) } });
      setConfigDraft(config);
      modelLabel = `${config.active_profile} · ${config.model_id}`;
      showMods = false;
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      busy = false;
    }
  }

  async function loadConfig() {
    config = await api.aiConfig();
    setConfigDraft(config);
    modelLabel = `${config.active_profile} · ${config.model_id}`;
    await syncFileWatcher();
  }

  async function setFeature(name: string, enabled: boolean) {
    config = await invoke<AiConfig>("ai_set_feature", { update: { name, enabled } });
    await syncFileWatcher();
  }

  async function syncFileWatcher() {
    try {
      if (config.features?.file_watcher ?? false) await api.fileWatchStart();
      else await api.fileWatchStop();
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  function mutationCount(items: Message[]) {
    return items.filter((message) => message.role === "tool" && /(^|\n)(edited|wrote)\s/.test(message.content)).length;
  }

  function resetStreamState() {
    if (streamFrame) {
      cancelAnimationFrame(streamFrame);
      streamFrame = 0;
    }
    streamBuffer = "";
    streamMessageOpen = false;
  }

  function setSession(session: SessionInfo) {
    const previousSessionId = activeSessionId;
    const activeSession = session.sessions.find((item) => item.id === session.active_session_id);
    if (previousSessionId && previousSessionId !== session.active_session_id) saveLiveSession(previousSessionId);
    resetStreamState();
    workspace = session.workspace;
    workspaceDraft = session.workspace;
    workspaces = session.workspaces;
    activeSessionId = session.active_session_id;
    sessions = session.sessions;
    sessionLabel = activeSession?.title ?? "session";
    messages = session.messages;
    if (activeSession?.running) loadLiveSession(session.active_session_id);
    else {
      const { [session.active_session_id]: _stale, ...rest } = liveSessions;
      liveSessions = rest;
    }
    messageGroupLimit = 80;
  }

  function markActiveSessionRunning(running: boolean) {
    sessions = sessions.map((session) => session.id === activeSessionId ? { ...session, running } : session);
  }

  async function loadSession() {
    const previousMutations = toolMutationCount;
    const session = await api.chatSession();
    const nextMutations = mutationCount(session.messages);
    setSession(session);
    toolMutationCount = nextMutations;
    if (nextMutations > previousMutations) await handleToolFileMutation();
  }

  async function loadFiles() {
    files = await api.workspaceTree();
    fileSearchResults = [];
    fileSearchTruncated = false;
    expandedDirs = new Set();
    loadedDirs = new Set();
    fileTreeVersion += 1;
  }

  function inputFileQuery(event: Event) {
    fileQuery = (event.currentTarget as HTMLInputElement).value;
    if (fileSearchTimer) window.clearTimeout(fileSearchTimer);
    const query = fileQuery.trim();
    if (!query) {
      fileSearchResults = [];
      fileSearchTruncated = false;
      fileSearching = false;
      return;
    }
    fileSearching = true;
    fileSearchTimer = window.setTimeout(() => {
      fileSearchTimer = 0;
      void runFileSearch(query);
    }, 150);
  }

  async function runFileSearch(query: string) {
    try {
      const results = await invoke<FileEntry[]>("workspace_search", { request: { query } });
      if (fileQuery.trim() !== query) return;
      fileSearchTruncated = results.length > 500;
      fileSearchResults = results.slice(0, 500);
    } catch (error) {
      if (fileQuery.trim() === query) {
        fileSearchResults = [];
        addMessage("error", String(error));
      }
    } finally {
      if (fileQuery.trim() === query) fileSearching = false;
    }
  }

  async function runContentSearch() {
    const query = contentQuery.trim();
    if (!query) {
      contentResults = [];
      return;
    }
    contentSearching = true;
    try {
      contentResults = await invoke<SearchHit[]>("content_search", { request: { query, max_results: 50 } });
    } catch (error) {
      addMessage("error", String(error));
      contentResults = [];
    } finally {
      contentSearching = false;
    }
  }

  function contentSearchKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter") return;
    event.preventDefault();
    void runContentSearch();
  }

  async function openSearchHit(hit: SearchHit) {
    await openFile({ name: fileName(hit.path), path: hit.path, kind: "file", depth: 0 });
  }

  async function refreshGit() {
    gitLoading = true;
    try {
      gitStatus = await api.gitStatus();
      gitDiff = "";
      gitDiffPath = "";
    } catch (error) {
      gitStatus = null;
      gitDiff = String(error);
    } finally {
      gitLoading = false;
    }
  }

  async function loadGitDiff(path = "") {
    gitLoading = true;
    gitDiffPath = path;
    try {
      gitDiff = await invoke<string>("git_diff", { request: { path: path || null } });
      if (!gitDiff.trim()) gitDiff = "no diff";
    } catch (error) {
      gitDiff = String(error);
    } finally {
      gitLoading = false;
    }
  }

  async function openGitDiff(path = "") {
    gitLoading = true;
    try {
      await ensureDiffPane();
      let diff = await invoke<string>("git_diff", { request: { path: path || null } });
      if (!diff.trim()) diff = "no diff";
      const id = `diff:${path || "workspace"}`;
      const title = path ? `diff ${fileName(path)}` : "diff workspace";
      const tab = { id, title, path, diff };
      diffTabs = [...diffTabs.filter((item) => item.id !== id), tab];
      activeTab = id;
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      gitLoading = false;
    }
  }

  function norm(path: string) {
    return path.replace(/\\/g, "/");
  }

  function parentPath(path: string) {
    const parts = norm(path).split("/");
    parts.pop();
    return parts.join("/");
  }

  function isDescendantPath(path: string, dir: string) {
    return norm(path).startsWith(`${norm(dir)}/`);
  }

  function filterFiles(items: FileEntry[], expanded: Set<string>, queryText: string) {
    const query = queryText.trim().toLowerCase();
    return items.filter((file) => {
      let parent = parentPath(file.path);
      while (parent) {
        if (!expanded.has(parent)) return false;
        parent = parentPath(parent);
      }

      return !query || file.name.toLowerCase().includes(query) || file.path.toLowerCase().includes(query);
    });
  }

  function isChildOf(file: FileEntry, dir: FileEntry) {
    return file.depth === dir.depth + 1 && isDescendantPath(file.path, dir.path);
  }

  function appendChildren(dir: FileEntry, children: FileEntry[]) {
    const index = files.findIndex((file) => file.path === dir.path);
    if (index < 0) return;
    const withoutStale = files.filter((file) => !isChildOf(file, dir));
    const nextIndex = withoutStale.findIndex((file) => file.path === dir.path) + 1;
    files = [...withoutStale.slice(0, nextIndex), ...children, ...withoutStale.slice(nextIndex)];
  }

  async function openFile(entry: FileEntry) {
    if (entry.kind === "dir") {
      if (searchingFiles) return;
      const path = norm(entry.path);
      const nextExpanded = new Set(expandedDirs);
      if (nextExpanded.has(path)) {
        expandedDirs = new Set([...nextExpanded].filter((dir) => dir !== path && !isDescendantPath(dir, path)));
        return;
      }

      nextExpanded.add(path);
      expandedDirs = nextExpanded;
      if (loadedDirs.has(path)) return;

      try {
        appendChildren(entry, await invoke<FileEntry[]>("workspace_children", { request: { path: entry.path } }));
        loadedDirs = new Set(loadedDirs).add(path);
      } catch (error) {
        const rollback = new Set(expandedDirs);
        rollback.delete(path);
        expandedDirs = rollback;
        addMessage("error", String(error));
      }
      return;
    }

    const existing = openFiles.find((file) => file.path === entry.path);
    if (existing) {
      activeTab = entry.path;
      return;
    }

    try {
      await ensureEditorPane();
      const file = await invoke<{ path: string; content: string }>("file_read", { request: { path: entry.path } });
      openFiles = [...openFiles, { path: file.path, content: file.content, original: file.content, dirty: false, stale: false, mode: "edit" }];
      activeTab = file.path;
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  async function handleToolFileMutation() {
    await loadFiles();
    const reconciled = await Promise.all(openFiles.map(async (file) => {
      try {
        const disk = await invoke<{ path: string; content: string }>("file_read", { request: { path: file.path } });
        if (disk.content === file.original) return file;
        if (file.dirty) return { ...file, stale: true };
        return { ...file, content: disk.content, original: disk.content, dirty: false, stale: false };
      } catch {
        return { ...file, stale: true };
      }
    }));
    openFiles = reconciled;
  }

  function updateOpenFile(path: string, content: string) {
    openFiles = openFiles.map((file) => file.path === path ? { ...file, content, dirty: content !== file.original } : file);
  }

  function setOpenFileDirty(path: string, dirty: boolean) {
    openFiles = openFiles.map((file) => file.path === path ? { ...file, dirty } : file);
  }

  function setFileMode(path: string, mode: "edit" | "diff") {
    openFiles = openFiles.map((file) => file.path === path ? { ...file, mode } : file);
  }

  async function saveOpenFile(path: string, content?: string) {
    const file = openFiles.find((item) => item.path === path);
    if (!file) return;
    const nextContent = content ?? file.content;
    try {
      const saved = await invoke<{ path: string; content: string }>("file_save", { request: { path, content: nextContent } });
      openFiles = openFiles.map((item) => item.path === path ? { ...item, content: saved.content, original: saved.content, dirty: false, stale: false } : item);
      await loadFiles();
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  function fileName(path: string) {
    return path.split(/[\\/]/).filter(Boolean).pop() || path;
  }

  function workspaceName(path: string) {
    return fileName(path) || path || "choose workspace";
  }

  function fallbackTab() {
    return chatOpen ? "chat" : terminalOpen ? "terminal" : openFiles[0]?.path ?? diffTabs[0]?.id ?? "empty";
  }

  function openChatTab() {
    chatOpen = true;
    activeTab = "chat";
  }

  function closeChatTab() {
    chatOpen = false;
    if (activeTab === "chat") activeTab = fallbackTab();
  }

  async function openTerminal() {
    await ensureTerminalPane();
    terminalOpen = true;
    activeTab = "terminal";
  }

  function closeTerminal() {
    terminalOpen = false;
    if (activeTab === "terminal") activeTab = fallbackTab();
  }

  function closeTab(path: string) {
    openFiles = openFiles.filter((file) => file.path !== path);
    if (activeTab === path) activeTab = fallbackTab();
  }

  function closeDiffTab(id: string) {
    diffTabs = diffTabs.filter((tab) => tab.id !== id);
    if (activeTab === id) activeTab = fallbackTab();
  }


  async function chooseWorkspaceFolder() {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select workspace" });
      if (typeof selected === "string") workspaceDraft = selected;
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  async function selectWorkspace(path: string) {
    busy = true;
    try {
      setSession(await invoke<SessionInfo>("chat_set_workspace", { request: { path } }));
      contentResults = [];
      gitStatus = null;
      gitDiff = "";
      diffTabs = [];
      await syncFileWatcher();
      await loadFiles();
      showWorkspace = false;
      addingWorkspace = false;
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      busy = false;
    }
  }

  async function deleteWorkspace(path: string) {
    busy = true;
    try {
      setSession(await invoke<SessionInfo>("chat_delete_workspace", { request: { path } }));
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      busy = false;
    }
  }

  async function newSession() {
    setSession(await api.chatNewSession());
    openChatTab();
  }

  async function selectSession(id: string) {
    setSession(await invoke<SessionInfo>("chat_select_session", { request: { id } }));
    openChatTab();
  }

  async function deleteSession(id: string) {
    setSession(await invoke<SessionInfo>("chat_delete_session", { request: { id } }));
  }

  function startRenameSession(session: SessionOption) {
    renameSessionId = session.id;
    renameDraft = session.title;
  }

  async function renameSession() {
    setSession(await invoke<SessionInfo>("chat_rename_session", { request: { id: renameSessionId, title: renameDraft } }));
    renameSessionId = "";
    renameDraft = "";
  }

  async function expandFileReferences(input: string) {
    const paths = [...new Set([...input.matchAll(/(?:^|\s)@([^\s@]+)/g)].map((match) => match[1]))];
    const files: string[] = [];
    const labels: string[] = [];
    for (const path of paths.slice(0, 8)) {
      try {
        const file = await invoke<{ path: string; content: string }>("file_read", { request: { path } });
        files.push(`--- ${file.path} ---\n${file.content}`);
        labels.push(`${file.path} (${file.content.length} chars)`);
      } catch {
        // Ignore unresolved refs; the raw @path remains in the prompt.
      }
    }
    if (!files.length) return { prompt: input, labels: [] };
    return { prompt: `${input}\n\nReferenced files:\n${files.join("\n\n")}`, labels };
  }

  async function sendPrompt() {
    const input = prompt.trim();
    if (!input || busy) return;

    prompt = "";
    closeMention();
    clearPromptHistory();
    busy = true;
    addMessage("user", input);

    try {
      const expanded = await expandFileReferences(input);
      if (expanded.labels.length) addMessage("tool", `file references\nstatus: ok\n${expanded.labels.join("\n")}`);
      await invoke("chat_send", { prompt: expanded.prompt });
      markActiveSessionRunning(true);
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      busy = false;
    }
  }

  async function compactSession() {
    if (busy || activeSessionRunning) return;
    compacting = true;
    addMessage("assistant", "Compacting session...");
    try {
      setSession(await api.chatCompact());
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      compacting = false;
    }
  }

  async function cancelPrompt() {
    try {
      setSession(await api.chatCancel());
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  async function deleteModel(model: ModelOption) {
    if (busy) return;
    busy = true;
    modelLabel = "deleting";
    try {
      config = await invoke<AiConfig>("ai_delete_model", { request: { model: model.name } });
      setConfigDraft(config);
      await loadConfig();
    } catch (error) {
      addMessage("error", String(error));
      modelLabel = "error";
    } finally {
      busy = false;
    }
  }

  async function saveConfig() {
    busy = true;
    const previousModel = `${config.provider}/${config.model_id}`;
    modelLabel = "saving";
    try {
      config = await invoke<AiConfig>("ai_save_config", { update: draft });
      setConfigDraft(config);
      showConfig = false;
      addingModel = false;
      await loadConfig();
      const nextModel = `${config.provider}/${config.model_id}`;
      if (nextModel !== previousModel) addMessage("assistant", `Model set: ${nextModel}`);
    } catch (error) {
      addMessage("error", String(error));
      modelLabel = "error";
    } finally {
      busy = false;
    }
  }

  async function saveProvider() {
    busy = true;
    try {
      config = await invoke<AiConfig>("ai_save_provider", { update: providerDraft });
      setConfigDraft(config);
      addingProvider = false;
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      busy = false;
    }
  }

  async function deleteProvider(provider: ProviderOption) {
    if (!confirm(`Delete provider ${provider.name}?`)) return;
    try {
      config = await invoke<AiConfig>("ai_delete_provider", { request: { provider: provider.name } });
      setConfigDraft(config);
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  function closeWindow() {
    void getCurrentWindow().close();
  }

  function keydown(event: KeyboardEvent) {
    if (handlePromptUndoRedo(event)) return;

    if (mentionResults.length) {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        mentionIndex = (mentionIndex + (event.key === "ArrowDown" ? 1 : -1) + mentionResults.length) % mentionResults.length;
        return;
      }
      if (event.key === "Tab" || event.key === "Enter") {
        event.preventDefault();
        insertMention(mentionResults[mentionIndex]);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        closeMention();
        return;
      }
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void sendPrompt();
    }
  }

  function globalKeydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    const key = event.key.toLowerCase();

    if (key === "w") {
      event.preventDefault();
      if (activeSessionId && !busy) void deleteSession(activeSessionId);
      return;
    }

    if (key !== "+" && key !== "=" && key !== "-" && key !== "_") return;
    event.preventDefault();
    const delta = key === "-" || key === "_" ? -0.05 : 0.05;
    void setUiScale(uiScale + delta);
  }

  $: if (messages.length) void scrollChatToBottom();
  $: if (activeTab === "chat" && chatOpen) void scrollChatToBottom();

  onMount(() => {
    let unlistenChat: UnlistenFn | undefined;
    let unlistenFiles: UnlistenFn | undefined;
    void listen<ChatStreamEvent>("chat_stream", (event) => handleStream(event.payload))
      .then((value) => (unlistenChat = value))
      .catch((error) => addMessage("error", String(error)));
    window.addEventListener("keydown", globalKeydown);
    void listen<FileChangedEvent>("file_changed", (event) => handleFileChanged(event.payload))
      .then((value) => (unlistenFiles = value))
      .catch((error) => addMessage("error", String(error)));

    void loadConfig().catch((error) => {
      modelLabel = "error";
      addMessage("error", String(error));
    });
    void loadSession()
      .then(() => {
        window.setTimeout(() => void loadFiles().catch((error) => addMessage("error", String(error))), 50);
      })
      .catch((error) => addMessage("error", String(error)));

    const poll = window.setInterval(() => {
      const active = sessions.find((session) => session.id === activeSessionId);
      if (!active?.running && sessions.some((session) => session.running)) void loadSession();
    }, 1000);
    return () => {
      window.clearInterval(poll);
      if (streamFrame) cancelAnimationFrame(streamFrame);
      if (fileChangeTimer) window.clearTimeout(fileChangeTimer);
      if (fileSearchTimer) window.clearTimeout(fileSearchTimer);
      window.removeEventListener("keydown", globalKeydown);
      void api.fileWatchStop();
      unlistenChat?.();
      unlistenFiles?.();
    };
  });
</script>

<main class="app">
  <AppHeader
    {appVersion}
    {config}
    {topProfileOptions}
    {switchProfile}
    {openMods}
    openTerminal={() => void openTerminal()}
    {closeWindow}
  />

  <div class="workbench">
    <AppSidebar
      {workspace}
      workspaceTitle={workspaceName(workspace)}
      showWorkspace={() => (showWorkspace = true)}
      {sideTab}
      setSideTab={(tab) => (sideTab = tab)}
      {featureContentSearch}
      {featureGit}
      {fileQuery}
      {inputFileQuery}
      {fileSearching}
      {fileSearchTruncated}
      fileTreeKey={`${workspace}:${fileTreeVersion}:${searchingFiles ? fileQuery : "tree"}`}
      {visibleFiles}
      {expandedFilePaths}
      {openFile}
      {contentQuery}
      setContentQuery={(value) => (contentQuery = value)}
      {contentSearchKeydown}
      {contentSearching}
      runContentSearch={() => void runContentSearch()}
      {contentResults}
      openSearchHit={(hit) => void openSearchHit(hit)}
      {fileName}
      {gitLoading}
      refreshGit={() => void refreshGit()}
      openGitDiff={(path) => void openGitDiff(path)}
      {gitStatus}
      {sessionQuery}
      setSessionQuery={(value) => (sessionQuery = value)}
      {sessionItems}
      newSession={() => void newSession()}
    />

    <section class="center">
      <div class="tabbar">
        {#if chatOpen}
          <button class:active={activeTab === "chat"} class="tab" type="button" on:click={() => (activeTab = "chat")}>{sessionLabel}</button>
          <button class="tab close-tab" type="button" on:click={closeChatTab}>×</button>
        {/if}
        {#if terminalOpen}
          <button class:active={activeTab === "terminal"} class="tab" type="button" on:click={() => (activeTab = "terminal")}>terminal</button>
          <button class="tab close-tab" type="button" on:click={closeTerminal}>×</button>
        {/if}
        {#each openFiles as file}
          <button class:active={activeTab === file.path} class="tab" type="button" title={file.path} on:click={() => (activeTab = file.path)}>{fileName(file.path)}{file.dirty ? " *" : ""}{file.stale ? " !" : ""}</button>
          <button class="tab close-tab" type="button" on:click={() => closeTab(file.path)}>×</button>
        {/each}
        {#each diffTabs as tab}
          <button class:active={activeTab === tab.id} class="tab" type="button" title={tab.path || "workspace"} on:click={() => (activeTab = tab.id)}>{tab.title}</button>
          <button class="tab close-tab" type="button" on:click={() => closeDiffTab(tab.id)}>×</button>
        {/each}
      </div>

      {#each openFiles as file (file.path)}
        <section class="editor-slot" class:hidden-pane={activeTab !== file.path}>
          {#if EditorPaneComponent}
            <svelte:component
              this={EditorPaneComponent}
              {file}
              onChange={(content: string) => updateOpenFile(file.path, content)}
              onDirtyChange={(dirty: boolean) => setOpenFileDirty(file.path, dirty)}
              onMode={(mode: "edit" | "diff") => setFileMode(file.path, mode)}
              onSave={(content: string) => void saveOpenFile(file.path, content)}
            />
          {:else}
            <section class="empty-editor">loading editor...</section>
          {/if}
        </section>
      {/each}

      {#if activeTab === "chat" && chatOpen}
        <ChatPanel
          messagesEl={messagesEl}
          setMessagesEl={(element) => (messagesEl = element)}
          {hiddenMessageGroupCount}
          {visibleMessageGroups}
          {showEarlierMessages}
          {isStreamingMessage}
          {contextUsed}
          {contextLimit}
          {contextPercent}
          {transcriptUsed}
          {inputTokens}
          {outputTokens}
          {formatContext}
          {activeSessionRunning}
          {compacting}
          promptEl={promptEl}
          setPromptEl={(element) => (promptEl = element)}
          {mentionResults}
          {mentionIndex}
          {insertMention}
          {prompt}
          {rememberPromptSnapshot}
          {inputPrompt}
          {keydown}
          sendPrompt={() => void sendPrompt()}
          {busy}
          {messages}
          compactSession={() => void compactSession()}
          cancelPrompt={() => void cancelPrompt()}
        />
      {:else if activeTab === "terminal" && terminalOpen}
        {#if TerminalPaneComponent}
          <svelte:component this={TerminalPaneComponent} id={terminalId} />
        {:else}
          <section class="empty-editor">loading terminal...</section>
        {/if}
      {:else if diffTabs.find((tab) => tab.id === activeTab)}
        {@const tab = diffTabs.find((tab) => tab.id === activeTab)!}
        {#if DiffPaneComponent}
          <svelte:component this={DiffPaneComponent} {tab} />
        {:else}
          <section class="empty-editor">loading diff...</section>
        {/if}
      {:else if !openFiles.find((file) => file.path === activeTab)}
        <section class="empty-editor">open file, terminal, or chat tab</section>
      {/if}
    </section>
  </div>

  <WorkspaceModal
    open={showWorkspace}
    adding={addingWorkspace}
    items={workspaceItems}
    draft={workspaceDraft}
    {busy}
    onClose={() => (showWorkspace = false)}
    onStartAdd={() => { addingWorkspace = true; workspaceDraft = workspace; }}
    onBack={() => (addingWorkspace = false)}
    onBrowse={() => void chooseWorkspaceFolder()}
    onDraftChange={(value) => (workspaceDraft = value)}
    onSave={(path) => void selectWorkspace(path)}
  />

  <RenameModal
    open={!!renameSessionId}
    value={renameDraft}
    {busy}
    onClose={() => (renameSessionId = "")}
    onValueChange={(value) => (renameDraft = value)}
    onSave={() => void renameSession()}
  />

  {#if showMods}
    <Modal title="Mods" fixed onClose={() => (showMods = false)}>
      <div class="mods-layout">
        <nav class="mods-nav" aria-label="mods sections">
          <button class:active={modsTab === "general"} class="ghost" type="button" on:click={() => (modsTab = "general")}>general</button>
          <button class:active={modsTab === "profile"} class="ghost" type="button" on:click={() => (modsTab = "profile")}>profile</button>
          <button class:active={modsTab === "providers"} class="ghost" type="button" on:click={() => (modsTab = "providers")}>providers</button>
          <button class:active={modsTab === "models"} class="ghost" type="button" on:click={() => (modsTab = "models")}>models</button>
          <button class:active={modsTab === "agents"} class="ghost" type="button" on:click={() => (modsTab = "agents")}>agents</button>
          <button class:active={modsTab === "subagents"} class="ghost" type="button" on:click={() => (modsTab = "subagents")}>subagents</button>
          <button class:active={modsTab === "mcp"} class="ghost" type="button" on:click={() => (modsTab = "mcp")}>mcp</button>
          <button class:active={modsTab === "extensions"} class="ghost" type="button" on:click={() => { modsTab = "extensions"; void loadExtensionsInfo(); }}>extensions</button>
        </nav>

        <section class="mods-content">
          {#if modsTab === "general"}
            <div class="mods-about" aria-label="app description">
              <pre>{`sandevistan@${appVersion}
----------------
profile       ${config.active_profile || "default"}
model         ${config.mods.main_model}
agent         ${config.mods.main_agent}
workspace     ${workspace || "none"}
features      git:${featureGit ? "on" : "off"} watcher:${featureFileWatcher ? "on" : "off"} search:${featureContentSearch ? "on" : "off"}`}</pre>
            </div>
            <div class="feature-list compact-feature-list">
              <div class="side-title">general settings</div>
              <Checkbox checked={featureGit} label={`git panel: ${featureGit ? "on" : "off"}`} onChange={(checked) => void setFeature("git", checked)} />
              <Checkbox checked={featureFileWatcher} label={`file watcher: ${featureFileWatcher ? "on" : "off"}`} onChange={(checked) => void setFeature("file_watcher", checked)} />
              <Checkbox checked={featureContentSearch} label={`content search: ${featureContentSearch ? "on" : "off"}`} onChange={(checked) => void setFeature("content_search", checked)} />
            </div>
          {:else if modsTab === "profile"}
            <label>Profile<SelectBox fit value={modsProfile} options={profileOptions} onChange={chooseProfile} /></label>
            <label>Profile name<input bind:value={modsProfile} placeholder="default" /></label>
            <label>Main model<SelectBox fit value={modsDraft.main_model} options={mainModelOptions} onChange={(value) => (modsDraft = { ...modsDraft, main_model: value })} /></label>
            <label>Main agent<SelectBox fit value={modsDraft.main_agent} options={mainAgentOptions} onChange={(value) => (modsDraft = { ...modsDraft, main_agent: value })} /></label>
            <label>Subagent concurrency<input bind:value={modsDraft.subagent_max_concurrency} type="number" min="1" max="5" step="1" /></label>
            <div class="feature-list compact-feature-list">
              <div class="side-title">profile toggles</div>
              <Checkbox checked={modsDraft.rtk_enabled} label={`rtk: ${config.rtk_available ? (modsDraft.rtk_enabled ? "on" : "off") : "not installed"}`} disabled={!config.rtk_available && !modsDraft.rtk_enabled} onChange={(checked) => (modsDraft = { ...modsDraft, rtk_enabled: checked })} />
              <Checkbox checked={modsDraft.shell_enabled} label={`shell tool: ${modsDraft.shell_enabled ? "on" : "off"}${modsDraft.rtk_enabled && config.rtk_available ? " · via rtk" : ""}`} onChange={(checked) => (modsDraft = { ...modsDraft, shell_enabled: checked })} />
              <Checkbox checked={modsDraft.subagents_enabled} label={`subagents: ${modsDraft.subagents_enabled ? "on" : "off"}`} onChange={(checked) => (modsDraft = { ...modsDraft, subagents_enabled: checked })} />
            </div>
            {#if modsDraft.subagents_enabled}
              <label>Subagent fallback model<SelectBox fit value={modsDraft.subagent_model} options={subagentModelOptions} onChange={(value) => (modsDraft = { ...modsDraft, subagent_model: value })} /></label>
              <div class="feature-list compact-feature-list">
                <div class="side-title">enabled subagents</div>
                {#each subagentEntries as name}
                  <Checkbox checked={modsDraft.subagents.includes(name)} label={name} onChange={() => toggleSubagent(name)} />
                {/each}
              </div>
            {/if}
            <p class="hint">profile = model + one agent + selected subagents. changes affect next run.</p>
          {:else if modsTab === "providers"}
            {#if !addingProvider}
              <div class="model-scroll"><ItemList items={providerItems} addTitle="+ add provider" addSubtitle="endpoint + key" onAdd={addProvider} /></div>
            {:else}
              <label>Provider name<input bind:value={providerDraft.name} placeholder="openai" /></label>
              <label>Kind<SelectBox fit value={providerDraft.kind} options={providerKindOptions} onChange={(value) => (providerDraft = { ...providerDraft, kind: value })} /></label>
              <label>API base<input bind:value={providerDraft.api_base} placeholder="https://api.openai.com/v1" /></label>
              <label>API key header<SelectBox fit value={providerDraft.api_key_header} options={apiKeyHeaderOptions} onChange={(value) => (providerDraft = { ...providerDraft, api_key_header: value })} /></label>
              <label>API key <small>{providerDraft.original_name && config.providers.find((p) => p.name === providerDraft.original_name)?.has_api_key ? "saved; leave blank to keep" : "not set"}</small><input bind:value={providerDraft.api_key} type="password" placeholder="sk-..." /></label>
              <div class="actions right"><button class="ghost" type="button" on:click={() => (addingProvider = false)}>back</button><button type="button" disabled={busy} on:click={() => void saveProvider()}>save provider</button></div>
            {/if}
          {:else if modsTab === "models"}
            {#if !addingModel}
              <div class="model-scroll"><ItemList items={modelItems} addTitle="+ add model" addSubtitle="select provider" onAdd={startAddModel} /></div>
            {:else}
              <label>Provider<SelectBox fit value={providerChoice} options={providerOptions} onChange={chooseProvider} /></label>
              <label>Model<input bind:value={draft.model} placeholder="gpt-4o-mini" /></label>
              <label>Context chars<input bind:value={draft.context_chars} type="number" min="4000" max="1000000" step="1000" /></label>
              <div class="actions right"><button class="ghost" type="button" on:click={() => (addingModel = false)}>back</button><button type="button" disabled={busy} on:click={saveConfig}>save model</button></div>
            {/if}
          {:else if modsTab === "agents"}
            {#if !addingAgent}
              <ItemList items={agentItems} addTitle="+ add agent" addSubtitle="main agent" onAdd={addAgent} />
            {:else}
              <label>Name<input bind:value={agentDraft.name} placeholder="custom" /></label>
              <label>Description<input bind:value={agentDraft.description} placeholder="main agent description" /></label>
              <label>Thinking<SelectBox fit value={agentDraft.thinking_level} options={thinkingOptions} onChange={(value) => (agentDraft = { ...agentDraft, thinking_level: value as ThinkingLevel })} /></label>
              <label>Persona<textarea bind:value={agentDraft.persona} rows="4"></textarea></label>
              <label>Prompt injection<textarea bind:value={agentDraft.prompt_injection} rows="4"></textarea></label>
              <div class="actions right"><button class="ghost" type="button" on:click={() => (addingAgent = false)}>back</button><button type="button" disabled={!agentDraft.name.trim()} on:click={() => void saveAgent()}>save agent</button><button class="ghost danger" type="button" disabled={!agentDraft.original_name || agentDraft.original_name === "custom"} on:click={() => void deleteAgent(agentDraft.original_name)}>delete</button></div>
            {/if}
          {:else if modsTab === "subagents"}
            {#if !addingSubagent}
              <ItemList items={subagentItems} addTitle="+ add subagent" addSubtitle="worker definition" onAdd={addSubagent} />
            {:else}
              <label>Name<input bind:value={subagentDraft.name} placeholder="scout" /></label>
              <label>Description<input bind:value={subagentDraft.description} placeholder="short purpose" /></label>
              <label>Model<SelectBox fit value={subagentDraft.model} options={subagentModelOptions} onChange={(value) => (subagentDraft = { ...subagentDraft, model: value })} /></label>
              <label>Max result chars<input bind:value={subagentDraft.max_result_chars} type="number" min="500" max="20000" step="500" /></label>
              <label>System<textarea bind:value={subagentDraft.system} rows="6" placeholder="subagent role and rules"></textarea></label>
              <div class="actions right"><button class="ghost" type="button" on:click={() => (addingSubagent = false)}>back</button><button type="button" disabled={!subagentDraft.name.trim() || !subagentDraft.system.trim()} on:click={() => void saveSubagent()}>save subagent</button><button class="ghost danger" type="button" disabled={!subagentDraft.original_name} on:click={() => void deleteSubagent(subagentDraft.original_name)}>delete</button></div>
            {/if}
          {:else if modsTab === "mcp"}
            <div class="feature-list compact-feature-list">
              <div class="side-title">MCP</div>
              <Checkbox checked={modsDraft.mcp_enabled} label={`MCP tools: ${modsDraft.mcp_enabled ? "on" : "off"}`} onChange={(checked) => (modsDraft = { ...modsDraft, mcp_enabled: checked })} />
            </div>
            {#if !editingMcp}
              <ItemList items={mcpItems} addTitle="+ add MCP server" addSubtitle="stdio server" onAdd={addMcpServer} />
              <p class="hint">Built-in lightweight stdio MCP client. Tools exposed when enabled: mcp.list, mcp.call. Save mods to apply.</p>
            {:else}
              <label>Name<input bind:value={mcpDraft.name} placeholder="github" /></label>
              <label>Command<input bind:value={mcpDraft.command} placeholder="npx" /></label>
              <label>Args <small>one per line</small><textarea bind:value={mcpDraft.args} rows="5" placeholder={`-y
@modelcontextprotocol/server-memory`}></textarea></label>
              <label>Timeout ms<input bind:value={mcpDraft.timeout_ms} type="number" min="1000" max="60000" step="1000" /></label>
              <label>Env <small>KEY=value, one per line</small><textarea bind:value={mcpDraft.env} rows="5" placeholder="GITHUB_PERSONAL_ACCESS_TOKEN=...
SUPABASE_ACCESS_TOKEN=..."></textarea></label>
              <div class="actions right"><button class="ghost" type="button" on:click={() => (editingMcp = false)}>back</button><button type="button" disabled={!mcpDraft.name.trim() || !mcpDraft.command.trim()} on:click={saveMcpServer}>save server</button><button class="ghost danger" type="button" disabled={!mcpDraft.original_name} on:click={() => deleteMcpServer(mcpDraft.original_name)}>delete</button></div>
            {/if}
          {:else if modsTab === "extensions"}
            {#if !editingExtension}
              <p class="hint">Config: {extensionsInfo.config_path || "~/.sandevistan/extensions.toml"}</p>
              <ItemList items={extensionItems} addTitle="+ rust extension" addSubtitle="native binary scaffold" onAdd={addExtension} />
            {:else}
              <label>Extension id<input bind:value={extensionDraft.id} disabled={!creatingExtension && Boolean(extensionDraft.path)} placeholder="my-extension" /></label>
              <label>Name<input bind:value={extensionDraft.name} disabled={!creatingExtension} /></label>
              <label>Description<textarea bind:value={extensionDraft.description} disabled={!creatingExtension} rows="3"></textarea></label>
              <div class="feature-list compact-feature-list">
                <div class="side-title">status</div>
                <Checkbox checked={extensionDraft.enabled} label={`enabled: ${extensionDraft.enabled ? "on" : "off"}`} disabled={creatingExtension || !extensionDraft.id.trim()} onChange={(checked) => void setExtensionEnabled(extensionDraft.id, checked)} />
                {#if extensionDraft.hooks.length}<p class="hint">hooks: {extensionDraft.hooks.join(", ")}</p>{/if}
                {#if extensionDraft.tools.length}<p class="hint">tools: {extensionDraft.tools.map((tool) => tool.name).join(", ")}</p>{/if}
                {#if extensionDraft.path}<p class="hint">manifest: {extensionDraft.path}</p>{:else}<p class="hint">create manifest: {extensionManifestHint(extensionDraft.id)}</p>{/if}
              </div>
              <div class="actions right">
                <button class="ghost" type="button" on:click={() => (editingExtension = false)}>back</button>
                {#if creatingExtension}<button type="button" disabled={!extensionDraft.id.trim()} on:click={() => void createRustExtension()}>create rust extension</button>{/if}
                <button class="ghost" type="button" on:click={() => void loadExtensionsInfo()}>reload</button>
              </div>
            {/if}
          {/if}
        </section>
      </div>
      <div class="actions right">
        <button class="ghost" type="button" on:click={() => (showMods = false)}>back</button>
        <button type="button" disabled={busy} on:click={() => void saveMods()}>save mods</button>
      </div>
    </Modal>
  {/if}

  {#if showConfig}
    <Modal title="Model" onClose={() => (showConfig = false)}>
        {#if !addingModel}
          <div class="model-scroll">
            <ItemList items={modelItems} addTitle="+ add model" addSubtitle="select provider" onAdd={startAddModel} />
          </div>
          <div class="feature-list">
            <div class="side-title">features</div>
            <button class="ghost" type="button" on:click={() => void setFeature("content_search", !featureContentSearch)}>content search: {featureContentSearch ? "on" : "off"}</button>
            <button class="ghost" type="button" on:click={() => void setFeature("file_watcher", !featureFileWatcher)}>file watcher: {featureFileWatcher ? "on" : "off"}</button>
          </div>
        {:else}
          <label>Provider<SelectBox fit value={providerChoice} options={providerOptions} onChange={chooseProvider} /></label>
          <label>Model<input bind:value={draft.model} placeholder="gpt-4o-mini" /></label>
          <label>Context chars<input bind:value={draft.context_chars} type="number" min="4000" max="1000000" step="1000" /></label>
          <p class="hint">Provider endpoint/key live in Mods → providers.</p>
          <div class="actions right">
            <button class="ghost" type="button" on:click={() => (addingModel = false)}>back</button>
            <button type="button" disabled={busy} on:click={saveConfig}>save model</button>
          </div>
        {/if}
    </Modal>
  {/if}
</main>
