<script lang="ts">
  export type OpenFile = {
    path: string;
    content: string;
    original: string;
    dirty: boolean;
    stale: boolean;
    mode: "edit" | "diff";
  };

  type TextSnapshot = { value: string; selectionStart: number; selectionEnd: number };

  export let file: OpenFile;
  export let onChange: (content: string) => void = () => {};
  export let onSave: () => void = () => {};
  export let onMode: (mode: "edit" | "diff") => void = () => {};

  let undoStack: TextSnapshot[] = [];
  let redoStack: TextSnapshot[] = [];
  let historyPath = "";
  let editorEl: HTMLTextAreaElement;
  let findInputEl: HTMLInputElement;
  let findOpen = false;
  let findQuery = "";
  let findIndex = -1;
  let findCount = 0;

  $: diff = makeDiff(file.original, file.content);
  $: if (file.path !== historyPath) {
    historyPath = file.path;
    undoStack = [];
    redoStack = [];
    findOpen = false;
    findQuery = "";
    findIndex = -1;
    findCount = 0;
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

  function restoreSnapshot(target: HTMLTextAreaElement, snapshot: TextSnapshot) {
    target.value = snapshot.value;
    onChange(snapshot.value);
    requestAnimationFrame(() => target.setSelectionRange(snapshot.selectionStart, snapshot.selectionEnd));
  }

  function rememberSnapshot(event: Event) {
    pushTextSnapshot(undoStack, textSnapshot(event.currentTarget as HTMLTextAreaElement));
    redoStack = [];
  }

  function input(event: Event) {
    onChange((event.currentTarget as HTMLTextAreaElement).value);
  }

  function handleUndoRedo(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return false;

    const key = event.key.toLowerCase();
    const undo = key === "z" && !event.shiftKey;
    const redo = key === "y" || (key === "z" && event.shiftKey);
    if (!undo && !redo) return false;

    const target = event.currentTarget as HTMLTextAreaElement;
    const from = undo ? undoStack : redoStack;
    const to = undo ? redoStack : undoStack;
    const snapshot = from.pop();
    if (!snapshot) return true;

    event.preventDefault();
    pushTextSnapshot(to, textSnapshot(target));
    restoreSnapshot(target, snapshot);
    return true;
  }

  function keydown(event: KeyboardEvent) {
    if (handleUndoRedo(event)) return;

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      openFind();
      return;
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      onSave();
    }

    if (event.key === "Tab") {
      event.preventDefault();
      const target = event.currentTarget as HTMLTextAreaElement;
      pushTextSnapshot(undoStack, textSnapshot(target));
      redoStack = [];
      const start = target.selectionStart;
      const end = target.selectionEnd;
      const next = file.content.slice(0, start) + "  " + file.content.slice(end);
      onChange(next);
      requestAnimationFrame(() => {
        target.selectionStart = target.selectionEnd = start + 2;
      });
    }
  }

  function openFind() {
    findOpen = true;
    findQuery = editorEl?.value.slice(editorEl.selectionStart, editorEl.selectionEnd) || findQuery;
    updateFindCount();
    requestAnimationFrame(() => findInputEl?.focus());
  }

  function closeFind() {
    findOpen = false;
    requestAnimationFrame(() => editorEl?.focus());
  }

  function updateFindCount() {
    if (!findQuery) {
      findCount = 0;
      findIndex = -1;
      return;
    }
    const text = file.content.toLowerCase();
    const query = findQuery.toLowerCase();
    let count = 0;
    let index = text.indexOf(query);
    while (index >= 0) {
      count += 1;
      index = text.indexOf(query, index + Math.max(query.length, 1));
    }
    findCount = count;
  }

  function findNext(reverse = false) {
    updateFindCount();
    if (!findQuery || !editorEl) return;
    const text = file.content.toLowerCase();
    const query = findQuery.toLowerCase();
    const cursor = reverse ? editorEl.selectionStart - 1 : editorEl.selectionEnd;
    let index = reverse ? text.lastIndexOf(query, Math.max(cursor, 0)) : text.indexOf(query, cursor);
    if (index < 0) index = reverse ? text.lastIndexOf(query) : text.indexOf(query);
    if (index < 0) return;
    findIndex = index;
    requestAnimationFrame(() => {
      editorEl.focus();
      editorEl.setSelectionRange(index, index + findQuery.length);
      const line = file.content.slice(0, index).split("\n").length;
      editorEl.scrollTop = Math.max(0, (line - 4) * 21);
    });
  }

  function findKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeFind();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      findNext(event.shiftKey);
    }
  }

  function makeDiff(oldText: string, newText: string) {
    const oldLines = oldText.split("\n");
    const newLines = newText.split("\n");
    const max = Math.max(oldLines.length, newLines.length);
    const lines: { kind: "same" | "add" | "del"; text: string }[] = [];

    for (let i = 0; i < max; i++) {
      const oldLine = oldLines[i];
      const newLine = newLines[i];
      if (oldLine === newLine) lines.push({ kind: "same", text: `  ${oldLine ?? ""}` });
      else {
        if (oldLine !== undefined) lines.push({ kind: "del", text: `- ${oldLine}` });
        if (newLine !== undefined) lines.push({ kind: "add", text: `+ ${newLine}` });
      }
    }
    return lines;
  }
