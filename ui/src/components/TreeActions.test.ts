import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import TabBar from "./TabBar.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { createFakeApi, type FakeSeed } from "../lib/testing/fakeApi";
import { pressEnter } from "../lib/testing/keys";
import { toasts } from "../lib/toasts.svelte";
import type { FolderDraft, RequestDraft } from "../lib/types";
import { Workspace } from "../lib/workspace.svelte";

const NAMES = strings.tree.newNode;

/**
 * Loads the workspace from a fake api and renders the sidebar over it. As in the window, a node opened in the
 * tree gets a tab; `expand` lists the nodes shown expanded.
 */
async function renderSidebar(seed: FakeSeed = {}, expand: string[] = []) {
  const api = createFakeApi(seed);
  const workspace = new Workspace();
  const onOpen = vi.fn((id: string) => void tabs.open(api, id));
  workspace.onOpen = onOpen;
  await workspace.load(api);
  workspace.expand(expand);
  const view = render(Sidebar, { props: { workspace }, context: new Map([["api", api]]) });
  return { api, workspace, onOpen, ...view };
}

/** What every row shows, top to bottom: the method of a request and the name. */
function treeItemNames(): string[] {
  return screen.queryAllByRole("treeitem").map((item) => item.textContent!.replace(/\s+/g, " ").trim());
}

/** Opens the menu of the "+" button of the sidebar. */
async function openAddMenu(): Promise<HTMLElement> {
  await fireEvent.click(screen.getByRole("button", { name: strings.tree.add }));
  return screen.findByRole("menu");
}

/** Chooses an item of the "+" menu. */
async function add(label: string) {
  const menu = await openAddMenu();
  await fireEvent.click(within(menu).getByRole("menuitem", { name: label }));
}

/** Opens the menu of a row with the right mouse button. */
async function openRowMenu(row: HTMLElement): Promise<HTMLElement> {
  await fireEvent.contextMenu(row);
  return screen.findByRole("menu");
}

/** Chooses an item of the menu of a row. */
async function choose(row: HTMLElement, label: string) {
  const menu = await openRowMenu(row);
  await fireEvent.click(within(menu).getByRole("menuitem", { name: label }));
}

/** The field a row shows instead of its name while it is being renamed. */
function renameInput(): Promise<HTMLInputElement> {
  return screen.findByRole<HTMLInputElement>("textbox", { name: strings.tree.renameLabel });
}

function queryRenameInput(): HTMLElement | null {
  return screen.queryByRole("textbox", { name: strings.tree.renameLabel });
}

async function type(input: HTMLInputElement, value: string) {
  await fireEvent.input(input, { target: { value } });
}

