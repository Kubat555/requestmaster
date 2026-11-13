<script lang="ts">
  import { getContext, tick } from "svelte";
  import Dialog from "./Dialog.svelte";
  import TreeMenu, { type TreeAction } from "./TreeMenu.svelte";
  import TreeRow from "./TreeRow.svelte";
  import Button from "./ui/Button.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import { tabs } from "../lib/tabs.svelte";
  import { buildTree, visibleRows, type TreeNode } from "../lib/tree";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  const api = getContext<Api>("api");

  /** Where a menu appears: under an element, or at a point of the window. */
  type Anchor = HTMLElement | { x: number; y: number };

  let container = $state<HTMLDivElement>();
  /** The menu of a row on the screen; `null` — none. */
  let menu = $state.raw<{ node: TreeNode; anchor: Anchor } | null>(null);
  /** The node of the menu shown last; stays after the menu is closed. */
  let menuNode: TreeNode | null = null;
  /** The node the user asked to delete, until the question is answered; `null` — no question on the screen. */
  let deleting = $state.raw<TreeNode | null>(null);
  /** How many requests go with the node of the question; 0 for a request: its question does not mention it. */
  const deletingRequests = $derived(
    deleting && deleting.kind !== "request"
      ? workspace.subtree(deleting.id).filter((row) => row.kind === "request").length
      : 0,
  );

  const tree = $derived(buildTree(workspace.rows));
  const rows = $derived(visibleRows(tree, workspace.expanded, workspace.filter));
  const filtering = $derived(workspace.filter.trim() !== "");
  /** The row Tab stops at: the last focused one while it is shown, otherwise the first. */
  const tabbableId = $derived(
    rows.some((row) => row.node.id === workspace.focusedId) ? workspace.focusedId : (rows[0]?.node.id ?? null),
  );

  /** Ids of the nodes whose open tab has unsaved edits: their rows are marked. */
  const dirtyIds = $derived(new Set(tabs.list.filter((tab) => tab.dirty).map((tab) => tab.nodeId)));

  /** Whether the children of the row at `index` are shown. A filter shows them whatever is expanded. */
  function isExpanded(index: number): boolean {
    const { node, depth } = rows[index];
    if (node.kind === "request") return false;
    return filtering ? (rows[index + 1]?.depth ?? 0) > depth : workspace.expanded.has(node.id);
  }

  /** A request and a collection are opened; a folder has nothing to open, so it expands or collapses. */
  function activate(node: TreeNode) {
    if (node.kind === "folder") {
      toggle(node);
      return;
    }
    workspace.select(node.id);
    workspace.onOpen(node.id);
  }

  /** Does nothing under a filter: the filter decides what is shown. */
  function toggle(node: TreeNode) {
    if (!filtering) workspace.toggle(node.id);
  }

  async function focusRow(index: number) {
    if (index < 0 || index >= rows.length) return;
    await tick();
    container?.querySelectorAll<HTMLElement>('[role="treeitem"]')[index]?.focus();
  }

  /** Moves the keyboard to the row of the node, if it is shown. */
  async function focusNode(id: string) {
    await tick();
    await focusRow(rows.findIndex((row) => row.node.id === id));
  }

  /**
   * Shows the menu of the row. A second request for the menu that is already shown closes it when it comes
   * from the same button, and is ignored otherwise: the key that opens the menu may be followed by a
   * `contextmenu` event of its own.
   */
  function openMenu(node: TreeNode, anchor: Anchor) {
    if (menu?.node.id === node.id) {
      if (menu.anchor === anchor) menu = null;
      return;
    }
    menu = { node, anchor };
    menuNode = node;
  }

  /**
   * An item of the menu was chosen. The action starts after the menu is gone and the focus is back where it
   * was: a field or a dialog that appears takes the focus from there, not from the menu being removed.
   */
  function onAction(action: TreeAction) {
    // The menu is closed before its item acts, so the node is taken from what was shown last.
    const node = menuNode;
    if (node) void tick().then(() => run(node, action));
  }

  async function run(node: TreeNode, action: TreeAction) {
    switch (action) {
      case "new_request":
        await workspace.createNode(api, node.id, "request");
        break;
      case "new_folder":
        await workspace.createNode(api, node.id, "folder");
        break;
      case "rename":
        workspace.renamingId = node.id;
        break;
      case "duplicate": {
        const copyId = await workspace.duplicateNode(api, node.id);
        if (copyId !== null) await focusNode(copyId);
        break;
      }
      case "delete":
        deleting = node;
        break;
    }
  }

  /**
   * The field for the name is closed. A changed name is saved, and the open tab of the node shows it at once;
   * its draft is not touched. After Enter and Escape the keyboard goes back to the row.
   */
  async function onRenamed(node: TreeNode, name: string | null, byKey: boolean) {
    workspace.renamingId = null;
    if (byKey) void focusNode(node.id);
    if (name === null) return;
    const saved = await workspace.renameNode(api, node.id, name);
    if (saved !== null) tabs.rename(node.id, saved);
  }

  /**
   * The user confirmed the deletion. The tabs of everything deleted are closed without a question about their
   * unsaved edits — there is nothing left to save them to; closing stops their sends and releases their
   * responses. The keyboard goes to the row that took the place of the deleted one.
   */
  async function confirmDelete() {
    const node = deleting;
    deleting = null;
    if (!node) return;
    const index = rows.findIndex((row) => row.node.id === node.id);
    const removed = await workspace.deleteNode(api, node.id);
    if (removed === null) return;
    tabs.closeNodes(removed);
    await tick();
    await focusRow(Math.min(index, rows.length - 1));
  }

  function onKeydown(event: KeyboardEvent, index: number) {
    const { node, depth } = rows[index];
    const isContainer = node.kind !== "request";
    switch (event.key) {
      case "F2":
        workspace.renamingId = node.id;
        break;
      case "Delete":
        deleting = node;
        break;
      case "F10":
        // Shift+F10 is the right click of the keyboard; F10 alone is not a key of the tree.
        if (!event.shiftKey) return;
        openMenu(node, event.currentTarget as HTMLElement);
        break;
      case "ContextMenu":
        openMenu(node, event.currentTarget as HTMLElement);
        break;
      case "ArrowDown":
        void focusRow(index + 1);
        break;
      case "ArrowUp":
        void focusRow(index - 1);
        break;
      case "ArrowRight":
        // Expands a collapsed node; on an expanded one goes to its first child.
        if (!isContainer) return;
        if (!isExpanded(index)) toggle(node);
        else void focusRow(index + 1);
        break;
      case "ArrowLeft":
        // Collapses an expanded node; otherwise goes to the parent.
        if (isContainer && isExpanded(index) && !filtering) {
          toggle(node);
        } else {
          let parent = index - 1;
          while (parent >= 0 && rows[parent].depth >= depth) parent--;
          void focusRow(parent);
        }
        break;
      case "Enter":
        activate(node);
        break;
      default:
        return;
    }
    event.preventDefault();
  }
