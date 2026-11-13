import { describe, expect, it } from "vitest";
import { buildTree, visibleRows } from "./tree";
import type { TreeRow } from "./types";

function row(id: string, parentId: string | null, kind: TreeRow["kind"], name: string, sortOrder: number): TreeRow {
  return { id, parent_id: parentId, kind, name, sort_order: sortOrder, method: kind === "request" ? "GET" : null };
}

/** Two collections, a nested folder and requests — listed out of order on purpose. */
const SHUFFLED: TreeRow[] = [
  row("req-get", "fold-inner", "request", "Get token", 0),
  row("col-b", null, "collection", "Second", 1),
  row("req-ping", "col-a", "request", "Ping", 1),
  row("fold-inner", "fold-auth", "folder", "Inner", 0),
  row("col-a", null, "collection", "First", 0),
  row("fold-auth", "col-a", "folder", "Auth", 0),
  row("req-list", "col-b", "request", "List items", 0),
];

/** `depth:name` of every visible row, in order. */
function shape(rows: TreeRow[], expanded: string[], filter: string): string[] {
  return visibleRows(buildTree(rows), new Set(expanded), filter).map(({ node, depth }) => `${depth}:${node.name}`);
}

describe("buildTree", () => {
  it("builds the nesting and the order from a shuffled list", () => {
    const tree = buildTree(SHUFFLED);

    expect(tree.map((node) => node.id)).toEqual(["col-a", "col-b"]);
    const [first, second] = tree;
    expect(first.children.map((node) => node.id)).toEqual(["fold-auth", "req-ping"]);
    expect(first.children[0].children.map((node) => node.id)).toEqual(["fold-inner"]);
    expect(first.children[0].children[0].children.map((node) => node.id)).toEqual(["req-get"]);
    expect(first.children[1].children).toEqual([]);
    expect(second.children.map((node) => node.id)).toEqual(["req-list"]);
  });

  it("keeps the fields of the rows", () => {
    const [first] = buildTree(SHUFFLED);
    expect(first).toMatchObject({ id: "col-a", parent_id: null, kind: "collection", name: "First", sort_order: 0 });
    expect(first.children[1].method).toBe("GET");
  });

  it("puts a node with an unknown parent into the root instead of losing it", () => {
    const tree = buildTree([...SHUFFLED, row("req-orphan", "no-such-node", "request", "Orphan", 5)]);
    expect(tree.map((node) => node.id)).toEqual(["col-a", "col-b", "req-orphan"]);
  });
});

describe("visibleRows", () => {
  it("shows only the roots when nothing is expanded", () => {
    expect(shape(SHUFFLED, [], "")).toEqual(["0:First", "0:Second"]);
  });

  it("shows the children of expanded nodes only", () => {
    expect(shape(SHUFFLED, ["col-a"], "")).toEqual(["0:First", "1:Auth", "1:Ping", "0:Second"]);
    expect(shape(SHUFFLED, ["col-a", "fold-auth", "fold-inner"], "")).toEqual([
      "0:First",
      "1:Auth",
      "2:Inner",
      "3:Get token",
      "1:Ping",
      "0:Second",
    ]);
  });

  it("does not show the children of an expanded node inside a collapsed one", () => {
    expect(shape(SHUFFLED, ["fold-auth", "fold-inner"], "")).toEqual(["0:First", "0:Second"]);
  });

  it("finds a request in a collapsed folder together with its ancestors, ignoring case", () => {
    expect(shape(SHUFFLED, [], "get")).toEqual(["0:First", "1:Auth", "2:Inner", "3:Get token"]);
    expect(shape(SHUFFLED, [], "GET")).toEqual(["0:First", "1:Auth", "2:Inner", "3:Get token"]);
  });

  it("returns an empty list when nothing matches", () => {
    expect(shape(SHUFFLED, ["col-a"], "no such name")).toEqual([]);
    expect(visibleRows([], new Set(), "")).toEqual([]);
  });
});