// The tabs and the notifications of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("creating in the tree", () => {
  it("creates a collection after the last one", async () => {
    const { api, workspace } = await renderSidebar();
    const createNode = vi.spyOn(api, "createNode");

    await add(NAMES.collection);
    const input = await renameInput();

    expect(createNode).toHaveBeenCalledTimes(1);
    expect(createNode).toHaveBeenCalledWith(null, "collection", NAMES.collection);
    const id = await createNode.mock.results[0].value;
    expect(workspace.selectedId).toBe(id);

    await fireEvent.keyDown(input, { key: "Escape" });
    expect(treeItemNames()).toEqual(["Demo API", NAMES.collection]);
  });

  it("creates a folder at the end of the first collection when nothing is focused or selected", async () => {
    const { api, workspace } = await renderSidebar();
    const createNode = vi.spyOn(api, "createNode");

    await add(NAMES.folder);
    const input = await renameInput();

    expect(createNode).toHaveBeenCalledWith("col-1", "folder", NAMES.folder);
    // The collection was collapsed: it is expanded so that the new row is seen.
    expect(workspace.expanded.has("col-1")).toBe(true);
    expect(workspace.selectedId).toBe(await createNode.mock.results[0].value);

    await fireEvent.keyDown(input, { key: "Escape" });
    expect(treeItemNames()).toEqual(["Demo API", "Users", "Orders", NAMES.folder]);
  });

  it("creates in the selected node when no row had the focus", async () => {
    const { api, workspace } = await renderSidebar({
      nodes: [
        { id: "c1", parent_id: null, kind: "collection", name: "First", content: {} },
        { id: "c2", parent_id: null, kind: "collection", name: "Second", content: {} },
      ],
    });
    const createNode = vi.spyOn(api, "createNode");
    workspace.select("c2");

    await add(NAMES.request);
    await renameInput();

    expect(createNode).toHaveBeenCalledWith("c2", "request", NAMES.request);
  });

  it("creates a request in the focused folder, though a click on a folder does not select it", async () => {
    const { api, workspace } = await renderSidebar({}, ["col-1"]);
    const createNode = vi.spyOn(api, "createNode");
    workspace.select("col-1");
    const folder = await screen.findByRole("treeitem", { name: "Orders" });

    folder.focus();
    await fireEvent.click(folder);
    expect(workspace.selectedId).toBe("col-1");
    expect(workspace.focusedId).toBe("fold-orders");

    await add(NAMES.request);
    await renameInput();

    expect(createNode).toHaveBeenCalledTimes(1);
    expect(createNode).toHaveBeenCalledWith("fold-orders", "request", NAMES.request);
  });

  it("creates a request next to the focused request, opens it in a tab and starts renaming it", async () => {
    const { api, workspace, onOpen } = await renderSidebar({}, ["col-1", "fold-users"]);
    const createNode = vi.spyOn(api, "createNode");
    (await screen.findByRole("treeitem", { name: "GET List users" })).focus();

    await add(NAMES.request);
    const input = await renameInput();

    expect(createNode).toHaveBeenCalledWith("fold-users", "request", NAMES.request);
    const id = await createNode.mock.results[0].value;
    // The row is selected and its name is being edited, the whole text selected.
    expect(workspace.selectedId).toBe(id);
    expect(input.closest('[role="treeitem"]')!.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(input);
    expect(input.value).toBe(NAMES.request);
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe(NAMES.request.length);
    // The new request is opened in a tab.
    expect(onOpen).toHaveBeenCalledWith(id);
    await waitFor(() => expect(tabs.list.map((tab) => tab.nodeId)).toEqual([id]));

    await fireEvent.keyDown(input, { key: "Escape" });
    expect(treeItemNames()).toEqual([
      "Demo API",
      "Users",
      "GET List users",
      "POST Create user",
      `GET ${NAMES.request}`,
      "Orders",
    ]);
  });

  it("renames the new row with Enter", async () => {
    const { api } = await renderSidebar();
    const createNode = vi.spyOn(api, "createNode");
    const renameNode = vi.spyOn(api, "renameNode");

    await add(NAMES.folder);
    const input = await renameInput();
    await type(input, "Payments");
    await fireEvent.keyDown(input, { key: "Enter" });

    await screen.findByRole("treeitem", { name: "Payments" });
    expect(renameNode).toHaveBeenCalledTimes(1);
    expect(renameNode).toHaveBeenCalledWith(await createNode.mock.results[0].value, "Payments");
    expect(queryRenameInput()).toBeNull();
  });

  it("lets only a collection be created while there are no collections", async () => {
    const { api } = await renderSidebar({ nodes: [] });
    const createNode = vi.spyOn(api, "createNode");

    const menu = await openAddMenu();
    for (const label of [NAMES.folder, NAMES.request]) {
      const item = within(menu).getByRole("menuitem", { name: label });
      expect(item.getAttribute("aria-disabled")).toBe("true");
      expect(item.title).toBe(strings.tree.needCollection);
      await fireEvent.click(item);
    }
    expect(createNode).not.toHaveBeenCalled();
    const collection = within(menu).getByRole("menuitem", { name: NAMES.collection });
    expect(collection.hasAttribute("aria-disabled")).toBe(false);

    await fireEvent.click(collection);
    const input = await renameInput();

    expect(createNode).toHaveBeenCalledTimes(1);
    expect(createNode).toHaveBeenCalledWith(null, "collection", NAMES.collection);
    expect(screen.queryByText(strings.sidebar.empty)).toBeNull();
    expect(input.closest('[role="treeitem"]')).not.toBeNull();
  });

  it("enables every item of the add menu when there is a collection", async () => {
    await renderSidebar();
    const menu = await openAddMenu();

    const items = within(menu).getAllByRole("menuitem");
    expect(items.map((item) => item.textContent!.trim())).toEqual([NAMES.collection, NAMES.folder, NAMES.request]);
    for (const item of items) expect(item.hasAttribute("aria-disabled")).toBe(false);
  });

  it("shows a notification and leaves the tree as it was when the creation fails", async () => {
    const { api, workspace } = await renderSidebar({}, ["col-1"]);
    vi.spyOn(api, "createNode").mockRejectedValue("the disk is full");
    const tree = vi.spyOn(api, "tree");
    const before = treeItemNames();

    await add(NAMES.folder);

    await waitFor(() => expect(toasts.items.map((toast) => toast.text)).toEqual(["the disk is full"]));
    expect(toasts.items[0].kind).toBe("error");
    expect(tree).not.toHaveBeenCalled();
    expect(treeItemNames()).toEqual(before);
    expect(queryRenameInput()).toBeNull();
    expect(workspace.selectedId).toBeNull();
  });
});

