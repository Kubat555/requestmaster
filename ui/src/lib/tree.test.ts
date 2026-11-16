import { describe, expect, it } from "vitest";
import { buildTree, firstVisibleIndex, revealScrollTop, ROW_HEIGHT, visibleRows, windowRange } from "./tree";
import tokens from "../styles/tokens.css?raw";
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

describe("visibleRows: the place among the shown siblings", () => {
  /** A collection with three folders, the second of them with two requests, and a second collection. */
  const SIBLINGS: TreeRow[] = [
    row("col", null, "collection", "Api", 0),
    row("fold-1", "col", "folder", "Users", 0),
    row("fold-2", "col", "folder", "Orders", 1),
    row("fold-3", "col", "folder", "Items", 2),
    row("req-1", "fold-2", "request", "List orders", 0),
    row("req-2", "fold-2", "request", "Get order", 1),
    row("col-2", null, "collection", "Other", 1),
  ];

  /** `posInSet/setSize` of the shown row of the node; `null` when the node is not shown. */
  function place(id: string, expanded: string[], filter: string): string | null {
    const found = visibleRows(buildTree(SIBLINGS), new Set(expanded), filter).find(({ node }) => node.id === id);
    return found ? `${found.posInSet}/${found.setSize}` : null;
  }

  it("numbers a row among its siblings", () => {
    const second = visibleRows(buildTree(SIBLINGS), new Set(["col"]), "").find(({ node }) => node.id === "fold-2");
    expect(second?.posInSet).toBe(2);
    expect(second?.setSize).toBe(3);

    expect(place("col", ["col"], "")).toBe("1/2");
    expect(place("col-2", ["col"], "")).toBe("2/2");
    expect(place("fold-3", ["col"], "")).toBe("3/3");
    expect(place("req-2", ["col", "fold-2"], "")).toBe("2/2");
  });

  it("counts only the siblings a filter leaves", () => {
    expect(place("col", [], "get order")).toBe("1/1");
    expect(place("fold-2", [], "get order")).toBe("1/1");
    expect(place("req-2", [], "get order")).toBe("1/1");
    expect(place("fold-1", [], "get order")).toBeNull();
    // Two of the three folders match: "Users" and "Orders", not "Items".
    expect(place("fold-2", [], "er")).toBe("2/2");
    expect(place("fold-3", [], "er")).toBeNull();
    expect(place("fold-3", [], "items")).toBe("1/1");
  });
});

describe("the drawn part of the tree", () => {
  it("has the row height of the stylesheet", () => {
    const match = /--row-height:\s*(\d+)px/.exec(tokens);
    expect(match).not.toBeNull();
    expect(ROW_HEIGHT).toBe(Number(match![1]));
  });

  it("draws the rows of the viewport and a margin on each side", () => {
    expect(windowRange(0, 300, 3000)).toEqual({ first: 0, last: 18 });
    expect(windowRange(3000, 300, 3000)).toEqual({ first: 92, last: 118 });
    expect(windowRange(89970, 300, 3000)).toEqual({ first: 2991, last: 3000 });
  });

  it("draws every row when the viewport has no height", () => {
    expect(windowRange(0, 0, 3000)).toEqual({ first: 0, last: 3000 });
  });

  it("scrolls only as far as a row needs to be fully visible", () => {
    expect(revealScrollTop(0, 600, 300)).toBe(0);
    expect(revealScrollTop(50, 0, 300)).toBe(1230);
    expect(revealScrollTop(5, 0, 300)).toBe(0);
  });

  it("finds the first row that lies wholly inside the viewport", () => {
    expect(firstVisibleIndex(3000, 300, 3000)).toBe(100);
    expect(firstVisibleIndex(3010, 300, 3000)).toBe(101);
    expect(firstVisibleIndex(0, 300, 3000)).toBe(0);
    expect(firstVisibleIndex(3000, 0, 3000)).toBe(0);
    expect(firstVisibleIndex(95000, 300, 3000)).toBe(2999);
    expect(firstVisibleIndex(0, 300, 0)).toBe(0);
  });
});
