<script lang="ts">
  import MessageView from "../components/chat/MessageView.svelte";
  import ToolGroup from "../components/chat/ToolGroup.svelte";
  import { bindElement } from "../lib/bindElement";
  import type { FileEntry, Message, MessageGroup } from "../types";

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
  export let prompt = "";
  export let rememberPromptSnapshot: (event: Event) => void = () => {};
  export let inputPrompt: (event: Event) => void = () => {};
  export let keydown: (event: KeyboardEvent) => void = () => {};
  export let sendPrompt: () => void = () => {};
  export let busy = false;
  export let messages: Message[] = [];
  export let compactSession: () => void = () => {};
  export let cancelPrompt: () => void = () => {};

  let showJumpLatest = false;

  function updateScrollState() {
    if (!messagesEl) return;
    showJumpLatest = messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight > 160;
  }

  async function jumpLatest() {
    if (!messagesEl) return;
    messagesEl.scrollTop = messagesEl.scrollHeight;
    updateScrollState();
  }
</script>

<section class="chat" aria-label="AI chat">
  <div class="chat-main">
    <div class="messages-wrap">
    <div class="messages" bind:this={messagesEl} use:bindElement={setMessagesEl} on:scroll={updateScrollState}>
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
    </div>
    {#if showJumpLatest}
      <button class="ghost jump-latest" type="button" on:click={jumpLatest}>jump to latest</button>
    {/if}
    </div>
    <aside class="stats-card" aria-label="Chat stats">
      <div class="side-title">stats</div>
      <div class="stat-row"><span>context</span><strong>{formatContext(contextUsed)} / {formatContext(contextLimit)}</strong></div>
      <div class="context-bar"><span style={`width:${contextPercent}%`}></span></div>
      <small>{contextPercent}% used · transcript {formatContext(transcriptUsed)}</small>
      <div class="stat-row"><span>in / out tokens</span><strong>{formatContext(inputTokens)} | {formatContext(outputTokens)}</strong></div>
    </aside>
  </div>

  {#if activeSessionRunning}
    <div class="running-status" role="status" aria-live="polite"><span class="run-dot" aria-hidden="true"></span>{compacting ? "compacting..." : "running..."}</div>
  {/if}
  <form class="prompt-form" on:submit|preventDefault={sendPrompt}>
    {#if mentionResults.length}
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
