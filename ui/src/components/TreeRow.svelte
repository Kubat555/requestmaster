<script lang="ts">
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
    /** The user activated the row: a click, or Enter. */
    onactivate: () => void;
    /** The user clicked the arrow of a collection or a folder. */
    ontoggle: () => void;
    onkeydown: (event: KeyboardEvent) => void;
    onfocus: () => void;
  }

  let { node, depth, expanded, selected, tabbable, onactivate, ontoggle, onkeydown, onfocus }: Props = $props();

  /** Methods that have a colour of their own; any other one is drawn with `--method-other`. */
  const COLOURED = ["get", "post", "put", "patch", "delete"];
  /** A longer method is cut so that the names of the requests stay aligned. */
  const METHOD_MAX = 6;

  const isRequest = $derived(node.kind === "request");
  const method = $derived(node.method ?? "");
  const methodClass = $derived(COLOURED.includes(method.toLowerCase()) ? `method-${method.toLowerCase()}` : "method-other");

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
    <span class={["method", methodClass]} title={method}>{method.slice(0, METHOD_MAX)}</span>
  {:else}
    <!-- The arrow is for the pointer only: the keyboard expands and collapses with the arrow keys. -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <span class="arrow" class:expanded aria-hidden="true" onclick={onArrowClick}></span>
  {/if}
  <span class="name">{node.name}</span>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 1.5);
    height: var(--row-height);
    padding-right: calc(var(--space) * 2);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }

  /* Hover and selection switch at once, without a transition. */
  .row:hover {
    background: var(--surface-2);
  }

  .row.selected {
    background: var(--border);
  }

  .arrow {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 100%;
  }

  /* A triangle pointing right; it points down when the node is expanded. */
  .arrow::before {
    content: "";
    border: 4px solid transparent;
    border-left: 5px solid var(--text-muted);
    border-right-width: 0;
  }

  .arrow.expanded::before {
    transform: rotate(90deg);
  }

  .method {
    flex: none;
    min-width: 44px;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
  }

  .method-get {
    color: var(--method-get);
  }

  .method-post {
    color: var(--method-post);
  }

  .method-put {
    color: var(--method-put);
  }

  .method-patch {
    color: var(--method-patch);
  }

  .method-delete {
    color: var(--method-delete);
  }

  .method-other {
    color: var(--method-other);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
