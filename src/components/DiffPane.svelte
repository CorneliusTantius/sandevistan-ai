<script lang="ts">
  export type DiffTab = {
    id: string;
    title: string;
    path: string;
    diff: string;
  };

  export let tab: DiffTab;

  type DiffLine = { kind: "file" | "hunk" | "add" | "del" | "same" | "meta"; text: string };

  $: lines = parseDiff(tab.diff);

  function parseDiff(diff: string): DiffLine[] {
    if (!diff.trim()) return [{ kind: "meta", text: "no diff" }];
    return diff.split("\n").map((text) => {
      if (text.startsWith("diff --git") || text.startsWith("--- ") || text.startsWith("+++ ")) return { kind: "file", text };
      if (text.startsWith("@@")) return { kind: "hunk", text };
      if (text.startsWith("+") && !text.startsWith("+++")) return { kind: "add", text };
      if (text.startsWith("-") && !text.startsWith("---")) return { kind: "del", text };
      if (text.startsWith(" ")) return { kind: "same", text };
      return { kind: "meta", text };
    });
  }
</script>

<div class="diff-pane">
  <div class="diff-toolbar">
    <span>{tab.path || "workspace diff"}</span>
    <span>{lines.length} lines</span>
  </div>
  <pre class="git-diff">{#each lines as line}<span class={line.kind}>{line.text || " "}</span>{"\n"}{/each}</pre>
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

  .git-diff {
    min-height: 0;
    overflow: auto;
    margin: 0;
    padding: 10px 0;
    background: var(--black);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    line-height: 1.45;
    tab-size: 2;
  }

  .git-diff span {
    display: block;
    min-height: 1.45em;
    padding: 0 12px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .file {
    color: var(--accent);
    background: var(--bg);
    font-weight: 900;
  }

  .hunk {
    color: var(--text);
    background: var(--surface);
    font-weight: 700;
  }

  .add {
    color: #7ee787;
    background: rgba(0, 173, 181, 0.12);
  }

  .del {
    color: #ff7b72;
    background: rgba(232, 69, 69, 0.12);
  }

  .same {
    color: var(--muted);
  }

  .meta {
    color: var(--text);
  }
</style>
