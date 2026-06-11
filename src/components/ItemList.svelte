<script lang="ts">
  export type ItemAction = { label: string; danger?: boolean; onClick: () => void };
  export type Item = {
    key: string;
    title: string;
    subtitle: string;
    active?: boolean;
    onSelect: () => void;
    actions?: ItemAction[];
  };

  export let items: Item[] = [];
  export let addTitle = "";
  export let addSubtitle = "";
  export let onAdd: () => void = () => {};
</script>

<div class="item-list">
  {#each items as item (item.key)}
    <div class:active={item.active} class="item-row">
      <button class="main" type="button" on:click={item.onSelect}>
        <strong>{item.title}</strong>
        <span>{item.subtitle}</span>
      </button>
      {#if item.actions?.length}
        <div class="actions">
          {#each item.actions as action}
            <button class:danger={action.danger} type="button" on:click={action.onClick}>{action.label}</button>
          {/each}
        </div>
      {/if}
    </div>
  {/each}
  <button class="add" type="button" on:click={onAdd}>
    <strong>{addTitle}</strong>
    <span>{addSubtitle}</span>
  </button>
</div>

<style>
  .item-list {
    display: grid;
    gap: 6px;
  }

  .item-row {
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
    overflow: hidden;
  }

  .item-row.active {
    border-color: var(--accent);
    background: var(--bg);
  }

  button {
    height: auto;
    display: grid;
    justify-items: start;
    justify-content: stretch;
    gap: 3px;
    text-align: left;
    padding: 9px 10px;
    color: var(--text);
    border: 0;
    background: transparent;
    cursor: pointer;
  }

  .main {
    min-width: 0;
    width: 100%;
  }

  .actions {
    display: grid;
    gap: 0;
    border-left: 1px solid var(--panel);
  }

  .actions button + button {
    border-top: 1px solid var(--panel);
  }

  .item-row.active .actions,
  .item-row.active .actions button + button {
    border-color: var(--panel);
  }

  button:hover,
  button:focus-visible {
    color: var(--text);
    background: var(--bg);
    outline: none;
  }

  .add {
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface);
  }

  button.danger {
    color: var(--danger);
  }

  button.danger:hover,
  button.danger:focus-visible {
    color: var(--white);
    background: var(--danger);
  }

  strong,
  span {
    max-width: 100%;
    min-width: 0;
    text-align: left;
  }

  span {
    overflow: hidden;
    color: var(--muted);
    font-size: 12px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
