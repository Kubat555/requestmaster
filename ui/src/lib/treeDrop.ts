// Where a moved node of the tree goes: the zone of the row under the pointer, the place a drop or an
// Alt+arrow key means, and the rows as they are after the move. Pure: nothing here touches the DOM.

import { ROW_HEIGHT } from "./tree";
import type { NodeKind, TreeRow } from "./types";

/** The type a dragged node travels under in `DataTransfer`; its data is the id of the node. */
export const NODE_DRAG_TYPE = "application/x-rqm-node";

/** Where the dragged node goes relative to the row under the pointer. */
export type DropZone = "before" | "inside" | "after";

/** A place in the tree: `index` counts the children of the parent without the moved node; `null` — the roots. */
export interface Place {
  parentId: string | null;
  index: number;
}

/** How high the upper and the lower edge of a folder are, px: there a drop goes next to the folder, not inside. */
const FOLDER_EDGE = 8;

/** Zone of the row under the pointer; `null` — the dragged kind cannot be dropped on this row at all. */
export function dropZone(offsetY: number, over: NodeKind, dragged: NodeKind): DropZone | null {
  const half = offsetY < ROW_HEIGHT / 2 ? "before" : "after";
  // A collection is only reordered among the collections.
  if (dragged === "collection") return over === "collection" ? half : null;
  // Only a collection is a root, so there is no place next to one for anything else.
  if (over === "collection") return "inside";
  if (over === "request") return half;
  if (offsetY < FOLDER_EDGE) return "before";
  return offsetY >= ROW_HEIGHT - FOLDER_EDGE ? "after" : "inside";
}

/** The children of the parent (`null` — the roots) in their order. */
function childrenOf(rows: TreeRow[], parentId: string | null): TreeRow[] {
  return rows.filter((row) => row.parent_id === parentId).sort((a, b) => a.sort_order - b.sort_order);
}

/** Whether `id` is the node `ancestorId` or lies under it. */
function isInside(byId: Map<string, TreeRow>, id: string, ancestorId: string): boolean {
  const seen = new Set<string>();
  // A node met twice would mean a cycle in the rows; the walk stops there.
  for (let cursor = byId.get(id); cursor && !seen.has(cursor.id); ) {
    if (cursor.id === ancestorId) return true;
    seen.add(cursor.id);
    cursor = cursor.parent_id === null ? undefined : byId.get(cursor.parent_id);
  }
  return false;
}

/**
 * The place, or `null` when the rules of the tree forbid it — a collection is a root and only a collection
 * is, a request has no children — or when the node already stands there.
 */
function checked(rows: TreeRow[], node: TreeRow, parent: TreeRow | null, index: number): Place | null {
  if ((node.kind === "collection") !== (parent === null) || parent?.kind === "request") return null;
  const parentId = parent?.id ?? null;
  if (node.parent_id === parentId && childrenOf(rows, parentId).findIndex((row) => row.id === node.id) === index) {
    return null;
  }
  return { parentId, index };
}

/**
 * Where the drop puts the node; `null` — forbidden or nothing would change. `overExpanded` — whether the
 * children of the row under the pointer are shown.
 */
export function dropPlace(
  rows: TreeRow[],
  draggedId: string,
  overId: string,
  zone: DropZone,
  overExpanded: boolean,
): Place | null {
  const byId = new Map(rows.map((row) => [row.id, row]));
  const dragged = byId.get(draggedId);
  const over = byId.get(overId);
  // Not into itself and not into its own descendant.
  if (!dragged || !over || isInside(byId, overId, draggedId)) return null;

  const without = (parentId: string | null) => childrenOf(rows, parentId).filter((row) => row.id !== draggedId);
  // As the last child of the row.
  if (zone === "inside") return checked(rows, dragged, over, without(over.id).length);
  // The row right under an expanded folder is its first child, so "after the folder" is the place before that child.
  if (zone === "after" && over.kind === "folder" && overExpanded && childrenOf(rows, over.id).length > 0) {
    return checked(rows, dragged, over, 0);
  }
  // Next to the row, under its parent.
  const at = without(over.parent_id).findIndex((row) => row.id === overId);
  const parent = over.parent_id === null ? null : (byId.get(over.parent_id) ?? null);
  return checked(rows, dragged, parent, zone === "after" ? at + 1 : at);
}

/**
 * Where an Alt+arrow key moves the node; `null` — nowhere. Up and down — by one place among the siblings.
 * Right — to the end of the previous sibling, when that is a folder. Left — out of the parent, right after it.
 * A collection only goes up and down.
 */
export function keyboardPlace(
  rows: TreeRow[],
  id: string,
  key: "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight",
): Place | null {
  const node = rows.find((row) => row.id === id);
  if (!node) return null;
  const siblings = childrenOf(rows, node.parent_id);
  const at = siblings.findIndex((row) => row.id === id);
  switch (key) {
    case "ArrowUp":
      return at === 0 ? null : { parentId: node.parent_id, index: at - 1 };
    case "ArrowDown":
      return at === siblings.length - 1 ? null : { parentId: node.parent_id, index: at + 1 };
    case "ArrowRight": {
      const previous = siblings[at - 1];
      if (node.kind === "collection" || previous?.kind !== "folder") return null;
      return { parentId: previous.id, index: childrenOf(rows, previous.id).length };
    }
    case "ArrowLeft": {
      const parent = rows.find((row) => row.id === node.parent_id);
      // Out of a collection there is only the root, and only a collection is a root.
      if (!parent || parent.kind === "collection") return null;
      const parentAt = childrenOf(rows, parent.parent_id).findIndex((row) => row.id === parent.id);
      return { parentId: parent.parent_id, index: parentAt + 1 };
    }
  }
}

/**
 * The rows after the move: `parent_id` of the node and `sort_order` 0..n of the old and the new siblings. An
 * index past the end means the last place, as in the core. The given rows are not changed.
 */
export function applyMove(rows: TreeRow[], id: string, place: Place): TreeRow[] {
  const node = rows.find((row) => row.id === id);
  if (!node) return rows;
  const siblings = childrenOf(rows, place.parentId).filter((row) => row.id !== id);
  siblings.splice(Math.min(place.index, siblings.length), 0, node);
  // The new order of the new siblings, and of the old ones when the parent changes.
  const order = new Map(siblings.map((row, index) => [row.id, index]));
  if (node.parent_id !== place.parentId) {
    for (const [index, row] of childrenOf(rows, node.parent_id).filter((row) => row.id !== id).entries()) {
      order.set(row.id, index);
    }
  }
  return rows.map((row) => {
    const sortOrder = order.get(row.id);
    if (sortOrder === undefined) return row;
    return { ...row, parent_id: row.id === id ? place.parentId : row.parent_id, sort_order: sortOrder };
  });
}
