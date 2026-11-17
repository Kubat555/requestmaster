import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { formatBytes, formatMs } from "../lib/format";
import { history } from "../lib/history.svelte";
import { strings } from "../lib/strings";
import { historyTabId, Tab, tabs } from "../lib/tabs.svelte";
import { createFakeApi, type FakeApi, type StoredHistoryEntry } from "../lib/testing/fakeApi";
import { pressEnter } from "../lib/testing/keys";
import { toasts } from "../lib/toasts.svelte";
import { normalizeContent, type RequestDraft, type SendResult } from "../lib/types";
import { workspace } from "../lib/workspace.svelte";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const TEXT = strings.history;

/** A moment of today, of yesterday (`daysAgo = 1`) and so on, at this local time. */
function at(daysAgo: number, hours: number, minutes = 0, seconds = 0): string {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() - daysAgo, hours, minutes, seconds).toISOString();
}

/** A successful send of "List users" (a request of the default tree) as the history keeps it. */
function entry(id: number, overrides: Partial<StoredHistoryEntry> = {}): StoredHistoryEntry {
  return {
    id,
    sent_at: at(0, 9, 5, 7),
    node_id: "req-list-users",
    name: "List users",
    method: "GET",
    url: "https://api.example.test/users?page=1",
    status: 200,
    error_kind: null,
    total_ms: 123.4,
    size: 2048,
    request_headers: [["accept", "application/json"]],
    request_body: null,
    request_body_truncated: false,
    response_headers: [
      ["content-type", "application/json"],
      ["x-request-id", "abc-123"],
    ],
    response_body: '{"users":[1,2]}',
    response_body_truncated: false,
    response_body_binary: false,
    error_message: null,
    warnings: [],
    ...overrides,
  };
}

/** A send that got no response. */
function failedEntry(id: number, overrides: Partial<StoredHistoryEntry> = {}): StoredHistoryEntry {
  return entry(id, {
    status: null,
    error_kind: "connect",
    total_ms: null,
    size: null,
    response_headers: [],
    response_body: null,
    error_message: "connection refused",
    ...overrides,
  });
}

/** Renders the window over a fake api with this history and shows the section of the history in the sidebar. */
async function openHistory(entries: StoredHistoryEntry[] = []) {
  const api = createFakeApi({ history: entries });
  const list = vi.spyOn(api, "historyList");
  render(App, { context: new Map([["api", api]]) });
  await fireEvent.click(screen.getByRole("tab", { name: strings.sidebar.history }));
  await waitFor(() => expect(list).toHaveBeenCalled());
  if (entries.length > 0) await waitFor(() => expect(rows().length).toBeGreaterThan(0));
  await settle();
  return { api, list };
}

/** Lets the commands that were started end and their results be handled. */
async function settle() {
  for (let turn = 0; turn < 10; turn++) await Promise.resolve();
}

/** The rows of the history on the screen, top to bottom. */
function rows(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>("button[data-history]")];
}

/** The row of the entry with this id. */
function row(id: number): HTMLElement {
  const found = rows().find((candidate) => candidate.dataset.history === String(id));
  if (!found) throw new Error(`no row of the history entry ${id}`);
  return found;
}

/** Opens the entry in a tab and returns what its tab panel shows. */
async function openEntry(id: number, name = "List users") {
  await fireEvent.click(row(id));
  await screen.findByRole("tab", { name: TEXT.tabName(name) });
  return within(document.querySelector<HTMLElement>("[data-tab-panel]:not([hidden])")!);
}

/** The button that reads older entries, if it is shown. Found by its text: a search by role over hundreds of rows is slow. */
function moreButton(): HTMLButtonElement | null {
  return screen.queryByText(TEXT.more)?.closest("button") ?? null;
}

function tabNames(): string[] {
  return tabs.list.map((tab) => tab.name);
}

function toastList(): { kind: string; text: string }[] {
  return toasts.items.map(({ kind, text }) => ({ kind, text }));
}

/** Presses a key combination on the window. */
async function press(init: KeyboardEventInit) {
  await fireEvent(document.body, createEvent.keyDown(document.body, init));
}

function pressed(button: HTMLElement): boolean {
  return button.getAttribute("aria-pressed") === "true";
}

