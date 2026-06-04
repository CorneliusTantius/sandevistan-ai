<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  export let title = "";
  export let onClose: () => void = () => {};

  function keydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    onClose();
  }

  onMount(() => window.addEventListener("keydown", keydown));
  onDestroy(() => window.removeEventListener("keydown", keydown));
</script>

<div class="backdrop" role="presentation" on:click={onClose}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
    <header>
      <h2>{title}</h2>
      <button class="ghost" type="button" on:click={onClose}>close</button>
    </header>
    <slot />
  </div>
</div>

<style>
  .backdrop {
    position: fixed !important;
    z-index: 1000;
    inset: 0;
    display: grid;
    place-items: center;
    width: 100vw;
    height: 100vh;
    padding: 14px;
    background: rgba(17, 24, 32, 0.88);
  }

  .modal {
    width: min(760px, 100%);
    max-height: calc(100vh - 28px);
    overflow: auto;
    display: grid;
    gap: 12px;
    padding: 14px;
    color: var(--text);
    border: 1px solid var(--panel);
    background: var(--panel);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--panel);
  }
</style>
