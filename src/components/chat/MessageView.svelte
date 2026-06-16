<script lang="ts">
  export type Role = "user" | "assistant" | "tool" | "error";
  export let role: Role;
  export let content = "";
  export let streaming = false;
  const LINE_LIMIT = 15;
  let expanded = false;
  let copied = false;
  let toolLevel = 0;

  type Block =
    | { kind: "code"; lang: string; text: string }
    | { kind: "heading"; text: string }
    | { kind: "list"; items: string[] }
    | { kind: "table"; headers: string[]; rows: string[][] }
    | { kind: "hr" }
    | { kind: "text"; text: string };

  $: renderedContent = renderedOnly(content);
  $: renderedLines = renderedContent.split("\n");
  $: isLong = role === "assistant" && !streaming && renderedLines.length > LINE_LIMIT;
  $: visibleContent = isLong && !expanded ? renderedLines.slice(0, LINE_LIMIT).join("\n") : renderedContent;
  $: blocks = parseMarkdown(visibleContent);
  $: toolTitle = content.split("\n", 1)[0] || "tool";
  $: toolBody = content.split("\n").slice(1).join("\n").trim();
  $: toolStatus = [...content.matchAll(/^status:\s*([^\n]+)/gm)].at(-1)?.[1]?.trim().toLowerCase() ?? "";
  $: toolFailed = /^(failed|error)\b/.test(toolStatus);

  function cycleTool() {
    toolLevel = (toolLevel + 1) % 3;
  }

  function renderedOnly(markdown: string) {
    const lines = markdown.split("\n");
    const rendered = lines.findIndex((line) => marker(line) === "rendered");
    const raw = lines.findIndex((line, index) => index > Math.max(rendered, -1) && marker(line) === "raw");

    if (rendered >= 0) return lines.slice(rendered + 1, raw >= 0 ? raw : lines.length).join("\n").trim();
    if (raw >= 0) return lines.slice(0, raw).join("\n").trim();
    return markdown;
  }

  function marker(line: string) {
    const value = line
      .trim()
      .replace(/^#{1,6}\s*/, "")
      .replace(/^\*{1,2}|\*{1,2}$/g, "")
      .replace(/^_{1,2}|_{1,2}$/g, "")
      .trim()
      .toLowerCase();
    if (/^rendered\s*:?$/.test(value)) return "rendered";
    if (/^raw\s*:?$/.test(value)) return "raw";
    return "";
  }

  function parseMarkdown(markdown: string): Block[] {
    const lines = markdown.split("\n");
    const blocks: Block[] = [];
    let i = 0;

    while (i < lines.length) {
      const line = lines[i];
      if (!line.trim()) {
        i++;
        continue;
      }

      const fence = readFence(line);
      if (fence) {
        const code: string[] = [];
        i++;
        while (i < lines.length) {
          const close = readFence(lines[i]);
          if (close && close.char === fence.char && close.length >= fence.length && !close.info) break;
          code.push(lines[i++]);
        }
        if (i < lines.length) i++;
        blocks.push({ kind: "code", lang: fence.info, text: code.join("\n") });
        continue;
      }

      if (line.startsWith("#")) {
        blocks.push({ kind: "heading", text: line.replace(/^#+\s*/, "") });
        i++;
        continue;
      }

      if (/^\s*[-*]\s+/.test(line)) {
        const items: string[] = [];
        while (i < lines.length && /^\s*[-*]\s+/.test(lines[i])) {
          items.push(lines[i++].replace(/^\s*[-*]\s+/, ""));
        }
        blocks.push({ kind: "list", items });
        continue;
      }

      if (/^\s*-{3,}\s*$/.test(line)) {
        blocks.push({ kind: "hr" });
        i++;
        continue;
      }

      if (isTableStart(lines, i)) {
        const headers = splitTableRow(lines[i]);
        const rows: string[][] = [];
        i += 2;
        while (i < lines.length && isTableRow(lines[i])) rows.push(splitTableRow(lines[i++]));
        blocks.push({ kind: "table", headers, rows });
        continue;
      }

      const text: string[] = [];
      while (i < lines.length && lines[i].trim() && !readFence(lines[i]) && !lines[i].startsWith("#") && !/^\s*-{3,}\s*$/.test(lines[i]) && !/^\s*[-*]\s+/.test(lines[i]) && !isTableStart(lines, i)) {
        text.push(lines[i++]);
      }
      blocks.push({ kind: "text", text: text.join("\n") });
    }

    return blocks;
  }

  function readFence(line: string) {
    const match = /^( {0,3})(`{3,}|~{3,})([^`~]*)$/.exec(line);
    if (!match) return null;
    return { char: match[2][0], length: match[2].length, info: match[3].trim() };
  }

  function isTableRow(line = "") {
    return line.includes("|") && splitTableRow(line).length > 1;
  }

  function isTableStart(lines: string[], index: number) {
    return isTableRow(lines[index]) && /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(lines[index + 1] ?? "");
  }

  function splitTableRow(line: string) {
    return line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((cell) => cell.trim());
  }

  function inlineMarkdown(value: string) {
    return escapeHtml(value)
      .replace(/`([^`]+)`/g, '<code class="inline-code">$1</code>')
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/\*([^*]+)\*/g, "<em>$1</em>")
      .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  }

  function highlightedCode(value: string) {
    return escapeHtml(value);
  }

  function escapeHtml(value: string) {
    return value
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  async function copyText(value: string) {
    await navigator.clipboard?.writeText(value);
    copied = true;
    window.setTimeout(() => (copied = false), 900);
  }

  function toggleExpand() {
    if (isLong) expanded = !expanded;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
<article class={`message ${role} ${toolFailed ? "tool-failed" : ""}`} class:collapsible={isLong} on:click={toggleExpand}>
  <div class="message-head">
    <span>{role}</span>
    <div class="message-actions">
      {#if isLong}
        <button class="mini message-expand" type="button" on:click|stopPropagation={toggleExpand}>{expanded ? "collapse" : `expand (${renderedLines.length} lines)`}</button>
      {/if}
      {#if role !== "tool"}
        <button class="mini message-copy" type="button" on:click|stopPropagation={() => void copyText(renderedContent)}>{copied ? "copied" : "copy"}</button>
      {/if}
    </div>
  </div>
  <div class="message-body">
    {#if role === "tool"}
      <button class="tool-toggle" class:tool-failed={toolFailed} type="button" on:click={cycleTool}>{toolTitle}{toolFailed ? " [failed]" : ""} {toolLevel === 0 ? "[+]" : toolLevel === 1 ? "[++ ]" : "[-]"}</button>
      {#if toolLevel === 1}
        <pre class="tool-body">{toolBody.slice(0, 700)}{toolBody.length > 700 ? "\n..." : ""}</pre>
      {:else if toolLevel === 2}
        <pre class="tool-body">{toolBody}</pre>
      {/if}
    {:else}
      <div class="markdown">
        {#each blocks as block}
          {#if block.kind === "code"}
            <div class="code-wrap"><button class="mini code-copy" type="button" on:click|stopPropagation={() => void copyText(block.text)}>copy</button><pre class="code"><code>{@html highlightedCode(block.text)}</code></pre></div>
          {:else if block.kind === "heading"}
            <h3>{@html inlineMarkdown(block.text)}</h3>
          {:else if block.kind === "list"}
            <ul>{#each block.items as item}<li>{@html inlineMarkdown(item)}</li>{/each}</ul>
          {:else if block.kind === "table"}
            <div class="table-wrap">
              <table>
                <thead><tr>{#each block.headers as header}<th>{@html inlineMarkdown(header)}</th>{/each}</tr></thead>
                <tbody>
                  {#each block.rows as row}
                    <tr>{#each block.headers as _, index}<td>{@html inlineMarkdown(row[index] ?? "")}</td>{/each}</tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {:else if block.kind === "hr"}
            <hr />
          {:else}
            <p>{@html inlineMarkdown(block.text)}</p>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
  <!-- collapse action lives in message header -->
</article>

<style>
  .message {
    width: 100%;
    max-width: 100%;
    justify-self: stretch;
    min-width: 0;
    display: grid;
    gap: 0;
    padding: 0;
    border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--panel) 62%, transparent);
    box-shadow: 0 1px 0 rgb(0 0 0 / 0.18);
  }

  .message-head {
    min-height: 22px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    background: color-mix(in srgb, var(--surface) 44%, transparent);
    border-radius: var(--radius) var(--radius) 0 0;
  }

  .message-body {
    min-width: 0;
    padding: 10px 12px;
  }

  .message.collapsible {
    cursor: pointer;
  }

  .message.collapsible .message-body:hover {
    background: color-mix(in srgb, var(--surface) 18%, transparent);
  }

  .message-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .message span {
    display: block;
    color: var(--muted);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .assistant {
    justify-self: stretch;
    border-color: color-mix(in srgb, var(--assistant) 28%, var(--border));
  }

  .assistant .message-head {
    background: color-mix(in srgb, var(--surface) 56%, var(--assistant) 8%);
  }

  .assistant span {
    color: color-mix(in srgb, var(--assistant) 82%, var(--muted));
  }

  .user {
    justify-self: stretch;
    border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
    background: color-mix(in srgb, var(--surface) 70%, var(--accent) 5%);
  }

  .user .message-head {
    background: color-mix(in srgb, var(--surface) 64%, var(--accent) 8%);
  }

  .user span {
    color: color-mix(in srgb, var(--accent) 82%, var(--muted));
  }

  .tool {
    color: var(--muted);
    border-color: var(--panel);
    background: color-mix(in srgb, var(--black) 82%, var(--panel));
  }

  .tool span {
    color: var(--muted);
  }

  .tool-failed {
    border-color: var(--danger);
  }

  .error {
    border-color: var(--danger);
    background: var(--bg);
  }

  .error span,
  :global(.error p) {
    color: var(--danger);
  }

  .markdown {
    display: grid;
    gap: 10px;
  }

  :global(.markdown p),
  :global(.markdown h3),
  :global(.markdown h4),
  :global(.markdown h5),
  :global(.markdown h6),
  :global(.markdown ul),
  :global(.markdown ol),
  :global(.markdown hr) {
    margin: 0;
  }

  :global(.markdown hr) {
    width: 100%;
    border: 0;
    border-top: 1px solid var(--border);
  }

  :global(.markdown table) {
    width: 100%;
    border-collapse: collapse;
    color: var(--text);
    font-size: 13px;
  }

  :global(.markdown th),
  :global(.markdown td) {
    padding: 6px 8px;
    border: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }

  :global(.markdown th) {
    color: var(--text);
    background: var(--surface);
    font-weight: 700;
  }

  :global(.markdown h3),
  :global(.markdown h4),
  :global(.markdown h5),
  :global(.markdown h6) {
    color: var(--text);
    font-size: 14px;
  }

  :global(.markdown ul),
  :global(.markdown ol) {
    padding-left: 18px;
  }

  :global(.markdown p),
  :global(.markdown li) {
    color: var(--text);
    white-space: pre-wrap;
  }

  :global(.inline-code) {
    padding: 1px 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--black);
    color: var(--muted);
    font: inherit;
  }

  .mini {
    min-height: 22px;
    align-self: center;
    padding: 0 7px;
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--surface) 82%, var(--black));
    font-size: 11px;
    line-height: 1;
  }

  .message-copy,
  .message-expand {
    flex: 0 0 auto;
    min-width: 0;
    min-height: 20px;
    opacity: 0.75;
  }

  .message-copy:hover,
  .message-copy:focus-visible {
    opacity: 1;
  }

  :global(.code),
  .tool-body {
    margin: 0;
    overflow: auto;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--black);
    color: var(--muted);
    white-space: pre;
  }

  .tool-toggle {
    width: 100%;
    justify-content: start;
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--black) 88%, var(--panel));
    text-align: left;
    cursor: pointer;
  }

  .tool-toggle.tool-failed {
    color: var(--danger);
    border-color: var(--danger);
  }
</style>
