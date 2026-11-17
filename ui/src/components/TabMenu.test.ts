import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { layout } from "../lib/layout.svelte";
import { session } from "../lib/session.svelte";
import { strings } from "../lib/strings";
import { tabs, type EnvironmentTab, type Tab } from "../lib/tabs.svelte";
import { createFakeApi, type FakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import type { RequestDraft } from "../lib/types";
import { workspace } from "../lib/workspace.svelte";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const ITEMS = strings.tabs.menuItems;
const ALL_ITEMS = [ITEMS.close, ITEMS.close_others, ITEMS.close_right, ITEMS.close_saved, ITEMS.close_all];

/** Renders the window, waits until it watches for changes, and opens the three requests of the default tree. */
async function openThree(): Promise<FakeApi> {
  const api = createFakeApi();
  render(App, { context: new Map([["api", api]]) });
  await screen.findByRole("treeitem", { name: /Demo API/ });
  for (let turn = 0; turn < 20; turn++) await Promise.resolve();
  for (const nodeId of ["req-list-users", "req-create-user", "req-get-order"]) await tabs.open(api, nodeId);
  await waitFor(() => expect(titles()).toHaveLength(3));
  return api;
}

/** The titles of the tabs, left to right. */
function titles(): HTMLElement[] {
  const list = screen.queryByRole("tablist", { name: strings.tabs.label });
  return list ? within(list).queryAllByRole("tab") : [];
}

/** Ids of the open tabs, left to right. */
function openIds(): string[] {
  return tabs.list.map((tab) => tab.id);
}

/** Changes the address of the open request, so that its tab has unsaved edits. */
function edit(nodeId: string) {
  (tabs.list.find((tab) => tab.id === nodeId) as Tab<RequestDraft>).draft.url = `https://changed.test/${nodeId}`;
}

/** Opens the menu of the tab number `index` with the right mouse button. */
async function openMenu(index: number): Promise<HTMLElement> {
  await fireEvent.contextMenu(titles()[index]);
  return screen.findByRole("menu", { name: strings.tabs.menu });
}

/** Chooses an item of the menu of the tab number `index`. */
async function choose(index: number, label: string) {
  const menu = await openMenu(index);
  await fireEvent.click(within(menu).getByRole("menuitem", { name: label }));
}

/** The names of the items of the menu that are not available. */
function disabledItems(menu: HTMLElement): string[] {
  return within(menu)
    .getAllByRole("menuitem")
    .filter((item) => item.getAttribute("aria-disabled") === "true")
    .map((item) => item.textContent!.trim());
}

function question(): Promise<HTMLElement> {
  return screen.findByRole("dialog", { name: strings.tabs.closeManyTitle });
}

// The tabs, the notifications, the session and the workspace of the window live in modules.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
  session.stop();
  layout.apply({});
  workspace.rows = [];
});

