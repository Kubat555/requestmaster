import { cleanup, createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import { SHORTCUTS } from "./lib/hotkeys";
import { layout } from "./lib/layout.svelte";
import { session } from "./lib/session.svelte";
import { strings } from "./lib/strings";
import { tabs } from "./lib/tabs.svelte";
import { createFakeApi, noScriptResults, type StoredHistoryEntry } from "./lib/testing/fakeApi";
import { toasts } from "./lib/toasts.svelte";
import type { SendResult } from "./lib/types";

vi.mock("./components/CodeEditor.svelte", async () => ({ default: (await import("./lib/testing/CodeEditorStub.svelte")).default }));

function renderApp() {
  const api = createFakeApi();
  render(App, { context: new Map([["api", api]]) });
  return api;
}

/** Waits until the window has restored what was remembered: from then on its changes are written down. */
async function restored(): Promise<void> {
  await screen.findByRole("treeitem", { name: /Demo API/ });
  for (let turn = 0; turn < 20; turn++) await Promise.resolve();
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
  vi.useRealTimers();
  session.stop();
  layout.apply({});
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
      await restored();
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
      const api = await renderWithTab();
      vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
      await choose(placement);
      await fireEvent.keyDown(divider(), { key: placement === "right" || placement === "left" ? "ArrowLeft" : "ArrowUp" });
      const remembered = split();
      expect(remembered).not.toBe(50);
      await vi.advanceTimersByTimeAsync(500);
      vi.useRealTimers();
      const written = api.windowStates.at(-1)!;
      expect(written).toMatchObject({ placement, split: remembered });

      cleanup();
      // The module forgets: the next window has only what was written down.
      layout.placement = placement === "auto" ? "right" : "auto";
      layout.split = 50;
      render(App, { context: new Map([["api", createFakeApi({ session: { window: written, drafts: [] } })]]) });

      await waitFor(() => expect(work().dataset.placement).toBe(placement));
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

  describe("Ctrl+F", () => {
    const CTRL_F = { key: "f", code: "KeyF", ctrlKey: true };

    /** A `200 OK` response whose body is stored under `b1`. */
    const ANSWER: SendResult = {
      response: {
        status: 200,
        status_text: "OK",
        http_version: "HTTP/1.1",
        headers: [["content-type", "application/json"]],
        cookies: [],
        redirects: [],
        ttfb_ms: 40,
        total_ms: 123.4,
        wire_size: 7,
        decoded_size: 7,
        decode_error: null,
        sent: { method: "GET", url: "https://api.example.test/users", headers: [] },
        body: { id: "b1", total: 7, truncated: false, json: true },
      },
      error: null,
      warnings: [],
      ...noScriptResults(),
    };

    /** Presses the combination on the element and returns the event. */
    async function pressCtrlF(target: Element = document.body): Promise<Event> {
      const event = createEvent.keyDown(target, CTRL_F);
      await fireEvent(target, event);
      return event;
    }

    /** The editors whose search panel is open. */
    const searching = () => document.querySelectorAll("[data-search-open]");

    async function settle() {
      for (let turn = 0; turn < 20; turn++) await Promise.resolve();
    }

    const response = () => within(screen.getByRole("region", { name: strings.response.label }));

    /** Draws the window with a request tab open; with `answer` the request is sent and its response is shown. */
    async function renderWithRequest(answer: boolean) {
      const api = createFakeApi({ onSend: async () => ANSWER, bodies: { b1: new TextEncoder().encode('{"a":1}') } });
      render(App, { context: new Map([["api", api]]) });
      await tabs.open(api, "req-list-users");
      await screen.findByRole("tab", { name: /List users/ });
      if (answer) {
        await fireEvent.click(screen.getByRole("button", { name: strings.editor.send }));
        await waitFor(() =>
          expect(response().getByRole<HTMLTextAreaElement>("textbox", { name: strings.response.bodyText }).value).toBe('{"a":1}'),
        );
      }
      return api;
    }

    it("opens the search in the body of the response of the shown request tab", async () => {
      await renderWithRequest(true);
      await fireEvent.click(response().getByRole("tab", { name: /Headers/ }));

      const event = await pressCtrlF();

      expect(event.defaultPrevented).toBe(true);
      await waitFor(() => expect(searching()).toHaveLength(1));
      expect(response().getByRole("textbox", { name: strings.response.bodyText }).hasAttribute("data-search-open")).toBe(true);
      expect(response().getByRole("tab", { name: /Body/ }).getAttribute("aria-selected")).toBe("true");
    });

    it("is left to the code editor when pressed inside one", async () => {
      await renderWithRequest(true);
      const editor = document.body.appendChild(document.createElement("div"));
      editor.className = "cm-editor";
      const line = editor.appendChild(document.createElement("div"));

      const event = await pressCtrlF(line);
      await settle();
      editor.remove();

      expect(event.defaultPrevented).toBe(false);
      expect(searching()).toHaveLength(0);
    });

    it("is cancelled and changes nothing when the request has no response", async () => {
      await renderWithRequest(false);

      const event = await pressCtrlF();
      await settle();

      expect(event.defaultPrevented).toBe(true);
      expect(searching()).toHaveLength(0);
      expect(screen.getByText(strings.response.empty)).toBeTruthy();
    });

    it("is cancelled without any tab", async () => {
      renderApp();
      await restored();

      expect((await pressCtrlF()).defaultPrevented).toBe(true);
    });

    it("does nothing while a dialog is open", async () => {
      await renderWithRequest(true);
      await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
      await screen.findByRole("dialog");

      const event = await pressCtrlF();
      await settle();

      expect(event.defaultPrevented).toBe(true);
      expect(searching()).toHaveLength(0);
    });
  });

  describe("list of the key combinations", () => {
    const TEXT = strings.shortcuts;
    const list = () => screen.queryByRole("dialog", { name: TEXT.title });

    it("opens from the button next to the settings, and gives the focus back to it when closed", async () => {
      renderApp();
      const button = screen.getByRole("button", { name: TEXT.button });
      expect(button.getAttribute("title")).toBe(TEXT.button);
      expect(button.nextElementSibling).toBe(screen.getByRole("button", { name: strings.settings.button }));
      button.focus();

      await fireEvent.click(button);
      const dialog = await screen.findByRole("dialog", { name: TEXT.title });
      await fireEvent.click(within(dialog).getByRole("button", { name: TEXT.close }));

      await waitFor(() => expect(list()).toBeNull());
      expect(document.activeElement).toBe(button);

      await fireEvent.click(button);
      await screen.findByRole("dialog", { name: TEXT.title });
      await fireEvent.keyDown(window, { key: "Escape" });

      await waitFor(() => expect(list()).toBeNull());
      expect(document.activeElement).toBe(button);
    });

    it("opens with F1, which is cancelled; a second F1 changes nothing", async () => {
      renderApp();
      const first = createEvent.keyDown(document.body, { key: "F1", code: "F1" });
      await fireEvent(document.body, first);

      expect(first.defaultPrevented).toBe(true);
      await screen.findByRole("dialog", { name: TEXT.title });

      const second = createEvent.keyDown(document.body, { key: "F1", code: "F1" });
      await fireEvent(document.body, second);

      expect(second.defaultPrevented).toBe(true);
      expect(screen.getAllByRole("dialog")).toHaveLength(1);
    });

    it("shows every group with its rows: the keys as key caps and what they do", async () => {
      renderApp();
      await fireEvent.click(screen.getByRole("button", { name: TEXT.button }));
      const dialog = await screen.findByRole("dialog", { name: TEXT.title });

      for (const group of SHORTCUTS) {
        const section = within(dialog).getByRole("region", { name: group.title });
        const rows = [...section.querySelectorAll(".row")];
        expect(rows).toHaveLength(group.shortcuts.length);
        group.shortcuts.forEach((shortcut, index) => {
          const caps = [...rows[index].querySelectorAll("kbd")].map((cap) => cap.textContent);
          expect(caps).toEqual(shortcut.keys.flat());
          expect(rows[index].querySelector("dd")!.textContent).toBe(shortcut.text);
        });
      }
      // Two presses that follow one another are told apart from two ways to do the same.
      const editor = within(dialog).getByRole("region", { name: TEXT.groups.editor });
      expect(within(editor).getAllByText(TEXT.then)).toHaveLength(1);
    });
  });

  it("offers no save to a file in a tab of the history", async () => {
    const entry: StoredHistoryEntry = {
      id: 1,
      sent_at: new Date().toISOString(),
      node_id: "req-list-users",
      name: "List users",
      method: "GET",
      url: "https://api.example.test/users",
      status: 200,
      error_kind: null,
      total_ms: 12,
      size: 7,
      request_headers: [],
      request_body: null,
      request_body_truncated: false,
      response_headers: [["content-type", "application/json"]],
      response_body: '{"a":1}',
      response_body_truncated: false,
      response_body_binary: false,
      error_message: null,
      warnings: [],
      tests: [],
      script_errors: [],
    };
    const api = createFakeApi({ history: [entry] });
    render(App, { context: new Map([["api", api]]) });

    await tabs.openHistory(api, 1);
    await screen.findByRole("tab", { name: strings.history.tabName("List users") });
    await screen.findByText("200", { exact: false });

    expect(screen.queryByRole("button", { name: strings.response.save })).toBeNull();
  });

  it("lets the window close at once when no tab has unsaved edits", async () => {
    const api = renderApp();
    await tabs.open(api, "req-list-users");

    expect(await api.requestClose()).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("writes the drafts and the state of the window and closes without a question, even with unsaved edits", async () => {
    const api = renderApp();
    await restored();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await openAndEdit(api, "req-list-users");
    // A tab without edits has no draft to write.
    await tabs.open(api, "req-get-order");

    expect(await api.requestClose()).toBe(true);

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.draftCalls).toEqual([
      ["req-list-users", expect.objectContaining({ url: "https://api.example.test/changed/req-list-users" })],
    ]);
    expect(api.windowStates).toEqual([
      {
        sidebar_width: 280,
        placement: "auto",
        split: 50,
        tabs: [
          { kind: "node", id: "req-list-users" },
          { kind: "node", id: "req-get-order" },
        ],
        active: "req-get-order",
      },
    ]);
    // The edits stay a draft: nothing is saved to the node, and the tab is still changed.
    expect(saveNodeContent).not.toHaveBeenCalled();
    expect(tabs.list[0].dirty).toBe(true);
    expect(toasts.items).toEqual([]);
  });

  it("asks when a draft is not written before the window closes; Close without saving closes it", async () => {
    const api = renderApp();
    await restored();
    await openAndEdit(api, "req-list-users");
    const saveDraft = vi.spyOn(api, "saveDraft").mockRejectedValue("disk is full");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeFailedTitle });

    expect(strings.app.closeFailedTitle).toBe("Не удалось сохранить состояние. Закрыть всё равно?");
    expect(saveDraft).toHaveBeenCalledTimes(1);
    expect(within(dialog).getAllByRole("button").map((button) => button.textContent!.trim())).toEqual([
      strings.app.closeWithoutSaving,
      strings.app.cancel,
    ]);
    // Enter right after the question appears closes nothing.
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: strings.app.cancel }));

    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.closeWithoutSaving }));

    expect(await closed).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps the window and the edits when the question about a failed write is cancelled", async () => {
    const api = renderApp();
    await restored();
    await openAndEdit(api, "req-list-users");
    vi.spyOn(api, "saveDraft").mockRejectedValue("disk is full");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeFailedTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.cancel }));

    expect(await closed).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list).toHaveLength(1);
    expect(tabs.list[0].dirty).toBe(true);
  });

  it("keeps the window when the question about a failed write is dismissed with Escape", async () => {
    const api = renderApp();
    await restored();
    await openAndEdit(api, "req-list-users");
    vi.spyOn(api, "saveDraft").mockRejectedValue("disk is full");

    const closed = api.requestClose();
    await screen.findByRole("dialog", { name: strings.app.closeFailedTitle });
    await fireEvent.keyDown(document.body, { key: "Escape" });

    expect(await closed).toBe(false);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list[0].dirty).toBe(true);
  });

  it("asks the same when the state of the window is not written before the window closes", async () => {
    const api = renderApp();
    await restored();
    await tabs.open(api, "req-list-users");
    const saveWindowState = vi.spyOn(api, "saveWindowState").mockRejectedValue("disk is full");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeFailedTitle });

    expect(saveWindowState).toHaveBeenCalledTimes(1);
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.closeWithoutSaving }));
    expect(await closed).toBe(true);
  });

  it("answers a second close request with the same question, not a second dialog", async () => {
    const api = renderApp();
    await restored();
    await openAndEdit(api, "req-list-users");
    const saveDraft = vi.spyOn(api, "saveDraft").mockRejectedValue("disk is full");

    const first = api.requestClose();
    await screen.findByRole("dialog", { name: strings.app.closeFailedTitle });
    expect(await api.requestClose()).toBe(false);
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    // The second request wrote nothing: the question is still to be answered.
    expect(saveDraft).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole("button", { name: strings.app.closeWithoutSaving }));
    expect(await first).toBe(true);
  });

  describe("measuring", () => {
    /** Two frames of the window. */
    const twoFrames = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));

    it("reports no timings in a usual run", async () => {
      const api = renderApp();
      await restored();
      await twoFrames();

      expect(api.timingCalls).toEqual([]);
    });

    it("reports `ready` when the core measures", async () => {
      const api = createFakeApi({
        session: {
          window: { sidebar_width: 280, placement: "auto", split: 50, tabs: [], active: null },
          drafts: [],
          measure: true,
        },
      });
      render(App, { context: new Map([["api", api]]) });

      await waitFor(() => expect(api.timingCalls).toContainEqual([{ name: "ready", ms: expect.any(Number) }]));
    });
  });

  it("draws the sidebar as wide as it was remembered", async () => {
    const api = createFakeApi({
      session: { window: { sidebar_width: 320, placement: "auto", split: 50, tabs: [], active: null }, drafts: [] },
    });
    render(App, { context: new Map([["api", api]]) });

    await waitFor(() => expect(sidebarWidth()).toBe(320));
    expect(screen.getByRole("separator").getAttribute("aria-valuenow")).toBe("320");
  });
});
