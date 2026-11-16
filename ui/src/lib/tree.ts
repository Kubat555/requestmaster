// Display helpers of the sidebar: the nested tree from the flat list of the `tree` command and the flat
// list of rows that are drawn.

import type { TreeRow } from "./types";

/** A row of the tree with its children, ordered by `sort_order`. */
export type TreeNode = TreeRow & { children: TreeNode[] };

/** Height of a row of the tree, px; equals `--row-height` of tokens.css (a test compares the two). */
export const ROW_HEIGHT = 30;

/** How many rows are drawn beyond each edge of the viewport. */
export const OVERSCAN = 8;

/**
 * A row to draw: the node, how deep it is nested (a root has depth 0), and its place among the siblings that
 * are shown, counted from 1.
 */
export interface VisibleRow {
  node: TreeNode;
  depth: number;
  posInSet: number;
  setSize: number;
}

/** Rows to draw: [first, last). A viewport of no height (no layout) means all rows. */
export function windowRange(scrollTop: number, viewportHeight: number, count: number): { first: number; last: number } {
  if (viewportHeight <= 0) return { first: 0, last: count };
  const last = Math.min(count, Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN);
  const first = Math.min(last, Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
  return { first, last };
}

/** The scroll position that makes the row fully visible; unchanged when it already is. */
export function revealScrollTop(index: number, scrollTop: number, viewportHeight: number): number {
  // Without a layout there is nothing to scroll.
  if (viewportHeight <= 0) return scrollTop;
  const top = index * ROW_HEIGHT;
  if (top < scrollTop) return top;
  const bottom = top + ROW_HEIGHT;
  return bottom > scrollTop + viewportHeight ? bottom - viewportHeight : scrollTop;
}

/** Index of the first row that lies wholly inside the viewport (not an overscan row); 0 without a layout or rows. */
export function firstVisibleIndex(scrollTop: number, viewportHeight: number, count: number): number {
  if (viewportHeight <= 0) return 0;
  return Math.max(0, Math.min(Math.ceil(scrollTop / ROW_HEIGHT), count - 1));
}

/**
 * Nests the rows by `parent_id`; siblings go by `sort_order`. A row whose parent is not in the list becomes
 * a root, so that it stays reachable.
 */
export function buildTree(rows: TreeRow[]): TreeNode[] {
  const nodes = new Map<string, TreeNode>();
  for (const row of rows) nodes.set(row.id, { ...row, children: [] });

  const roots: TreeNode[] = [];
  for (const node of nodes.values()) {
    const parent = node.parent_id === null ? undefined : nodes.get(node.parent_id);
    (parent ? parent.children : roots).push(node);
  }

  const bySortOrder = (a: TreeNode, b: TreeNode) => a.sort_order - b.sort_order;
  roots.sort(bySortOrder);
  for (const node of nodes.values()) node.children.sort(bySortOrder);
  return roots;
}

/**
 * The rows to draw, top to bottom. Without a filter — the roots and the children of every node of `expanded`
 * that is itself shown. With a filter — the nodes whose name contains it (case is ignored) together with all
 * their ancestors, whatever is expanded.
 */
export function visibleRows(tree: TreeNode[], expanded: Set<string>, filter: string): VisibleRow[] {
  const needle = filter.trim().toLowerCase();
  const rows: VisibleRow[] = [];

  if (needle === "") {
    const walk = (nodes: TreeNode[], depth: number) => {
      for (const [index, node] of nodes.entries()) {
        rows.push({ node, depth, posInSet: index + 1, setSize: nodes.length });
        if (expanded.has(node.id)) walk(node.children, depth + 1);
      }
    };
    walk(tree, 0);
    return rows;
  }

  /** Adds the matching parts of the subtrees of the siblings; `false` when nothing in them matches. */
  const collect = (nodes: TreeNode[], depth: number): boolean => {
    const shown: VisibleRow[] = [];
    for (const node of nodes) {
      const at = rows.length;
      const row = { node, depth, posInSet: 0, setSize: 0 };
      rows.push(row);
      const inside = collect(node.children, depth + 1);
      if (inside || node.name.toLowerCase().includes(needle)) shown.push(row);
      else rows.length = at;
    }
    for (const [index, row] of shown.entries()) {
      row.posInSet = index + 1;
      row.setSize = shown.length;
    }
    return shown.length > 0;
  };
  collect(tree, 0);
  return rows;
}
