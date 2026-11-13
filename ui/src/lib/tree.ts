// Display helpers of the sidebar: the nested tree from the flat list of the `tree` command and the flat
// list of rows that are drawn.

import type { TreeRow } from "./types";

/** A row of the tree with its children, ordered by `sort_order`. */
export type TreeNode = TreeRow & { children: TreeNode[] };

/** A row to draw: the node and how deep it is nested; a root has depth 0. */
export interface VisibleRow {
  node: TreeNode;
  depth: number;
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
      for (const node of nodes) {
        rows.push({ node, depth });
        if (expanded.has(node.id)) walk(node.children, depth + 1);
      }
    };
    walk(tree, 0);
    return rows;
  }

  /** Adds the matching part of the subtree; `false` when nothing in it matches. */
  const collect = (node: TreeNode, depth: number): boolean => {
    const at = rows.length;
    rows.push({ node, depth });
    let matches = node.name.toLowerCase().includes(needle);
    for (const child of node.children) {
      if (collect(child, depth + 1)) matches = true;
    }
    if (!matches) rows.length = at;
    return matches;
  };
  for (const node of tree) collect(node, 0);
  return rows;
}
