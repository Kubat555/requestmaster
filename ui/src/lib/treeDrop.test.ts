import { describe, expect, it } from "vitest";
import { applyMove, dropPlace, dropZone, keyboardPlace, NODE_DRAG_TYPE } from "./treeDrop";
import type { TreeRow } from "./types";

function row(id: string, parentId: string | null, kind: TreeRow["kind"], sortOrder: number): TreeRow {
  return { id, parent_id: parentId, kind, name: id, sort_order: sortOrder, method: kind === "request" ? "GET" : null };
}

/** `C1[ F1[ R1, R2 ], F2[], R3 ], C2[]` */
const ROWS: TreeRow[] = [
  row("C1", null, "collection", 0),
  row("F1", "C1", "folder", 0),
  row("R1", "F1", "request", 0),
  row("R2", "F1", "request", 1),
  row("F2", "C1", "folder", 1),
  row("R3", "C1", "request", 2),
  row("C2", null, "collection", 1),
];

/** Ids of the children of the parent in the order of `sort_order`, which must be 0..n. */
function children(rows: TreeRow[], parentId: string | null): string[] {
  const siblings = rows.filter((candidate) => candidate.parent_id === parentId).sort((a, b) => a.sort_order - b.sort_order);
  expect(siblings.map((sibling) => sibling.sort_order)).toEqual(siblings.map((_, index) => index));
  return siblings.map((sibling) => sibling.id);
}

describe("NODE_DRAG_TYPE", () => {
  it("is the type a dragged node travels under", () => {
    expect(NODE_DRAG_TYPE).toBe("application/x-rqm-node");
  });
});

describe("dropZone", () => {
  it("splits a request in two halves", () => {
    expect(dropZone(0, "request", "request")).toBe("before");
    expect(dropZone(14, "request", "request")).toBe("before");
    expect(dropZone(15, "request", "request")).toBe("after");
    expect(dropZone(29, "request", "folder")).toBe("after");
    expect(dropZone(14, "request", "folder")).toBe("before");
  });

  it("gives a folder an edge above, an edge below and the middle", () => {
    expect(dropZone(0, "folder", "request")).toBe("before");
    expect(dropZone(7, "folder", "request")).toBe("before");
    expect(dropZone(8, "folder", "request")).toBe("inside");
    expect(dropZone(21, "folder", "folder")).toBe("inside");
    expect(dropZone(22, "folder", "folder")).toBe("after");
    expect(dropZone(29, "folder", "request")).toBe("after");
  });

  it("splits a collection in two halves for a dragged collection", () => {
    expect(dropZone(14, "collection", "collection")).toBe("before");
    expect(dropZone(15, "collection", "collection")).toBe("after");
  });

  it("puts a folder and a request inside a collection wherever the pointer is", () => {
    expect(dropZone(0, "collection", "folder")).toBe("inside");
    expect(dropZone(29, "collection", "folder")).toBe("inside");
    expect(dropZone(0, "collection", "request")).toBe("inside");
    expect(dropZone(29, "collection", "request")).toBe("inside");
  });

  it("has no zone for a collection over a folder or a request", () => {
    expect(dropZone(3, "folder", "collection")).toBeNull();
    expect(dropZone(15, "folder", "collection")).toBeNull();
    expect(dropZone(3, "request", "collection")).toBeNull();
    expect(dropZone(25, "request", "collection")).toBeNull();
  });
});

describe("dropPlace", () => {
  it("reorders among the siblings, counting the place without the moved node", () => {
    expect(dropPlace(ROWS, "R2", "R1", "before", false)).toEqual({ parentId: "F1", index: 0 });
    expect(dropPlace(ROWS, "R1", "R2", "after", false)).toEqual({ parentId: "F1", index: 1 });
  });

  it("is null when the place would not change", () => {
    expect(dropPlace(ROWS, "R1", "R2", "before", false)).toBeNull();
    expect(dropPlace(ROWS, "R2", "R1", "after", false)).toBeNull();
  });

  it("puts a node dropped inside as the last child", () => {
    expect(dropPlace(ROWS, "R3", "F2", "inside", true)).toEqual({ parentId: "F2", index: 0 });
    expect(dropPlace(ROWS, "R3", "F1", "inside", true)).toEqual({ parentId: "F1", index: 2 });
    expect(dropPlace(ROWS, "F1", "F2", "inside", false)).toEqual({ parentId: "F2", index: 0 });
    expect(dropPlace(ROWS, "R3", "C2", "inside", false)).toEqual({ parentId: "C2", index: 0 });
  });

  it("puts a node dropped after an expanded folder with children as its first child", () => {
    expect(dropPlace(ROWS, "R3", "F1", "after", true)).toEqual({ parentId: "F1", index: 0 });
  });

  it("puts a node dropped after a collapsed folder next to it", () => {
    expect(dropPlace(ROWS, "R3", "F1", "after", false)).toEqual({ parentId: "C1", index: 1 });
  });

  it("puts a node dropped after an expanded folder without children next to it", () => {
    expect(dropPlace(ROWS, "R1", "F2", "after", true)).toEqual({ parentId: "C1", index: 2 });
    expect(dropPlace(ROWS, "R1", "F2", "before", true)).toEqual({ parentId: "C1", index: 1 });
  });

  it("is null over the node itself and over its descendants", () => {
    expect(dropPlace(ROWS, "F1", "R1", "before", true)).toBeNull();
    expect(dropPlace(ROWS, "F1", "R1", "after", true)).toBeNull();
    expect(dropPlace(ROWS, "F1", "F1", "inside", true)).toBeNull();
    expect(dropPlace(ROWS, "F1", "F1", "before", true)).toBeNull();
    expect(dropPlace(ROWS, "C1", "R1", "after", true)).toBeNull();
  });

  it("reorders the collections", () => {
    expect(dropPlace(ROWS, "C2", "C1", "before", true)).toEqual({ parentId: null, index: 0 });
    expect(dropPlace(ROWS, "C1", "C2", "after", false)).toEqual({ parentId: null, index: 1 });
  });

  it("is null for a place the tree rules forbid", () => {
    // a request has no children
    expect(dropPlace(ROWS, "R3", "R1", "inside", false)).toBeNull();
    // only a collection is a root
    expect(dropPlace(ROWS, "R3", "C2", "after", false)).toBeNull();
    expect(dropPlace(ROWS, "F1", "C2", "before", false)).toBeNull();
    // a collection is only a root
    expect(dropPlace(ROWS, "C2", "F1", "inside", false)).toBeNull();
    expect(dropPlace(ROWS, "C2", "R3", "after", false)).toBeNull();
    // unknown nodes
    expect(dropPlace(ROWS, "nope", "R1", "after", false)).toBeNull();
    expect(dropPlace(ROWS, "R1", "nope", "after", false)).toBeNull();
  });
});

