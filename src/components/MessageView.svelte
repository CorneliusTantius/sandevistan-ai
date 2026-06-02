<script lang="ts">
  export type Role = "user" | "assistant" | "tool" | "error";
  export let role: Role;
  export let content = "";

  type Block =
    | { kind: "code"; lang: string; text: string }
    | { kind: "heading"; text: string }
    | { kind: "list"; items: string[] }
    | { kind: "table"; headers: string[]; rows: string[][] }
    | { kind: "text"; text: string };

  let toolLevel = 0;
  $: renderedContent = renderedOnly(content);
  $: blocks = parseMarkdown(renderedContent);
  $: toolTitle = content.split("\n", 1)[0] || "tool";
  $: toolBody = content.split("\n").slice(1).join("\n").trim();
  $: toolFailed = /^status:\s*failed\b/m.test(content);

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

      if (isTableStart(lines, i)) {
        const headers = splitTableRow(lines[i]);
        const rows: string[][] = [];
        i += 2;
        while (i < lines.length && isTableRow(lines[i])) rows.push(splitTableRow(lines[i++]));
        blocks.push({ kind: "table", headers, rows });
        continue;
      }

      const text: string[] = [];
      while (i < lines.length && lines[i].trim() && !lines[i].startsWith("```") && !lines[i].startsWith("#") && !/^\s*[-*]\s+/.test(lines[i]) && !isTableStart(lines, i)) {
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
  {:else}
    <div class="markdown">
      {#each blocks as block}
        {#if block.kind === "code"}
          <pre class="code"><code>{block.text}</code></pre>
        {:else if block.kind === "heading"}
          <h3>{block.text}</h3>
        {:else if block.kind === "list"}
          <ul>{#each block.items as item}<li>{item}</li>{/each}</ul>
        {:else if block.kind === "table"}
          <div class="table-wrap">
            <table>
              <thead><tr>{#each block.headers as header}<th>{header}</th>{/each}</tr></thead>
              <tbody>
                {#each block.rows as row}
                  <tr>{#each block.headers as _, index}<td>{row[index] ?? ""}</td>{/each}</tr>
                {/each}
              </tbody>
            </table>
          </div>
        {:else}
          <p>{block.text}</p>
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
  ul {
    margin: 0;
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
