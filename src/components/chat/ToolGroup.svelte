<script lang="ts">
  export type ToolMessage = { role: "tool"; content: string };
  export let tools: ToolMessage[] = [];

  let open = false;
  let openItems: Record<number, boolean> = {};

  function title(content: string) {
    return content.split("\n", 1)[0] || "tool";
  }

  function body(content: string) {
    return content.split("\n").slice(1).join("\n").trim();
  }

  function status(content: string) {
    const statuses = [...content.matchAll(/^status:\s*([^\n]+)/gm)];
    const last = statuses.at(-1)?.[1]?.trim().toLowerCase() ?? "";
    if (last.startsWith("running")) return "running";
    if (/^(failed|error)\b/.test(last)) return "failed";
    if (/^(ok|done|success|succeeded)\b/.test(last)) return "done";
    return "running";
  }

  function counts() {
    return tools.reduce(
      (acc, tool) => {
        acc[status(tool.content)] += 1;
        return acc;
      },
      { done: 0, running: 0, failed: 0 } as Record<"done" | "running" | "failed", number>,
    );
  }

  function groupStatus() {
    if (tools.some((tool) => status(tool.content) === "failed")) return "failed";
    if (tools.some((tool) => status(tool.content) === "running")) return "running";
    return "done";
  }

  function toggleItem(index: number) {
    openItems = { ...openItems, [index]: !openItems[index] };
  }

  $: count = counts();
</script>

<article class="tool-group">
  <button class="group-toggle" type="button" on:click|stopPropagation={() => (open = !open)}>
    <span class={`status-dot ${groupStatus()}`} aria-hidden="true"></span>
    <span class="summary">tools × {tools.length} ({count.done} ok | {count.running} hang | {count.failed} fail) {open ? "[-]" : "[+]"}</span>
  </button>

  {#if open}
    <div class="tool-list">
      {#each tools as tool, index}
        <section class="tool-item">
          <button class="tool-toggle" type="button" on:click|stopPropagation={() => toggleItem(index)}>
            {title(tool.content)} {openItems[index] ? "[-]" : "[+]"}<span class={`status-dot ${status(tool.content)}`} aria-hidden="true"></span>
          </button>
          {#if openItems[index]}
            <pre>{body(tool.content)}</pre>
          {/if}
        </section>
      {/each}
    </div>
  {/if}
</article>

<style>
  .tool-group {
    width: 100%;
    max-width: 100%;
    justify-self: stretch;
    min-width: 0;
    padding: 2px 0;
  }

  .group-toggle,
  .tool-toggle {
    width: 100%;
    min-height: 28px;
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: start;
    color: var(--muted);
    border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    background: color-mix(in srgb, var(--surface-soft) 88%, var(--black));
    text-align: left;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
  }

  .group-toggle:hover,
  .group-toggle:focus-visible,
  .tool-toggle:hover,
  .tool-toggle:focus-visible {
    color: var(--text);
    border-color: color-mix(in srgb, var(--border) 90%, var(--accent));
    background: var(--surface);
    outline: none;
  }

  .summary {
    min-width: 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border-radius: 999px;
    background: var(--accent);
  }

  .status-dot.done {
    background: #22c55e;
  }

  .status-dot.running {
    background: #facc15;
  }

  .status-dot.failed {
    background: var(--danger);
  }

  .tool-list {
    display: grid;
    gap: 6px;
    margin-top: 6px;
    margin-left: 10px;
    padding-left: 10px;
    border-left: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  }

  .tool-item {
    display: grid;
    gap: 5px;
    min-width: 0;
  }

  pre {
    margin: 0;
    overflow: auto;
    max-height: 300px;
    padding: 8px 10px;
    border: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    border-radius: var(--radius-sm);
    background: var(--black);
    color: var(--muted);
    font-size: 12px;
    line-height: 1.45;
    white-space: pre;
  }
</style>