// The tabs, the notifications, the history and the workspace of the window live in modules; every test starts
// without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
  history.items = [];
  history.hasMore = false;
  workspace.rows = [];
});

describe("the history section", () => {
  it("says what will appear here while nothing was sent", async () => {
    const { list } = await openHistory();

    expect(screen.getByText(strings.sidebar.historyEmpty)).toBeTruthy();
    expect(strings.sidebar.historyEmpty).toBe("Здесь появятся отправленные запросы");
    expect(rows()).toEqual([]);
    expect(moreButton()).toBeNull();
    expect(list).toHaveBeenCalledTimes(1);
    expect(list).toHaveBeenCalledWith(100, null);
  });

  it("groups the entries by days, the newest first", async () => {
    await openHistory([
      entry(1, { name: "Oldest", sent_at: new Date(2020, 2, 7, 15, 0).toISOString() }),
      entry(2, { name: "Yesterday", sent_at: at(1, 23, 59, 59) }),
      entry(3, { name: "Morning", sent_at: at(0, 0, 0, 1) }),
      entry(4, { name: "Latest", sent_at: at(0, 9, 5, 7) }),
    ]);

    expect(screen.getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent)).toEqual([
      TEXT.today,
      TEXT.yesterday,
      "07.03.2020",
    ]);
    expect(rows().map((item) => item.dataset.history)).toEqual(["4", "3", "2", "1"]);
    const namesOf = (day: string) =>
      within(screen.getByRole("list", { name: day }))
        .getAllByRole("button")
        .map((item) => item.querySelector(".name")!.textContent);
    expect(namesOf(TEXT.today)).toEqual(["Latest", "Morning"]);
    expect(namesOf(TEXT.yesterday)).toEqual(["Yesterday"]);
    expect(namesOf("07.03.2020")).toEqual(["Oldest"]);
    expect(screen.queryByText(strings.sidebar.historyEmpty)).toBeNull();
  });

  it("shows the time, the method, the name, the status and the duration of a successful send", async () => {
    await openHistory([entry(1, { method: "POST", status: 404 })]);

    const item = within(row(1));
    expect(item.getByText("09:05:07")).toBeTruthy();
    expect(item.getByText("POST").classList.contains("method-post")).toBe(true);
    expect(item.getByText("List users")).toBeTruthy();
    expect(item.getByText("404").classList.contains("client")).toBe(true);
    expect(item.getByText(formatMs(123.4))).toBeTruthy();
    // The address is in the tooltip.
    expect(row(1).title).toBe("https://api.example.test/users?page=1");
  });

  it.each([
    ["cancelled", "отменён"],
    ["connect", "нет соединения"],
    ["timeout", "таймаут"],
  ] as const)("shows what went wrong instead of the status of a failed send: %s", async (kind, text) => {
    await openHistory([failedEntry(1, { error_kind: kind })]);

    const item = within(row(1));
    expect(TEXT.errors[kind]).toBe(text);
    expect(item.getByText(text)).toBeTruthy();
    expect(item.getByText("09:05:07")).toBeTruthy();
    expect(item.getByText("List users")).toBeTruthy();
    expect(row(1).querySelector(".status")).toBeNull();
    expect(row(1).querySelector(".time")).toBeNull();
  });

  it("shows the first 100 entries and reads 100 more with the button, which goes away with the last ones", async () => {
    const { list } = await openHistory(Array.from({ length: 150 }, (_, index) => entry(index + 1)));

    expect(rows()).toHaveLength(100);
    expect(rows()[0].dataset.history).toBe("150");
    expect(rows()[99].dataset.history).toBe("51");

    await fireEvent.click(moreButton()!);
    await waitFor(() => expect(rows()).toHaveLength(150));

    expect(list).toHaveBeenCalledTimes(2);
    expect(list).toHaveBeenLastCalledWith(100, 51);
    expect(rows()[149].dataset.history).toBe("1");
    // Fewer than 100 came: there is nothing more to read.
    expect(moreButton()).toBeNull();
  });

  it("keeps the button while a whole page of 100 comes", async () => {
    await openHistory(Array.from({ length: 200 }, (_, index) => entry(index + 1)));

    await fireEvent.click(moreButton()!);
    await waitFor(() => expect(rows()).toHaveLength(200));

    expect(moreButton()).not.toBeNull();
  });

  it("keeps the older entries that were read with the button when a send adds a new one", async () => {
    const { api } = await openHistory(Array.from({ length: 150 }, (_, index) => entry(index + 1)));
    await fireEvent.click(moreButton()!);
    await waitFor(() => expect(rows()).toHaveLength(150));

    api.history.push(entry(151));
    await history.reload(api);

    await waitFor(() => expect(rows()).toHaveLength(151));
    expect(rows().map((item) => Number(item.dataset.history))).toEqual(Array.from({ length: 151 }, (_, index) => 151 - index));
    expect(moreButton()).toBeNull();
  });

  it("shows the entry of a send as soon as the send ends", async () => {
    const { api, list } = await openHistory();
    api.onSend = async () => {
      // The shell writes the entry before the command returns.
      api.history.push(entry(1));
      return { response: null, error: { kind: "connect", message: "", fields: [] }, warnings: [] };
    };
    await tabs.open(api, "req-list-users");
    const tab = tabs.active as Tab<RequestDraft>;

    await tab.sending.send(api, tab);

    await waitFor(() => expect(rows()).toHaveLength(1));
    expect(list).toHaveBeenCalledTimes(2);
    expect(list).toHaveBeenLastCalledWith(100, null);
  });

  it("shows a failure to read the history as a notification", async () => {
    const api = createFakeApi();
    api.historyList = () => Promise.reject("database is locked");
    render(App, { context: new Map([["api", api]]) });

    await fireEvent.click(screen.getByRole("tab", { name: strings.sidebar.history }));

    await waitFor(() => expect(toastList()).toEqual([{ kind: "error", text: "database is locked" }]));
    expect(screen.getByText(strings.sidebar.historyEmpty)).toBeTruthy();
  });
});