describe("renaming in the tree", () => {
  it("starts with F2 on the row and saves when the field loses the focus", async () => {
    const { api } = await renderSidebar({}, ["col-1"]);
    const renameNode = vi.spyOn(api, "renameNode");
    const row = await screen.findByRole("treeitem", { name: "Users" });
    row.focus();

    await fireEvent.keyDown(row, { key: "F2" });
    const input = await renameInput();
    expect(row.contains(input)).toBe(true);
    expect(document.activeElement).toBe(input);
    expect(input.value).toBe("Users");
    expect(input.selectionEnd).toBe("Users".length);

    await type(input, "People");
    await fireEvent.blur(input);

    await screen.findByRole("treeitem", { name: "People" });
    expect(renameNode).toHaveBeenCalledTimes(1);
    expect(renameNode).toHaveBeenCalledWith("fold-users", "People");
    expect(treeItemNames()).toEqual(["Demo API", "People", "Orders"]);
  });

  it("starts from the menu of the row", async () => {
    await renderSidebar({}, ["col-1"]);
    const row = await screen.findByRole("treeitem", { name: "Orders" });

    await choose(row, strings.tree.rename);
    const input = await renameInput();

    expect(row.contains(input)).toBe(true);
    await waitFor(() => expect(document.activeElement).toBe(input));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("does not rename with Escape", async () => {
    const { api } = await renderSidebar({}, ["col-1"]);
    const renameNode = vi.spyOn(api, "renameNode");
    const row = await screen.findByRole("treeitem", { name: "Users" });

    await fireEvent.keyDown(row, { key: "F2" });
    const input = await renameInput();
    await type(input, "People");
    await fireEvent.keyDown(input, { key: "Escape" });

    await waitFor(() => expect(queryRenameInput()).toBeNull());
    expect(renameNode).not.toHaveBeenCalled();
    expect(treeItemNames()).toEqual(["Demo API", "Users", "Orders"]);
    // The keyboard stays on the row.
    await waitFor(() => expect(document.activeElement).toBe(row));
  });

  it("does not save an empty name", async () => {
    const { api } = await renderSidebar({}, ["col-1"]);
    const renameNode = vi.spyOn(api, "renameNode");
    const row = await screen.findByRole("treeitem", { name: "Users" });

    await fireEvent.keyDown(row, { key: "F2" });
    const input = await renameInput();
    expect(input.getAttribute("aria-invalid")).not.toBe("true");
    await type(input, "   ");
    await fireEvent.keyDown(input, { key: "Enter" });

    // The field stays, marked, and says why.
    expect(queryRenameInput()).toBe(input);
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.title).toBe(strings.tree.emptyName);
    expect(renameNode).not.toHaveBeenCalled();

    // Leaving the field with an empty name keeps the old one.
    await fireEvent.blur(input);
    await waitFor(() => expect(queryRenameInput()).toBeNull());
    expect(renameNode).not.toHaveBeenCalled();
    expect(treeItemNames()).toEqual(["Demo API", "Users", "Orders"]);
  });

  it("keeps Delete, the arrows and Enter of the field away from the row", async () => {
    const { api, onOpen } = await renderSidebar({}, ["col-1"]);
    const renameNode = vi.spyOn(api, "renameNode");
    const deleteNode = vi.spyOn(api, "deleteNode");
    const row = await screen.findByRole("treeitem", { name: "Demo API" });
    row.focus();

    await fireEvent.keyDown(row, { key: "F2" });
    const input = await renameInput();

    await fireEvent.keyDown(input, { key: "Delete" });
    expect(screen.queryByRole("dialog")).toBeNull();

    // On the row the left arrow would collapse the collection, the down arrow would move the focus.
    const left = createEvent.keyDown(input, { key: "ArrowLeft" });
    await fireEvent(input, left);
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(left.defaultPrevented).toBe(false);
    expect(row.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(input);

    // Neither do the clicks inside the field reach the row: the collection is not opened.
    await fireEvent.click(input);
    await fireEvent.dblClick(input);
    const contextMenu = createEvent.contextMenu(input);
    await fireEvent(input, contextMenu);
    expect(contextMenu.defaultPrevented).toBe(false);
    expect(screen.queryByRole("menu")).toBeNull();

    await type(input, "Shop API");
    await fireEvent.keyDown(input, { key: "Enter" });

    await screen.findByRole("treeitem", { name: "Shop API" });
    expect(renameNode).toHaveBeenCalledWith("col-1", "Shop API");
    expect(deleteNode).not.toHaveBeenCalled();
    expect(onOpen).not.toHaveBeenCalled();
    expect(tabs.list).toHaveLength(0);
  });

  it("renames the open tab at once and keeps its draft and its mark of unsaved edits", async () => {
    const { api } = await renderSidebar({}, ["col-1", "fold-users"]);
    render(TabBar, { props: { tabs, onclose: () => {}, onclosemany: () => {} } });
    const row = await screen.findByRole("treeitem", { name: "GET List users" });
    await tabs.open(api, "req-list-users");
    const tab = tabs.list[0] as Tab<RequestDraft>;
    tab.draft.url = "https://api.example.test/people";
    await waitFor(() => expect(row.querySelector(".dirty")).not.toBeNull());

    await fireEvent.keyDown(row, { key: "F2" });
    const input = await renameInput();
    await type(input, "All people");
    await fireEvent.keyDown(input, { key: "Enter" });

    const renamed = await screen.findByRole("treeitem", { name: "GET All people" });
    await waitFor(() => expect(tab.name).toBe("All people"));
    expect(await screen.findByRole("tab", { name: /All people/ })).toBeTruthy();
    expect(screen.queryByRole("tab", { name: /List users/ })).toBeNull();
    // The same tab with the same unsaved draft.
    expect(tabs.list).toEqual([tab]);
    expect(tab.dirty).toBe(true);
    expect(tab.draft.url).toBe("https://api.example.test/people");
    expect(renamed.querySelector(".dirty")).not.toBeNull();
    expect(within(screen.getByRole("tab", { name: /All people/ })).getByRole("img", { name: strings.tabs.unsaved })).toBeTruthy();
  });

  it("renames the open tab of a folder", async () => {
    const { api } = await renderSidebar({}, ["col-1"]);
    render(TabBar, { props: { tabs, onclose: () => {}, onclosemany: () => {} } });
    const row = await screen.findByRole("treeitem", { name: "Users" });
    await tabs.open(api, "fold-users");
    const tab = tabs.list[0] as Tab<FolderDraft>;
    expect(await screen.findByRole("tab", { name: /Users/ })).toBeTruthy();
    tab.draft.scripts.test = "after();";
    await waitFor(() => expect(row.querySelector(".dirty")).not.toBeNull());

    await fireEvent.keyDown(row, { key: "F2" });
    const input = await renameInput();
    await type(input, "People");
    await fireEvent.keyDown(input, { key: "Enter" });

    const renamed = await screen.findByRole("treeitem", { name: "People" });
    await waitFor(() => expect(tab.name).toBe("People"));
    expect(await screen.findByRole("tab", { name: /People/ })).toBeTruthy();
    expect(screen.queryByRole("tab", { name: /Users/ })).toBeNull();
    // The same tab with the same unsaved draft.
    expect(tabs.list).toEqual([tab]);
    expect(tab.dirty).toBe(true);
    expect(renamed.querySelector(".dirty")).not.toBeNull();
  });

  it("shows a notification and keeps the name when the renaming fails", async () => {
    const { api } = await renderSidebar({}, ["col-1"]);
    vi.spyOn(api, "renameNode").mockRejectedValue("the storage is locked");
    const row = await screen.findByRole("treeitem", { name: "Users" });

    await fireEvent.keyDown(row, { key: "F2" });
    const input = await renameInput();
    await type(input, "People");
    await fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(toasts.items.map((toast) => toast.text)).toEqual(["the storage is locked"]));
    expect(treeItemNames()).toEqual(["Demo API", "Users", "Orders"]);
  });
});

describe("deleting in the tree", () => {
  it("asks with the number of requests, and closes the tabs of a deleted folder, stopping their sends", async () => {
    const { api, workspace } = await renderSidebar({}, ["col-1", "fold-users", "fold-orders"]);
    const deleteNode = vi.spyOn(api, "deleteNode");
    const onSend = vi.fn(() => new Promise<never>(() => {}));
    api.onSend = onSend;
    const row = await screen.findByRole("treeitem", { name: "Users" });
    await tabs.open(api, "req-list-users");
    await tabs.open(api, "req-create-user");
    await tabs.open(api, "req-get-order");
    // One tab of the folder has unsaved edits, another one is sending.
    ((tabs.list[1] as Tab).draft as RequestDraft).url = "https://api.example.test/changed";
    const sendingTab = tabs.list[0] as Tab<RequestDraft>;
    void sendingTab.sending.send(api, sendingTab);
    const sendId = (onSend.mock.calls[0] as unknown as [string])[0];
    workspace.select("req-list-users");
    row.focus();

    await fireEvent.keyDown(row, { key: "Delete" });
    let dialog = await screen.findByRole("dialog", { name: strings.tree.deleteTitle("Users") });
    expect(within(dialog).getByText(strings.tree.deleteContents(2))).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: strings.tree.delete }).classList.contains("danger")).toBe(true);

    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tree.cancel }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(deleteNode).not.toHaveBeenCalled();
    expect(tabs.list).toHaveLength(3);
    expect(api.cancelled).toEqual([]);
    expect(screen.getByRole("treeitem", { name: "Users" })).toBe(row);

    await fireEvent.keyDown(row, { key: "Delete" });
    dialog = await screen.findByRole("dialog", { name: strings.tree.deleteTitle("Users") });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tree.delete }));

    await waitFor(() => expect(screen.queryByRole("treeitem", { name: "Users" })).toBeNull());
    expect(deleteNode).toHaveBeenCalledTimes(1);
    expect(deleteNode).toHaveBeenCalledWith("fold-users");
    expect(treeItemNames()).toEqual(["Demo API", "Orders", "GET Get order"]);
    // No question about the unsaved edits: the tabs of the folder are gone, the other one stays.
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list.map((tab) => tab.nodeId)).toEqual(["req-get-order"]);
    expect(api.cancelled).toEqual([sendId]);
    expect(sendingTab.sending.state).toBe("idle");
    // The selected node was deleted with the folder.
    expect(workspace.selectedId).toBeNull();
  });

  it("opens the question with the focus on Cancel, so Enter right after Delete deletes nothing", async () => {
    const { api } = await renderSidebar({}, ["col-1", "fold-users"]);
    const deleteNode = vi.spyOn(api, "deleteNode");
    const row = await screen.findByRole("treeitem", { name: "Users" });
    const before = treeItemNames();
    row.focus();

    await fireEvent.keyDown(row, { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: strings.tree.deleteTitle("Users") });
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: strings.tree.cancel }));

    await pressEnter();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(deleteNode).not.toHaveBeenCalled();
    expect(treeItemNames()).toEqual(before);
  });

  it("deletes a request from the menu without mentioning any content", async () => {
    const { api } = await renderSidebar({}, ["col-1", "fold-users"]);
    const deleteNode = vi.spyOn(api, "deleteNode");
    await tabs.open(api, "req-create-user");
    const row = await screen.findByRole("treeitem", { name: "POST Create user" });

    await choose(row, strings.tree.delete);
    const dialog = await screen.findByRole("dialog", { name: strings.tree.deleteTitle("Create user") });
    expect(within(dialog).queryByText(/\d/)).toBeNull();
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tree.delete }));

    await waitFor(() => expect(treeItemNames()).toEqual(["Demo API", "Users", "GET List users", "Orders"]));
    expect(deleteNode).toHaveBeenCalledWith("req-create-user");
    expect(tabs.list).toHaveLength(0);
  });

  it("deletes a collection with its own tab and everything inside", async () => {
    const { api } = await renderSidebar();
    await tabs.open(api, "col-1");
    await tabs.open(api, "req-get-order");
    const row = screen.getByRole("treeitem", { name: "Demo API" });

    await fireEvent.keyDown(row, { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: strings.tree.deleteTitle("Demo API") });
    expect(within(dialog).getByText(strings.tree.deleteContents(3))).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tree.delete }));

    await screen.findByText(strings.sidebar.empty);
    expect(tabs.list).toHaveLength(0);
  });

  it("closes the tab of a deleted folder, its unsaved edits included", async () => {
    const { api } = await renderSidebar({}, ["col-1"]);
    const deleteNode = vi.spyOn(api, "deleteNode");
    await tabs.open(api, "fold-users");
    await tabs.open(api, "fold-orders");
    (tabs.list[0] as Tab<FolderDraft>).draft.auth = { type: "no_auth" };
    expect(tabs.list.map((tab) => tab.kind)).toEqual(["folder", "folder"]);
    const row = await screen.findByRole("treeitem", { name: "Users" });

    await choose(row, strings.tree.delete);
    const dialog = await screen.findByRole("dialog", { name: strings.tree.deleteTitle("Users") });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tree.delete }));

    await waitFor(() => expect(screen.queryByRole("treeitem", { name: "Users" })).toBeNull());
    expect(deleteNode).toHaveBeenCalledWith("fold-users");
    // No question about the unsaved edits; the tab of the other folder stays.
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list.map((tab) => tab.nodeId)).toEqual(["fold-orders"]);
  });

  it("shows a notification and keeps the tree and the tabs when the deletion fails", async () => {
    const { api } = await renderSidebar({}, ["col-1", "fold-users"]);
    vi.spyOn(api, "deleteNode").mockRejectedValue("the storage is locked");
    await tabs.open(api, "req-list-users");
    const row = await screen.findByRole("treeitem", { name: "Users" });
    const before = treeItemNames();

    await fireEvent.keyDown(row, { key: "Delete" });
    const dialog = await screen.findByRole("dialog");
    const confirm = within(dialog).getByRole<HTMLButtonElement>("button", { name: strings.tree.delete });
    await fireEvent.click(confirm);

    await waitFor(() => expect(toasts.items.map((toast) => toast.text)).toEqual(["the storage is locked"]));
    expect(treeItemNames()).toEqual(before);
    expect(tabs.list).toHaveLength(1);
    // The question stays, and its buttons can be pressed again.
    await waitFor(() => expect(confirm.disabled).toBe(false));
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(confirm.getAttribute("aria-busy")).toBeNull();
    expect(within(dialog).getByRole<HTMLButtonElement>("button", { name: strings.tree.cancel }).disabled).toBe(false);
  });

  it("keeps the question on the screen, busy and not dismissable, until the deletion has ended", async () => {
    const { api } = await renderSidebar({}, ["col-1", "fold-users"]);
    let finish = () => {};
    const stored = api.deleteNode.bind(api);
    const deleteNode = vi.spyOn(api, "deleteNode").mockImplementation(async (id) => {
      await new Promise<void>((resolve) => (finish = resolve));
      return stored(id);
    });
    await tabs.open(api, "req-list-users");
    const row = await screen.findByRole("treeitem", { name: "Users" });

    await fireEvent.keyDown(row, { key: "Delete" });
    const dialog = await screen.findByRole("dialog");
    const confirm = within(dialog).getByRole<HTMLButtonElement>("button", { name: strings.tree.delete });
    const cancel = within(dialog).getByRole<HTMLButtonElement>("button", { name: strings.tree.cancel });
    await fireEvent.click(confirm);

    // The busy state is on the screen before the command starts.
    expect(confirm.getAttribute("aria-busy")).toBe("true");
    expect(confirm.disabled).toBe(true);
    expect(cancel.disabled).toBe(true);
    expect(deleteNode).not.toHaveBeenCalled();

    await waitFor(() => expect(deleteNode).toHaveBeenCalledTimes(1));
    // Neither Escape nor a second press closes the question or starts a second deletion.
    await fireEvent.keyDown(window, { key: "Escape" });
    await fireEvent.click(confirm);
    await fireEvent.click(cancel);
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(tabs.list).toHaveLength(1);

    finish();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(deleteNode).toHaveBeenCalledTimes(1);
    expect(tabs.list).toHaveLength(0);
    expect(screen.queryByRole("treeitem", { name: "Users" })).toBeNull();
  });
});

