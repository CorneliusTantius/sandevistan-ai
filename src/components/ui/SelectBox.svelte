<script lang="ts">
  import { tick } from "svelte";

  export type SelectOption = { value: string; label: string };

  export let value = "";
  export let options: SelectOption[] = [];
  export let onChange: (value: string) => void = () => {};
  export let fit = false;

  let open = false;
  $: label = options.find((option) => option.value === value)?.label ?? value;

  async function choose(next: string) {
    open = false;
    await tick();
    onChange(next);
  }

  function blur(event: FocusEvent) {
    const next = event.relatedTarget as Node | null;
    if (!next || !event.currentTarget || !(event.currentTarget as HTMLElement).contains(next)) {
      open = false;
    }
  }
</script>

<div class:fit class="selectbox" on:focusout={blur}>
  <button class="trigger" type="button" on:click|stopPropagation={() => (open = !open)}>{label}</button>
  {#if open}
    <div class="menu">
      {#each options as option}
        <button class:active={option.value === value} type="button" on:mousedown|preventDefault|stopPropagation={() => choose(option.value)}>{option.label}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .selectbox {
    position: relative;
    width: max-content;
    min-width: max-content;
  }

  .trigger {
    width: max-content;
    min-width: max-content;
    white-space: nowrap;
    color: var(--muted);
    border: 1px solid transparent;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .menu {
    position: absolute;
    z-index: 1001;
    top: 100%;
    left: 0;
    right: auto;
    min-width: 100%;
    display: grid;
    padding: 4px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: 0 12px 30px rgb(0 0 0 / 0.28);
  }

  .selectbox.fit,
  .selectbox.fit .trigger {
    width: 100%;
    min-width: 0;
  }

  .selectbox.fit .menu {
    right: 0;
  }

  .menu button {
    justify-content: start;
    color: var(--muted);
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .menu button.active,
  .menu button:hover,
  .menu button:focus-visible {
    color: var(--text);
    background: var(--surface-soft);
  }
</style>
