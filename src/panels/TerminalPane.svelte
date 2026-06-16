<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";

  export let id = "main";

  let host: HTMLDivElement;
  let term: { open: (el: HTMLElement) => void; onData: (cb: (data: string) => void) => { dispose: () => void }; write: (data: string) => void; resize: (cols: number, rows: number) => void; dispose: () => void } | undefined;
  let inputDisposable: { dispose: () => void } | undefined;
  let unlisten: UnlistenFn | undefined;
  let resizeObserver: ResizeObserver | undefined;
  let resizeTimer = 0;
  let terminalStarted = false;
  let destroyed = false;

  onMount(async () => {
    const [{ Terminal }] = await Promise.all([
      import("@xterm/xterm"),
      import("@xterm/xterm/css/xterm.css"),
    ]);

    if (destroyed) return;

    term = new Terminal({
      cursorBlink: false,
      convertEol: true,
      fontFamily: '"Open Sans Mono", "DejaVu Sans Mono", "Liberation Mono", "Courier New", monospace',
      fontSize: 13,
      rows: 28,
      cols: 100,
      theme: {
        background: "#0D1117",
        foreground: "#EEEEEE",
        cursor: "#00ADB5",
        selectionBackground: "#393E46",
        black: "#111820",
        red: "#E84545",
        green: "#00ADB5",
        yellow: "#EEEEEE",
        blue: "#00ADB5",
        magenta: "#00ADB5",
        cyan: "#00ADB5",
        white: "#EEEEEE",
        brightBlack: "#222831",
        brightRed: "#E84545",
        brightGreen: "#00ADB5",
        brightYellow: "#F6F6F6",
        brightBlue: "#00ADB5",
        brightMagenta: "#00ADB5",
        brightCyan: "#00ADB5",
        brightWhite: "#F6F6F6",
      },
    });
    term.open(host);
    inputDisposable?.dispose();
    inputDisposable = term.onData((data) => {
      if (terminalStarted) void invoke("terminal_write", { request: { id, data } });
    });
    unlisten = await listen<{ id: string; data: string }>("terminal-output", (event) => {
      if (!destroyed && event.payload.id === id) term?.write(event.payload.data);
    });
    if (destroyed) return;

    const size = terminalSize();
    term.resize(size.cols, size.rows);
    await invoke("terminal_stop", { request: { id } });
    if (destroyed) return;
    await invoke("terminal_start", { request: { id, cols: size.cols, rows: size.rows } });
    if (destroyed) return;
    terminalStarted = true;

    if (destroyed) return;
    const resize = () => resizeTerminal();
    resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(host);
    requestAnimationFrame(resize);
  });

  function terminalSize() {
    return {
      cols: Math.max(20, Math.floor((host?.clientWidth ?? 816) / 8)),
      rows: Math.max(6, Math.floor((host?.clientHeight ?? 520) / 18)),
    };
  }

  function resizeTerminal() {
    if (!term || !host || !terminalStarted || resizeTimer) return;
    resizeTimer = requestAnimationFrame(() => {
      resizeTimer = 0;
      if (!term || !host || !terminalStarted) return;
      const { cols, rows } = terminalSize();
      term.resize(cols, rows);
      void invoke("terminal_resize", { request: { id, cols, rows } });
    });
  }

  onDestroy(() => {
    destroyed = true;
    terminalStarted = false;
    inputDisposable?.dispose();
    inputDisposable = undefined;
    resizeObserver?.disconnect();
    if (resizeTimer) cancelAnimationFrame(resizeTimer);
    resizeTimer = 0;
    resizeObserver = undefined;
    if (unlisten) unlisten();
    unlisten = undefined;
    void invoke("terminal_stop", { request: { id } });
    term?.dispose();
    term = undefined;
  });
</script>

<div class="terminal-host" bind:this={host}></div>

<style>
  .terminal-host {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    padding: 8px;
    border: 0;
    background: var(--surface);
  }

  :global(.xterm) {
    width: 100%;
    height: 100%;
  }

  :global(.xterm-screen),
  :global(.xterm-rows) {
    width: 100% !important;
    height: 100% !important;
  }

  :global(.xterm-viewport) {
    background: var(--surface) !important;
  }
</style>
