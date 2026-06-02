<script lang="ts">
  export type FileEntry = {
    name: string;
    path: string;
    kind: "dir" | "file";
    depth: number;
  };

  export let entries: FileEntry[] = [];
  export let expandedPaths: string[] = [];
  export let onOpen: (entry: FileEntry) => void = () => {};

  $: expanded = new Set(expandedPaths.map(normalize));

  function normalize(path: string) {
    return path.replace(/\\/g, "/");
  }

  function isExpanded(entry: FileEntry) {
    return expanded.has(normalize(entry.path));
  }

  function select(entry: FileEntry) {
    onOpen(entry);
  }
</script>

<div class="file-tree">
  {#each entries as entry (entry.path)}
    <button class:open={isExpanded(entry)} class="file-row" type="button" style={`--depth:${entry.depth}`} aria-expanded={entry.kind === "dir" ? isExpanded(entry) : undefined} on:click={() => select(entry)}>
      <span class="kind">{entry.kind === "dir" ? (isExpanded(entry) ? "▾" : "▸") : "·"}</span>
      <span class="name">{entry.name}</span>
    </button>
  {/each}
  {#if entries.length === 0}
    <div class="empty">empty</div>
  {/if}
</div>

<style>
  .file-tree {
    display: grid;
    align-content: start;
    gap: 1px;
  }

  .file-row {
    min-height: 26px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 6px;
    padding: 3px 8px 3px calc(8px + var(--depth) * 12px);
    color: var(--text);
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .file-row.open {
    background: var(--surface);
  }

  .file-row:hover,
  .file-row:focus-visible {
    color: var(--text);
    background: var(--surface);
    outline: none;
  }

  .kind {
    color: var(--muted);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    color: var(--muted);
    font-size: 12px;
  }
</style>
