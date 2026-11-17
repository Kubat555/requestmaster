import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { history } from "./history.svelte";
import { layout } from "./layout.svelte";
import { DRAFT_SAVE_DELAY_MS, session, WINDOW_SAVE_DELAY_MS } from "./session.svelte";
import { strings } from "./strings";
import { tabs, type Tab } from "./tabs.svelte";
import { createFakeApi, type FakeSeed, type StoredHistoryEntry } from "./testing/fakeApi";
import { toasts } from "./toasts.svelte";
import type { RequestDraft, SessionDto, WindowState } from "./types";
import { workspace } from "./workspace.svelte";

vi.mock("../components/CodeEditor.svelte", async () => ({ default: (await import("./testing/CodeEditorStub.svelte")).default }));

const CTRL_S = { key: "s", code: "KeyS", ctrlKey: true };

/** The window as a new database gives it. */
function windowState(more: Partial<WindowState> = {}): WindowState {
  return { sidebar_width: 280, placement: "auto", split: 50, tabs: [], active: null, ...more };
}

/** A send of "List users" as the history keeps it. */
function historyEntry(id: number): StoredHistoryEntry {
  return {
    id,
    sent_at: new Date().toISOString(),
    node_id: "req-list-users",
    name: "List users",
    method: "GET",
    url: "https://api.example.test/users",
    status: 200,
    error_kind: null,
    total_ms: 12,
    size: 2,
    request_headers: [],
    request_body: null,
    request_body_truncated: false,
    response_headers: [],
    response_body: "{}",
    response_body_truncated: false,
    response_body_binary: false,
    error_message: null,
    warnings: [],
  };
}

/** Lets the commands that were started end and their results be handled. */
async function settle() {
  for (let turn = 0; turn < 20; turn++) await Promise.resolve();
}

/**
 * Draws the window over a fake api and waits until what was remembered is restored. From then on the timers are
 * fake — only they: the promises of the fake api and the rendering go on as usual.
 */
async function renderApp(seed: FakeSeed = {}) {
  const api = createFakeApi(seed);
  render(App, { context: new Map([["api", api]]) });
  await screen.findByRole("treeitem", { name: /Demo API/ });
  await settle();
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  return api;
}

/** Opens the request in a tab and returns its URL field. */
async function openRequest(api: ReturnType<typeof createFakeApi>, nodeId: string): Promise<HTMLInputElement> {
  await tabs.open(api, nodeId);
  return waitFor(() => {
    const input = document.querySelector<HTMLInputElement>("[data-tab-panel]:not([hidden]) [data-url-input]");
    expect(input).not.toBeNull();
    return input!;
  });
}

async function type(input: HTMLInputElement, value: string) {
  await fireEvent.input(input, { target: { value } });
}

const divider = () => screen.getByRole("separator", { name: strings.layout.resize });

// The tabs, the notifications, the layout and what is remembered live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  vi.useRealTimers();
  vi.restoreAllMocks();
  session.stop();
  layout.apply({});
  toasts.items = [];
  history.items = [];
  history.hasMore = false;
  workspace.rows = [];
});

