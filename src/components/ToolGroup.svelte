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

  function toggleItem(index: number) {
    openItems = { ...openItems, [index]: !openItems[index] };
  }
</script>

<article class="tool-group">
  <button class="group-toggle" type="button" on:click|stopPropagation={() => (open = !open)}>
    tools × {tools.length} {open ? "[-]" : "[+]"}
  </button>

  {#if open}
    <div class="tool-list">
      {#each tools as tool, index}
        <section class="tool-item">
          <button class="tool-toggle" type="button" on:click|stopPropagation={() => toggleItem(index)}>
            {title(tool.content)} {openItems[index] ? "[-]" : "[+]"}
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
    min-width: 0;
    padding: 10px 12px;
    border: 1px solid var(--panel);
    background: color-mix(in srgb, var(--black) 82%, var(--panel));
  }

  .group-toggle,
  .tool-toggle {
    width: 100%;
    justify-content: start;
    color: var(--muted);
    border: 1px solid var(--panel);
    background: color-mix(in srgb, var(--black) 88%, var(--panel));
    text-align: left;
    cursor: pointer;
  }

  .tool-list {
    display: grid;
    gap: 8px;
    margin-top: 8px;
  }

  .tool-item {
    display: grid;
    gap: 6px;
  }

  pre {
    margin: 0;
    overflow: auto;
    max-height: 320px;
    padding: 8px;
    border: 1px solid var(--panel);
    background: color-mix(in srgb, var(--black) 92%, var(--panel));
    color: var(--muted);
    white-space: pre;
  }
</style>
