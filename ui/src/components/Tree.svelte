<script lang="ts">
  import { tick } from "svelte";
  import TreeRow from "./TreeRow.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import { strings } from "../lib/strings";
  import { tabs } from "../lib/tabs.svelte";
  import { buildTree, visibleRows, type TreeNode } from "../lib/tree";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  let container: HTMLDivElement;
  /** Id of the row that had the focus last: the one Tab comes back to. */
  let focusedId = $state<string | null>(null);

  const tree = $derived(buildTree(workspace.rows));
  const rows = $derived(visibleRows(tree, workspace.expanded, workspace.filter));
  const filtering = $derived(workspace.filter.trim() !== "");
  /** The row Tab stops at: the last focused one while it is shown, otherwise the first. */
  const tabbableId = $derived(rows.some((row) => row.node.id === focusedId) ? focusedId : (rows[0]?.node.id ?? null));

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
    container.querySelectorAll<HTMLElement>('[role="treeitem"]')[index]?.focus();
  }

  function onKeydown(event: KeyboardEvent, index: number) {
    const { node, depth } = rows[index];
    const isContainer = node.kind !== "request";
    switch (event.key) {
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
      onactivate={() => activate(node)}
      ontoggle={() => toggle(node)}
      onkeydown={(event) => onKeydown(event, index)}
      onfocus={() => (focusedId = node.id)}
    />
  {/each}
</div>
