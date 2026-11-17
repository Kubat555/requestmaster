<script lang="ts">
  import RenameInput from "./RenameInput.svelte";
  import Icon from "./ui/Icon.svelte";
  import { methodClass, methodLabel } from "../lib/format";
  import type { TreeNode } from "../lib/tree";
  import type { DropZone } from "../lib/treeDrop";

  interface Props {
    node: TreeNode;
    /** Nesting of the node; a root has depth 0. */
    depth: number;
    /** Place of the node among the shown siblings, from 1, and how many they are: not all of them are in the DOM. */
    posInSet: number;
    setSize: number;
    /** Distance from the top of the tree to the row, px: the rows are placed, not stacked. */
    top: number;
    /** Whether the children of a collection or a folder are shown; ignored for a request. */
    expanded: boolean;
    /** Whether the node is highlighted. */
    selected: boolean;
    /** Whether Tab stops at this row: one row of the tree at a time. */
    tabbable: boolean;
    /** Whether the open tab of the node has unsaved edits. */
    dirty: boolean;
    /** Whether the row shows a field instead of the name. */
    renaming: boolean;
    /** Whether the row can be dragged to another place of the tree. */
    draggable: boolean;
    /** Whether the row is the one being dragged: at the rich motion level it looks lifted. */
    dragging: boolean;
    /** Whether the row has just appeared in the tree: at the rich motion level it fades in. */
    fresh: boolean;
    /** Where the node being dragged would go if dropped on this row; `null` — the row is not the target. */
    drop: DropZone | null;
    /** The user activated the row: a click, or Enter. */
    onactivate: () => void;
    /** The user clicked the arrow of a collection or a folder. */
    ontoggle: () => void;
    onkeydown: (event: KeyboardEvent) => void;
    onfocus: () => void;
    /** The user asked for the menu of the row: at the point of a right click, or under the "…" button. */
    onmenu: (anchor: HTMLElement | { x: number; y: number }) => void;
    /** The renaming is over; see `RenameInput`. */
    onrenamed: (name: string | null, byKey: boolean) => void;
  }

  let {
    node,
    depth,
    posInSet,
    setSize,
    top,
    expanded,
    selected,
    tabbable,
    dirty,
    renaming,
    draggable,
    dragging,
    fresh,
    drop,
    onactivate,
    ontoggle,
    onkeydown,
    onfocus,
    onmenu,
    onrenamed,
  }: Props = $props();

  const isRequest = $derived(node.kind === "request");
  const method = $derived(node.method ?? "");

  function onArrowClick(event: MouseEvent) {
    // The arrow only toggles; without this the click would also activate the row.
    event.stopPropagation();
    ontoggle();
  }

  /**
   * Whether the event came from the field for the name. The field keeps its events to itself; this is the
   * second lock, so that a key typed into a name can never become a command of the tree.
   */
  function fromInput(event: Event): boolean {
    return event.target instanceof HTMLInputElement;
  }

  function onRowClick(event: MouseEvent) {
    if (!fromInput(event)) onactivate();
  }

  function onRowKeydown(event: KeyboardEvent) {
    if (!fromInput(event)) onkeydown(event);
  }

  /** Our menu instead of the one of the webview; a text field keeps its own, with copy and paste. */
  function onContextMenu(event: MouseEvent) {
    if (fromInput(event)) return;
    event.preventDefault();
    onmenu({ x: event.clientX, y: event.clientY });
  }

  function onMoreClick(event: MouseEvent) {
    // The button only shows the menu; without this the click would also activate the row.
    event.stopPropagation();
    onmenu(event.currentTarget as HTMLElement);
  }
</script>

<div
  class="row hover-row"
  class:selected
  class:dragging
  class:fresh
  class:drop-before={drop === "before"}
  class:drop-inside={drop === "inside"}
  class:drop-after={drop === "after"}
  role="treeitem"
  tabindex={tabbable ? 0 : -1}
  data-id={node.id}
  {draggable}
  aria-level={depth + 1}
  aria-posinset={posInSet}
  aria-setsize={setSize}
  aria-expanded={isRequest ? undefined : expanded}
  aria-selected={selected}
  style:top="{top}px"
  style:padding-left="calc(var(--space) + {depth * 16}px)"
  onclick={onRowClick}
  onkeydown={onRowKeydown}
  oncontextmenu={onContextMenu}
  {onfocus}
>
  {#if isRequest}
    <span class={["method", methodClass(method)]} title={method}>{methodLabel(method)}</span>
  {:else}
    <!-- The arrow is for the pointer only: the keyboard expands and collapses with the arrow keys. -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <span class="arrow" class:open={expanded} aria-hidden="true" onclick={onArrowClick}>
      <Icon name="chevron-right" />
    </span>
  {/if}
  {#if renaming}
    <RenameInput value={node.name} ondone={onrenamed} />
  {:else}
    <span class="name">{node.name}</span>
  {/if}
  {#if dirty}
    <!-- Decoration: the tab of the node says the same in words. -->
    <span class="dirty" aria-hidden="true"></span>
  {/if}
  <!-- For the pointer only, hence hidden from the accessibility tree and nameless: a name here would become a
       part of the name of the row. The keyboard opens the same menu with Shift+F10 or the menu key. A press
       does not take the focus: it stays on the row it was on. -->
  <!-- svelte-ignore a11y_consider_explicit_label -->
  <button
    type="button"
    class="more"
    aria-hidden="true"
    tabindex="-1"
    onmousedown={(event) => event.preventDefault()}
    onclick={onMoreClick}
  >
    <Icon name="more" />
  </button>
</div>

<style>
  /* Placed by `top` inside the tree, which is as high as all its rows together. */
  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 1.5);
    height: var(--row-height);
    padding-right: calc(var(--space) * 3);
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }

  .row.selected {
    background: var(--surface-2);
  }

  /* Where the dragged node would go: a line above or below the row, or a frame around it. Drawn inside the
     row, so that nothing moves, and at once, without a transition. */
  .row.drop-before {
    box-shadow: inset 0 2px 0 var(--accent);
  }

  .row.drop-after {
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .row.drop-inside {
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  /* The rich motion level: a row that has just appeared fades in at its place — its height and its place are
     as always, so the tree measures nothing anew — and the row being dragged looks lifted off the tree. It
     shrinks a little rather than grows, so that it never sticks out of the tree. */
  :global(:root[data-motion="rich"]) .row.fresh {
    animation: row-fresh var(--dur-3) var(--ease);
  }

  @keyframes row-fresh {
    from {
      opacity: 0;
      transform: translateX(-6px);
    }
  }

  :global(:root[data-motion="rich"]) .row.dragging {
    z-index: 1;
    background: var(--surface-2);
    box-shadow: var(--shadow-lift);
    transform: scale(0.98);
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

  /* One arrow, pointing right; it turns down when the children are shown. */
  .arrow > :global(.icon) {
    transition: transform var(--dur-2) var(--ease);
  }

  .arrow.open > :global(.icon) {
    transform: rotate(90deg);
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

  /* Takes its place always, so that the name does not jump; seen while the pointer is over the row. */
  .more {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius);
    color: var(--text-muted);
    cursor: pointer;
    visibility: hidden;
  }

  .row:hover .more {
    visibility: visible;
  }

  .more:hover {
    background: var(--border);
    color: var(--text);
  }
</style>
