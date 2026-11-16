import { cleanup, createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import { layout } from "./lib/layout.svelte";
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

// The tabs, the notifications and the layout of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
  localStorage.clear();
  layout.load();
});

/** Width of the sidebar in pixels as the layout applies it. */
function sidebarWidth(): number {
  return parseInt(screen.getByRole("navigation").style.width, 10);
}

describe("App", () => {
  describe("placement of the response", () => {
    const PLACEMENTS = ["right", "bottom", "left", "top", "auto"] as const;
    const WINDOW_WIDTH = window.innerWidth;

    afterEach(() => {
      window.innerWidth = WINDOW_WIDTH;
    });

    /** The area the editor and the response share. */
    const work = () => document.querySelector<HTMLElement>("main .work")!;
    const editor = () => screen.getByRole("region", { name: strings.editor.label });
    const response = () => screen.getByRole("region", { name: strings.response.label });
    const divider = () => screen.getByRole("separator", { name: strings.layout.resize });
    const split = () => Number(divider().getAttribute("aria-valuenow"));

    /** Draws the window with a request tab open: the editor and the response share the area. */
    async function renderWithTab() {
      const api = renderApp();
      await tabs.open(api, "req-list-users");
      await waitFor(() => expect(work().hasAttribute("data-side")).toBe(true));
      return api;
    }

    /** Chooses the placement in the menu of the button in the row of the tabs. */
    async function choose(placement: (typeof PLACEMENTS)[number]) {
      await fireEvent.click(screen.getByRole("button", { name: strings.layout.button }));
      await fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitemradio", { name: strings.layout.placements[placement] }));
    }

    it("has an icon button with a name and the same tooltip next to the environment switcher", () => {
      renderApp();
      const button = screen.getByRole("button", { name: strings.layout.button });

      expect(button.title).toBe(strings.layout.button);
      expect(button.textContent!.trim()).toBe("");
      expect(button.getAttribute("aria-haspopup")).toBe("menu");
      expect(button.closest(".top")).not.toBeNull();
    });

    it("offers the five placements with an icon each and marks the current one", async () => {
      renderApp();
      await fireEvent.click(screen.getByRole("button", { name: strings.layout.button }));
      const items = within(screen.getByRole("menu", { name: strings.layout.button })).getAllByRole("menuitemradio");

      expect(items.map((item) => item.textContent!.trim())).toEqual(PLACEMENTS.map((placement) => strings.layout.placements[placement]));
      expect(items.every((item) => item.querySelector("svg") !== null)).toBe(true);
      expect(items.map((item) => item.getAttribute("aria-checked"))).toEqual(["false", "false", "false", "false", "true"]);

      await fireEvent.click(items[2]);
      expect(screen.queryByRole("menu")).toBeNull();
      await fireEvent.click(screen.getByRole("button", { name: strings.layout.button }));
      expect(screen.getAllByRole("menuitemradio").map((item) => item.getAttribute("aria-checked"))).toEqual([
        "false",
        "false",
        "true",
        "false",
        "false",
      ]);
    });

    it.each(["right", "bottom", "left", "top"] as const)("puts the response on the chosen side: %s", async (placement) => {
      await renderWithTab();
      await choose(placement);

      expect(work().dataset.placement).toBe(placement);
      expect(work().dataset.side).toBe(placement);
      expect(divider().getAttribute("aria-orientation")).toBe(placement === "right" || placement === "left" ? "vertical" : "horizontal");
    });

    it("on Auto puts the response below in a narrow window and on the right in a wide one", async () => {
      window.innerWidth = 1024;
      await renderWithTab();
      await choose("left");
      await choose("auto");

      expect(work().dataset.placement).toBe("auto");
      expect(work().dataset.side).toBe("bottom");
      expect(divider().getAttribute("aria-orientation")).toBe("horizontal");

      window.innerWidth = 1280;
      await fireEvent(window, new Event("resize"));
      expect(work().dataset.side).toBe("right");
      expect(divider().getAttribute("aria-orientation")).toBe("vertical");
    });

    it.each(PLACEMENTS)("keeps the editor before the response in the markup: %s", async (placement) => {
      await renderWithTab();
      await choose(placement);

      const following = Node.DOCUMENT_POSITION_FOLLOWING;
      expect(editor().compareDocumentPosition(divider()) & following).toBe(following);
      expect(divider().compareDocumentPosition(response()) & following).toBe(following);
      expect([...work().children].map((child) => child.className.split(" ")[0])).toEqual(["editor", "divider", "response"]);
    });

    it.each(PLACEMENTS)("remembers the choice for the next window: %s", async (placement) => {
      await renderWithTab();
      await choose(placement);
      await fireEvent.keyDown(divider(), { key: placement === "right" || placement === "left" ? "ArrowLeft" : "ArrowUp" });
      const remembered = split();
      expect(remembered).not.toBe(50);

      cleanup();
      // The module forgets: the next window has only what was written down.
      layout.placement = placement === "auto" ? "right" : "auto";
      layout.split = 50;
      renderApp();

      expect(work().dataset.placement).toBe(placement);
      expect(split()).toBe(remembered);
      expect(work().style.getPropertyValue("--editor-share")).toBe(`${remembered}fr`);
      expect(work().style.getPropertyValue("--response-share")).toBe(`${100 - remembered}fr`);
    });

    it("moves the divider by 2 points with the arrows, the way they point", async () => {
      await renderWithTab();
      await choose("right");
      expect(split()).toBe(50);
      expect(divider().getAttribute("aria-valuemin")).toBe("20");
      expect(divider().getAttribute("aria-valuemax")).toBe("80");

      // The editor is on the left: the divider going right makes it bigger.
      await fireEvent.keyDown(divider(), { key: "ArrowRight" });
      expect(split()).toBe(52);
      await fireEvent.keyDown(divider(), { key: "ArrowLeft" });
      await fireEvent.keyDown(divider(), { key: "ArrowLeft" });
      expect(split()).toBe(48);
      // The other pair of arrows is not for a vertical line.
      await fireEvent.keyDown(divider(), { key: "ArrowDown" });
      expect(split()).toBe(48);

      // The editor is on the right: the divider going right makes it smaller.
      await choose("left");
      await fireEvent.keyDown(divider(), { key: "ArrowRight" });
      expect(split()).toBe(46);

      // The editor is above: the divider going down makes it bigger.
      await choose("bottom");
      await fireEvent.keyDown(divider(), { key: "ArrowDown" });
      expect(split()).toBe(48);
      await fireEvent.keyDown(divider(), { key: "ArrowRight" });
      expect(split()).toBe(48);

      // The editor is below: the divider going down makes it smaller.
      await choose("top");
      await fireEvent.keyDown(divider(), { key: "ArrowDown" });
      expect(split()).toBe(46);
    });

    it("keeps the share of the editor between 20 and 80", async () => {
      await renderWithTab();
      await choose("right");

      for (let i = 0; i < 20; i++) await fireEvent.keyDown(divider(), { key: "ArrowRight" });
      expect(split()).toBe(80);
      expect(work().style.getPropertyValue("--editor-share")).toBe("80fr");
      expect(work().style.getPropertyValue("--response-share")).toBe("20fr");

      for (let i = 0; i < 40; i++) await fireEvent.keyDown(divider(), { key: "ArrowLeft" });
      expect(split()).toBe(20);
    });

    it("brings the shares back to equal on a double click", async () => {
      await renderWithTab();
      for (let i = 0; i < 5; i++) await fireEvent.keyDown(divider(), { key: "ArrowDown" });
      expect(split()).toBe(60);

      await fireEvent.dblClick(divider());
      expect(split()).toBe(50);
      expect(layout.split).toBe(50);
    });

    it("has no divider while no tab is open", () => {
      renderApp();

      expect(screen.queryByRole("separator", { name: strings.layout.resize })).toBeNull();
      expect(screen.getAllByRole("separator")).toHaveLength(1);
      expect(work().hasAttribute("data-side")).toBe(false);
    });
  });

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
