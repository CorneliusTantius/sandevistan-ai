<script lang="ts">
  export let checked = false;
  export let label = "";
  export let disabled = false;
  export let onChange: (checked: boolean) => void = () => {};
</script>

<button class:checked class="check" type="button" {disabled} on:click={() => onChange(!checked)}>
  <span class="box">{checked ? "✓" : ""}</span>
  <span>{label}</span>
</button>

<style>
  .check {
    min-height: 32px;
    display: flex;
    align-items: center;
    justify-content: start;
    gap: 8px;
    color: var(--text);
    border: 1px solid var(--panel);
    background: var(--surface);
    text-align: left;
  }

  .box {
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    color: var(--black);
    border: 1px solid var(--accent);
    background: var(--accent);
    font-size: 12px;
    font-weight: 900;
  }

  .check:not(.checked) .box {
    color: transparent;
    border-color: var(--muted);
    background: transparent;
  }
</style>
