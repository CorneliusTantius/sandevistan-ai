<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Checkbox from "./components/Checkbox.svelte";
  import DiffPane, { type DiffTab } from "./components/DiffPane.svelte";
  import EditorPane, { type OpenFile } from "./components/EditorPane.svelte";
  import FileTree, { type FileEntry } from "./components/FileTree.svelte";
  import ItemList, { type Item } from "./components/ItemList.svelte";
  import MessageView from "./components/MessageView.svelte";
  import Modal from "./components/Modal.svelte";
  import SelectBox, { type SelectOption } from "./components/SelectBox.svelte";
  import TerminalPane from "./components/TerminalPane.svelte";
  import ToolGroup from "./components/ToolGroup.svelte";

  type Role = "user" | "assistant" | "tool" | "error";
  type SearchHit = { path: string; line: number; column: number; text: string };
  type GitStatusEntry = { path: string; status: string; raw: string };
  type GitStatus = { branch: string; entries: GitStatusEntry[] };
  type Message = { role: Role; content: string };
  type TextSnapshot = { value: string; selectionStart: number; selectionEnd: number };
  type MessageGroup = { key: string; kind: "message"; message: Message } | { key: string; kind: "tools"; tools: Message[] };
  type WorkspaceOption = { path: string; name: string; deletable: boolean };
  type SessionOption = { id: string; title: string; preview: string; message_count: number; updated_at: number; running: boolean };
  type SessionInfo = { workspace: string; active_session_id: string; messages: Message[]; sessions: SessionOption[]; workspaces: WorkspaceOption[] };
  type ChatStreamEvent = { session_id: string; kind: "start" | "delta" | "tool" | "done" | "error"; role?: Role; text?: string; content?: string };
  type FileChangedEvent = { workspace: string; paths: string[] };
  type ProviderOption = { name: string; api_base: string };
  type ModelOption = { name: string; provider: string; id: string; context_chars: number };
  type ThinkingLevel = "auto" | "low" | "medium" | "high";
  type AgentOption = { name: string; description: string; persona: string; thinking_level: ThinkingLevel; prompt_injection: string };
  type SubagentOption = { name: string; description: string; system: string; model: string; max_result_chars: number };
  type AiMods = {
    main_model: string;
    main_agent: string;
    subagents: string[];
    persona: string;
    thinking_level: ThinkingLevel;
    prompt_injection: string;
    rtk_enabled: boolean;
    shell_enabled: boolean;
    git_panel_enabled: boolean;
    subagents_enabled: boolean;
    subagent_model: string;
    subagent_max_concurrency: number;
    subagents_config: string;
  };
  type ProfileOption = AiMods & { name: string };
  type AiConfig = {
    config_dir: string;
    provider: string;
    api_base: string;
    model: string;
    model_id: string;
    context_chars: number;
    has_api_key: boolean;
    providers: ProviderOption[];
    models: ModelOption[];
    features: Record<string, boolean>;
    mods: AiMods;
    active_profile: string;
    profiles: ProfileOption[];
    agents: AgentOption[];
    subagents_registry: SubagentOption[];
    rtk_available: boolean;
    ui_scale: number;
  };

  const emptyConfig: AiConfig = {
    config_dir: "",
    provider: "openai",
    api_base: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    model_id: "gpt-4o-mini",
    context_chars: 80000,
    has_api_key: false,
    providers: [],
    models: [],
    features: { content_search: true, git: true, file_watcher: false },
    mods: { main_model: "gpt-4o-mini", main_agent: "custom", subagents: ["scout", "reviewer", "planner"], persona: "", thinking_level: "auto", prompt_injection: "", rtk_enabled: true, shell_enabled: false, git_panel_enabled: true, subagents_enabled: true, subagent_model: "", subagent_max_concurrency: 3, subagents_config: "" },
    active_profile: "default",
    profiles: [{ name: "default", main_model: "gpt-4o-mini", main_agent: "custom", subagents: ["scout", "reviewer", "planner"], persona: "", thinking_level: "auto", prompt_injection: "", rtk_enabled: true, shell_enabled: false, git_panel_enabled: true, subagents_enabled: true, subagent_model: "", subagent_max_concurrency: 3, subagents_config: "" }],
    agents: [{ name: "custom", description: "Default main agent", persona: "", thinking_level: "auto", prompt_injection: "" }],
    subagents_registry: [],
    rtk_available: false,
    ui_scale: 1,
  };

  let modelLabel = "model";
  let prompt = "";
  let promptUndoStack: TextSnapshot[] = [];
  let promptRedoStack: TextSnapshot[] = [];
  let busy = false;
  let showConfig = false;
  let showMods = false;
  let showWorkspace = false;
  let addingModel = false;
  let messages: Message[] = [];
  let messagesEl: HTMLDivElement;
  let streamBuffer = "";
  let streamFrame = 0;
  let fileChangeTimer = 0;
  let streamMessageOpen = false;
  let files: FileEntry[] = [];
  let fileIndex: FileEntry[] = [];
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
  let draft = { provider: "openai", api_base: "https://api.openai.com/v1", model: "gpt-4o-mini", original_model: "", api_key: "", context_chars: 80000 };
  let modsDraft: AiMods = { main_model: "gpt-4o-mini", main_agent: "custom", subagents: ["scout", "reviewer", "planner"], persona: "", thinking_level: "auto", prompt_injection: "", rtk_enabled: true, shell_enabled: false, git_panel_enabled: true, subagents_enabled: true, subagent_model: "", subagent_max_concurrency: 3, subagents_config: "" };
  let modsProfile = "default";
  let modsTab: "general" | "profile" | "models" | "agents" | "subagents" = "profile";
  let addingAgent = false;
  let addingSubagent = false;
  let agentDraft = { name: "", original_name: "", description: "", persona: "", thinking_level: "auto" as ThinkingLevel, prompt_injection: "" };
  let subagentDraft = { name: "", original_name: "", description: "", system: "", model: "", max_result_chars: 4000 };
  $: providerOptions = [
    ...config.providers.map((provider): SelectOption => ({ value: provider.name, label: provider.name })),
    { value: "__new__", label: "+ provider" },
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
  $: visibleFiles = searchingFiles ? searchIndex(fileIndex, fileQuery) : filterFiles(files, expandedDirs, fileQuery);
  $: expandedFilePaths = searchingFiles ? visibleFiles.filter((file) => file.kind === "dir").map((file) => norm(file.path)) : [...expandedDirs];
  $: messageGroups = groupMessages(messages);
  $: activeSessionRunning = sessions.find((session) => session.id === activeSessionId)?.running ?? false;
  $: featureContentSearch = config.features?.content_search ?? true;
  $: featureGit = config.features?.git ?? true;
  $: featureFileWatcher = config.features?.file_watcher ?? false;
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
    draft = { provider: value.provider, api_base: value.api_base, model: value.model, original_model: value.model, api_key: "", context_chars: value.context_chars || 80000 };
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

  function handleFileChanged(event: FileChangedEvent) {
    if (event.workspace !== workspace || !featureFileWatcher) return;
    if (fileChangeTimer) window.clearTimeout(fileChangeTimer);
    fileChangeTimer = window.setTimeout(() => {
      fileChangeTimer = 0;
      void handleToolFileMutation();
    }, 300);
  }

  function handleStream(event: ChatStreamEvent) {
    if (event.session_id !== activeSessionId) return;
    if (event.kind === "start") {
      streamBuffer = "";
      ensureStreamMessage();
    } else if (event.kind === "delta") {
      ensureStreamMessage();
      streamBuffer += event.text ?? "";
      scheduleStreamFlush();
    } else if (event.kind === "tool") {
      flushStreamNow();
      streamMessageOpen = false;
      addMessage("tool", event.content ?? "");
    } else if (event.kind === "error") {
      flushStreamNow();
      streamMessageOpen = false;
      addMessage("error", event.content ?? "stream error");
    } else if (event.kind === "done") {
      flushStreamNow();
      streamMessageOpen = false;
      void loadSession();
    }
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

  function openConfig() {
    addingModel = false;
    setConfigDraft(config);
    modsTab = "models";
    showMods = true;
  }

  async function selectModel(model: ModelOption) {
    const provider = config.providers.find((entry) => entry.name === model.provider);
    draft = {
      provider: model.provider,
      api_base: provider?.api_base || draft.api_base,
      model: model.name,
      original_model: model.name,
      api_key: "",
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
    const provider = config.providers.find((entry) => entry.name === model.provider);
    addingModel = true;
    providerChoice = model.provider;
    draft = {
      provider: model.provider,
      api_base: provider?.api_base || draft.api_base,
      model: model.name,
      original_model: model.name,
      api_key: "",
      context_chars: model.context_chars || 80000,
    };
  }

  function chooseProvider(value: string) {
    providerChoice = value;
    if (value === "__new__") {
      draft = { ...draft, provider: "", api_base: "" };
      return;
    }

    const provider = config.providers.find((entry) => entry.name === value);
    draft = { ...draft, provider: value, api_base: provider?.api_base || draft.api_base };
  }

  function openMods() {
    modsProfile = config.active_profile || "default";
    modsDraft = normalizeMods(config.mods);
    addingAgent = false;
    addingSubagent = false;
    showMods = true;
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
      subagent_max_concurrency: Math.min(4, Math.max(1, Number(value.subagent_max_concurrency) || 3)),
    };
  }

  function defaultMods(): AiMods {
    return { main_model: config.model || "gpt-4o-mini", main_agent: "custom", subagents: ["scout", "reviewer", "planner"], persona: "", thinking_level: "auto", prompt_injection: "", rtk_enabled: config.rtk_available, shell_enabled: false, git_panel_enabled: true, subagents_enabled: true, subagent_model: "", subagent_max_concurrency: 3, subagents_config: "" };
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
    config = await invoke<AiConfig>("ai_config");
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
      if (config.features?.file_watcher ?? false) await invoke("file_watch_start");
      else await invoke("file_watch_stop");
    } catch (error) {
      addMessage("error", String(error));
    }
  }

  function mutationCount(items: Message[]) {
    return items.filter((message) => message.role === "tool" && /(^|\n)(edited|wrote)\s/.test(message.content)).length;
  }

  function setSession(session: SessionInfo) {
    workspace = session.workspace;
    workspaceDraft = session.workspace;
    workspaces = session.workspaces;
    activeSessionId = session.active_session_id;
    sessions = session.sessions;
    sessionLabel = session.sessions.find((item) => item.id === session.active_session_id)?.title ?? "session";
    messages = session.messages;
  }

  async function loadSession() {
    const previousMutations = toolMutationCount;
    const session = await invoke<SessionInfo>("chat_session");
    const nextMutations = mutationCount(session.messages);
    setSession(session);
    toolMutationCount = nextMutations;
    if (nextMutations > previousMutations) await handleToolFileMutation();
  }

  async function loadFiles() {
    const [tree, index] = await Promise.all([
      invoke<FileEntry[]>("workspace_tree"),
      invoke<FileEntry[]>("workspace_index"),
    ]);
    files = tree;
    fileIndex = index;
    expandedDirs = new Set();
    loadedDirs = new Set();
    fileTreeVersion += 1;
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
      gitStatus = await invoke<GitStatus>("git_status");
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

  function searchIndex(items: FileEntry[], queryText: string) {
    const query = queryText.trim().toLowerCase();
    if (!query) return [];

    const paths = new Set<string>();
    for (const file of items) {
      if (!file.name.toLowerCase().includes(query) && !file.path.toLowerCase().includes(query)) continue;
      paths.add(norm(file.path));
      let parent = parentPath(file.path);
      while (parent) {
        paths.add(parent);
        parent = parentPath(parent);
      }
    }

    return items.filter((file) => paths.has(norm(file.path)));
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

  function openTerminal() {
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
    setSession(await invoke<SessionInfo>("chat_new_session"));
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

  async function sendPrompt() {
    const input = prompt.trim();
    if (!input || busy) return;

    prompt = "";
    clearPromptHistory();
    busy = true;
    addMessage("user", input);

    try {
      await invoke("chat_send", { prompt: input });
      await loadSession();
    } catch (error) {
      addMessage("error", String(error));
    } finally {
      busy = false;
    }
  }

  async function cancelPrompt() {
    try {
      setSession(await invoke<SessionInfo>("chat_cancel"));
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

  function closeWindow() {
    void getCurrentWindow().close();
  }

  function keydown(event: KeyboardEvent) {
    if (handlePromptUndoRedo(event)) return;

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void sendPrompt();
    }
  }

  function globalKeydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    const key = event.key.toLowerCase();
    if (key !== "+" && key !== "=" && key !== "-" && key !== "_") return;
    event.preventDefault();
    const delta = key === "-" || key === "_" ? -0.05 : 0.05;
    void setUiScale(uiScale + delta);
  }

  $: if (messages.length) void scrollChatToBottom();

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
      window.removeEventListener("keydown", globalKeydown);
      void invoke("file_watch_stop");
      unlistenChat?.();
      unlistenFiles?.();
    };
  });
</script>

<main class="app">
  <header class="topbar" data-tauri-drag-region>
    <div>
      <pre class="ascii" aria-label="sandevistan">███████╗ █████╗ ███╗   ██╗██████╗ ███████╗██╗   ██╗██╗███████╗████████╗ █████╗ ███╗   ██╗         █████╗ ██╗
██╔════╝██╔══██╗████╗  ██║██╔══██╗██╔════╝██║   ██║██║██╔════╝╚══██╔══╝██╔══██╗████╗  ██║        ██╔══██╗██║
███████╗███████║██╔██╗ ██║██║  ██║█████╗  ██║   ██║██║███████╗   ██║   ███████║██╔██╗ ██║        ███████║██║
╚════██║██╔══██║██║╚██╗██║██║  ██║██╔══╝  ╚██╗ ██╔╝██║╚════██║   ██║   ██╔══██║██║╚██╗██║        ██╔══██║██║
███████║██║  ██║██║ ╚████║██████╔╝███████╗ ╚████╔╝ ██║███████║   ██║   ██║  ██║██║ ╚████║███████╗██║  ██║██║
╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚═════╝ ╚══════╝  ╚═══╝  ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝╚═╝  ╚═╝╚═╝</pre>
    </div>
    <div class="header-actions">
      <div class="top-profile"><SelectBox value={config.active_profile} options={topProfileOptions} onChange={(value) => void switchProfile(value)} /></div>
      <button class="ghost" type="button" on:click={openMods}>mods</button>
      <button class="ghost" type="button" on:click={openTerminal}>term</button>
      <button class="window-close" type="button" aria-label="close" on:click={closeWindow}>×</button>
    </div>
  </header>

  <div class="workbench">
    <aside class="sidebar">
      <section class="side-section workspace-section">
        <div class="side-title">workspace</div>
        <button class="ghost workspace-button" type="button" title={workspace} on:click={() => (showWorkspace = true)}>{workspaceName(workspace)}</button>
      </section>

      <section class="side-section files-section">
        <div class="side-tabs">
          <button class:active={sideTab === "files"} type="button" on:click={() => (sideTab = "files")}>files</button>
          {#if featureContentSearch}<button class:active={sideTab === "content"} type="button" on:click={() => (sideTab = "content")}>content</button>{/if}
          {#if featureGit}<button class:active={sideTab === "git"} type="button" on:click={() => (sideTab = "git")}>git</button>{/if}
        </div>

        {#if sideTab === "files"}
          <input class="side-search" bind:value={fileQuery} placeholder="search" />
          {#key `${workspace}:${fileTreeVersion}`}
            <FileTree entries={visibleFiles} expandedPaths={expandedFilePaths} onOpen={openFile} />
          {/key}
        {:else if sideTab === "content" && featureContentSearch}
          <div class="inline-row">
            <input bind:value={contentQuery} placeholder="rg search" on:keydown={contentSearchKeydown} />
            <button class="ghost compact" type="button" disabled={contentSearching} on:click={() => void runContentSearch()}>go</button>
          </div>
          <div class="compact-list">
            {#each contentResults as hit (`${hit.path}:${hit.line}:${hit.column}`)}
              <button class="content-result" type="button" title={`${hit.path}:${hit.line}:${hit.column}\n${hit.text}`} on:click={() => void openSearchHit(hit)}>
                <div class="result-file">
                  <span>{fileName(hit.path)}</span>
                  <small>L{hit.line}:C{hit.column}</small>
                </div>
                <div class="result-line">{hit.text.trim() || " "}</div>
              </button>
            {:else}
              <span class="empty-state">{contentQuery.trim() ? "no matches found" : "type query + press Enter"}</span>
            {/each}
          </div>
        {:else if sideTab === "git" && featureGit}
          <div class="inline-row">
            <button class="ghost compact" type="button" disabled={gitLoading} on:click={() => void refreshGit()}>status</button>
            <button class="ghost compact" type="button" disabled={gitLoading} on:click={() => void openGitDiff()}>diff</button>
          </div>
          {#if gitStatus}
            <div class="hint">{gitStatus.branch} · {gitStatus.entries.length} changed</div>
            <div class="compact-list">
              {#each gitStatus.entries as entry (entry.raw)}
                <button class="result-row" type="button" title={entry.raw} on:click={() => void openGitDiff(entry.path)}>
                  <strong>{entry.status} {entry.path}</strong>
                </button>
              {:else}
                <span class="empty-state">working tree clean</span>
              {/each}
            </div>
          {/if}
        {/if}
      </section>

      <section class="side-section sessions-section">
        <div class="side-title">sessions</div>
        <input bind:value={sessionQuery} placeholder="search" />
        <ItemList items={sessionItems} addTitle="+ new session" addSubtitle="empty chat" onAdd={() => void newSession()} />
      </section>
    </aside>

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
          <EditorPane
            {file}
            onChange={(content) => updateOpenFile(file.path, content)}
            onDirtyChange={(dirty) => setOpenFileDirty(file.path, dirty)}
            onMode={(mode) => setFileMode(file.path, mode)}
            onSave={(content) => void saveOpenFile(file.path, content)}
          />
        </section>
      {/each}

      {#if activeTab === "chat" && chatOpen}
      <section class="chat" aria-label="AI chat">
        <div class="messages" bind:this={messagesEl}>
          {#each messageGroups as group (group.key)}
            {#if group.kind === "message"}
              <MessageView role={group.message.role} content={group.message.content} />
            {:else}
              <ToolGroup tools={group.tools} />
            {/if}
          {/each}
        </div>

        <form class="prompt-form" on:submit|preventDefault={sendPrompt}>
          <textarea value={prompt} on:beforeinput={rememberPromptSnapshot} on:input={inputPrompt} on:keydown={keydown} rows="4" placeholder="message · Enter = send · Shift+Enter = newline" autocomplete="off"></textarea>
          {#if activeSessionRunning}<span class="run-dot" aria-label="working"></span>{/if}
          <button type="submit" disabled={busy || activeSessionRunning || !prompt.trim()}>send</button>
          <button class="ghost danger" type="button" disabled={!activeSessionRunning} on:click={() => void cancelPrompt()}>abort</button>
        </form>
      </section>
      {:else if activeTab === "terminal" && terminalOpen}
        <TerminalPane id={terminalId} />
      {:else if diffTabs.find((tab) => tab.id === activeTab)}
        {@const tab = diffTabs.find((tab) => tab.id === activeTab)!}
        <DiffPane {tab} />
      {:else if !openFiles.find((file) => file.path === activeTab)}
        <section class="empty-editor">open file, terminal, or chat tab</section>
      {/if}
    </section>
  </div>

  {#if showWorkspace}
    <Modal title="Workspace" onClose={() => (showWorkspace = false)}>
      {#if !addingWorkspace}
        <ItemList
          items={workspaceItems}
          addTitle="+ add workspace"
          addSubtitle="directory"
          onAdd={() => { addingWorkspace = true; workspaceDraft = workspace; }}
        />
      {:else}
        <label>
          Path
          <input bind:value={workspaceDraft} placeholder="~/code/project" />
        </label>
        <div class="actions right">
          <button class="ghost" type="button" on:click={() => (addingWorkspace = false)}>back</button>
          <button type="button" disabled={busy} on:click={() => void selectWorkspace(workspaceDraft)}>save workspace</button>
        </div>
      {/if}
    </Modal>
  {/if}

  {#if renameSessionId}
    <Modal title="Rename" onClose={() => (renameSessionId = "")}>
      <label>
        Title
        <input bind:value={renameDraft} placeholder="session title" />
      </label>
      <div class="actions right">
        <button class="ghost" type="button" on:click={() => (renameSessionId = "")}>back</button>
        <button type="button" disabled={busy || !renameDraft.trim()} on:click={() => void renameSession()}>save</button>
      </div>
    </Modal>
  {/if}

  {#if showMods}
    <Modal title="Mods" onClose={() => (showMods = false)}>
      <div class="mods-layout">
        <nav class="mods-nav" aria-label="mods sections">
          <button class:active={modsTab === "general"} class="ghost" type="button" on:click={() => (modsTab = "general")}>general</button>
          <button class:active={modsTab === "profile"} class="ghost" type="button" on:click={() => (modsTab = "profile")}>profile</button>
          <button class:active={modsTab === "models"} class="ghost" type="button" on:click={() => (modsTab = "models")}>models</button>
          <button class:active={modsTab === "agents"} class="ghost" type="button" on:click={() => (modsTab = "agents")}>agents</button>
          <button class:active={modsTab === "subagents"} class="ghost" type="button" on:click={() => (modsTab = "subagents")}>subagents</button>
        </nav>

        <section class="mods-content">
          {#if modsTab === "general"}
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
            <label>Subagent concurrency<input bind:value={modsDraft.subagent_max_concurrency} type="number" min="1" max="4" step="1" /></label>
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
          {:else if modsTab === "models"}
            {#if !addingModel}
              <div class="model-scroll"><ItemList items={modelItems} addTitle="+ add model" addSubtitle="OpenAI-compatible" onAdd={startAddModel} /></div>
            {:else}
              <label>Provider<SelectBox fit value={providerChoice} options={providerOptions} onChange={chooseProvider} /></label>
              {#if providerChoice === "__new__"}<label>Provider name<input bind:value={draft.provider} placeholder="openai" /></label>{/if}
              <label>API base<input bind:value={draft.api_base} placeholder="https://api.openai.com/v1" /></label>
              <label>Model<input bind:value={draft.model} placeholder="gpt-4o-mini" /></label>
              <label>Context chars<input bind:value={draft.context_chars} type="number" min="4000" max="1000000" step="1000" /></label>
              <label>API key <small>{config.has_api_key ? "saved; leave blank to keep" : "not set"}</small><input bind:value={draft.api_key} type="password" placeholder="sk-..." /></label>
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
          {:else}
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
            <ItemList items={modelItems} addTitle="+ add model" addSubtitle="OpenAI-compatible" onAdd={startAddModel} />
          </div>
          <div class="feature-list">
            <div class="side-title">features</div>
            <button class="ghost" type="button" on:click={() => void setFeature("content_search", !featureContentSearch)}>content search: {featureContentSearch ? "on" : "off"}</button>
            <button class="ghost" type="button" on:click={() => void setFeature("file_watcher", !featureFileWatcher)}>file watcher: {featureFileWatcher ? "on" : "off"}</button>
          </div>
        {:else}
          <label>
            Provider
            <SelectBox fit value={providerChoice} options={providerOptions} onChange={chooseProvider} />
          </label>
          {#if providerChoice === "__new__"}
            <label>
              Provider name
              <input bind:value={draft.provider} placeholder="openai" />
            </label>
          {/if}
          <label>
            API base
            <input bind:value={draft.api_base} placeholder="https://api.openai.com/v1" />
          </label>
          <label>
            Model
            <input bind:value={draft.model} placeholder="gpt-4o-mini" />
          </label>
          <label>
            Context chars
            <input bind:value={draft.context_chars} type="number" min="4000" max="1000000" step="1000" />
          </label>
          <label>
            API key <small>{config.has_api_key ? "saved; leave blank to keep" : "not set"}</small>
            <input bind:value={draft.api_key} type="password" placeholder="sk-..." />
          </label>
          <p class="hint">{config.config_dir || "~/.sandevistan"}</p>
          <div class="actions right">
            <button class="ghost" type="button" on:click={() => (addingModel = false)}>back</button>
            <button type="button" disabled={busy} on:click={saveConfig}>save model</button>
          </div>
        {/if}
    </Modal>
  {/if}
</main>
