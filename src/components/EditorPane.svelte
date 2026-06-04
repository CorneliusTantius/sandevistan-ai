<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { EditorState, type Extension } from "@codemirror/state";
  import { EditorView, drawSelection, dropCursor, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers, rectangularSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { HighlightStyle, bracketMatching, indentOnInput, syntaxHighlighting } from "@codemirror/language";
  import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
  import { MergeView } from "@codemirror/merge";
  import { tags as t } from "@lezer/highlight";
  import { javascript } from "@codemirror/lang-javascript";
  import { html } from "@codemirror/lang-html";
  import { css } from "@codemirror/lang-css";
  import { json } from "@codemirror/lang-json";
  import { markdown } from "@codemirror/lang-markdown";

  export type OpenFile = {
    path: string;
    content: string;
    original: string;
    dirty: boolean;
    stale: boolean;
    mode: "edit" | "diff";
  };

  const SYNTAX_LIMIT = 200_000;
  const LARGE_FILE_LIMIT = 1_000_000;

  export let file: OpenFile;
  export let onChange: (content: string) => void = () => {};
  export let onDirtyChange: (dirty: boolean) => void = () => {};
  export let onSave: (content: string) => void = () => {};
  export let onMode: (mode: "edit" | "diff") => void = () => {};

  let editorHost: HTMLDivElement;
  let mergeHost: HTMLDivElement;
  let view: EditorView | undefined;
  let mergeView: MergeView | undefined;
  let currentContent = "";
  let localDirty = false;
  let mountedPath = "";
  let syncedContent = "";
  let lastPropContent = "";
  let mergeOpening = false;

  $: syntaxEnabled = currentContent.length <= SYNTAX_LIMIT;
  $: largeFile = currentContent.length > LARGE_FILE_LIMIT;

  $: if (view && file.path !== mountedPath) resetEditor(file.content);
  $: if (view && file.content !== lastPropContent) handlePropContentChange();
  $: if (view && file.mode === "diff") void ensureMergeView();
  $: if (view && file.mode === "edit" && mergeView) closeMergeView(true);
  $: if (view) updateDirty();

  onMount(() => {
    currentContent = file.content;
    syncedContent = file.content;
    mountedPath = file.path;
    lastPropContent = file.content;
    localDirty = file.dirty;
    view = new EditorView({ state: createState(currentContent), parent: editorHost });
  });

  onDestroy(() => {
    flushContent();
    closeMergeView(false);
    view?.destroy();
    view = undefined;
  });

  function createState(doc: string, extra: Extension[] = []) {
    return EditorState.create({ doc, extensions: editorExtensions(doc, extra) });
  }

  function editorExtensions(doc: string, extra: Extension[] = []): Extension[] {
    const syntax = doc.length <= SYNTAX_LIMIT ? languageForPath(file.path) : [];
    const undoDepth = doc.length > SYNTAX_LIMIT ? 20 : 100;
    return [
      lineNumbers(),
      highlightActiveLineGutter(),
      drawSelection(),
      dropCursor(),
      rectangularSelection(),
      highlightActiveLine(),
      indentOnInput(),
      bracketMatching(),
      search({ top: true }),
      highlightSelectionMatches(),
      history({ minDepth: undoDepth }),
      syntaxHighlighting(sandevistanHighlightStyle),
      syntax,
      keymap.of([
        { key: "Mod-s", run: () => { saveCurrent(); return true; } },
        indentWithTab,
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
      ]),
      EditorView.lineWrapping,
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return;
        if (mergeView?.a === update.view) return;
        currentContent = update.state.doc.toString();
        updateDirty();
      }),
      editorTheme,
      extra,
    ];
  }

  const sandevistanHighlightStyle = HighlightStyle.define([
    { tag: t.keyword, color: "#7aa2ff", fontWeight: "700" },
    { tag: [t.atom, t.bool, t.null], color: "#ffb86c" },
    { tag: [t.number, t.integer, t.float], color: "#ffb86c" },
    { tag: [t.string, t.special(t.string)], color: "#7ee787" },
    { tag: [t.regexp, t.escape], color: "#00adb5" },
    { tag: [t.comment, t.lineComment, t.blockComment], color: "#6b7b8c", fontStyle: "italic" },
    { tag: [t.name, t.variableName], color: "#e6edf3" },
    { tag: [t.definition(t.variableName), t.function(t.variableName)], color: "#c792ea" },
    { tag: [t.function(t.propertyName), t.propertyName], color: "#8bd5ff" },
    { tag: [t.className, t.typeName, t.namespace], color: "#ffd166" },
    { tag: [t.tagName, t.heading], color: "#00adb5", fontWeight: "700" },
    { tag: [t.attributeName, t.labelName], color: "#8bd5ff" },
    { tag: t.attributeValue, color: "#7ee787" },
    { tag: [t.operator, t.punctuation, t.separator], color: "#94a3b2" },
    { tag: [t.bracket, t.squareBracket, t.paren, t.brace], color: "#94a3b2" },
    { tag: [t.invalid, t.deleted], color: "#ff7b72" },
    { tag: t.inserted, color: "#7ee787" },
    { tag: [t.link, t.url], color: "#00adb5", textDecoration: "underline" },
    { tag: t.emphasis, fontStyle: "italic" },
    { tag: t.strong, fontWeight: "700" },
  ]);

  const editorTheme = EditorView.theme({
    "&": { height: "100%", backgroundColor: "var(--black)", color: "var(--text)" },
    ".cm-scroller": { fontFamily: "inherit", lineHeight: "1.45" },
    ".cm-content": { padding: "10px 12px", caretColor: "var(--accent)" },
    ".cm-gutters": { backgroundColor: "var(--black)", color: "#5f6f80", borderRight: "1px solid var(--panel)" },
    ".cm-activeLine": { backgroundColor: "rgba(26, 35, 45, 0.55)" },
    ".cm-activeLineGutter": { backgroundColor: "rgba(26, 35, 45, 0.75)", color: "var(--text)" },
    "&.cm-focused": { outline: "none" },
    "&.cm-focused .cm-cursor": { borderLeftColor: "var(--accent)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": { backgroundColor: "rgba(0, 173, 181, 0.35)" },
    ".cm-search": { backgroundColor: "var(--panel)", color: "var(--text)", borderTop: "1px solid var(--panel)" },
    ".cm-search input": { width: "auto", backgroundColor: "var(--surface)", color: "var(--text)", border: "1px solid var(--panel)" },
    ".cm-search button": { minHeight: "24px", padding: "0 8px", color: "var(--text)", border: "1px solid var(--panel)", backgroundColor: "var(--surface)" },
    ".cm-tooltip": { backgroundColor: "var(--panel)", color: "var(--text)", border: "1px solid var(--surface)" },
  });

  function readonlyExtensions(): Extension[] {
    return [EditorState.readOnly.of(true), EditorView.editable.of(false)];
  }

  function languageForPath(path: string): Extension {
    const lower = path.toLowerCase();
    if (/\.(js|jsx|ts|tsx|mjs|cjs)$/.test(lower)) return javascript({ typescript: /\.(ts|tsx)$/.test(lower), jsx: /\.(jsx|tsx)$/.test(lower) });
    if (/\.(html|svelte|xml|svg)$/.test(lower)) return html();
    if (/\.(css|scss|sass|less)$/.test(lower)) return css();
    if (/\.(json|jsonc)$/.test(lower)) return json();
    if (/\.(md|markdown)$/.test(lower)) return markdown();
    return [];
  }

  async function ensureMergeView() {
    if (mergeView || mergeOpening) return;
    mergeOpening = true;
    await tick();
    mergeOpening = false;
    if (!mergeHost || file.mode !== "diff" || mergeView) return;
    const doc = currentDoc();
    currentContent = doc;
    mergeView = new MergeView({
      a: { doc: file.original, extensions: editorExtensions(file.original, readonlyExtensions()) },
      b: { doc, extensions: editorExtensions(doc) },
      parent: mergeHost,
      orientation: "a-b",
      revertControls: "a-to-b",
      highlightChanges: true,
      gutter: true,
      collapseUnchanged: { margin: 3, minSize: 8 },
    });
  }

  function closeMergeView(commit: boolean) {
    if (!mergeView) return;
    if (commit) {
      const doc = mergeView.b.state.doc.toString();
      currentContent = doc;
      if (view && view.state.doc.toString() !== doc) {
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: doc } });
      }
    }
    mergeView.destroy();
    mergeView = undefined;
  }

  function currentDoc() {
    return mergeView?.b.state.doc.toString() ?? view?.state.doc.toString() ?? currentContent;
  }

  function resetEditor(content: string) {
    closeMergeView(false);
    currentContent = content;
    syncedContent = content;
    lastPropContent = content;
    mountedPath = file.path;
    localDirty = content !== file.original;
    view?.setState(createState(content));
    onDirtyChange(localDirty);
  }

  function handlePropContentChange() {
    const content = file.content;
    lastPropContent = content;
    if (localDirty) return;
    replaceDoc(content);
  }

  function replaceDoc(content: string) {
    if (!view) return;
    closeMergeView(false);
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: content } });
    currentContent = content;
    syncedContent = content;
    updateDirty();
  }

  function updateDirty() {
    const dirty = currentContent !== file.original;
    if (dirty === localDirty) return;
    localDirty = dirty;
    onDirtyChange(dirty);
  }

  function flushContent() {
    const doc = currentDoc();
    currentContent = doc;
    updateDirty();
    if (doc === syncedContent) return;
    syncedContent = doc;
    onChange(doc);
  }

  function saveCurrent() {
    flushContent();
    onSave(currentContent);
  }

  function chooseMode(mode: "edit" | "diff") {
    flushContent();
    onMode(mode);
  }