describe("the history after a send", () => {
  /** A fake api whose send stays unanswered until the test ends it, and a request tab. */
  function setup() {
    const api: FakeApi = createFakeApi();
    const list = vi.spyOn(api, "historyList");
    let resolve: (result: SendResult) => void = () => {};
    let reject: (error: string) => void = () => {};
    api.onSend = () =>
      new Promise<SendResult>((done, fail) => {
        resolve = done;
        reject = fail;
      });
    const tab = new Tab<RequestDraft>(
      { id: "req-list-users", kind: "request", name: "List users" },
      normalizeContent("request", { method: "GET", url: "https://api.example.test/users" }),
    );
    return { api, list, tab, resolve: (result: SendResult) => resolve(result), reject: (error: string) => reject(error) };
  }

  const cancelled: SendResult = { response: null, error: { kind: "cancelled", message: "", fields: [] }, warnings: [] };

  it("is not read on Cancel, and is read once when the command itself ends", async () => {
    const { api, list, tab, resolve } = setup();

    const sent = tab.sending.send(api, tab);
    tab.sending.cancel(api);
    await settle();

    // The shell writes the cancelled entry only when the command returns.
    expect(api.cancelled).toHaveLength(1);
    expect(tab.sending.state).toBe("idle");
    expect(list).not.toHaveBeenCalled();

    resolve(cancelled);
    await sent;
    await settle();

    expect(list).toHaveBeenCalledTimes(1);
    expect(list).toHaveBeenCalledWith(100, null);
    expect(tab.sending.result?.error?.kind).toBe("cancelled");
  });

  it("is read once after a send that was answered", async () => {
    const { api, list, tab, resolve } = setup();

    const sent = tab.sending.send(api, tab);
    await settle();
    expect(list).not.toHaveBeenCalled();
    resolve({ response: null, error: { kind: "timeout", message: "", fields: [] }, warnings: [] });
    await sent;
    await settle();

    expect(list).toHaveBeenCalledTimes(1);
    expect(tab.sending.result?.error?.kind).toBe("timeout");
  });

  it("is read once after a send whose command failed", async () => {
    const { api, list, tab, reject } = setup();

    const sent = tab.sending.send(api, tab);
    reject("request not found");
    await sent;
    await settle();

    expect(list).toHaveBeenCalledTimes(1);
    expect(toastList()).toEqual([{ kind: "error", text: "request not found" }]);
  });

  it("leaves the result of the send as it is when the history cannot be read", async () => {
    const { api, tab, resolve } = setup();
    api.historyList = () => Promise.reject("database is locked");

    const sent = tab.sending.send(api, tab);
    resolve({ response: null, error: { kind: "timeout", message: "late", fields: [] }, warnings: [] });
    await sent;
    await settle();

    expect(tab.sending.state).toBe("idle");
    expect(tab.sending.result).toEqual({ response: null, error: { kind: "timeout", message: "late", fields: [] }, warnings: [] });
    expect(toastList()).toEqual([{ kind: "error", text: "database is locked" }]);
  });
});