describe("duplicating in the tree", () => {
  it("puts the copy right under the original and selects it", async () => {
    const { api, workspace } = await renderSidebar({}, ["col-1"]);
    const duplicateNode = vi.spyOn(api, "duplicateNode");
    const row = await screen.findByRole("treeitem", { name: "Users" });

    await choose(row, strings.tree.duplicate);

    const copy = await screen.findByRole("treeitem", { name: "Users Copy" });
    expect(duplicateNode).toHaveBeenCalledTimes(1);
    expect(duplicateNode).toHaveBeenCalledWith("fold-users");
    expect(treeItemNames()).toEqual(["Demo API", "Users", "Users Copy", "Orders"]);
    const id = await duplicateNode.mock.results[0].value;
    expect(id).not.toBe("fold-users");
    expect(workspace.selectedId).toBe(id);
    await waitFor(() => expect(copy.getAttribute("aria-selected")).toBe("true"));
    expect(queryRenameInput()).toBeNull();
  });
});

describe("the menu of a row", () => {
  it("opens with the right mouse button instead of the menu of the webview", async () => {
    const { onOpen } = await renderSidebar({}, ["col-1", "fold-users"]);
    const labels = () => within(screen.getByRole("menu")).getAllByRole("menuitem").map((item) => item.textContent!.trim());
    // Only a folder has "open": a click on its row expands it instead.
    const folderItems = [strings.tree.open, NAMES.request, NAMES.folder, strings.tree.rename, strings.tree.duplicate, strings.tree.delete];
    // Only a collection is exported.
    const collectionItems = [NAMES.request, NAMES.folder, strings.tree.rename, strings.tree.duplicate, strings.export.item, strings.tree.delete];

    const collection = screen.getByRole("treeitem", { name: "Demo API" });
    const event = createEvent.contextMenu(collection);
    await fireEvent(collection, event);
    const menu = await screen.findByRole("menu");

    expect(event.defaultPrevented).toBe(true);
    expect(labels()).toEqual(collectionItems);
    expect(within(menu).getByRole("menuitem", { name: strings.tree.delete }).classList.contains("danger")).toBe(true);
    // The menu is not a part of the row.
    expect(menu.closest('[role="treeitem"]')).toBeNull();
    await fireEvent.keyDown(menu, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();

    await openRowMenu(screen.getByRole("treeitem", { name: "Users" }));
    expect(labels()).toEqual(folderItems);
    expect(labels()[0]).toBe("Открыть");
    await fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });

    await openRowMenu(screen.getByRole("treeitem", { name: "GET List users" }));
    expect(labels()).toEqual([strings.tree.rename, strings.tree.duplicate, strings.tree.delete]);
    // Opening a menu opens nothing else.
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("opens a folder in a tab with the first item of its menu and selects its row", async () => {
    const { workspace, onOpen } = await renderSidebar({}, ["col-1"]);
    const row = await screen.findByRole("treeitem", { name: "Users" });

    const menu = await openRowMenu(row);
    const first = within(menu).getAllByRole("menuitem")[0];
    expect(first.textContent!.trim()).toBe(strings.tree.open);
    await fireEvent.click(first);

    await waitFor(() => expect(tabs.list.map((tab) => tab.nodeId)).toEqual(["fold-users"]));
    expect(tabs.active?.kind).toBe("folder");
    expect(tabs.active?.name).toBe("Users");
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledWith("fold-users");
    expect(workspace.selectedId).toBe("fold-users");
    expect(row.getAttribute("aria-selected")).toBe("true");
    expect(screen.queryByRole("menu")).toBeNull();
    // Opening the folder does not expand it.
    expect(row.getAttribute("aria-expanded")).toBe("false");
  });

  it("only expands and collapses a folder on a click and on Enter, without opening a tab", async () => {
    const { workspace, onOpen } = await renderSidebar({}, ["col-1"]);
    const row = await screen.findByRole("treeitem", { name: "Users" });
    expect(row.getAttribute("aria-expanded")).toBe("false");

    await fireEvent.click(row);
    expect(row.getAttribute("aria-expanded")).toBe("true");
    await fireEvent.keyDown(row, { key: "Enter" });
    expect(row.getAttribute("aria-expanded")).toBe("false");
    await fireEvent.dblClick(row);

    expect(onOpen).not.toHaveBeenCalled();
    expect(tabs.list).toHaveLength(0);
    expect(workspace.selectedId).toBeNull();
  });

  it("opens with Shift+F10 and with the context menu key", async () => {
    await renderSidebar({}, ["col-1"]);
    const row = await screen.findByRole("treeitem", { name: "Users" });
    row.focus();

    const shiftF10 = createEvent.keyDown(row, { key: "F10", shiftKey: true });
    await fireEvent(row, shiftF10);
    const menu = await screen.findByRole("menu");
    expect(shiftF10.defaultPrevented).toBe(true);
    // The keyboard is in the menu; Escape gives it back to the row.
    expect(menu.contains(document.activeElement)).toBe(true);
    await fireEvent.keyDown(menu, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(row));

    // F10 alone is not ours.
    await fireEvent.keyDown(row, { key: "F10" });
    expect(screen.queryByRole("menu")).toBeNull();

    const menuKey = createEvent.keyDown(row, { key: "ContextMenu" });
    await fireEvent(row, menuKey);
    expect(await screen.findByRole("menu")).toBeTruthy();
    expect(menuKey.defaultPrevented).toBe(true);
  });

  it("creates in the folder of the menu, whatever row has the focus", async () => {
    const { api } = await renderSidebar({}, ["col-1"]);
    const createNode = vi.spyOn(api, "createNode");
    screen.getByRole("treeitem", { name: "Demo API" }).focus();

    await choose(screen.getByRole("treeitem", { name: "Users" }), NAMES.request);
    const input = await renameInput();

    expect(createNode).toHaveBeenCalledWith("fold-users", "request", NAMES.request);
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(treeItemNames()).toEqual(["Demo API", "Users", "GET List users", "POST Create user", `GET ${NAMES.request}`, "Orders"]);

    await choose(screen.getByRole("treeitem", { name: "Demo API" }), NAMES.folder);
    await renameInput();
    expect(createNode).toHaveBeenLastCalledWith("col-1", "folder", NAMES.folder);
  });

  it("has a button for the pointer that adds nothing to the name of the row", async () => {
    const { onOpen } = await renderSidebar({}, ["col-1", "fold-users"]);

    // Every row is still found by its exact name.
    const collection = screen.getByRole("treeitem", { name: "Demo API" });
    expect(screen.getByRole("treeitem", { name: "Users" })).toBeTruthy();
    const request = screen.getByRole("treeitem", { name: "GET List users" });

    for (const row of [collection, request]) {
      const more = row.querySelector<HTMLButtonElement>("button.more")!;
      expect(more.getAttribute("aria-hidden")).toBe("true");
      expect(more.tabIndex).toBe(-1);
      expect(more.hasAttribute("aria-label")).toBe(false);
      expect(more.hasAttribute("title")).toBe(false);
      expect(more.textContent!.trim()).toBe("");
    }

    await fireEvent.click(request.querySelector("button.more")!);
    const menu = await screen.findByRole("menu");

    expect(within(menu).getAllByRole("menuitem")).toHaveLength(3);
    // The click on the button is not a click on the row.
    expect(onOpen).not.toHaveBeenCalled();
  });
});
