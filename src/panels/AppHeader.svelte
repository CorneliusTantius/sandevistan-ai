<script lang="ts">
  import SelectBox, { type SelectOption } from "../components/ui/SelectBox.svelte";
  import type { AiConfig } from "../types";

  export let appVersion = "dev";
  export let config: AiConfig;
  export let topProfileOptions: SelectOption[] = [];
  export let switchProfile: (value: string) => void = () => {};
  export let openMods: () => void = () => {};
  export let openTerminal: () => void = () => {};
  export let closeWindow: () => void = () => {};
</script>

<header class="topbar" data-tauri-drag-region>
  <div class="brand-mark">
    <pre class="ascii" aria-label="sandevistan">███████╗ █████╗ ███╗   ██╗██████╗ ███████╗██╗   ██╗██╗███████╗████████╗ █████╗ ███╗   ██╗         █████╗ ██╗
██╔════╝██╔══██╗████╗  ██║██╔══██╗██╔════╝██║   ██║██║██╔════╝╚══██╔══╝██╔══██╗████╗  ██║        ██╔══██╗██║
███████╗███████║██╔██╗ ██║██║  ██║█████╗  ██║   ██║██║███████╗   ██║   ███████║██╔██╗ ██║        ███████║██║
╚════██║██╔══██║██║╚██╗██║██║  ██║██╔══╝  ╚██╗ ██╔╝██║╚════██║   ██║   ██╔══██║██║╚██╗██║        ██╔══██║██║
███████║██║  ██║██║ ╚████║██████╔╝███████╗ ╚████╔╝ ██║███████║   ██║   ██║  ██║██║ ╚████║███████╗██║  ██║██║
╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚═════╝ ╚══════╝  ╚═══╝  ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝╚═╝  ╚═╝╚═╝</pre>
    <span class="app-version">v{appVersion}</span>
  </div>
  <div class="header-actions">
    <div class="top-profile"><SelectBox value={config.active_profile} options={topProfileOptions} onChange={(value) => switchProfile(value)} /></div>
    <button class="ghost" type="button" on:click={openMods}>mods</button>
    <button class="ghost" type="button" on:click={() => openTerminal()}>term</button>
    <button class="window-close" type="button" aria-label="close" on:click={closeWindow}>×</button>
  </div>
</header>