</script>

<div class="editor-pane">
  <div class="editor-toolbar">
    <span>{file.path}{file.dirty ? " *" : ""}{file.stale ? " !" : ""}</span>
    <div class="actions">
      <button class:active={file.mode === "edit"} type="button" on:click={() => onMode("edit")}>edit</button>
      <button class:active={file.mode === "diff"} type="button" on:click={() => onMode("diff")}>diff</button>
      <button type="button" disabled={!file.dirty} on:click={onSave}>save</button>
    </div>
  </div>

  {#if file.mode === "edit"}
    <div class="edit-wrap" class:with-find={findOpen}>
      {#if findOpen}
        <div class="findbar">
          <input bind:this={findInputEl} bind:value={findQuery} placeholder="find" on:input={updateFindCount} on:keydown={findKeydown} />
          <span>{findCount ? `${findCount} match${findCount === 1 ? "" : "es"}` : "no matches"}</span>
          <button type="button" on:click={() => findNext(true)}>↑</button>
          <button type="button" on:click={() => findNext(false)}>↓</button>
          <button type="button" on:click={closeFind}>×</button>
        </div>
      {/if}
      <textarea bind:this={editorEl} class="editor" value={file.content} spellcheck="false" on:beforeinput={rememberSnapshot} on:input={input} on:keydown={keydown}></textarea>
    </div>
  {:else}
    <pre class="diff">{#each diff as line}<span class={line.kind}>{line.text}</span>{"\n"}{/each}</pre>
  {/if}
</div>

<style>
  .editor-pane {
    height: 100%;
    min-height: 0;
    overflow: hidden;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    border: 1px solid var(--panel);
    background: var(--black);
  }

  .editor-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--panel);
    color: var(--muted);
    font-size: 12px;
    background: var(--panel);
  }

  .actions {
    display: flex;
    gap: 6px;
  }

  button {
    min-height: 28px;
    padding: 0 10px;
  }

  button.active {
    color: var(--text);
    background: var(--surface);
  }

  .edit-wrap {
    height: 100%;
    min-height: 0;
    overflow: hidden;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
  }

  .edit-wrap.with-find {
    grid-template-rows: auto minmax(0, 1fr);
  }

  .findbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto auto auto;
    align-items: center;
    gap: 6px;
    padding: 6px;
    border-bottom: 1px solid var(--panel);
    background: var(--panel);
  }

  .findbar input {
    min-height: 28px;
    padding: 4px 8px;
  }

  .findbar span {
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
  }

  .editor {
    width: 100%;
    height: 100%;
    min-height: 0;
    max-height: none;
    overflow: auto;
    resize: none;
    border: 0;
    border-left: 0;
    background: var(--black);
    line-height: 1.45;
    tab-size: 2;
  }

  .diff {
    min-height: 0;
    margin: 0;
    overflow: auto;
    padding: 10px 12px;
    color: var(--text);
    background: var(--black);
    line-height: 1.45;
    white-space: pre-wrap;
  }

  .diff span {
    display: block;
  }

  .add {
    color: var(--accent);
  }

  .del {
    color: var(--danger);
  }

  .same {
    color: var(--muted);
  }
</style>
