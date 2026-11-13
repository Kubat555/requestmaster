<script lang="ts">
  import Icon from "./ui/Icon.svelte";
  import { methodClass, methodLabel } from "../lib/format";
  import type { TreeNode } from "../lib/tree";

  interface Props {
    node: TreeNode;
    /** Nesting of the node; a root has depth 0. */
    depth: number;
    /** Whether the children of a collection or a folder are shown; ignored for a request. */
    expanded: boolean;
    /** Whether the node is highlighted. */
    selected: boolean;
    /** Whether Tab stops at this row: one row of the tree at a time. */
    tabbable: boolean;
    /** Whether the open tab of the node has unsaved edits. */
    dirty: boolean;
    /** The user activated the row: a click, or Enter. */
    onactivate: () => void;
    /** The user clicked the arrow of a collection or a folder. */
    ontoggle: () => void;
    onkeydown: (event: KeyboardEvent) => void;
    onfocus: () => void;
  }

  let { node, depth, expanded, selected, tabbable, dirty, onactivate, ontoggle, onkeydown, onfocus }: Props = $props();

  const isRequest = $derived(node.kind === "request");
  const method = $derived(node.method ?? "");

  function onArrowClick(event: MouseEvent) {
    // The arrow only toggles; without this the click would also activate the row.
    event.stopPropagation();
    ontoggle();
  }
</script>

<div
  class="row"
  class:selected
  role="treeitem"
  tabindex={tabbable ? 0 : -1}
  aria-level={depth + 1}
  aria-expanded={isRequest ? undefined : expanded}
  aria-selected={selected}
  style:padding-left="calc(var(--space) + {depth * 16}px)"
  onclick={onactivate}
  {onkeydown}
  {onfocus}
>
  {#if isRequest}
    <span class={["method", methodClass(method)]} title={method}>{methodLabel(method)}</span>
  {:else}
    <!-- The arrow is for the pointer only: the keyboard expands and collapses with the arrow keys. -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <span class="arrow" aria-hidden="true" onclick={onArrowClick}>
      <Icon name={expanded ? "chevron-down" : "chevron-right"} />
    </span>
  {/if}
  <span class="name">{node.name}</span>
  {#if dirty}
    <!-- Decoration: the tab of the node says the same in words. -->
    <span class="dirty" aria-hidden="true"></span>
  {/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 1.5);
    height: var(--row-height);
    padding-right: calc(var(--space) * 3);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }

  /* Hover and selection switch at once, without a transition. */
  .row:hover {
    background: color-mix(in srgb, var(--surface-2) 60%, transparent);
  }

  .row.selected {
    background: var(--surface-2);
  }

  .arrow {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 100%;
    color: var(--text-muted);
  }

  .method {
    flex: none;
    min-width: 44px;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dirty {
    flex: none;
    width: 6px;
    height: 6px;
    background: var(--accent);
    border-radius: 50%;
  }
</style>