describe("menu of a tab", () => {
  it("opens on a right click instead of the menu of the webview, with the five items", async () => {
    await openThree();
    const event = createEvent.contextMenu(titles()[1]);
    await fireEvent(titles()[1], event);

    const menu = await screen.findByRole("menu", { name: strings.tabs.menu });

    expect(event.defaultPrevented).toBe(true);
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent!.trim())).toEqual(ALL_ITEMS);
    expect(disabledItems(menu)).toEqual([]);
    // The right click does not show the tab.
    expect(tabs.activeId).toBe("req-get-order");
  });

  it("opens from the keyboard with the menu key and with Shift+F10, and gives the focus back on Escape", async () => {
    await openThree();
    const title = titles()[2];
    title.focus();

    await fireEvent.keyDown(title, { key: "ContextMenu" });
    let menu = await screen.findByRole("menu", { name: strings.tabs.menu });
    expect(menu.contains(document.activeElement)).toBe(true);
    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(document.activeElement).toBe(title);

    // F10 alone is not a key of the bar.
    await fireEvent.keyDown(title, { key: "F10" });
    expect(screen.queryByRole("menu")).toBeNull();
    await fireEvent.keyDown(title, { key: "F10", shiftKey: true });
    menu = await screen.findByRole("menu", { name: strings.tabs.menu });
    expect(within(menu).getAllByRole("menuitem")).toHaveLength(5);
  });

  it("makes the items that would close nothing unavailable", async () => {
    await openThree();
    // The last tab has nothing on its right.
    let menu = await openMenu(2);
    expect(disabledItems(menu)).toEqual([ITEMS.close_right]);
    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });

    for (const nodeId of ["req-list-users", "req-create-user", "req-get-order"]) edit(nodeId);
    menu = await openMenu(0);
    expect(disabledItems(menu)).toEqual([ITEMS.close_saved]);
    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });

    tabs.close("req-create-user");
    tabs.close("req-get-order");
    await waitFor(() => expect(titles()).toHaveLength(1));
    menu = await openMenu(0);
    expect(disabledItems(menu)).toEqual([ITEMS.close_others, ITEMS.close_right, ITEMS.close_saved]);
  });

  it("closes the other tabs at once and shows the tab of the menu", async () => {
    await openThree();

    await choose(0, ITEMS.close_others);

    await waitFor(() => expect(openIds()).toEqual(["req-list-users"]));
    expect(tabs.activeId).toBe("req-list-users");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("closes the tabs on the right and shows the tab of the menu", async () => {
    await openThree();

    await choose(1, ITEMS.close_right);

    await waitFor(() => expect(openIds()).toEqual(["req-list-users", "req-create-user"]));
    expect(tabs.activeId).toBe("req-create-user");
  });

  it("closes all tabs", async () => {
    await openThree();

    await choose(1, ITEMS.close_all);

    await waitFor(() => expect(openIds()).toEqual([]));
  });

  it("closes the saved tabs without a question and leaves the changed ones", async () => {
    await openThree();
    edit("req-create-user");

    await choose(0, ITEMS.close_saved);

    await waitFor(() => expect(openIds()).toEqual(["req-create-user"]));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list[0].dirty).toBe(true);
  });

  it("asks about one tab the way the cross does", async () => {
    await openThree();
    edit("req-create-user");

    await choose(1, ITEMS.close);

    const dialog = await screen.findByRole("dialog", { name: strings.tabs.closeTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tabs.discard }));
    await waitFor(() => expect(openIds()).toEqual(["req-list-users", "req-get-order"]));
  });

  it("stops the send of a request it closes, as closing one tab does", async () => {
    const api = await openThree();
    api.onSend = () => new Promise(() => {});
    const sending = (tabs.list[1] as Tab<RequestDraft>).sending;
    void sending.send(api, tabs.list[1] as Tab<RequestDraft>);
    await waitFor(() => expect(sending.state).toBe("sending"));

    await choose(0, ITEMS.close_others);

    await waitFor(() => expect(openIds()).toEqual(["req-list-users"]));
    expect(api.cancelled).toHaveLength(1);
  });

  it("saves what an environment was waiting to save before closing its tab", async () => {
    const api = await openThree();
    await tabs.openEnvironment(api, "globals");
    const saveEnvironment = vi.spyOn(api, "saveEnvironment");
    const environment = tabs.list[3] as EnvironmentTab;
    environment.variables.push({ key: "token", value: "1", secret: false, enabled: true, postman_extra: {} });
    environment.edited(api);
    await waitFor(() => expect(titles()).toHaveLength(4));

    await choose(0, ITEMS.close_all);

    await waitFor(() => expect(openIds()).toEqual([]));
    expect(saveEnvironment).toHaveBeenCalledTimes(1);
    expect(saveEnvironment.mock.calls[0][2]).toEqual([expect.objectContaining({ key: "token" })]);
  });

  it("leaves the tab of an environment whose save fails", async () => {
    const api = await openThree();
    await tabs.openEnvironment(api, "globals");
    vi.spyOn(api, "saveEnvironment").mockRejectedValue("the storage is locked");
    const environment = tabs.list[3] as EnvironmentTab;
    environment.edited(api);
    await waitFor(() => expect(titles()).toHaveLength(4));

    await choose(0, ITEMS.close_all);

    await waitFor(() => expect(openIds()).toEqual([environment.id]));
    expect(toasts.items.map((toast) => toast.text)).toEqual(["the storage is locked"]);
  });
});

describe("question about closing several changed tabs", () => {
  it("is asked once with the number of changed tabs, with the focus on Cancel; Cancel closes nothing", async () => {
    await openThree();
    edit("req-list-users");
    edit("req-get-order");

    await choose(1, ITEMS.close_all);

    const dialog = await question();
    expect(within(dialog).getByText(strings.tabs.closeManyText(2))).toBeTruthy();
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    // Nothing closes before the answer, the tabs without edits included.
    expect(openIds()).toHaveLength(3);
    const cancel = within(dialog).getByRole("button", { name: strings.tabs.cancel });
    expect(document.activeElement).toBe(cancel);

    await fireEvent.click(cancel);

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(openIds()).toHaveLength(3);
    expect(tabs.list.filter((tab) => tab.dirty)).toHaveLength(2);
  });

  it("closes nothing when it is dismissed with Escape", async () => {
    await openThree();
    edit("req-get-order");

    await choose(0, ITEMS.close_right);
    await question();
    await fireEvent.keyDown(window, { key: "Escape" });

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(openIds()).toHaveLength(3);
  });

  it("on confirmation closes the tabs and deletes their stored drafts", async () => {
    const api = await openThree();
    edit("req-create-user");
    edit("req-get-order");
    // The drafts are in the storage, as after a pause in the editing.
    expect(await session.flush(api)).toBe(true);
    expect(api.draftCalls.map(([id, draft]) => [id, draft !== null])).toEqual([
      ["req-create-user", true],
      ["req-get-order", true],
    ]);
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");

    await choose(0, ITEMS.close_others);
    const dialog = await question();
    expect(within(dialog).getByText(strings.tabs.closeManyText(2))).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tabs.closeManyConfirm }));

    await waitFor(() => expect(openIds()).toEqual(["req-list-users"]));
    expect(tabs.activeId).toBe("req-list-users");
    await waitFor(() =>
      expect(api.draftCalls.slice(2)).toEqual([
        ["req-create-user", null],
        ["req-get-order", null],
      ]),
    );
    expect(saveNodeContent).not.toHaveBeenCalled();
  });
});
