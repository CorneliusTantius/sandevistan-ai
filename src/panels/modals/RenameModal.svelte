<script lang="ts">
  import Modal from "../../components/ui/Modal.svelte";

  export let open = false;
  export let title = "Rename";
  export let value = "";
  export let busy = false;
  export let onClose: () => void = () => {};
  export let onValueChange: (value: string) => void = () => {};
  export let onSave: () => void = () => {};
</script>

{#if open}
  <Modal {title} onClose={onClose}>
    <label>
      Title
      <input value={value} placeholder="session title" on:input={(event) => onValueChange((event.currentTarget as HTMLInputElement).value)} />
    </label>
    <div class="actions right">
      <button class="ghost" type="button" on:click={onClose}>back</button>
      <button type="button" disabled={busy || !value.trim()} on:click={onSave}>save</button>
    </div>
  </Modal>
{/if}
