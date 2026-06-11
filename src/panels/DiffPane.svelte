<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { EditorState, RangeSetBuilder, StateField } from "@codemirror/state";
  import { Decoration, type DecorationSet, EditorView, highlightActiveLineGutter, keymap, lineNumbers } from "@codemirror/view";
  import { defaultKeymap } from "@codemirror/commands";
  import { search, searchKeymap } from "@codemirror/search";

  export type DiffTab = {
    id: string;
    title: string;
    path: string;
    diff: string;
  };

  export let tab: DiffTab;

  let host: HTMLDivElement;
  let view: EditorView | undefined;
  let mountedId = "";

  $: lineCount = tab.diff.trim() ? tab.diff.split("\n").length : 1;
  $: if (view && tab.id !== mountedId) resetView();
  $: if (view && tab.id === mountedId && view.state.doc.toString() !== diffText()) resetView();

  onMount(() => resetView());

  onDestroy(() => {
    view?.destroy();
    view = undefined;
  });

  function diffText() {
    return tab.diff.trim() ? tab.diff : "no diff";
  }

  function resetView() {
    mountedId = tab.id;
    const state = EditorState.create({ doc: diffText(), extensions: extensions() });
    if (view) view.setState(state);
    else view = new EditorView({ state, parent: host });
  }

  function extensions() {
    return [
      EditorState.readOnly.of(true),
      EditorView.editable.of(false),
      lineNumbers(),
      highlightActiveLineGutter(),
      search({ top: true }),
      keymap.of([...defaultKeymap, ...searchKeymap]),
      EditorView.lineWrapping,
      diffLineClasses,
      EditorView.theme({
        "&": { height: "100%", backgroundColor: "var(--black)", color: "var(--text)", fontSize: "13px" },
        ".cm-scroller": { fontFamily: "inherit", lineHeight: "1.45" },
        ".cm-content": { padding: "10px 0" },
        ".cm-line": { padding: "0 12px" },
        ".cm-gutters": { backgroundColor: "var(--black)", color: "var(--muted)", borderRight: "1px solid var(--panel)" },
        ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "var(--surface)" },
        "&.cm-focused": { outline: "none" },
        "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": { backgroundColor: "rgba(0, 173, 181, 0.35)" },
        ".cm-search": { backgroundColor: "var(--panel)", color: "var(--text)", borderTop: "1px solid var(--panel)" },
        ".cm-search input": { width: "auto", backgroundColor: "var(--surface)", color: "var(--text)", border: "1px solid var(--panel)" },
        ".cm-search button": { minHeight: "24px", padding: "0 8px", color: "var(--text)", border: "1px solid var(--panel)", backgroundColor: "var(--surface)" },
        ".cm-tooltip": { backgroundColor: "var(--panel)", color: "var(--text)", border: "1px solid var(--surface)" },
      }),
    ];
  }

  const diffLineClasses = StateField.define<DecorationSet>({
    create(state) {
      return buildLineClasses(state);
    },
    update(value, transaction) {
      if (!transaction.docChanged) return value.map(transaction.changes);
      return buildLineClasses(transaction.state);
    },
    provide(field) {
      return EditorView.decorations.from(field);
    },
  });

  function buildLineClasses(state: EditorState) {
    const builder = new RangeSetBuilder<Decoration>();
    for (let lineNumber = 1; lineNumber <= state.doc.lines; lineNumber += 1) {
      const line = state.doc.line(lineNumber);
      builder.add(line.from, line.from, Decoration.line({ class: classForLine(line.text) }));
    }
    return builder.finish();
  }

  function classForLine(text: string) {
    if (text.startsWith("diff --git") || text.startsWith("--- ") || text.startsWith("+++ ")) return "cm-diff-file";
    if (text.startsWith("@@")) return "cm-diff-hunk";
    if (text.startsWith("+") && !text.startsWith("+++")) return "cm-diff-add";
    if (text.startsWith("-") && !text.startsWith("---")) return "cm-diff-del";
    if (text.startsWith(" ")) return "cm-diff-same";
    return "cm-diff-meta";
  }
</script>

<div class="diff-pane">
  <div class="diff-toolbar">
    <span>{tab.path || "workspace diff"}</span>
    <span>{lineCount} lines</span>
  </div>
  <div class="diff-editor" bind:this={host}></div>
</div>

<style>
  .diff-pane {
    height: 100%;
    min-height: 0;
    overflow: hidden;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    border: 1px solid var(--panel);
    background: var(--black);
  }

  .diff-toolbar {
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

  .diff-editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .diff-editor :global(.cm-diff-file) {
    color: var(--accent);
    background: var(--bg);
    font-weight: 900;
  }

  .diff-editor :global(.cm-diff-hunk) {
    color: var(--text);
    background: var(--surface);
    font-weight: 700;
  }

  .diff-editor :global(.cm-diff-add) {
    color: #7ee787;
    background: rgba(0, 173, 181, 0.12);
  }

  .diff-editor :global(.cm-diff-del) {
    color: #ff7b72;
    background: rgba(232, 69, 69, 0.12);
  }

  .diff-editor :global(.cm-diff-same) {
    color: var(--muted);
  }

  .diff-editor :global(.cm-diff-meta) {
    color: var(--text);
  }
</style>