describe("Session", () => {
  it("waits 500 ms before writing the state of the window and 1000 ms before writing a draft", () => {
    expect(WINDOW_SAVE_DELAY_MS).toBe(500);
    expect(DRAFT_SAVE_DELAY_MS).toBe(1000);
  });

  describe("drafts", () => {
    it("writes the draft 1000 ms after an edit of the URL", async () => {
      const api = await renderApp();
      const url = await openRequest(api, "req-list-users");

      await type(url, "https://api.example.test/changed");
      await vi.advanceTimersByTimeAsync(999);
      expect(api.draftCalls).toEqual([]);

      await vi.advanceTimersByTimeAsync(1);
      expect(api.draftCalls).toHaveLength(1);
      expect(api.draftCalls[0][0]).toBe("req-list-users");
      expect(api.draftCalls[0][1]).toEqual((tabs.active as Tab<RequestDraft>).draft);
      expect(api.draftCalls[0][1]).toMatchObject({ method: "GET", url: "https://api.example.test/changed" });
      expect(toasts.items).toEqual([]);
    });

    it("writes a quick series of edits with one call, 1000 ms after the last one", async () => {
      const api = await renderApp();
      const url = await openRequest(api, "req-list-users");

      for (const text of ["https://a", "https://ab", "https://abc"]) {
        await type(url, text);
        await vi.advanceTimersByTimeAsync(600);
      }
      expect(api.draftCalls).toEqual([]);

      await vi.advanceTimersByTimeAsync(400);
      expect(api.draftCalls).toHaveLength(1);
      expect(api.draftCalls[0][1]).toMatchObject({ url: "https://abc" });
      await vi.advanceTimersByTimeAsync(5000);
      expect(api.draftCalls).toHaveLength(1);
    });

    it("deletes the written draft at once when Ctrl+S saves the tab", async () => {
      const api = await renderApp();
      const url = await openRequest(api, "req-list-users");
      await type(url, "https://api.example.test/changed");
      await vi.advanceTimersByTimeAsync(1000);
      expect(api.draftCalls).toHaveLength(1);

      await fireEvent.keyDown(window, CTRL_S);
      await vi.advanceTimersByTimeAsync(0);

      expect(tabs.active?.dirty).toBe(false);
      expect(api.draftCalls).toHaveLength(2);
      expect(api.draftCalls[1]).toEqual(["req-list-users", null]);
      await vi.advanceTimersByTimeAsync(5000);
      expect(api.draftCalls).toHaveLength(2);
    });

    it("drops the write that was waiting when Ctrl+S saves the tab before it", async () => {
      const api = await renderApp();
      const url = await openRequest(api, "req-list-users");
      await type(url, "https://api.example.test/changed");
      await vi.advanceTimersByTimeAsync(300);

      await fireEvent.keyDown(window, CTRL_S);
      await vi.advanceTimersByTimeAsync(5000);

      expect(tabs.active?.dirty).toBe(false);
      // Nothing was written, so there is nothing to delete.
      expect(api.draftCalls).toEqual([]);
    });

    it("deletes the written draft when the edit is put back", async () => {
      const api = await renderApp();
      const url = await openRequest(api, "req-list-users");
      await type(url, "https://api.example.test/changed");
      await vi.advanceTimersByTimeAsync(1000);

      await type(url, "https://api.example.test/users");
      await vi.advanceTimersByTimeAsync(0);

      expect(tabs.active?.dirty).toBe(false);
      expect(api.draftCalls.map(([id, content]) => [id, content === null])).toEqual([
        ["req-list-users", false],
        ["req-list-users", true],
      ]);
    });

    it("deletes the written draft when the tab is closed without saving", async () => {
      const api = await renderApp();
      const saveNodeContent = vi.spyOn(api, "saveNodeContent");
      const url = await openRequest(api, "req-list-users");
      await type(url, "https://api.example.test/changed");
      await vi.advanceTimersByTimeAsync(1000);

      await fireEvent.click(screen.getByRole("button", { name: `${strings.tabs.close}: List users` }));
      await fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: strings.tabs.discard }));
      await vi.advanceTimersByTimeAsync(0);

      expect(tabs.list).toHaveLength(0);
      expect(saveNodeContent).not.toHaveBeenCalled();
      expect(api.draftCalls).toHaveLength(2);
      expect(api.draftCalls[1]).toEqual(["req-list-users", null]);
    });

    it("never calls saveDraft for a tab without edits", async () => {
      const api = await renderApp();
      await openRequest(api, "req-list-users");

      await vi.advanceTimersByTimeAsync(5000);
      await fireEvent.keyDown(window, CTRL_S);
      await vi.advanceTimersByTimeAsync(5000);
      tabs.close("req-list-users");
      await vi.advanceTimersByTimeAsync(5000);

      expect(api.draftCalls).toEqual([]);
    });

    it("shows one notification when the write fails and tries again after the next edit", async () => {
      const api = await renderApp();
      const saveDraft = vi.spyOn(api, "saveDraft").mockRejectedValueOnce("storage is locked");
      const notified = vi.spyOn(toasts, "error");
      const url = await openRequest(api, "req-list-users");

      await type(url, "https://api.example.test/one");
      await vi.advanceTimersByTimeAsync(1000);
      expect(saveDraft).toHaveBeenCalledTimes(1);
      expect(toasts.items.map(({ kind, text }) => ({ kind, text }))).toEqual([{ kind: "error", text: "storage is locked" }]);

      // Nothing is tried again by itself.
      await vi.advanceTimersByTimeAsync(5000);
      expect(saveDraft).toHaveBeenCalledTimes(1);

      await type(url, "https://api.example.test/two");
      await vi.advanceTimersByTimeAsync(1000);
      expect(saveDraft).toHaveBeenCalledTimes(2);
      expect(api.draftCalls).toEqual([["req-list-users", expect.objectContaining({ url: "https://api.example.test/two" })]]);
      // The second write went through: nothing more was notified.
      expect(notified).toHaveBeenCalledTimes(1);
    });
  });

  describe("state of the window", () => {
    it("writes the new share once, 500 ms after the last of five presses of an arrow on the divider", async () => {
      const api = await renderApp();
      await openRequest(api, "req-list-users");
      await vi.advanceTimersByTimeAsync(500);
      api.windowStates.length = 0;

      for (let press = 0; press < 5; press++) {
        await fireEvent.keyDown(divider(), { key: "ArrowDown" });
        await vi.advanceTimersByTimeAsync(100);
      }
      await vi.advanceTimersByTimeAsync(399);
      expect(api.windowStates).toEqual([]);

      await vi.advanceTimersByTimeAsync(1);
      expect(api.windowStates).toEqual([
        windowState({ split: 60, tabs: [{ kind: "node", id: "req-list-users" }], active: "req-list-users" }),
      ]);
      await vi.advanceTimersByTimeAsync(5000);
      expect(api.windowStates).toHaveLength(1);
    });

    it("writes the chosen placement and the width of the sidebar", async () => {
      const api = await renderApp();

      await fireEvent.click(screen.getByRole("button", { name: strings.layout.button }));
      await fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitemradio", { name: strings.layout.placements.left }));
      await fireEvent.keyDown(screen.getByRole("separator", { name: strings.sidebar.resize }), { key: "ArrowRight" });
      await vi.advanceTimersByTimeAsync(500);

      expect(api.windowStates).toEqual([windowState({ placement: "left", sidebar_width: 296 })]);
    });

    it("writes the tabs when one is opened and when one is closed", async () => {
      const api = await renderApp({ environments: [{ id: "env-dev", name: "Dev" }] });

      await tabs.open(api, "req-list-users");
      await tabs.open(api, "fold-users");
      await tabs.openEnvironment(api, "env-dev");
      await vi.advanceTimersByTimeAsync(500);
      expect(api.windowStates).toEqual([
        windowState({
          tabs: [
            { kind: "node", id: "req-list-users" },
            { kind: "node", id: "fold-users" },
            { kind: "environment", id: "env-dev" },
          ],
          active: "env:env-dev",
        }),
      ]);

      tabs.close("req-list-users");
      await vi.advanceTimersByTimeAsync(500);
      expect(api.windowStates).toHaveLength(2);
      expect(api.windowStates[1].tabs).toEqual([
        { kind: "node", id: "fold-users" },
        { kind: "environment", id: "env-dev" },
      ]);
      expect(api.windowStates[1].active).toBe("env:env-dev");

      tabs.activate("fold-users");
      await vi.advanceTimersByTimeAsync(500);
      expect(api.windowStates).toHaveLength(3);
      expect(api.windowStates[2].active).toBe("fold-users");
    });

    it("leaves the tab of a history entry out", async () => {
      const api = await renderApp({ history: [historyEntry(7)] });

      await tabs.open(api, "req-get-order");
      await tabs.openHistory(api, 7);
      await vi.advanceTimersByTimeAsync(500);

      expect(tabs.list).toHaveLength(2);
      expect(api.windowStates).toEqual([windowState({ tabs: [{ kind: "node", id: "req-get-order" }], active: null })]);
    });

    it("shows one notification when the write fails and tries again after the next change", async () => {
      const api = await renderApp();
      const notified = vi.spyOn(toasts, "error");
      const save = vi.spyOn(api, "saveWindowState").mockRejectedValueOnce("storage is locked");

      layout.choose("top");
      await vi.advanceTimersByTimeAsync(500);
      expect(save).toHaveBeenCalledTimes(1);
      expect(toasts.items.map(({ kind, text }) => ({ kind, text }))).toEqual([{ kind: "error", text: "storage is locked" }]);
      await vi.advanceTimersByTimeAsync(5000);
      expect(save).toHaveBeenCalledTimes(1);

      layout.resize(40);
      await vi.advanceTimersByTimeAsync(500);
      expect(api.windowStates).toEqual([windowState({ placement: "top", split: 40 })]);
      expect(notified).toHaveBeenCalledTimes(1);
    });
  });

  describe("flush", () => {
    it("writes what was waiting for its time at once", async () => {
      const api = await renderApp();
      const url = await openRequest(api, "req-list-users");
      await vi.advanceTimersByTimeAsync(500);
      api.windowStates.length = 0;
      await type(url, "https://api.example.test/changed");
      layout.choose("left");

      expect(await session.flush(api)).toBe(true);

      expect(api.draftCalls).toEqual([["req-list-users", expect.objectContaining({ url: "https://api.example.test/changed" })]]);
      expect(api.windowStates).toEqual([
        windowState({ placement: "left", tabs: [{ kind: "node", id: "req-list-users" }], active: "req-list-users" }),
      ]);
      // Nothing is left for the timers.
      await vi.advanceTimersByTimeAsync(5000);
      expect(api.draftCalls).toHaveLength(1);
      expect(api.windowStates).toHaveLength(1);
    });

    it("writes nothing when nothing is waiting", async () => {
      const api = await renderApp();
      await openRequest(api, "req-list-users");
      await vi.advanceTimersByTimeAsync(500);
      api.windowStates.length = 0;

      expect(await session.flush(api)).toBe(true);
      expect(api.draftCalls).toEqual([]);
      expect(api.windowStates).toEqual([]);
    });

    it("tells that something was not written, without a notification", async () => {
      const api = await renderApp();
      const url = await openRequest(api, "req-list-users");
      await type(url, "https://api.example.test/changed");

      vi.spyOn(api, "saveDraft").mockRejectedValueOnce("storage is locked");
      expect(await session.flush(api)).toBe(false);
      expect(toasts.items).toEqual([]);

      // The next time everything is written.
      expect(await session.flush(api)).toBe(true);
      expect(api.draftCalls).toHaveLength(1);

      vi.spyOn(api, "saveWindowState").mockRejectedValueOnce("storage is locked");
      layout.choose("top");
      expect(await session.flush(api)).toBe(false);
      expect(await session.flush(api)).toBe(true);
      expect(api.windowStates.at(-1)).toMatchObject({ placement: "top" });
      expect(toasts.items).toEqual([]);
    });
  });

  describe("start", () => {
    it("shows a notification and the default layout when the session cannot be read", async () => {
      layout.choose("left");
      layout.resize(30);
      const api = createFakeApi();
      vi.spyOn(api, "loadSession").mockRejectedValue("storage is unavailable");

      await expect(session.start(api)).resolves.toBeUndefined();

      expect(toasts.items.map(({ kind, text }) => ({ kind, text }))).toEqual([{ kind: "error", text: "storage is unavailable" }]);
      expect(layout.placement).toBe("auto");
      expect(layout.split).toBe(50);
      expect(session.sidebarWidth).toBe(280);
    });

    it("applies the remembered layout before the window is drawn", async () => {
      const api = createFakeApi({ session: { window: windowState({ sidebar_width: 320, placement: "left", split: 36 }), drafts: [] } });

      await session.start(api);

      expect(layout.placement).toBe("left");
      expect(layout.split).toBe(36);
      expect(session.sidebarWidth).toBe(320);
    });

    it.each([
      [100, 200],
      [900, 480],
      ["wide", 280],
    ])("brings a remembered width of the sidebar of %s to %d and repairs the rest", async (width, expected) => {
      const window = { sidebar_width: width, placement: "diagonal", split: 95, tabs: [], active: null } as unknown as WindowState;
      const api = createFakeApi({ session: { window, drafts: [] } });

      await session.start(api);

      expect(session.sidebarWidth).toBe(expected);
      expect(layout.placement).toBe("auto");
      expect(layout.split).toBe(80);
    });

    it("reads the session once for start and open", async () => {
      const api = createFakeApi();
      const loadSession = vi.spyOn(api, "loadSession");

      await session.start(api);
      await session.open(api);

      expect(loadSession).toHaveBeenCalledTimes(1);
    });
  });

  describe("restoring", () => {
    const REMEMBERED: SessionDto = {
      window: windowState({
        sidebar_width: 320,
        placement: "left",
        split: 36,
        tabs: [
          { kind: "node", id: "req-list-users" },
          { kind: "node", id: "req-create-user" },
        ],
        active: "req-create-user",
      }),
      drafts: [{ node_id: "req-create-user", content: { method: "POST", url: "https://api.example.test/draft" } }],
      measure: false,
    };

    it("opens the remembered tabs with their drafts and writes nothing by itself", async () => {
      const api = await renderApp({ session: REMEMBERED });

      expect(tabs.list.map((tab) => [tab.id, tab.dirty])).toEqual([
        ["req-list-users", false],
        ["req-create-user", true],
      ]);
      expect(tabs.activeId).toBe("req-create-user");
      expect((tabs.active as Tab<RequestDraft>).draft.url).toBe("https://api.example.test/draft");
      // The response of a restored tab is empty.
      expect((tabs.active as Tab<RequestDraft>).sending.result).toBeNull();

      await vi.advanceTimersByTimeAsync(5000);
      expect(api.draftCalls).toEqual([]);
      expect(api.windowStates).toEqual([]);
      expect(toasts.items).toEqual([]);
    });

    it("deletes a restored draft when its tab is saved", async () => {
      const api = await renderApp({ session: REMEMBERED });

      await fireEvent.keyDown(window, CTRL_S);
      await vi.advanceTimersByTimeAsync(0);

      expect(tabs.active?.dirty).toBe(false);
      expect(api.draftCalls).toEqual([["req-create-user", null]]);
    });

    it("writes a restored draft again after it is edited", async () => {
      const api = await renderApp({ session: REMEMBERED });

      (tabs.active as Tab<RequestDraft>).draft.url = "https://api.example.test/edited";
      await vi.advanceTimersByTimeAsync(1000);

      expect(api.draftCalls).toEqual([["req-create-user", expect.objectContaining({ url: "https://api.example.test/edited" })]]);
    });
  });

  it("stop drops what was waiting for its time", async () => {
    const api = await renderApp();
    const url = await openRequest(api, "req-list-users");
    await type(url, "https://api.example.test/changed");
    layout.choose("left");

    session.stop();
    await vi.advanceTimersByTimeAsync(5000);

    expect(api.draftCalls).toEqual([]);
    expect(api.windowStates).toEqual([]);
  });
});
