import { createEvent, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { bigTree } from "../lib/testing/bigTree";
import { createFakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import { NODE_DRAG_TYPE } from "../lib/treeDrop";
import type { NodeDto, RequestDraft } from "../lib/types";
import { Workspace } from "../lib/workspace.svelte";

/** `C1[ F1[ R1, R2 ], F2[], R3 ], C2[]` */
const NODES: NodeDto[] = [
  { id: "C1", parent_id: null, kind: "collection", name: "Shop", content: {} },
  { id: "F1", parent_id: "C1", kind: "folder", name: "Users", content: {} },
  { id: "R1", parent_id: "F1", kind: "request", name: "List users", content: { method: "GET", url: "https://api.example.test/users" } },
  { id: "R2", parent_id: "F1", kind: "request", name: "Create user", content: { method: "POST", url: "https://api.example.test/users" } },
  { id: "F2", parent_id: "C1", kind: "folder", name: "Orders", content: {} },
  { id: "R3", parent_id: "C1", kind: "request", name: "Ping", content: { method: "GET", url: "https://api.example.test/ping" } },
  { id: "C2", parent_id: null, kind: "collection", name: "Billing", content: {} },
];

/** The rows of {@link NODES} with everything expanded, top to bottom. */
const ORDER = ["C1", "F1", "R1", "R2", "F2", "R3", "C2"];

/** What a field of the transfer holds until somebody writes it. */
const UNSET = "unset";

/**
 * What a drag carries, as a plain object: the test environment has no `DataTransfer`. The events of one drag
 * carry the same object, so a test reads from it what the handlers wrote.
 */
function transfer(types: string[] = [], files: File[] = []) {
  const data = new Map<string, string>();
  return {
    types: [...types],
    files,
    effectAllowed: UNSET,
    dropEffect: UNSET,
    setData(type: string, value: string) {
      data.set(type, value);
      if (!this.types.includes(type)) this.types.push(type);
    },
    getData(type: string) {
      return data.get(type) ?? "";
    },
  };
}

/**
 * Fires an event of a drag at the height `clientY` of the window. The test environment has no `DragEvent`: the
 * event is a plain one, and the coordinate does not get into it from the init.
 */
async function drag(type: "dragStart" | "dragOver" | "drop", target: Element, clientY: number, dataTransfer: object) {
  const event = createEvent[type](target, { dataTransfer });
  Object.defineProperty(event, "clientY", { value: clientY });
  await fireEvent(target, event);
  return event;
}

/** Every row starts 100 px from the top of the window and is 30 px high: the test environment has no layout. */
function placeRows() {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({
    top: 100,
    bottom: 130,
    left: 0,
    right: 280,
    x: 0,
    y: 100,
    width: 280,
    height: 30,
    toJSON: () => ({}),
  });
}

/** Renders the sidebar over the tree with everything expanded. As in the window, an opened node gets a tab. */
async function renderTree(nodes: NodeDto[] = NODES) {
  const api = createFakeApi({ nodes });
  const workspace = new Workspace();
  workspace.onOpen = (id) => void tabs.open(api, id);
  await workspace.load(api);
  workspace.expand(nodes.filter((node) => node.kind !== "request").map((node) => node.id));
  const { container } = render(Sidebar, { props: { workspace }, context: new Map([["api", api]]) });
  placeRows();

  /** The row of the node, if it is in the DOM. */
  const rowOf = (id: string) => container.querySelector<HTMLElement>(`[role="treeitem"][data-id="${id}"]`);
  /** The ids of the rows in the DOM, top to bottom. */
  const order = () =>
    [...container.querySelectorAll<HTMLElement>('[role="treeitem"]')]
      .sort((a, b) => parseFloat(a.style.top) - parseFloat(b.style.top))
      .map((item) => item.dataset.id);
  /** The classes of the rows that say where a drop would go. */
  const dropClasses = () =>
    [...container.querySelectorAll<HTMLElement>('[role="treeitem"]')].flatMap((item) =>
      [...item.classList].filter((name) => name.startsWith("drop-")).map((name) => `${item.dataset.id}:${name}`),
    );
  const moveNode = vi.spyOn(api, "moveNode");
  const importFiles = vi.spyOn(api, "importFiles");

  /** Starts dragging the node and moves it over the row of `overId` at the height `clientY`. */
  const dragOver = async (id: string, overId: string, clientY: number) => {
    const dataTransfer = transfer();
    await drag("dragStart", rowOf(id)!, 110, dataTransfer);
    const event = await drag("dragOver", rowOf(overId)!, clientY, dataTransfer);
    return { dataTransfer, event };
  };
  /** Drags the node over the row of `overId` and drops it there. */
  const dragAndDrop = async (id: string, overId: string, clientY: number) => {
    const { dataTransfer } = await dragOver(id, overId, clientY);
    return drag("drop", rowOf(overId)!, clientY, dataTransfer);
  };
  return { api, workspace, container, rowOf, order, dropClasses, moveNode, importFiles, dragOver, dragAndDrop };
}

/** Lets a started command reach the api, if there were one. */
function settle() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

afterEach(() => {
  vi.restoreAllMocks();
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("dragging a node of the tree: the zones of a row", () => {
  it("puts a request dropped on the middle of a folder inside it", async () => {
    const { workspace, rowOf, order, dropClasses, moveNode, dragOver } = await renderTree();
    workspace.toggle("F2");
    await waitFor(() => expect(rowOf("F2")!.getAttribute("aria-expanded")).toBe("false"));

    const { dataTransfer } = await dragOver("R1", "F2", 115);
    expect(dataTransfer.getData(NODE_DRAG_TYPE)).toBe("R1");
    expect(dataTransfer.effectAllowed).toBe("move");
    expect(dropClasses()).toEqual(["F2:drop-inside"]);

    await drag("drop", rowOf("F2")!, 115, dataTransfer);

    expect(moveNode).toHaveBeenCalledTimes(1);
    expect(moveNode).toHaveBeenCalledWith("R1", "F2", 0);
    // The row is under the folder at once, and the folder is expanded so that it is seen.
    expect(order()).toEqual(["C1", "F1", "R2", "F2", "R1", "R3", "C2"]);
    expect(rowOf("R1")!.getAttribute("aria-level")).toBe("3");
    expect(rowOf("F2")!.getAttribute("aria-expanded")).toBe("true");
    expect(dropClasses()).toEqual([]);
    await waitFor(() => expect(document.activeElement).toBe(rowOf("R1")));
    expect(workspace.selectedId).toBeNull();
  });

  it("puts a request dropped on the upper edge of a folder before it", async () => {
    const { rowOf, order, dropClasses, moveNode, dragOver } = await renderTree();

    const { dataTransfer } = await dragOver("R1", "F2", 103);
    expect(dropClasses()).toEqual(["F2:drop-before"]);
    await drag("drop", rowOf("F2")!, 103, dataTransfer);

    expect(moveNode).toHaveBeenCalledTimes(1);
    expect(moveNode).toHaveBeenCalledWith("R1", "C1", 1);
    expect(order()).toEqual(["C1", "F1", "R2", "R1", "F2", "R3", "C2"]);
  });

  it("puts a request dropped on the lower edge of an empty folder after it", async () => {
    const { rowOf, order, dropClasses, moveNode, dragOver } = await renderTree();

    const { dataTransfer } = await dragOver("R1", "F2", 126);
    expect(dropClasses()).toEqual(["F2:drop-after"]);
    await drag("drop", rowOf("F2")!, 126, dataTransfer);

    expect(moveNode).toHaveBeenCalledTimes(1);
    expect(moveNode).toHaveBeenCalledWith("R1", "C1", 2);
    expect(order()).toEqual(["C1", "F1", "R2", "F2", "R1", "R3", "C2"]);
    expect(rowOf("R1")!.getAttribute("aria-level")).toBe("2");
  });

  it("puts a request dropped on the lower half of a request after it", async () => {
    const { rowOf, order, dropClasses, moveNode, dragOver } = await renderTree();

    const { dataTransfer } = await dragOver("R3", "R1", 120);
    expect(dropClasses()).toEqual(["R1:drop-after"]);
    await drag("drop", rowOf("R1")!, 120, dataTransfer);

    expect(moveNode).toHaveBeenCalledTimes(1);
    expect(moveNode).toHaveBeenCalledWith("R3", "F1", 1);
    expect(order()).toEqual(["C1", "F1", "R1", "R3", "R2", "F2", "C2"]);
  });

  it("reorders the collections", async () => {
    const { order, dropClasses, moveNode, dragOver, rowOf } = await renderTree();

    const { dataTransfer } = await dragOver("C2", "C1", 103);
    expect(dropClasses()).toEqual(["C1:drop-before"]);
    await drag("drop", rowOf("C1")!, 103, dataTransfer);

    expect(moveNode).toHaveBeenCalledWith("C2", null, 0);
    expect(order()).toEqual(["C2", "C1", "F1", "R1", "R2", "F2", "R3"]);
  });

  it("moves the mark to the row the pointer went to and removes it when the pointer leaves the rows", async () => {
    const { rowOf, dropClasses, dragOver } = await renderTree();

    const { dataTransfer } = await dragOver("R1", "F2", 115);
    await drag("dragOver", rowOf("R3")!, 103, dataTransfer);
    expect(dropClasses()).toEqual(["R3:drop-before"]);

    await fireEvent.dragLeave(rowOf("R3")!);
    expect(dropClasses()).toEqual([]);
  });

  it("removes the mark when the drag ends without a drop", async () => {
    const { rowOf, dropClasses, moveNode, dragOver } = await renderTree();

    await dragOver("R1", "F2", 115);
    await fireEvent.dragEnd(rowOf("R1")!);

    expect(dropClasses()).toEqual([]);
    expect(moveNode).not.toHaveBeenCalled();
  });
});

describe("dragging a node of the tree: what the browser is told", () => {
  it("allows the drop over a row that takes the node, and the listener of the window leaves it so", async () => {
    const { dragOver } = await renderTree();

    // The event goes from the row up to the window, where the sidebar listens for dragged files.
    const { dataTransfer, event } = await dragOver("R1", "F2", 115);

    expect(dataTransfer.dropEffect).toBe("move");
    expect(event.defaultPrevented).toBe(true);
  });

  it("does not allow a folder over its own descendant", async () => {
    const { rowOf, order, dropClasses, moveNode, dragOver } = await renderTree();

    const { dataTransfer, event } = await dragOver("F1", "R1", 120);

    expect(event.defaultPrevented).toBe(false);
    expect(dataTransfer.dropEffect).toBe(UNSET);
    expect(dropClasses()).toEqual([]);

    await drag("drop", rowOf("R1")!, 120, dataTransfer);
    await settle();

    expect(moveNode).not.toHaveBeenCalled();
    expect(order()).toEqual(ORDER);
  });

  it("does not allow a node over itself", async () => {
    const { rowOf, dropClasses, moveNode, dragOver } = await renderTree();

    const { dataTransfer, event } = await dragOver("F1", "F1", 115);

    expect(event.defaultPrevented).toBe(false);
    expect(dropClasses()).toEqual([]);
    await drag("drop", rowOf("F1")!, 115, dataTransfer);
    await settle();
    expect(moveNode).not.toHaveBeenCalled();
  });

  it("does not allow a collection over a folder, and a place that changes nothing", async () => {
    const { rowOf, dropClasses, moveNode, dragOver } = await renderTree();

    const collection = await dragOver("C2", "F1", 115);
    expect(collection.event.defaultPrevented).toBe(false);
    expect(dropClasses()).toEqual([]);
    await fireEvent.dragEnd(rowOf("C2")!);

    // R1 already stands right before R2.
    const same = await dragOver("R1", "R2", 103);
    expect(same.event.defaultPrevented).toBe(false);
    expect(dropClasses()).toEqual([]);
    await drag("drop", rowOf("R2")!, 103, same.dataTransfer);
    await settle();

    expect(moveNode).not.toHaveBeenCalled();
  });

  it("imports a file dragged over the tree as before, even while a node is being dragged", async () => {
    const { rowOf, order, dropClasses, moveNode, importFiles, dragOver } = await renderTree();
    importFiles.mockResolvedValue([]);
    // The drag of a node that was never finished must not take a file for that node.
    await dragOver("R1", "F1", 103);
    const file = new File(["{}"], "dropped.json", { type: "application/json" });
    const dataTransfer = transfer(["Files"], [file]);

    const event = await drag("dragOver", rowOf("F2")!, 115, dataTransfer);

    expect(dataTransfer.dropEffect).toBe("copy");
    expect(event.defaultPrevented).toBe(true);
    expect(dropClasses()).toEqual([]);

    const drop = await drag("drop", rowOf("F2")!, 115, dataTransfer);
    await screen.findByRole("dialog");

    expect(drop.defaultPrevented).toBe(true);
    expect(importFiles).toHaveBeenCalledWith([{ name: "dropped.json", text: "{}" }]);
    expect(moveNode).not.toHaveBeenCalled();
    expect(order()).toEqual(ORDER);
  });

  it("forbids a file outside the sidebar", async () => {
    await renderTree();
    const dataTransfer = transfer(["Files"], [new File(["{}"], "stray.json")]);

    const event = await drag("dragOver", document.body, 115, dataTransfer);

    expect(dataTransfer.dropEffect).toBe("none");
    expect(event.defaultPrevented).toBe(true);
  });

  it("forbids a link or a text dragged from another program, without following it", async () => {
    const { rowOf, dropClasses, moveNode, importFiles } = await renderTree();
    const dataTransfer = transfer(["text/uri-list"]);

    const event = await drag("dragOver", rowOf("F2")!, 115, dataTransfer);

    expect(event.defaultPrevented).toBe(true);
    expect(dataTransfer.dropEffect).toBe("none");
    expect(dropClasses()).toEqual([]);

    const drop = await drag("drop", rowOf("F2")!, 115, dataTransfer);
    await settle();

    expect(drop.defaultPrevented).toBe(true);
    expect(importFiles).not.toHaveBeenCalled();
    expect(moveNode).not.toHaveBeenCalled();
  });

  it("does not import when a node is dropped beside the rows", async () => {
    const { rowOf, order, moveNode, importFiles } = await renderTree();
    const beside = screen.getByRole("searchbox", { name: strings.sidebar.filter });
    const dataTransfer = transfer();
    await drag("dragStart", rowOf("R1")!, 110, dataTransfer);

    const over = await drag("dragOver", beside, 20, dataTransfer);
    const drop = await drag("drop", beside, 20, dataTransfer);
    await settle();

    // The browser itself forbids the drop there; the window only keeps the webview from acting on it.
    expect(over.defaultPrevented).toBe(false);
    expect(dataTransfer.dropEffect).toBe(UNSET);
    expect(drop.defaultPrevented).toBe(true);
    expect(importFiles).not.toHaveBeenCalled();
    expect(moveNode).not.toHaveBeenCalled();
    expect(order()).toEqual(ORDER);
  });
});

describe("dragging a node of the tree: when it is off and when it fails", () => {
  it("makes the rows draggable", async () => {
    const { rowOf } = await renderTree();

    for (const id of ORDER) expect(rowOf(id)!.getAttribute("draggable")).toBe("true");
  });

  it("is off under a filter", async () => {
    const { rowOf, order, dropClasses, moveNode, dragOver } = await renderTree();
    await fireEvent.input(screen.getByRole("searchbox", { name: strings.sidebar.filter }), { target: { value: "user" } });
    expect(order()).toEqual(["C1", "F1", "R1", "R2"]);

    for (const id of order()) {
      expect(rowOf(id!)!.draggable).toBe(false);
      expect(rowOf(id!)!.getAttribute("draggable")).toBe("false");
    }

    // An event that comes all the same starts nothing: no node is put into the transfer, no row takes it.
    const { dataTransfer } = await dragOver("R2", "R1", 103);
    expect(dataTransfer.types).toEqual([]);
    expect(dataTransfer.effectAllowed).toBe(UNSET);
    expect(dataTransfer.dropEffect).not.toBe("move");
    expect(dropClasses()).toEqual([]);
    await drag("drop", rowOf("R1")!, 103, dataTransfer);
    await settle();
    expect(moveNode).not.toHaveBeenCalled();
  });

  it("is off for the row that is being renamed", async () => {
    const { rowOf } = await renderTree();

    await fireEvent.keyDown(rowOf("R1")!, { key: "F2" });
    await screen.findByRole("textbox", { name: strings.tree.renameLabel });

    expect(rowOf("R1")!.draggable).toBe(false);
    expect(rowOf("R1")!.getAttribute("draggable")).toBe("false");
    expect(rowOf("R2")!.getAttribute("draggable")).toBe("true");
  });

  it("shows a notification and the order as it was when the move is refused", async () => {
    const { order, moveNode, dragAndDrop } = await renderTree();
    moveNode.mockRejectedValue("disk is full");

    await dragAndDrop("R1", "F2", 115);

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ kind: "error", text: "disk is full" });
    await waitFor(() => expect(order()).toEqual(ORDER));
  });

  it("keeps the tab, the draft and the mark of a moved request with unsaved edits", async () => {
    const { api, rowOf, moveNode, dragAndDrop } = await renderTree();
    await tabs.open(api, "R1");
    const tab = tabs.list[0] as Tab;
    (tab.draft as RequestDraft).url = "https://api.example.test/people";
    await waitFor(() => expect(rowOf("R1")!.querySelector(".dirty")).not.toBeNull());
    expect(tabs.active).toBe(tab);

    await dragAndDrop("R1", "F2", 115);
    await waitFor(() => expect(document.activeElement).toBe(rowOf("R1")));

    expect(moveNode).toHaveBeenCalledWith("R1", "F2", 0);
    expect(tabs.list).toHaveLength(1);
    expect(tabs.list[0]).toBe(tab);
    expect(tabs.active).toBe(tab);
    expect(tab.dirty).toBe(true);
    expect((tab.draft as RequestDraft).url).toBe("https://api.example.test/people");
    expect(rowOf("R1")!.querySelector(".dirty")).not.toBeNull();
    // The stored request is as it was: a move writes the place only.
    expect((await api.getNode("R1")).content).toMatchObject({ url: "https://api.example.test/users" });
  });

  it("keeps the dragged row in the DOM when the tree is scrolled away from it", async () => {
    vi.spyOn(Element.prototype, "clientHeight", "get").mockImplementation(function (this: Element) {
      return this.classList.contains("tree-scroll") ? 300 : 0;
    });
    const big = bigTree(30, 100);
    const { container, rowOf } = await renderTree(big);
    const scroller = container.querySelector<HTMLElement>(".tree-scroll")!;
    const row = rowOf(big[3].id)!;

    await drag("dragStart", row, 110, transfer());
    scroller.scrollTop = 3000;
    await fireEvent.scroll(scroller);

    expect(rowOf(big[0].id)).toBeNull();
    expect(rowOf(big[100].id)).not.toBeNull();
    expect(rowOf(big[3].id)).toBe(row);
    expect(row.isConnected).toBe(true);

    // Once the drag is over the row is not held any more.
    await fireEvent.dragEnd(row);
    expect(rowOf(big[3].id)).toBeNull();
  });
});

describe("moving a node of the tree with the keyboard", () => {
  it("moves a request down among its siblings with Alt+ArrowDown and keeps the focus on it", async () => {
    const { workspace, rowOf, order, moveNode } = await renderTree();
    const row = rowOf("R1")!;
    row.focus();

    const event = createEvent.keyDown(row, { key: "ArrowDown", altKey: true });
    await fireEvent(row, event);

    expect(event.defaultPrevented).toBe(true);
    expect(moveNode).toHaveBeenCalledTimes(1);
    expect(moveNode).toHaveBeenCalledWith("R1", "F1", 1);
    expect(order()).toEqual(["C1", "F1", "R2", "R1", "F2", "R3", "C2"]);
    await settle();
    expect(document.activeElement).toBe(rowOf("R1"));
    expect(rowOf("R1")).toBe(row);
    expect(workspace.selectedId).toBeNull();
  });

  it("moves a request into the folder above it with Alt+ArrowRight", async () => {
    const { rowOf, order, moveNode } = await renderTree();
    const row = rowOf("R3")!;
    row.focus();

    await fireEvent.keyDown(row, { key: "ArrowRight", altKey: true });

    expect(moveNode).toHaveBeenCalledTimes(1);
    expect(moveNode).toHaveBeenCalledWith("R3", "F2", 0);
    expect(order()).toEqual(["C1", "F1", "R1", "R2", "F2", "R3", "C2"]);
    expect(rowOf("R3")!.getAttribute("aria-level")).toBe("3");
    await settle();
    expect(document.activeElement).toBe(rowOf("R3"));
  });

  it("moves a request out of its folder with Alt+ArrowLeft", async () => {
    const { rowOf, order, moveNode } = await renderTree();

    await fireEvent.keyDown(rowOf("R1")!, { key: "ArrowLeft", altKey: true });

    expect(moveNode).toHaveBeenCalledWith("R1", "C1", 1);
    expect(order()).toEqual(["C1", "F1", "R2", "R1", "F2", "R3", "C2"]);
  });

  it("does nothing at the edge, and does not move the focus instead", async () => {
    const { rowOf, order, moveNode } = await renderTree();
    const row = rowOf("R1")!;
    row.focus();

    const event = createEvent.keyDown(row, { key: "ArrowUp", altKey: true });
    await fireEvent(row, event);
    await settle();

    expect(event.defaultPrevented).toBe(true);
    expect(moveNode).not.toHaveBeenCalled();
    expect(order()).toEqual(ORDER);
    expect(document.activeElement).toBe(row);
  });

  it("does nothing under a filter", async () => {
    const { rowOf, order, moveNode } = await renderTree();
    await fireEvent.input(screen.getByRole("searchbox", { name: strings.sidebar.filter }), { target: { value: "user" } });
    const row = rowOf("R1")!;
    row.focus();

    await fireEvent.keyDown(row, { key: "ArrowDown", altKey: true });
    await settle();

    expect(moveNode).not.toHaveBeenCalled();
    expect(order()).toEqual(["C1", "F1", "R1", "R2"]);
    expect(document.activeElement).toBe(row);
  });

  it("sends two moves in a row one after the other", async () => {
    const { rowOf, order, moveNode } = await renderTree();
    let finishFirst!: () => void;
    // The first command is answered only when the test says so.
    moveNode.mockImplementationOnce(() => new Promise<void>((resolve) => (finishFirst = resolve)));
    const row = rowOf("R1")!;
    row.focus();

    await fireEvent.keyDown(row, { key: "ArrowDown", altKey: true });
    await fireEvent.keyDown(row, { key: "ArrowUp", altKey: true });
    await settle();

    // The window shows both at once; the second command waits for the first one.
    expect(order()).toEqual(ORDER);
    expect(moveNode).toHaveBeenCalledTimes(1);
    expect(moveNode).toHaveBeenLastCalledWith("R1", "F1", 1);

    finishFirst();
    await waitFor(() => expect(moveNode).toHaveBeenCalledTimes(2));
    expect(moveNode).toHaveBeenLastCalledWith("R1", "F1", 0);
    await settle();
    expect(order()).toEqual(ORDER);
    expect(toasts.items).toEqual([]);
  });
});
