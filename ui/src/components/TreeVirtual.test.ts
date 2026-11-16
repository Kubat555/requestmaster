import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { tick } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import { strings } from "../lib/strings";
import { tabs } from "../lib/tabs.svelte";
import { bigTree } from "../lib/testing/bigTree";
import { createFakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import { Workspace } from "../lib/workspace.svelte";

/** One collection, 30 folders, 100 requests in each: 3031 rows when everything is expanded, in this order. */
const NODES = bigTree(30, 100);
const ROW_COUNT = 3031;

/**
 * Gives the scrolling area of the tree a height: the test environment has no layout, and an area without a
 * height draws every row.
 */
function setViewportHeight(height: number) {
  vi.spyOn(Element.prototype, "clientHeight", "get").mockImplementation(function (this: Element) {
    return this.classList.contains("tree-scroll") ? height : 0;
  });
}

/** Renders the sidebar over the big tree with everything expanded. */
async function renderTree() {
  const api = createFakeApi({ nodes: NODES });
  const workspace = new Workspace();
  workspace.onOpen = (id) => void tabs.open(api, id);
  await workspace.load(api);
  workspace.expand(NODES.filter((node) => node.kind !== "request").map((node) => node.id));
  const { container } = render(Sidebar, { props: { workspace }, context: new Map([["api", api]]) });
  const scroller = container.querySelector<HTMLElement>(".tree-scroll")!;

  /** The rows in the DOM, top to bottom. */
  const items = () => [...container.querySelectorAll<HTMLElement>('[role="treeitem"]')];
  /** The row with this index in the list of rows, if it is in the DOM. */
  const rowAt = (index: number) => items().find((item) => item.dataset.id === NODES[index].id) ?? null;
  /** The rows Tab stops at. */
  const tabStops = () => items().filter((item) => item.getAttribute("tabindex") === "0");
  const scrollTo = async (top: number) => {
    scroller.scrollTop = top;
    await fireEvent.scroll(scroller);
  };
  return { api, workspace, container, scroller, items, rowAt, tabStops, scrollTo };
}

afterEach(() => {
  vi.restoreAllMocks();
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("a tree of 3000 requests", () => {
  it("draws only the rows near the viewport over a spacer of the full height", async () => {
    setViewportHeight(300);
    const { items, rowAt } = await renderTree();

    expect(items().length).toBeGreaterThan(0);
    expect(items().length).toBeLessThanOrEqual(30);
    expect(rowAt(0)).not.toBeNull();
    expect(screen.getByRole("tree").style.height).toBe("90930px");
  });

  it("puts every row where its index says and tells its place among the siblings", async () => {
    setViewportHeight(300);
    const { rowAt, scrollTo } = await renderTree();
    await scrollTo(3000);

    // Index 100 is the 99th request of the first folder.
    const row = rowAt(100)!;
    expect(row.style.top).toBe("3000px");
    expect(row.getAttribute("aria-posinset")).toBe("99");
    expect(row.getAttribute("aria-setsize")).toBe("100");
    expect(row.getAttribute("aria-level")).toBe("3");
  });

  it("draws the rows of the new place after a scroll", async () => {
    setViewportHeight(300);
    const { items, rowAt, scrollTo } = await renderTree();

    await scrollTo(3000);

    expect(rowAt(100)).not.toBeNull();
    expect(rowAt(0)).toBeNull();
    expect(items().length).toBeLessThanOrEqual(30);
  });

  it("keeps the focused row in the DOM and focused when it is scrolled away", async () => {
    setViewportHeight(300);
    const { rowAt, scrollTo } = await renderTree();
    const row = rowAt(3)!;
    row.focus();
    await tick();
    expect(document.activeElement).toBe(row);

    await scrollTo(3000);

    expect(rowAt(0)).toBeNull();
    expect(rowAt(3)).toBe(row);
    expect(row.isConnected).toBe(true);
    expect(document.activeElement).toBe(row);
  });

  it("keeps the field of a row being renamed, with what was typed, when it is scrolled away", async () => {
    setViewportHeight(300);
    const { rowAt, scrollTo } = await renderTree();
    await fireEvent.keyDown(rowAt(3)!, { key: "F2" });
    const input = await screen.findByRole<HTMLInputElement>("textbox", { name: strings.tree.renameLabel });
    await fireEvent.input(input, { target: { value: "Typed name" } });

    await scrollTo(3000);

    expect(rowAt(0)).toBeNull();
    const still = screen.getByRole<HTMLInputElement>("textbox", { name: strings.tree.renameLabel });
    expect(still).toBe(input);
    expect(still.value).toBe("Typed name");
    expect(document.activeElement).toBe(input);
  });

  it("moves the focus with ArrowDown from the last drawn row to the next one", async () => {
    setViewportHeight(300);
    const { items, rowAt, scroller } = await renderTree();
    const last = items().at(-1)!;
    expect(last).toBe(rowAt(17));
    expect(rowAt(18)).toBeNull();
    last.focus();

    await fireEvent.keyDown(last, { key: "ArrowDown" });

    await waitFor(() => expect(rowAt(18)).not.toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(rowAt(18)));
    // The row is scrolled to just far enough to be seen whole: 19 rows of 30 px end at 570.
    expect(scroller.scrollTop).toBe(270);
  });

  it("moves the focus with ArrowUp to a row above the drawn ones", async () => {
    setViewportHeight(300);
    const { items, rowAt, scroller, scrollTo } = await renderTree();
    await scrollTo(3000);
    const first = items()[0];
    expect(first).toBe(rowAt(92));
    first.focus();

    await fireEvent.keyDown(first, { key: "ArrowUp" });

    await waitFor(() => expect(document.activeElement).toBe(rowAt(91)));
    expect(scroller.scrollTop).toBe(2730);
  });

  it("leaves three rows under a filter by the name of one request", async () => {
    setViewportHeight(300);
    const { items, rowAt, scrollTo } = await renderTree();
    await scrollTo(3000);
    // The 51st request of the 21st folder.
    const index = 1 + 20 * 101 + 51;
    const request = NODES[index];
    expect(request.kind).toBe("request");

    await fireEvent.input(screen.getByRole("searchbox", { name: strings.sidebar.filter }), {
      target: { value: request.name },
    });

    expect(items()).toHaveLength(3);
    expect(items().map((item) => item.dataset.id)).toEqual([NODES[0].id, request.parent_id, request.id]);
    expect(rowAt(index)!.style.top).toBe("60px");
    expect(screen.getByRole("tree").style.height).toBe("90px");
  });

  it("closes the menu of a row when the tree is scrolled", async () => {
    setViewportHeight(300);
    const { rowAt, scrollTo } = await renderTree();
    await fireEvent.contextMenu(rowAt(2)!);
    await screen.findByRole("menu");

    await scrollTo(3000);

    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("draws every row while the area has no height", async () => {
    const { items, scrollTo } = await renderTree();
    expect(items()).toHaveLength(ROW_COUNT);

    await scrollTo(3000);

    expect(items()).toHaveLength(ROW_COUNT);
  }, 60_000);
});

describe("the row Tab stops at", () => {
  it("is the first row while the tree was neither focused nor scrolled", async () => {
    setViewportHeight(300);
    const { rowAt, tabStops } = await renderTree();

    expect(tabStops()).toHaveLength(1);
    expect(tabStops()[0]).toBe(rowAt(0));
  });

  it("is the first row wholly inside the viewport after a scroll without a focus", async () => {
    setViewportHeight(300);
    const { rowAt, tabStops, scrollTo } = await renderTree();

    await scrollTo(3000);

    expect(tabStops()).toHaveLength(1);
    expect(tabStops()[0]).toBe(rowAt(100));
  });

  it("is the focused row wherever the tree is scrolled", async () => {
    setViewportHeight(300);
    const { rowAt, tabStops, scrollTo } = await renderTree();
    const row = rowAt(3)!;
    row.focus();
    await tick();

    await scrollTo(3000);

    expect(tabStops()).toHaveLength(1);
    expect(tabStops()[0]).toBe(row);
  });
});
