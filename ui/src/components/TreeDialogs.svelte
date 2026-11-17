<script lang="ts">
  import { getContext } from "svelte";
  import Dialog from "./Dialog.svelte";
  import Button from "./ui/Button.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import type { TreeActions } from "../lib/treeActions.svelte";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    /** The questions of the tree: which node is being deleted or exported. */
    actions: TreeActions;
    workspace: Workspace;
    /** The user confirmed the deletion; the tree deletes, since it also moves the keyboard afterwards. */
    ondelete: () => void;
  }

  let { actions, workspace, ondelete }: Props = $props();

  const api = getContext<Api>("api");

  /** How many requests go with the node of the question; 0 for a request: its question does not mention it. */
  const deletingRequests = $derived(
    actions.deleting && actions.deleting.kind !== "request"
      ? workspace.subtree(actions.deleting.id).filter((row) => row.kind === "request").length
      : 0,
  );
  const exportingUnsaved = $derived(actions.exporting ? actions.unsavedTabs(actions.exporting.id).length : 0);
</script>

<!-- The questions of the tree: about a deletion, and about exporting a collection with unsaved edits. -->
{#if actions.deleting}
  <!-- While the deletion runs the question cannot be dismissed: both buttons wait, and Escape does nothing. -->
  <Dialog title={strings.tree.deleteTitle(actions.deleting.name)} onclose={() => actions.cancelDelete()}>
    {#if actions.deleting.kind !== "request"}
      <p class="contents">{strings.tree.deleteContents(deletingRequests)}</p>
    {/if}
    <div class="dialog-actions">
      <Button class="danger" busy={actions.deletingBusy} onclick={ondelete}>{strings.tree.delete}</Button>
      <Button data-initial-focus disabled={actions.deletingBusy} onclick={() => actions.cancelDelete()}>{strings.tree.cancel}</Button>
    </div>
  </Dialog>
{/if}

{#if actions.exporting}
  <Dialog title={strings.export.unsavedTitle(exportingUnsaved)} onclose={() => (actions.exporting = null)}>
    <p class="contents">{strings.export.unsavedText}</p>
    <div class="dialog-actions">
      <Button variant="primary" onclick={() => actions.confirmExport(api, true)}>{strings.export.saveAndExport}</Button>
      <Button onclick={() => actions.confirmExport(api, false)}>{strings.export.exportAsIs}</Button>
      <Button onclick={() => (actions.exporting = null)}>{strings.export.cancel}</Button>
    </div>
  </Dialog>
{/if}

<style>
  .contents {
    margin: 0;
  }

  /* The button that destroys. The class goes to the `<button>` of `Button`, hence `:global`. */
  .dialog-actions :global(.danger:not(:disabled)) {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
