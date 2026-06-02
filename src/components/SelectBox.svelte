<script lang="ts">
  import { tick } from "svelte";

  export type SelectOption = { value: string; label: string };

  export let value = "";
  export let options: SelectOption[] = [];
  export let onChange: (value: string) => void = () => {};

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

<div class="selectbox" on:focusout={blur}>
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
  }

  .trigger {
    width: 100%;
    color: var(--text);
    border: 1px solid var(--panel);
    background: var(--surface);
    text-align: left;
    cursor: pointer;
  }

  .menu {
    position: absolute;
    z-index: 10;
    top: 100%;
    left: 0;
    right: 0;
    display: grid;
    border: 1px solid var(--panel);
    border-top: 0;
    background: var(--bg);
  }

  .menu button {
    justify-content: start;
    color: var(--text);
    border: 0;
    background: var(--bg);
    text-align: left;
    cursor: pointer;
  }

  .menu button.active {
    background: var(--surface);
  }
</style>
