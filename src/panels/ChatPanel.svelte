<script lang="ts">
  import { tick } from "svelte";
  import MessageView from "../components/chat/MessageView.svelte";
  import ToolGroup from "../components/chat/ToolGroup.svelte";
  import { bindElement } from "../lib/bindElement";
  import type { FileEntry, Message, MessageGroup } from "../types";

  type PromptShortcut = { name: string; template: string };

  export let messagesEl: HTMLDivElement;
  export let setMessagesEl: (element: HTMLDivElement) => void = () => {};
  export let hiddenMessageGroupCount = 0;
  export let visibleMessageGroups: MessageGroup[] = [];
  export let showEarlierMessages: () => void = () => {};
  export let isStreamingMessage: (group: MessageGroup) => boolean = () => false;
  export let contextUsed = 0;
  export let contextLimit = 1;
  export let contextPercent = 0;
  export let transcriptUsed = 0;
  export let inputTokens = 0;
  export let outputTokens = 0;
  export let formatContext: (value: number) => string = String;
  export let activeSessionRunning = false;
  export let compacting = false;
  export let promptEl: HTMLTextAreaElement;
  export let setPromptEl: (element: HTMLTextAreaElement) => void = () => {};
  export let mentionResults: FileEntry[] = [];
  export let mentionIndex = 0;
  export let insertMention: (entry: FileEntry) => void = () => {};
  export let shortcutResults: PromptShortcut[] = [];
  export let shortcutIndex = 0;
  export let insertShortcut: (entry: PromptShortcut) => void = () => {};
  export let prompt = "";
  export let rememberPromptSnapshot: (event: Event) => void = () => {};
  export let inputPrompt: (event: Event) => void = () => {};
  export let keydown: (event: KeyboardEvent) => void = () => {};
  export let sendPrompt: () => void = () => {};
  export let busy = false;
  export let messages: Message[] = [];
  export let compactSession: () => void = () => {};
  export let cancelPrompt: () => void = () => {};
  export let sessionKey = "";

  let showJumpLatest = false;
  let wasNearBottom = true;
  let lastGroupCount = -1;
  let lastTailSignature = "";
  let lastSessionKey = "";
  let pendingOpenScroll = false;
  let scrollToken = 0;
  let latestEl: HTMLDivElement;

  function isNearBottom() {
    if (!messagesEl) return true;
    return messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight < 180;
  }

  $: tailSignature = visibleMessageGroups.at(-1)?.key ?? "";

  $: if (messagesEl && sessionKey !== lastSessionKey) {
    lastSessionKey = sessionKey;
    lastGroupCount = visibleMessageGroups.length;
    lastTailSignature = tailSignature;
    wasNearBottom = true;
    showJumpLatest = false;
    pendingOpenScroll = true;
    void scrollLatestSoon(true, 12);
  }

  $: if (messagesEl && (visibleMessageGroups.length !== lastGroupCount || tailSignature !== lastTailSignature)) {
    const grew = visibleMessageGroups.length > lastGroupCount || tailSignature !== lastTailSignature;
    const shouldPin = pendingOpenScroll || (grew && wasNearBottom);
    lastGroupCount = visibleMessageGroups.length;
    lastTailSignature = tailSignature;
    if (shouldPin) void scrollLatestSoon(pendingOpenScroll, pendingOpenScroll ? 12 : 2);
  }

  function forceScrollLatest() {
    if (!messagesEl) return;
    messagesEl.scrollTop = messagesEl.scrollHeight;
    latestEl?.scrollIntoView({ block: "end" });
  }

  async function scrollLatestSoon(force = false, frames = 1) {
    const token = ++scrollToken;
    await tick();
    const scroll = (remaining: number) => {
      requestAnimationFrame(() => {
        if (token !== scrollToken || !messagesEl || (!force && !wasNearBottom)) return;
        forceScrollLatest();
        updateScrollState();
        if (remaining > 1) scroll(remaining - 1);
        else if (force) pendingOpenScroll = false;
      });
    };
    scroll(frames);
  }

  function updateScrollState() {
    if (!messagesEl) return;
    wasNearBottom = isNearBottom();
    showJumpLatest = !wasNearBottom;
  }

  async function jumpLatest() {
    if (!messagesEl) return;
    forceScrollLatest();
    updateScrollState();
  }