</script>

{#if rows.length === 0}
  <!-- Only a filter leaves the tree without rows: the sidebar does not draw the tree while it is empty. -->
  <EmptyState icon="search" title={strings.sidebar.nothingFound} />
{/if}
<div bind:this={container} role="tree" aria-label={strings.sidebar.label}>
  {#each rows as { node, depth }, index (node.id)}
    <TreeRow
      {node}
      {depth}
      expanded={isExpanded(index)}
      selected={workspace.selectedId === node.id}
      tabbable={tabbableId === node.id}
      dirty={dirtyIds.has(node.id)}
      renaming={workspace.renamingId === node.id}
      onactivate={() => activate(node)}
      ontoggle={() => toggle(node)}
      onkeydown={(event) => onKeydown(event, index)}
      onfocus={() => (workspace.focusedId = node.id)}
      onmenu={(anchor) => openMenu(node, anchor)}
      onrenamed={(name, byKey) => onRenamed(node, name, byKey)}
    />
  {/each}
</div>

<!-- Outside the rows: a menu inside a `treeitem` would be a part of the row and of its name. -->
{#if menu}
  {@const { node, anchor } = menu}
  {#key menu}
    <TreeMenu kind={node.kind} {anchor} onaction={onAction} onclose={() => (menu = null)} />
  {/key}
{/if}

{#if deleting}
  <Dialog title={strings.tree.deleteTitle(deleting.name)} onclose={() => (deleting = null)}>
    {#if deleting.kind !== "request"}
      <p class="contents">{strings.tree.deleteContents(deletingRequests)}</p>
    {/if}
    <div class="dialog-actions">
      <Button class="danger" onclick={confirmDelete}>{strings.tree.delete}</Button>
      <Button onclick={() => (deleting = null)}>{strings.tree.cancel}</Button>
    </div>
  </Dialog>
{/if}

<style>
  .contents {
    margin: 0;
  }

  /* The button that destroys. The class goes to the `<button>` of `Button`, hence `:global`. */
  .dialog-actions :global(.danger) {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
