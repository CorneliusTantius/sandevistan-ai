<script lang="ts">
  import Modal from "../../components/ui/Modal.svelte";
  import ItemList, { type Item } from "../../components/ui/ItemList.svelte";

  export let open = false;
  export let adding = false;
  export let items: Item[] = [];
  export let draft = "";
  export let busy = false;
  export let onClose: () => void = () => {};
  export let onStartAdd: () => void = () => {};
  export let onBack: () => void = () => {};
  export let onBrowse: () => void = () => {};
  export let onDraftChange: (value: string) => void = () => {};
  export let onSave: (path: string) => void = () => {};
</script>

{#if open}
  <Modal title="Workspace" onClose={onClose}>
    {#if !adding}
      <ItemList items={items} addTitle="+ add workspace" addSubtitle="directory" onAdd={onStartAdd} />
    {:else}
      <label>
        Path
        <div class="inline-row">
          <input value={draft} placeholder="~/code/project" on:input={(event) => onDraftChange((event.currentTarget as HTMLInputElement).value)} />
          <button class="ghost compact" type="button" on:click={onBrowse}>browse</button>
        </div>
      </label>
      <div class="actions right">
        <button class="ghost" type="button" on:click={onBack}>back</button>
        <button type="button" disabled={busy || !draft.trim()} on:click={() => onSave(draft)}>save workspace</button>
      </div>
    {/if}
  </Modal>
{/if}