describe("keyboardPlace", () => {
  it("moves by one place among the siblings and stops at the edges", () => {
    expect(keyboardPlace(ROWS, "R1", "ArrowDown")).toEqual({ parentId: "F1", index: 1 });
    expect(keyboardPlace(ROWS, "R1", "ArrowUp")).toBeNull();
    expect(keyboardPlace(ROWS, "R2", "ArrowUp")).toEqual({ parentId: "F1", index: 0 });
    expect(keyboardPlace(ROWS, "R2", "ArrowDown")).toBeNull();
    expect(keyboardPlace(ROWS, "C2", "ArrowUp")).toEqual({ parentId: null, index: 0 });
    expect(keyboardPlace(ROWS, "C1", "ArrowDown")).toEqual({ parentId: null, index: 1 });
  });

  it("goes with the right arrow to the end of the previous sibling when that is a folder", () => {
    expect(keyboardPlace(ROWS, "R3", "ArrowRight")).toEqual({ parentId: "F2", index: 0 });
    expect(keyboardPlace(ROWS, "F2", "ArrowRight")).toEqual({ parentId: "F1", index: 2 });
    // the previous sibling is a request; there is no previous sibling
    expect(keyboardPlace(ROWS, "R2", "ArrowRight")).toBeNull();
    expect(keyboardPlace(ROWS, "F1", "ArrowRight")).toBeNull();
  });

  it("goes with the left arrow out of the parent, right after it", () => {
    expect(keyboardPlace(ROWS, "R1", "ArrowLeft")).toEqual({ parentId: "C1", index: 1 });
    // the parent is a collection: only a collection is a root
    expect(keyboardPlace(ROWS, "F1", "ArrowLeft")).toBeNull();
    expect(keyboardPlace(ROWS, "R3", "ArrowLeft")).toBeNull();
  });

  it("moves a collection only up and down", () => {
    expect(keyboardPlace(ROWS, "C1", "ArrowRight")).toBeNull();
    expect(keyboardPlace(ROWS, "C2", "ArrowRight")).toBeNull();
    expect(keyboardPlace(ROWS, "C2", "ArrowLeft")).toBeNull();
  });

  it("is null for an unknown node", () => {
    expect(keyboardPlace(ROWS, "nope", "ArrowDown")).toBeNull();
  });
});

describe("applyMove", () => {
  it("reorders the siblings and numbers them from 0", () => {
    const moved = applyMove(ROWS, "R3", { parentId: "C1", index: 0 });

    expect(children(moved, "C1")).toEqual(["R3", "F1", "F2"]);
    expect(children(moved, "F1")).toEqual(["R1", "R2"]);
    expect(children(moved, null)).toEqual(["C1", "C2"]);
  });

  it("moves to another parent and numbers the old and the new siblings from 0", () => {
    const moved = applyMove(ROWS, "R1", { parentId: "C1", index: 1 });

    expect(moved.find((candidate) => candidate.id === "R1")!.parent_id).toBe("C1");
    expect(children(moved, "C1")).toEqual(["F1", "R1", "F2", "R3"]);
    expect(children(moved, "F1")).toEqual(["R2"]);
    expect(moved.find((candidate) => candidate.id === "R2")!.sort_order).toBe(0);
  });

  it("takes an index past the end as the last place", () => {
    const moved = applyMove(ROWS, "R1", { parentId: "F2", index: 99 });

    expect(children(moved, "F2")).toEqual(["R1"]);
    expect(children(moved, "F1")).toEqual(["R2"]);
  });

  it("reorders the collections", () => {
    const moved = applyMove(ROWS, "C2", { parentId: null, index: 0 });

    expect(children(moved, null)).toEqual(["C2", "C1"]);
    expect(children(moved, "C1")).toEqual(["F1", "F2", "R3"]);
  });

  it("keeps the given rows and their other fields as they were", () => {
    const before = structuredClone(ROWS);
    const moved = applyMove(ROWS, "R1", { parentId: "F2", index: 0 });

    expect(ROWS).toEqual(before);
    expect(moved).toHaveLength(ROWS.length);
    expect(moved.find((candidate) => candidate.id === "R1")).toEqual({ ...ROWS[2], parent_id: "F2", sort_order: 0 });
  });

  it("returns the rows unchanged for an unknown node", () => {
    expect(applyMove(ROWS, "nope", { parentId: "C1", index: 0 })).toEqual(ROWS);
  });
});