</script>

<section class="chat" aria-label="AI chat">
  <div class="chat-main">
    <div class="messages-wrap">
      <div class="messages" bind:this={messagesEl} use:bindElement={setMessagesEl} on:scroll={updateScrollState}>
        {#if visibleMessageGroups.length === 0}
          <div class="chat-empty">
            <strong>Start coding with Sandevistan</strong>
            <span>Ask for a fix, open a file, or mention @file for context.</span>
          </div>
        {/if}
        {#if hiddenMessageGroupCount > 0}
          <button class="ghost show-earlier" type="button" on:click={showEarlierMessages}>show {Math.min(80, hiddenMessageGroupCount)} earlier ({hiddenMessageGroupCount} hidden)</button>
        {/if}
        {#each visibleMessageGroups as group (group.key)}
          {#if group.kind === "message"}
            <MessageView role={group.message.role} content={group.message.content} streaming={isStreamingMessage(group)} />
          {:else}
            <ToolGroup tools={group.tools} />
          {/if}
        {/each}
        <div class="latest-anchor" bind:this={latestEl} aria-hidden="true"></div>
      </div>
      {#if showJumpLatest}
        <button class="ghost jump-latest" type="button" on:click={jumpLatest}>jump to latest</button>
      {/if}
    </div>
    <aside class="stats-card" aria-label="Chat stats">
      <div class="stats-head">
        <span>session</span>
        <strong>{contextPercent}%</strong>
      </div>
      <div class="context-bar"><span style={`width:${contextPercent}%`}></span></div>
      <div class="stat-list">
        <div class="stat-row"><span>context</span><strong>{formatContext(contextUsed)} / {formatContext(contextLimit)}</strong></div>
        <div class="stat-row"><span>transcript</span><strong>{formatContext(transcriptUsed)}</strong></div>
        <div class="stat-row"><span>tokens in</span><strong>{formatContext(inputTokens)}</strong></div>
        <div class="stat-row"><span>tokens out</span><strong>{formatContext(outputTokens)}</strong></div>
      </div>
    </aside>
  </div>

  {#if activeSessionRunning}
    <div class="running-status" role="status" aria-live="polite"><span class="run-dot" aria-hidden="true"></span>{compacting ? "compacting..." : "running..."}</div>
  {/if}
  <form class="prompt-form" on:submit|preventDefault={sendPrompt}>
    {#if shortcutResults.length}
      <div class="mention-menu">
        {#each shortcutResults as entry, index (entry.name)}
          <button class:active={index === shortcutIndex} type="button" on:mousedown|preventDefault={() => insertShortcut(entry)}>
            <span>!{entry.name}</span>
          </button>
        {/each}
      </div>
    {:else if mentionResults.length}
      <div class="mention-menu">
        {#each mentionResults as entry, index (entry.path)}
          <button class:active={index === mentionIndex} type="button" on:mousedown|preventDefault={() => insertMention(entry)}>
            <span>{entry.path}</span>
          </button>
        {/each}
      </div>
    {/if}
    <textarea bind:this={promptEl} use:bindElement={setPromptEl} value={prompt} on:beforeinput={rememberPromptSnapshot} on:input={inputPrompt} on:keydown={keydown} rows="4" placeholder="message · @file · Enter = send · Shift+Enter = newline" autocomplete="off"></textarea>
    <button type="submit" disabled={busy || activeSessionRunning || !prompt.trim()}>send</button>
    <button class="ghost" type="button" disabled={busy || activeSessionRunning || messages.length < 2} on:click={compactSession}>compact</button>
    <button class="ghost danger" type="button" disabled={!activeSessionRunning || compacting} on:click={cancelPrompt}>abort</button>
  </form>
</section>
