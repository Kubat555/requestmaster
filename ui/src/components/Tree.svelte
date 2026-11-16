<script lang="ts">
  import { getContext, onMount, tick } from "svelte";
  import TreeDialogs from "./TreeDialogs.svelte";
  import TreeMenu, { type TreeAction } from "./TreeMenu.svelte";
  import TreeRow from "./TreeRow.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import { tabs } from "../lib/tabs.svelte";
  import {
    buildTree,
    firstVisibleIndex,
    revealScrollTop,
    ROW_HEIGHT,
    visibleRows,
    windowRange,
    type TreeNode,
    type VisibleRow,
  } from "../lib/tree";
  import { TreeActions } from "../lib/treeActions.svelte";
  import { descendantIds, dropPlace, dropZone, keyboardPlace, NODE_DRAG_TYPE, type DropZone, type Place } from "../lib/treeDrop";
  import type { NodeKind } from "../lib/types";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  const api = getContext<Api>("api");

  /** Where a menu appears: under an element, or at a point of the window. */
  type Anchor = HTMLElement | { x: number; y: number };

  /** What the menu of a row does, and the questions about a deletion and an export. */
  // The workspace of a tree is not replaced while the tree is on the screen.
  // svelte-ignore state_referenced_locally
  const actions = new TreeActions(workspace);

  /** The area that scrolls; the rows stand inside it at the places their indexes give. */
  let scroller = $state<HTMLDivElement>();
  let container = $state<HTMLDivElement>();
  /** How far the area is scrolled and how high it is, as read last. No height — no layout: all rows are drawn. */
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  /**
   * Whether the height was read at least once. Until then no row is drawn: with the height still unknown a big
   * tree would draw every row only to remove almost all of them a moment later.
   */
  let measured = $state(false);
  /** The menu of a row on the screen; `null` — none. */
  let menu = $state.raw<{ node: TreeNode; anchor: Anchor } | null>(null);
  /** The node of the menu shown last; stays after the menu is closed. */
  let menuNode: TreeNode | null = null;
  /**
   * The node being dragged with the mouse; `null` — none. What lies under it is found once, when the drag
   * starts: the question "is this row inside the dragged node" is asked on every move of the pointer.
   */
  let dragging = $state.raw<{ id: string; kind: NodeKind; descendants: Set<string> } | null>(null);
  /** The row a drop would go to and where relative to it; `null` — the pointer is over no place for the node. */
  let dropTarget = $state.raw<{ id: string; zone: DropZone } | null>(null);

  const tree = $derived(buildTree(workspace.rows));
  const rows = $derived(visibleRows(tree, workspace.expanded, workspace.filter));
  const filtering = $derived(workspace.filter.trim() !== "");
  /** Where every shown node stands in `rows`. */
  const indexById = $derived(new Map(rows.map((row, index) => [row.node.id, index])));
  /**
   * How far the rows are scrolled: what was read last, but never beyond the end of the rows. The list may have
   * become shorter since (a filter, a collapsed folder), and the area tells about it only with its next event.
   */
  const offset = $derived(Math.min(scrollTop, Math.max(0, rows.length * ROW_HEIGHT - viewportHeight)));

  /**
   * The rows in the DOM, top to bottom: the ones in and near the viewport, and always the row with the focus,
   * the row being renamed and the row being dragged — taking one out would lose the focus or the typed name,
   * or break the drag off.
   */
  const drawn = $derived.by<(VisibleRow & { index: number })[]>(() => {
    if (!measured) return [];
    const { first, last } = windowRange(offset, viewportHeight, rows.length);
    const kept = new Set<number>();
    for (const id of [workspace.focusedId, workspace.renamingId, dragging?.id]) {
      const index = id == null ? undefined : indexById.get(id);
      if (index !== undefined && (index < first || index >= last)) kept.add(index);
    }
    const outside = [...kept].sort((a, b) => a - b);
    const indexes = [
      ...outside.filter((index) => index < first),
      ...Array.from({ length: last - first }, (_, at) => first + at),
      ...outside.filter((index) => index >= last),
    ];
    return indexes.map((index) => ({ ...rows[index], index }));
  });

  /**
   * The row Tab stops at: the last focused one while it is shown, otherwise the first one that lies wholly
   * inside the viewport. Not the first drawn one: that row is above the viewport, and a Tab to it would scroll
   * the tree back.
   */
  const tabbableId = $derived(
    workspace.focusedId !== null && indexById.has(workspace.focusedId)
      ? workspace.focusedId
      : (rows[firstVisibleIndex(offset, viewportHeight, rows.length)]?.node.id ?? null),
  );

  /** Ids of the nodes whose open tab has unsaved edits: their rows are marked. */
  const dirtyIds = $derived(new Set(tabs.list.filter((tab) => tab.dirty).map((tab) => tab.nodeId)));

  /** Reads how far the area is scrolled and how high it is. */
  function measure() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    viewportHeight = scroller.clientHeight;
  }

  /**
   * The area was scrolled. The menu of a row goes away: the row it hangs on may leave the DOM. The late event of
   * a scroll the tree made itself, to show the row the keyboard went to, changes nothing and closes nothing.
   */
  function onScroll() {
    const before = scrollTop;
    measure();
    if (scrollTop !== before) menu = null;
  }

  // The height is read when the tree appears and, where the environment can tell, whenever the area is resized.
  onMount(() => {
    measure();
    measured = true;
    if (typeof ResizeObserver === "undefined" || !scroller) return;
    const observer = new ResizeObserver(measure);
    observer.observe(scroller);
    return () => observer.disconnect();
  });

  /** Whether the children of the row at `index` are shown. A filter shows them whatever is expanded. */
  function isExpanded(index: number): boolean {
    const { node, depth } = rows[index];
    if (node.kind === "request") return false;
    return filtering ? (rows[index + 1]?.depth ?? 0) > depth : workspace.expanded.has(node.id);
  }

  /** A request and a collection are opened; a folder expands or collapses: it is opened from its menu. */
  function activate(node: TreeNode) {
    if (node.kind === "folder") toggle(node);
    else void actions.run(api, node, "open");
  }

  /** Does nothing under a filter: the filter decides what is shown. */
  function toggle(node: TreeNode) {
    if (!filtering) workspace.toggle(node.id);
  }

  /**
   * Moves the keyboard to the row at `index` of `rows`. The row may be far from the drawn ones: the area is
   * scrolled first, so that the row is drawn and seen, and then the row is found by its node.
   */
  async function focusRow(index: number) {
    if (index < 0 || index >= rows.length) return;
    const id = rows[index].node.id;
    if (scroller) {
      scroller.scrollTop = revealScrollTop(index, scroller.scrollTop, scroller.clientHeight);
      // The event of this scroll comes later; the rows of the new place are needed now.
      measure();
    }
    await tick();
    const inDom = container?.querySelectorAll<HTMLElement>("[data-id]") ?? [];
    [...inDom].find((row) => row.dataset.id === id)?.focus();
  }

  /** Moves the keyboard to the row of the node, if it is shown. */
  async function focusNode(id: string) {
    await tick();
    await focusRow(indexById.get(id) ?? -1);
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

  /** Does the action; the keyboard follows to the row the action names, e.g. the copy of a duplicated node. */
  async function run(node: TreeNode, action: TreeAction) {
    const focusId = await actions.run(api, node, action);
    if (focusId !== null) await focusNode(focusId);
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

  /** The user confirmed the deletion. The keyboard goes to the row that took the place of the deleted one. */
  async function confirmDelete() {
    const node = actions.deleting;
    const index = node ? (indexById.get(node.id) ?? -1) : -1;
    const removed = await actions.confirmDelete(api);
    if (removed === null) return;
    await tick();
    await focusRow(Math.min(index, rows.length - 1));
  }

  /** The row of the tree the event happened in. */
  function rowOf(event: Event): HTMLElement | null {
    return event.target instanceof Element ? event.target.closest<HTMLElement>("[data-id]") : null;
  }

  /**
   * A row starts to be dragged. Not under a filter, which shows a part of the children, so a place among them
   * would be a guess; and not the row being renamed, where the mouse selects the text of the name.
   */
  function onDragStart(event: DragEvent) {
    const id = rowOf(event)?.dataset.id;
    const index = id === undefined ? undefined : indexById.get(id);
    if (id === undefined || index === undefined || !event.dataTransfer || filtering || workspace.renamingId === id) return;
    event.dataTransfer.setData(NODE_DRAG_TYPE, id);
    event.dataTransfer.effectAllowed = "move";
    menu = null;
    dragging = { id, kind: rows[index].node.kind, descendants: descendantIds(workspace.rows, id) };
  }

  /**
   * Where a drop at the pointer would put the dragged node; `null` — nowhere: the pointer is not over a row,
   * the row is the node itself or lies inside it, takes no node of this kind, or the place is the one the node
   * has. A file or a text dragged over the tree is not a node, even if a drag of a node was never finished.
   */
  function dropAt(event: DragEvent): { id: string; zone: DropZone; place: Place } | null {
    if (!dragging || filtering || !event.dataTransfer?.types.includes(NODE_DRAG_TYPE)) return null;
    const row = rowOf(event);
    const id = row?.dataset.id;
    const index = id === undefined ? undefined : indexById.get(id);
    if (!row || id === undefined || index === undefined) return null;
    if (id === dragging.id || dragging.descendants.has(id)) return null;
    // The zone is counted from the top of the row, wherever inside the row the pointer is.
    const zone = dropZone(event.clientY - row.getBoundingClientRect().top, rows[index].node.kind, dragging.kind);
    if (zone === null) return null;
    const place = dropPlace(workspace.rows, dragging.id, id, zone, isExpanded(index));
    return place === null ? null : { id, zone, place };
  }

  /**
   * The pointer moves over the rows with something dragged. Over a place for the node the drop is allowed and
   * the row is marked; over anything else the event is left alone, and the browser itself forbids the drop.
   */
  function onDragOver(event: DragEvent) {
    const found = dropAt(event);
    if (found?.id !== dropTarget?.id || found?.zone !== dropTarget?.zone) {
      dropTarget = found && { id: found.id, zone: found.zone };
    }
    if (!found || !event.dataTransfer) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }

  /** The pointer left a row. Going from one part of the row to another is not leaving it. */
  function onDragLeave(event: DragEvent) {
    const row = rowOf(event);
    if (row && event.relatedTarget instanceof Node && row.contains(event.relatedTarget)) return;
    if (row?.dataset.id === dropTarget?.id) dropTarget = null;
  }

  /**
   * The node is dropped: it moves, and the keyboard goes to its row. What is selected and the open tabs stay
   * as they are.
   */
  function onDrop(event: DragEvent) {
    const found = dropAt(event);
    const id = dragging?.id;
    dragging = null;
    dropTarget = null;
    if (!found || id === undefined) return;
    event.preventDefault();
    void workspace.moveNode(api, id, found.place);
    void focusNode(id);
  }

  /** The drag is over, with a drop or without one. */
  function onDragEnd() {
    dragging = null;
    dropTarget = null;
  }

  const MOVE_KEYS = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"] as const;

  /**
   * Alt with an arrow moves the node instead of the focus, which stays on its row. Under a filter the keys do
   * nothing, as the mouse does.
   */
  function moveByKey(node: TreeNode, key: (typeof MOVE_KEYS)[number]) {
    if (filtering) return;
    const place = keyboardPlace(workspace.rows, node.id, key);
    if (place === null) return;
    void workspace.moveNode(api, node.id, place);
    // The row keeps the focus; this scrolls the tree after it when the new place is out of sight.
    void focusNode(node.id);
  }

  function onKeydown(event: KeyboardEvent, index: number) {
    const { node, depth } = rows[index];
    const isContainer = node.kind !== "request";
    const moveKey = event.altKey ? MOVE_KEYS.find((key) => key === event.key) : undefined;
    if (moveKey !== undefined) {
      moveByKey(node, moveKey);
      event.preventDefault();
      return;
    }
    switch (event.key) {
      case "F2":
        workspace.renamingId = node.id;
        break;
      case "Delete":
        actions.deleting = node;
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

<div bind:this={scroller} class="tree-scroll" onscroll={onScroll}>
  {#if rows.length === 0}
    <!-- Only a filter leaves the tree without rows: the sidebar does not draw the tree while it is empty. -->
    <EmptyState icon="search" title={strings.sidebar.nothingFound} />
  {/if}
  <!-- As high as all the rows together; the drawn ones stand inside at the places their indexes give. -->
  <!-- The events of a drag come up from the rows and are for the pointer only: the keyboard moves a node with
       Alt and an arrow on its row. The focus belongs to the rows, not to the tree around them. -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    bind:this={container}
    class="rows"
    role="tree"
    aria-label={strings.sidebar.label}
    style:height="{rows.length * ROW_HEIGHT}px"
    ondragstart={onDragStart}
    ondragover={onDragOver}
    ondragleave={onDragLeave}
    ondrop={onDrop}
    ondragend={onDragEnd}
  >
    {#each drawn as { node, depth, posInSet, setSize, index } (node.id)}
      <TreeRow
        {node}
        {depth}
        {posInSet}
        {setSize}
        top={index * ROW_HEIGHT}
        expanded={isExpanded(index)}
        selected={workspace.selectedId === node.id}
        tabbable={tabbableId === node.id}
        dirty={dirtyIds.has(node.id)}
        renaming={workspace.renamingId === node.id}
        draggable={!filtering && workspace.renamingId !== node.id}
        drop={dropTarget?.id === node.id ? dropTarget.zone : null}
        onactivate={() => activate(node)}
        ontoggle={() => toggle(node)}
        onkeydown={(event) => onKeydown(event, index)}
        onfocus={() => (workspace.focusedId = node.id)}
        onmenu={(anchor) => openMenu(node, anchor)}
        onrenamed={(name, byKey) => onRenamed(node, name, byKey)}
      />
    {/each}
  </div>
</div>

<!-- Outside the rows: a menu inside a `treeitem` would be a part of the row and of its name. -->
{#if menu}
  {@const { node, anchor } = menu}
  {#key menu}
    <TreeMenu kind={node.kind} {anchor} onaction={onAction} onclose={() => (menu = null)} />
  {/key}
{/if}

<TreeDialogs {actions} {workspace} ondelete={confirmDelete} />

<style>
  /* Fills the area the sidebar gives to the tree, which is a column: the tree scrolls inside itself. */
  .tree-scroll {
    flex: 1;
    min-height: 0;
    height: 100%;
    overflow: auto;
  }

  .rows {
    position: relative;
  }
</style>