describe("a history entry", () => {
  it("opens in a read-only tab that shows the request and the response", async () => {
    const { api } = await openHistory([
      entry(7, {
        method: "POST",
        sent_at: new Date(2026, 10, 13, 9, 5, 7).toISOString(),
        request_headers: [
          ["content-type", "text/plain"],
          ["authorization", "Bearer abc"],
        ],
        request_body: "hello",
        response_body: "plain answer",
      }),
    ]);
    const get = vi.spyOn(api, "historyGet");

    const view = await openEntry(7);

    expect(get).toHaveBeenCalledTimes(1);
    expect(get).toHaveBeenCalledWith(7);
    expect(tabs.active?.id).toBe(historyTabId(7));
    expect(tabs.active?.kind).toBe("history");
    expect(tabNames()).toEqual(["История · List users"]);

    // The header: when, what, and how it ended.
    expect(view.getByText("13.11.2026 09:05:07")).toBeTruthy();
    expect(view.getByText("POST").classList.contains("method-post")).toBe(true);
    expect(view.getByText("https://api.example.test/users?page=1")).toBeTruthy();
    expect(view.getByText("200").classList.contains("ok")).toBe(true);
    expect(view.getByText(formatMs(123.4))).toBeTruthy();
    expect(view.getByText(formatBytes(2048))).toBeTruthy();

    const request = within(view.getByRole("region", { name: TEXT.request }));
    expect(within(request.getByRole("table")).getByText("authorization")).toBeTruthy();
    expect(within(request.getByRole("table")).getByText("Bearer abc")).toBeTruthy();
    const requestBody = request.getByRole<HTMLTextAreaElement>("textbox", { name: TEXT.requestBody });
    expect(requestBody.value).toBe("hello");
    expect(requestBody.readOnly).toBe(true);

    const response = within(view.getByRole("region", { name: TEXT.response }));
    expect(within(response.getByRole("table")).getByText("x-request-id")).toBeTruthy();
    expect(within(response.getByRole("table")).getByText("abc-123")).toBeTruthy();
    const responseBody = response.getByRole<HTMLTextAreaElement>("textbox", { name: strings.response.bodyText });
    expect(responseBody.value).toBe("plain answer");
    expect(responseBody.readOnly).toBe(true);
  });

  it("takes both the area of the editor and the area of the response", async () => {
    const { api } = await openHistory([entry(1)]);
    /** The area of the window where the response of a request tab is shown. */
    const responseArea = () => document.querySelector<HTMLElement>("main section.response")!;
    await tabs.open(api, "req-list-users");
    await screen.findByRole("tab", { name: /List users/ });
    expect(responseArea().hidden).toBe(false);
    // `data-side` is on the area only while the editor and the response share it.
    expect(responseArea().closest(".work")!.hasAttribute("data-side")).toBe(true);
    expect(screen.queryByRole("separator", { name: strings.layout.resize })).not.toBeNull();

    await openEntry(1);

    // The only "response" on the screen is the one of the entry, inside the panel of its tab.
    expect(responseArea().hidden).toBe(true);
    expect(responseArea().closest(".work")!.hasAttribute("data-side")).toBe(false);
    expect(screen.queryByRole("separator", { name: strings.layout.resize })).toBeNull();
    expect(screen.getByRole("region", { name: TEXT.response }).closest("[data-tab-panel]")).not.toBeNull();

    tabs.activate("req-list-users");
    await waitFor(() => expect(responseArea().hidden).toBe(false));
    expect(responseArea().closest(".work")!.hasAttribute("data-side")).toBe(true);
  });

  it("shows an entry that is already open instead of opening it again", async () => {
    const { api } = await openHistory([entry(1), entry(2, { name: "Other" })]);
    const get = vi.spyOn(api, "historyGet");

    await openEntry(1);
    await openEntry(2, "Other");
    await fireEvent.click(row(1));
    await settle();

    expect(get).toHaveBeenCalledTimes(2);
    expect(tabNames()).toEqual(["История · List users", "История · Other"]);
    expect(tabs.active?.id).toBe(historyTabId(1));
  });

  it("shows a failure to read the entry as a notification and opens nothing", async () => {
    const { api } = await openHistory([entry(1)]);
    api.history.length = 0;

    await fireEvent.click(row(1));

    await waitFor(() => expect(toastList()).toHaveLength(1));
    expect(toastList()[0].kind).toBe("error");
    expect(tabs.list).toEqual([]);
  });

  it("asks the core to format each body once and shows a JSON body as Pretty", async () => {
    const { api } = await openHistory([entry(1, { request_body: '{"a":1}', response_body: '{"users":[1,2]}' })]);
    const format = vi.spyOn(api, "formatJson");

    const view = await openEntry(1);
    const response = within(view.getByRole("region", { name: TEXT.response }));
    const pretty = response.getByRole("button", { name: strings.response.pretty });
    const raw = response.getByRole("button", { name: strings.response.raw });
    const body = () => response.getByRole<HTMLTextAreaElement>("textbox", { name: strings.response.bodyText });

    await waitFor(() => expect(pressed(pretty)).toBe(true));
    expect(body().value).toBe('{\n  "users": [\n    1,\n    2\n  ]\n}');
    expect(body().dataset.language).toBe("json");
    expect(format.mock.calls).toEqual([['{"a":1}'], ['{"users":[1,2]}']]);
    const request = within(view.getByRole("region", { name: TEXT.request }));
    const requestBody = request.getByRole<HTMLTextAreaElement>("textbox", { name: TEXT.requestBody });
    expect(requestBody.value).toBe('{\n  "a": 1\n}');
    // What was sent and what came back are shown as they were: the variables are neither marked nor offered.
    expect(requestBody.dataset.variables).toBe("false");
    expect(body().dataset.variables).toBe("false");

    await fireEvent.click(raw);

    expect(pressed(raw)).toBe(true);
    expect(pressed(pretty)).toBe(false);
    expect(body().value).toBe('{"users":[1,2]}');
    expect(body().dataset.language).toBe("text");

    await fireEvent.click(pretty);

    expect(body().value).toBe('{\n  "users": [\n    1,\n    2\n  ]\n}');
    // The text was formatted when the entry was opened; switching the view asks for nothing.
    expect(format).toHaveBeenCalledTimes(2);
  });

  it("offers only Raw for a body that is not JSON", async () => {
    const { api } = await openHistory([entry(1, { request_body: "a=1&b=2", response_body: "<html></html>" })]);
    const format = vi.spyOn(api, "formatJson");

    const view = await openEntry(1);
    await waitFor(() => expect(format).toHaveBeenCalledTimes(2));
    await settle();

    for (const [region, label, text] of [
      [TEXT.request, TEXT.requestBody, "a=1&b=2"],
      [TEXT.response, strings.response.bodyText, "<html></html>"],
    ]) {
      const part = within(view.getByRole("region", { name: region }));
      expect(part.getByRole<HTMLButtonElement>("button", { name: strings.response.pretty }).disabled).toBe(true);
      expect(pressed(part.getByRole("button", { name: strings.response.pretty }))).toBe(false);
      expect(pressed(part.getByRole("button", { name: strings.response.raw }))).toBe(true);
      const body = part.getByRole<HTMLTextAreaElement>("textbox", { name: label });
      expect(body.value).toBe(text);
      expect(body.dataset.language).toBe("text");
    }
  });

  it("says that a body was cut to 1 MB", async () => {
    await openHistory([
      entry(1, { response_body: "cut", response_body_truncated: true }),
      entry(2, { name: "Upload", request_body: "cut too", request_body_truncated: true }),
    ]);

    let view = await openEntry(1);
    expect(TEXT.truncated).toBe("Тело обрезано до 1 МБ");
    expect(within(view.getByRole("region", { name: TEXT.response })).getByText(TEXT.truncated)).toBeTruthy();
    expect(within(view.getByRole("region", { name: TEXT.request })).queryByText(TEXT.truncated)).toBeNull();

    view = await openEntry(2, "Upload");
    expect(within(view.getByRole("region", { name: TEXT.request })).getByText(TEXT.truncated)).toBeTruthy();
    expect(within(view.getByRole("region", { name: TEXT.response })).queryByText(TEXT.truncated)).toBeNull();
  });

  it("says that a response that is not text is not kept", async () => {
    const { api } = await openHistory([entry(1, { response_body: null, response_body_binary: true })]);
    const format = vi.spyOn(api, "formatJson");

    const view = await openEntry(1);
    await settle();

    const response = within(view.getByRole("region", { name: TEXT.response }));
    expect(TEXT.binary).toBe("Ответ не текстовый — в истории не хранится");
    expect(response.getByText(TEXT.binary)).toBeTruthy();
    expect(response.queryByRole("textbox")).toBeNull();
    expect(response.queryByRole("button", { name: strings.response.raw })).toBeNull();
    // The headers of the response are still there.
    expect(within(response.getByRole("table")).getByText("x-request-id")).toBeTruthy();
    // Neither body has anything to format.
    expect(format).not.toHaveBeenCalled();
  });

  it("says that the request or the response had no body", async () => {
    await openHistory([entry(1, { request_body: null, response_body: "" })]);

    const view = await openEntry(1);

    expect(within(view.getByRole("region", { name: TEXT.request })).getByText(strings.body.empty)).toBeTruthy();
    expect(within(view.getByRole("region", { name: TEXT.response })).getByText(strings.response.noBody)).toBeTruthy();
    expect(view.queryByRole("textbox")).toBeNull();
  });

  it("shows the error and the warnings of the send as the response panel does", async () => {
    await openHistory([
      failedEntry(1, {
        warnings: [
          { kind: "unresolved_variables", items: ["host", "token"] },
          { kind: "scripts_not_executed", items: ["Demo API"] },
        ],
      }),
    ]);

    const view = await openEntry(1);

    expect(view.getByText(strings.response.errors.connect).classList.contains("error")).toBe(true);
    expect(view.getByText("connection refused")).toBeTruthy();
    expect(view.getByText(strings.response.warnings.unresolved_variables(["host", "token"]))).toBeTruthy();
    expect(view.getByText(strings.response.warnings.scripts_not_executed(["Demo API"]))).toBeTruthy();
    const response = within(view.getByRole("region", { name: TEXT.response }));
    expect(response.queryByRole("table")).toBeNull();
    expect(response.queryByRole("textbox")).toBeNull();
    // The request went out, so its part is as usual.
    expect(within(view.getByRole("region", { name: TEXT.request })).getByRole("table")).toBeTruthy();
  });

  it("does not show a cancelled send as an error", async () => {
    await openHistory([failedEntry(1, { error_kind: "cancelled", error_message: "" })]);

    const view = await openEntry(1);

    expect(view.getByText(strings.response.errors.cancelled).classList.contains("error")).toBe(false);
  });

  it("opens the tab of the request it was sent from", async () => {
    await openHistory([entry(1)]);

    const view = await openEntry(1);
    const open = view.getByRole<HTMLButtonElement>("button", { name: TEXT.openRequest });
    expect(open.disabled).toBe(false);
    await fireEvent.click(open);

    await waitFor(() => expect(tabs.active?.id).toBe("req-list-users"));
    expect(tabs.active?.kind).toBe("request");
    expect(tabNames()).toEqual(["История · List users", "List users"]);
  });

  it("cannot open a request that was deleted, and says why", async () => {
    await openHistory([entry(1, { node_id: "req-gone", name: "Gone" })]);

    const view = await openEntry(1, "Gone");
    const open = view.getByRole<HTMLButtonElement>("button", { name: TEXT.openRequest });

    expect(open.disabled).toBe(true);
    expect(TEXT.requestDeleted).toBe("Запрос удалён");
    expect(open.title).toBe(TEXT.requestDeleted);
    expect(tabNames()).toEqual(["История · Gone"]);
  });

  it("is never unsaved: Ctrl+S saves nothing, the tab closes without a question, and so does the window", async () => {
    const { api } = await openHistory([entry(1)]);
    const save = vi.spyOn(api, "saveNodeContent");
    await openEntry(1);

    expect(tabs.active?.dirty).toBe(false);
    expect(screen.queryByRole("img", { name: strings.tabs.unsaved })).toBeNull();

    await press({ key: "s", code: "KeyS", ctrlKey: true });
    await settle();
    expect(save).not.toHaveBeenCalled();
    expect(toastList()).toEqual([]);

    expect(await api.requestClose()).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();

    await press({ key: "w", code: "KeyW", ctrlKey: true });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabs.list).toEqual([]);
  });

  it("is not counted among the unsaved tabs when the window is closed", async () => {
    const { api } = await openHistory([entry(1)]);
    await openEntry(1);
    await tabs.open(api, "req-create-user");
    (tabs.active as Tab<RequestDraft>).draft.url += "?edited";

    const closing = api.requestClose();

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(strings.app.closeText(1))).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.cancel }));
    expect(await closing).toBe(false);
  });
});

