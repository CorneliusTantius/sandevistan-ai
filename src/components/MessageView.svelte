<script lang="ts">
  export type Role = "user" | "assistant" | "tool" | "error";
  export let role: Role;
  export let content = "";
  export let streaming = false;

  type Block =
    | { kind: "code"; lang: string; text: string }
    | { kind: "heading"; text: string }
    | { kind: "list"; items: string[] }
    | { kind: "table"; headers: string[]; rows: string[][] }
    | { kind: "hr" }
    | { kind: "text"; text: string };

  let toolLevel = 0;
  $: renderedContent = streaming ? content : renderedOnly(content);
  $: blocks = streaming ? [] : parseMarkdown(renderedContent);
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

      if (line.startsWith("```")) {
        const lang = line.slice(3).trim();
        const code: string[] = [];
        i++;
        while (i < lines.length && !lines[i].startsWith("```")) code.push(lines[i++]);
        if (i < lines.length) i++;
        blocks.push({ kind: "code", lang, text: code.join("\n") });
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
      while (i < lines.length && lines[i].trim() && !lines[i].startsWith("```") && !lines[i].startsWith("#") && !/^\s*-{3,}\s*$/.test(lines[i]) && !/^\s*[-*]\s+/.test(lines[i]) && !isTableStart(lines, i)) {
        text.push(lines[i++]);
      }
      blocks.push({ kind: "text", text: text.join("\n") });
    }

    return blocks;
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

  function escapeHtml(value: string) {
    return value
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function inlineMarkdown(value: string) {
    return escapeHtml(value)
      .replace(/`([^`\n]+)`/g, '<code class="inline-code">$1</code>')
      .replace(/==([^=\n]+)==/g, "<mark>$1</mark>")
      .replace(/\*\*([^*\n]+)\*\*/g, "<strong>$1</strong>");
  }

  function highlightedCode(value: string, lang: string) {
    const html = escapeHtml(value);
    const normalized = lang.toLowerCase();
    if (/^(js|jsx|ts|tsx|svelte|json)$/.test(normalized)) {
      return html
        .replace(/(&quot;.*?&quot;|'.*?'|`.*?`)/g, '<span class="tok-string">$1</span>')
        .replace(/\b(const|let|var|function|return|if|else|for|while|class|type|interface|import|from|export|async|await|try|catch|throw|new|true|false|null|undefined)\b/g, '<span class="tok-keyword">$1</span>')
        .replace(/\b(\d+(?:\.\d+)?)\b/g, '<span class="tok-number">$1</span>')
        .replace(/(\/\/.*)$/gm, '<span class="tok-comment">$1</span>');
    }
    if (/^(sh|bash|zsh|shell)$/.test(normalized)) {
      return html
        .replace(/(#.*)$/gm, '<span class="tok-comment">$1</span>')
        .replace(/\b(cd|ls|cat|grep|rg|npm|git|cargo|sudo|echo|export)\b/g, '<span class="tok-keyword">$1</span>')
        .replace(/(--?[\w-]+)/g, '<span class="tok-number">$1</span>');
    }
    return html;
  }
</script>

<article class={`message ${role} ${toolFailed ? "tool-failed" : ""}`}>
  <span>{role}</span>
  {#if role === "tool"}
    <button class="tool-toggle" class:tool-failed={toolFailed} type="button" on:click={cycleTool}>{toolTitle}{toolFailed ? " [failed]" : ""} {toolLevel === 0 ? "[+]" : toolLevel === 1 ? "[++ ]" : "[-]"}</button>
    {#if toolLevel === 1}
      <pre class="tool-body">{toolBody.slice(0, 700)}{toolBody.length > 700 ? "\n..." : ""}</pre>
    {:else if toolLevel === 2}
      <pre class="tool-body">{toolBody}</pre>
    {/if}
  {:else if streaming}
    <pre class="streaming-text">{renderedContent}</pre>
  {:else}
    <div class="markdown">
      {#each blocks as block}
        {#if block.kind === "code"}
          <pre class="code"><code>{@html highlightedCode(block.text, block.lang)}</code></pre>
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
</article>

<style>
  .message {
    width: 100%;
    min-width: 0;
    padding: 10px 12px;
    border: 1px solid var(--panel);
    background: var(--bg);
  }

  .message span {
    display: block;
    margin-bottom: 5px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .assistant {
    border-color: var(--assistant);
    background: var(--bg);
  }

  .assistant span {
    color: var(--assistant);
  }

  .user {
    border-color: var(--alt);
    background: var(--surface);
  }

  .user span {
    color: var(--alt);
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
  .error p {
    color: var(--danger);
  }

  .markdown {
    display: grid;
    gap: 10px;
  }

  p,
  h3,
  ul,
  hr {
    margin: 0;
  }

  hr {
    width: 100%;
    border: 0;
    border-top: 1px solid var(--panel);
  }

  .table-wrap {
    overflow: auto;
    border: 1px solid var(--panel);
    background: var(--black);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    color: var(--text);
    font-size: 13px;
  }

  th,
  td {
    padding: 6px 8px;
    border: 1px solid var(--panel);
    text-align: left;
    vertical-align: top;
  }

  th {
    color: var(--text);
    background: var(--surface);
    font-weight: 700;
  }

  h3 {
    color: var(--text);
    font-size: 14px;
  }

  ul {
    padding-left: 18px;
  }

  p,
  li {
    color: var(--text);
    white-space: pre-wrap;
  }

  :global(.inline-code) {
    padding: 1px 4px;
    border: 1px solid var(--panel);
    background: var(--black);
    color: var(--muted);
    font: inherit;
  }

  :global(mark) {
    padding: 0 3px;
    background: color-mix(in srgb, var(--assistant) 24%, transparent);
    color: var(--text);
  }

  :global(.tok-keyword) {
    color: var(--assistant);
    font-weight: 700;
  }

  :global(.tok-string) {
    color: var(--alt);
  }

  :global(.tok-number) {
    color: var(--muted);
  }

  :global(.tok-comment) {
    color: color-mix(in srgb, var(--muted) 72%, transparent);
    font-style: italic;
  }

  .streaming-text {
    margin: 0;
    color: var(--text);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font: inherit;
  }

  .code,
  .tool-body {
    margin: 0;
    overflow: auto;
    padding: 8px;
    border: 1px solid var(--panel);
    background: var(--black);
    color: var(--muted);
    white-space: pre;
  }

  .tool-toggle {
    width: 100%;
    justify-content: start;
    color: var(--muted);
    border: 1px solid var(--panel);
    background: color-mix(in srgb, var(--black) 88%, var(--panel));
    text-align: left;
    cursor: pointer;
  }

  .tool-toggle.tool-failed {
    color: var(--danger);
    border-color: var(--danger);
  }
</style>