</script>

<div class="editor-pane">
  <div class="editor-toolbar">
    <span>{file.path}{localDirty ? " *" : ""}{file.stale ? " !" : ""}</span>
    <div class="status">
      {#if !syntaxEnabled}<span>syntax off</span>{/if}
      {#if largeFile}<span>large file</span>{/if}
    </div>
    <div class="actions">
      <button class:active={file.mode === "edit"} type="button" on:click={() => chooseMode("edit")}>edit</button>
      <button class:active={file.mode === "diff"} type="button" on:click={() => chooseMode("diff")}>merge</button>
      <button type="button" disabled={!localDirty} on:click={saveCurrent}>save</button>
    </div>
  </div>

  <div class="edit-wrap" class:hidden={file.mode !== "edit"} bind:this={editorHost} on:focusout={flushContent}></div>
  <div class="merge-wrap" class:hidden={file.mode !== "diff"} bind:this={mergeHost} on:focusout={flushContent}></div>
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--panel);
    color: var(--muted);
    font-size: 12px;
    background: var(--panel);
  }

  .editor-toolbar > span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status {
    display: flex;
    gap: 6px;
    color: var(--muted);
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

  .edit-wrap,
  .merge-wrap {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .hidden {
    display: none;
  }

  .merge-wrap :global(.cm-mergeView) {
    height: 100%;
    overflow: auto;
    background: var(--black);
  }

  .merge-wrap :global(.cm-mergeViewEditors) {
    height: 100%;
  }

  .merge-wrap :global(.cm-mergeView .cm-editor) {
    min-width: 0;
  }

  .merge-wrap :global(.cm-changedLine) {
    background: rgba(0, 173, 181, 0.1);
  }

  .merge-wrap :global(.cm-deletedChunk) {
    background: rgba(232, 69, 69, 0.12);
  }

  .merge-wrap :global(.cm-insertedLine) {
    background: rgba(0, 173, 181, 0.14);
  }
</style>