describe("clearing the history", () => {
  it("asks first, then deletes everything and closes the tabs of the history", async () => {
    const { api } = await openHistory([entry(1), entry(2, { name: "Other" })]);
    const clear = vi.spyOn(api, "historyClear");
    await openEntry(1);
    await openEntry(2, "Other");
    await tabs.open(api, "req-get-order");

    await fireEvent.click(screen.getByRole("button", { name: TEXT.clear }));

    const dialog = await screen.findByRole("dialog", { name: TEXT.clearTitle });
    expect(TEXT.clearTitle).toBe("Удалить всю историю?");
    expect(TEXT.clearText).toBe("Это нельзя отменить");
    expect(within(dialog).getByText(TEXT.clearText)).toBeTruthy();
    expect(clear).not.toHaveBeenCalled();

    await fireEvent.click(within(dialog).getByRole("button", { name: TEXT.clearConfirm }));
    await waitFor(() => expect(rows()).toEqual([]));

    expect(clear).toHaveBeenCalledTimes(1);
    expect(api.history).toEqual([]);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tabNames()).toEqual(["Get order"]);
    expect(await screen.findByText(strings.sidebar.historyEmpty)).toBeTruthy();
  });

  it("deletes nothing when the question is cancelled", async () => {
    const { api } = await openHistory([entry(1)]);
    const clear = vi.spyOn(api, "historyClear");
    await openEntry(1);

    await fireEvent.click(screen.getByRole("button", { name: TEXT.clear }));
    await fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: TEXT.cancel }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(clear).not.toHaveBeenCalled();
    expect(rows()).toHaveLength(1);
    expect(tabNames()).toEqual(["История · List users"]);
  });

  it("opens the question with the focus on Cancel, so Enter right away deletes nothing", async () => {
    const { api } = await openHistory([entry(1)]);
    const clear = vi.spyOn(api, "historyClear");

    await fireEvent.click(screen.getByRole("button", { name: TEXT.clear }));
    const dialog = await screen.findByRole("dialog", { name: TEXT.clearTitle });
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: TEXT.cancel }));

    await pressEnter();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(clear).not.toHaveBeenCalled();
    expect(rows()).toHaveLength(1);
  });

  it("keeps the history and its tabs when the deletion fails", async () => {
    const { api } = await openHistory([entry(1)]);
    api.historyClear = () => Promise.reject("database is locked");
    await openEntry(1);

    await fireEvent.click(screen.getByRole("button", { name: TEXT.clear }));
    await fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: TEXT.clearConfirm }));

    await waitFor(() => expect(toastList()).toEqual([{ kind: "error", text: "database is locked" }]));
    expect(rows()).toHaveLength(1);
    expect(tabNames()).toEqual(["История · List users"]);
  });

  it("has nothing to delete while the history is empty", async () => {
    await openHistory();

    expect(screen.getByRole<HTMLButtonElement>("button", { name: TEXT.clear }).disabled).toBe(true);
  });
});
