import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import { strings } from "./lib/strings";
import { tabs } from "./lib/tabs.svelte";
import { createFakeApi } from "./lib/testing/fakeApi";
import { toasts } from "./lib/toasts.svelte";

function renderApp() {
  const api = createFakeApi();
  render(App, { context: new Map([["api", api]]) });
  return api;
}

/** Opens the request in a tab and changes its URL, so that the tab has unsaved edits. */
async function openAndEdit(api: ReturnType<typeof createFakeApi>, nodeId: string): Promise<void> {
  await tabs.open(api, nodeId);
  const url = await waitFor(() => {
    const input = document.querySelector<HTMLInputElement>("[data-tab-panel]:not([hidden]) [data-url-input]");
    expect(input).not.toBeNull();
    return input!;
  });
  await fireEvent.input(url, { target: { value: `https://api.example.test/changed/${nodeId}` } });
}

/** Right-clicks the element and tells whether the menu of the webview was cancelled. */
async function contextMenuPrevented(target: Element): Promise<boolean> {
  const event = createEvent.contextMenu(target);
  await fireEvent(target, event);
  return event.defaultPrevented;
}

// The tabs and the notifications of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

/** Width of the sidebar in pixels as the layout applies it. */
function sidebarWidth(): number {
  return parseInt(screen.getByRole("navigation").style.width, 10);
}

describe("App", () => {
  it("renders the sidebar and the main area", () => {
    renderApp();
    expect(screen.getByRole("navigation")).toBeTruthy();
    expect(screen.getByRole("main")).toBeTruthy();
  });

  it("resizes the sidebar by 16 px with the arrow keys on the separator", async () => {
    renderApp();
    const separator = screen.getByRole("separator");
    expect(sidebarWidth()).toBe(280);

    await fireEvent.keyDown(separator, { key: "ArrowRight" });
    expect(sidebarWidth()).toBe(296);
    expect(separator.getAttribute("aria-valuenow")).toBe("296");

    await fireEvent.keyDown(separator, { key: "ArrowLeft" });
    await fireEvent.keyDown(separator, { key: "ArrowLeft" });
    expect(sidebarWidth()).toBe(264);
  });

  it("keeps the sidebar width between 200 and 480 px", async () => {
    renderApp();
    const separator = screen.getByRole("separator");

    for (let i = 0; i < 20; i++) await fireEvent.keyDown(separator, { key: "ArrowRight" });
    expect(sidebarWidth()).toBe(480);

    for (let i = 0; i < 30; i++) await fireEvent.keyDown(separator, { key: "ArrowLeft" });
    expect(sidebarWidth()).toBe(200);
  });

  it("cancels the menu of the webview on the page and on a row of the tree", async () => {
    renderApp();

    expect(await contextMenuPrevented(document.body)).toBe(true);
    expect(await contextMenuPrevented(await screen.findByRole("treeitem", { name: /Demo API/ }))).toBe(true);
  });

  it("leaves the menu inside a text field and inside the code editor", async () => {
    renderApp();
    await screen.findByRole("treeitem", { name: /Demo API/ });

    expect(await contextMenuPrevented(screen.getByLabelText(strings.sidebar.filter))).toBe(false);

    const textarea = document.body.appendChild(document.createElement("textarea"));
    expect(await contextMenuPrevented(textarea)).toBe(false);
    textarea.remove();

    // The text of CodeMirror: lines inside `.cm-content`.
    const content = document.body.appendChild(document.createElement("div"));
    content.className = "cm-content";
    const line = content.appendChild(document.createElement("div"));
    expect(await contextMenuPrevented(line)).toBe(false);
    content.remove();
  });

  it("lets the window close at once when no tab has unsaved edits", async () => {
    const api = renderApp();
    await tabs.open(api, "req-list-users");

    expect(await api.requestClose()).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("asks before the window closes with unsaved edits; Cancel keeps the window", async () => {
    const api = renderApp();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await openAndEdit(api, "req-list-users");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeTitle });

    expect(within(dialog).getByText(strings.app.closeText(1))).toBeTruthy();
    expect(within(dialog).getAllByRole("button").map((button) => button.textContent!.trim())).toEqual([
      strings.app.saveAllAndClose,
      strings.app.closeWithoutSaving,
      strings.app.cancel,
    ]);

    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.cancel }));

    expect(await closed).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list[0].dirty).toBe(true);
    expect(saveNodeContent).not.toHaveBeenCalled();
  });

  it("keeps the window when the question is dismissed with Escape", async () => {
    const api = renderApp();
    await openAndEdit(api, "req-list-users");

    const closed = api.requestClose();
    await screen.findByRole("dialog", { name: strings.app.closeTitle });
    await fireEvent.keyDown(document.body, { key: "Escape" });

    expect(await closed).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("closes the window without saving on request", async () => {
    const api = renderApp();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await openAndEdit(api, "req-list-users");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.closeWithoutSaving }));

    expect(await closed).toBe(true);
    expect(saveNodeContent).not.toHaveBeenCalled();
  });

  it("saves every tab with unsaved edits and then lets the window close", async () => {
    const api = renderApp();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await openAndEdit(api, "req-list-users");
    await openAndEdit(api, "req-create-user");
    // A tab without edits is not saved.
    await tabs.open(api, "req-get-order");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeTitle });
    expect(within(dialog).getByText(strings.app.closeText(2))).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.saveAllAndClose }));

    expect(await closed).toBe(true);
    expect(saveNodeContent.mock.calls.map(([id]) => id)).toEqual(["req-list-users", "req-create-user"]);
    expect(saveNodeContent).toHaveBeenCalledWith(
      "req-list-users",
      expect.objectContaining({ url: "https://api.example.test/changed/req-list-users" }),
    );
    expect(tabs.list.some((tab) => tab.dirty)).toBe(false);
  });

  it("keeps the window open and shows a notification when a save before closing fails", async () => {
    const api = renderApp();
    vi.spyOn(api, "saveNodeContent").mockRejectedValue("disk is full");
    await openAndEdit(api, "req-list-users");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.saveAllAndClose }));

    expect(await closed).toBe(false);
    expect((await screen.findByRole("alert")).textContent).toContain("disk is full");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list[0].dirty).toBe(true);
  });

  it("answers a second close request with the same question, not a second dialog", async () => {
    const api = renderApp();
    await openAndEdit(api, "req-list-users");

    const first = api.requestClose();
    await screen.findByRole("dialog", { name: strings.app.closeTitle });
    expect(await api.requestClose()).toBe(false);
    expect(screen.getAllByRole("dialog")).toHaveLength(1);

    await fireEvent.click(screen.getByRole("button", { name: strings.app.closeWithoutSaving }));
    expect(await first).toBe(true);
  });
});
