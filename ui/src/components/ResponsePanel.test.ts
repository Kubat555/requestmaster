import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import ResponsePanel from "./ResponsePanel.svelte";
import { formatBytes, formatMs } from "../lib/format";
import { Sending } from "../lib/sending.svelte";
import { strings } from "../lib/strings";
import { tabs } from "../lib/tabs.svelte";
import { toasts } from "../lib/toasts.svelte";
import { createFakeApi, noScriptResults, type SendHandler } from "../lib/testing/fakeApi";
import {
  normalizeContent,
  type NodeDto,
  type RequestDraft,
  type ResponseDto,
  type SendErrorDto,
  type SendResult,
  type Warning,
} from "../lib/types";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const utf8 = new TextEncoder();

function nodes(url: string): NodeDto[] {
  return [
    { id: "col", parent_id: null, kind: "collection", name: "API", content: {} },
    { id: "req", parent_id: "col", kind: "request", name: "List users", content: { method: "GET", url } },
  ];
}

/** A `200 OK` JSON response whose body is stored under `b1`; `overrides` replace whole fields. */
function response(overrides: Partial<ResponseDto> = {}): ResponseDto {
  return {
    status: 200,
    status_text: "OK",
    http_version: "HTTP/1.1",
    headers: [
      ["content-type", "application/json"],
      ["x-request-id", "abc-123"],
    ],
    cookies: [],
    redirects: [],
    ttfb_ms: 40,
    total_ms: 123.4,
    wire_size: 700,
    decoded_size: 2048,
    decode_error: null,
    sent: { method: "GET", url: "https://api.test/users", headers: [] },
    body: { id: "b1", total: 2048, truncated: false, json: true },
    ...overrides,
  };
}

function answered(overrides: Partial<ResponseDto> = {}, warnings: Warning[] = []): SendResult {
  return { response: response(overrides), error: null, warnings, ...noScriptResults() };
}

function failed(error: Partial<SendErrorDto> & Pick<SendErrorDto, "kind">, warnings: Warning[] = []): SendResult {
  return { response: null, error: { message: "", fields: [], ...error }, warnings, ...noScriptResults() };
}

/** A send that stays unanswered until the test resolves it. */
function deferred() {
  let resolve: (result: SendResult) => void = () => {};
  const onSend = vi.fn<SendHandler>(() => new Promise<SendResult>((done) => (resolve = done)));
  return { onSend, resolve: (result: SendResult) => resolve(result) };
}

interface Seed {
  url?: string;
  onSend?: SendHandler;
  bodies?: Record<string, Uint8Array>;
}

/** Renders the window over a fake api and opens the request `req` in a tab. */
async function openRequest({ url = "https://api.test/users", onSend, bodies = { b1: utf8.encode('{"a":1}') } }: Seed = {}) {
  const api = createFakeApi({ nodes: nodes(url), onSend, bodies });
  render(App, { context: new Map([["api", api]]) });
  await tabs.open(api, "req");
  await screen.findByRole("tab", { name: /List users/ });
  return api;
}

/** The response part of the window. */
function panel() {
  return within(screen.getByRole("region", { name: strings.response.label }));
}

function sendButton(): HTMLButtonElement {
  return screen.getByRole("button", { name: strings.editor.send });
}

async function clickSend(): Promise<void> {
  await fireEvent.click(sendButton());
}

/** Presses a key combination on the window and returns the event. */
async function press(init: KeyboardEventInit): Promise<KeyboardEvent> {
  const event = createEvent.keyDown(document.body, init);
  await fireEvent(document.body, event);
  return event as KeyboardEvent;
}

const CTRL_ENTER = { key: "Enter", code: "Enter", ctrlKey: true };

function bodyText(): HTMLTextAreaElement {
  return panel().getByRole("textbox", { name: strings.response.bodyText });
}

// The tabs and the notifications of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("ResponsePanel", () => {
  it("asks to press Send before the first send", async () => {
    await openRequest();

    expect(panel().getByText(strings.response.empty)).toBeTruthy();
    expect(sendButton().disabled).toBe(false);
  });

  it("sends the draft of the shown tab with Ctrl+Enter and cancels the default action", async () => {
    const onSend = vi.fn<SendHandler>(async () => answered());
    await openRequest({ onSend });
    await fireEvent.input(screen.getByRole("textbox", { name: strings.editor.url }), { target: { value: "https://api.test/people" } });

    const event = await press(CTRL_ENTER);

    expect(event.defaultPrevented).toBe(true);
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith(expect.any(String), "req", expect.objectContaining({ url: "https://api.test/people", method: "GET" }));
    expect(await panel().findByText("200 OK")).toBeTruthy();
  });

  it("does not let Ctrl+Enter reach the listeners below", async () => {
    await openRequest({ onSend: async () => answered() });
    const below = vi.fn();
    document.body.addEventListener("keydown", below);
    try {
      await press(CTRL_ENTER);
      expect(below).not.toHaveBeenCalled();
    } finally {
      document.body.removeEventListener("keydown", below);
    }
  });

  it("turns Send into Cancel while the request runs; Ctrl+Enter does nothing then", async () => {
    const { onSend, resolve } = deferred();
    await openRequest({ onSend });

    await clickSend();

    const cancel = screen.getByRole("button", { name: strings.editor.cancel });
    expect(screen.queryByRole("button", { name: strings.editor.send })).toBeNull();
    // The arrows of the button stay and move while the request is on its way.
    expect(cancel.classList.contains("exchanging")).toBe(true);
    expect(cancel.querySelectorAll("svg path")).toHaveLength(2);

    const event = await press(CTRL_ENTER);
    expect(event.defaultPrevented).toBe(true);
    expect(onSend).toHaveBeenCalledTimes(1);

    resolve(answered());
    expect(await panel().findByText("200 OK")).toBeTruthy();
    expect(sendButton().disabled).toBe(false);
    expect(sendButton().classList.contains("exchanging")).toBe(false);
    expect(screen.queryByRole("button", { name: strings.editor.cancel })).toBeNull();
  });

  it("the status of a response drops into its place once: again for the next response only", async () => {
    await openRequest({ onSend: async () => answered() });
    await clickSend();
    const status = await panel().findByText("200 OK");
    expect(status.classList.contains("pop-in")).toBe(true);

    await fireEvent.animationEnd(status);
    expect(status.classList.contains("pop-in")).toBe(false);

    await clickSend();
    await waitFor(() => expect(panel().getByText("200 OK").classList.contains("pop-in")).toBe(true));
  });

  it("shows the waiting indicator only after a delay", async () => {
    const { onSend, resolve } = deferred();
    await openRequest({ onSend });

    await clickSend();
    // A quick answer must not make the panel blink.
    expect(panel().queryByRole("status", { name: strings.response.sending })).toBeNull();
    expect(panel().getByText(strings.response.empty)).toBeTruthy();

    expect(await panel().findByRole("status", { name: strings.response.sending })).toBeTruthy();
    // The sentence is the name of the indicator, not a text on the screen.
    expect(panel().queryByText(strings.response.sending)).toBeNull();
    expect(panel().queryByText(strings.response.empty)).toBeNull();

    resolve(answered());
    expect(await panel().findByText("200 OK")).toBeTruthy();
    expect(panel().queryByRole("status", { name: strings.response.sending })).toBeNull();
  });

  describe("the waiting indicator", () => {
    afterEach(() => {
      vi.useRealTimers();
    });

    /** The panel of a send that stays unanswered until the test resolves it, under a clock the test moves. */
    function waitingPanel() {
      vi.useFakeTimers();
      const { onSend, resolve } = deferred();
      const api = createFakeApi({ onSend });
      const sending = new Sending();
      const view = render(ResponsePanel, { props: { sending }, context: new Map([["api", api]]) });
      const draft: RequestDraft = normalizeContent("request", { method: "GET", url: "https://api.test/users" });
      const sent = sending.send(api, { nodeId: "req", draft, sending });
      flushSync();
      const indicator = () => screen.queryByRole("status", { name: strings.response.sending });
      /** Moves the clock and lets the panel redraw. */
      const pass = (ms: number) => {
        vi.advanceTimersByTime(ms);
        flushSync();
      };
      return { view, sending, sent, resolve, indicator, pass };
    }

    it("is not shown during the first 150 ms and is shown after them", () => {
      const { indicator, pass } = waitingPanel();

      pass(149);
      expect(indicator()).toBeNull();

      pass(1);
      expect(indicator()).not.toBeNull();
    });

    it("is a turning ring, a progress line and the time, all of them hidden from a screen reader", () => {
      const { indicator, pass } = waitingPanel();
      pass(150);

      const parts = [...indicator()!.children];

      expect(indicator()!.querySelector(".spinner.lg")).not.toBeNull();
      expect(indicator()!.querySelector(".progress")).not.toBeNull();
      expect(parts.every((part) => part.getAttribute("aria-hidden") === "true")).toBe(true);
    });

    it("counts the time from the start of the send, in tenths of a second, ten times a second", () => {
      const { indicator, pass } = waitingPanel();
      const elapsed = () => indicator()!.querySelector(".elapsed")!.textContent;

      pass(150);
      // Not from zero: 150 ms had passed before the indicator appeared.
      expect(elapsed()).toBe(`0,1 ${strings.units.seconds}`);

      pass(100);
      expect(elapsed()).toBe(`0,2 ${strings.units.seconds}`);

      pass(200);
      expect(elapsed()).toBe(`0,4 ${strings.units.seconds}`);

      pass(1000);
      expect(elapsed()).toBe(`1,4 ${strings.units.seconds}`);
    });

    it("stops its timers when the send ends", async () => {
      const { sent, resolve, indicator, pass } = waitingPanel();
      pass(400);
      expect(vi.getTimerCount()).toBe(1);

      resolve(answered());
      await sent;
      flushSync();

      expect(indicator()).toBeNull();
      expect(vi.getTimerCount()).toBe(0);
    });

    it("sets no timer going when the send ends before the delay", async () => {
      const { sent, resolve, indicator, pass } = waitingPanel();
      pass(100);

      resolve(answered());
      await sent;
      flushSync();
      pass(1000);

      expect(indicator()).toBeNull();
      expect(vi.getTimerCount()).toBe(0);
    });

    it("stops its timers when the panel is removed", () => {
      const { view, pass } = waitingPanel();
      pass(400);

      view.unmount();

      expect(vi.getTimerCount()).toBe(0);
    });
  });

  it("Cancel shows that the request was cancelled; the late response does not replace it", async () => {
    const { onSend, resolve } = deferred();
    const api = await openRequest({ onSend });
    await clickSend();

    await fireEvent.click(screen.getByRole("button", { name: strings.editor.cancel }));

    expect(panel().getByText(strings.response.errors.cancelled)).toBeTruthy();
    expect(sendButton().disabled).toBe(false);
    expect(api.cancelled).toHaveLength(1);
    expect(api.cancelled[0]).toBe(onSend.mock.calls[0][0]);

    resolve(answered());
    await waitFor(() => expect(api.released).toEqual(["b1"]));
    expect(panel().getByText(strings.response.errors.cancelled)).toBeTruthy();
    expect(panel().queryByText("200 OK")).toBeNull();
  });

  it("shows a 200 JSON response: status, time, size and the pretty body; Raw asks for the body as received", async () => {
    const api = await openRequest({ onSend: async () => answered() });
    const responseBody = vi.spyOn(api, "responseBody");

    await clickSend();

    const status = await panel().findByText("200 OK");
    expect(status.classList.contains("ok")).toBe(true);
    const time = panel().getByText(formatMs(123.4));
    expect(time.title).toBe(strings.response.ttfb(formatMs(40)));
    const size = panel().getByText(formatBytes(2048));
    expect(size.title).toBe(strings.response.wireSize(formatBytes(700)));

    await waitFor(() => expect(bodyText().value).toBe('{"a":1}'));
    expect(responseBody.mock.calls).toEqual([["b1", true]]);
    expect(bodyText().readOnly).toBe(true);
    expect(bodyText().dataset.language).toBe("json");
    // A `{{name}}` in a response is a plain text, not a variable.
    expect(bodyText().dataset.variables).toBe("false");
    const pretty = panel().getByRole<HTMLButtonElement>("button", { name: strings.response.pretty });
    const raw = panel().getByRole<HTMLButtonElement>("button", { name: strings.response.raw });
    expect(pretty.disabled).toBe(false);
    expect(pretty.getAttribute("aria-pressed")).toBe("true");
    expect(raw.getAttribute("aria-pressed")).toBe("false");

    await fireEvent.click(raw);

    await waitFor(() => expect(responseBody.mock.calls).toEqual([["b1", true], ["b1", false]]));
    expect(raw.getAttribute("aria-pressed")).toBe("true");
    await waitFor(() => expect(bodyText().dataset.language).toBe("text"));
  });

  it("colours the status by its class and leaves out a size on the wire equal to the decoded one", async () => {
    await openRequest({
      onSend: async () => answered({ status: 404, status_text: "Not Found", wire_size: 2048 }),
    });

    await clickSend();

    const status = await panel().findByText("404 Not Found");
    expect(status.classList.contains("client")).toBe(true);
    expect(status.classList.contains("ok")).toBe(false);
    expect(panel().getByText(formatBytes(2048)).title).toBe("");
  });

  it("does not offer Pretty for a body that is not JSON", async () => {
    const api = await openRequest({
      onSend: async () => answered({ body: { id: "b1", total: 5, truncated: false, json: false } }),
      bodies: { b1: utf8.encode("hello") },
    });
    const responseBody = vi.spyOn(api, "responseBody");

    await clickSend();

    await waitFor(() => expect(bodyText().value).toBe("hello"));
    expect(responseBody.mock.calls).toEqual([["b1", false]]);
    expect(bodyText().dataset.language).toBe("text");
    expect(panel().getByRole<HTMLButtonElement>("button", { name: strings.response.pretty }).disabled).toBe(true);
    expect(panel().getByRole("button", { name: strings.response.raw }).getAttribute("aria-pressed")).toBe("true");
  });

  it("says how much of a truncated body is shown", async () => {
    const total = 5 * 1024 * 1024;
    await openRequest({
      onSend: async () => answered({ decoded_size: total, body: { id: "b1", total, truncated: true, json: false } }),
      bodies: { b1: utf8.encode("beginning") },
    });

    await clickSend();

    expect(await panel().findByText(strings.response.truncated(formatBytes(total)))).toBeTruthy();
    expect(strings.response.truncated(formatBytes(total))).toContain("5,0 МБ");
    await waitFor(() => expect(bodyText().value).toBe("beginning"));
  });

  it("shows no truncation note for a whole body", async () => {
    await openRequest({ onSend: async () => answered() });
    await clickSend();
    await waitFor(() => expect(bodyText().value).toBe('{"a":1}'));

    expect(panel().queryByText(strings.response.truncated(formatBytes(2048)))).toBeNull();
    expect(panel().queryByText(strings.response.decodeError)).toBeNull();
  });

  it("shows bytes that are not UTF-8 without failing", async () => {
    await openRequest({
      onSend: async () => answered({ body: { id: "b1", total: 6, truncated: false, json: false } }),
      bodies: { b1: new Uint8Array([0xff, 0xfe, 0x00, 0x41, 0xc3, 0x28]) },
    });

    await clickSend();

    await waitFor(() => expect(bodyText().value).toContain("A"));
    expect(bodyText().value).toContain("�");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(panel().getByText("200 OK")).toBeTruthy();
  });

  it("says that the response could not be unpacked", async () => {
    await openRequest({
      onSend: async () => answered({ decode_error: "corrupt gzip stream", body: { id: "b1", total: 3, truncated: false, json: false } }),
      bodies: { b1: new Uint8Array([0x1f, 0x8b, 0x08]) },
    });

    await clickSend();

    expect(await panel().findByText(strings.response.decodeError)).toBeTruthy();
  });

  it("says that a response has no body and asks for none", async () => {
    const api = await openRequest({
      onSend: async () =>
        answered({ status: 204, status_text: "No Content", decoded_size: 0, wire_size: 0, body: { id: "b1", total: 0, truncated: false, json: false } }),
    });
    const responseBody = vi.spyOn(api, "responseBody");

    await clickSend();

    expect(await panel().findByText(strings.response.noBody)).toBeTruthy();
    expect(panel().queryByRole("textbox", { name: strings.response.bodyText })).toBeNull();
    expect(responseBody).not.toHaveBeenCalled();
  });

  it("lists the headers and the cookies with their counts", async () => {
    await openRequest({
      onSend: async () =>
        answered({
          cookies: [{ name: "sid", value: "s3cr3t", domain: "api.test", path: "/", expires_unix: null, secure: true, http_only: false }],
        }),
    });
    await clickSend();
    await panel().findByText("200 OK");

    const headersTab = panel().getByRole("tab", { name: /^Headers/ });
    const cookiesTab = panel().getByRole("tab", { name: /^Cookies/ });
    expect(panel().getByRole("tab", { name: /^Body/ }).getAttribute("aria-selected")).toBe("true");
    expect(headersTab.textContent).toMatch(/Headers\s*2/);
    expect(cookiesTab.textContent).toMatch(/Cookies\s*1/);

    await fireEvent.click(headersTab);
    const headerRows = within(panel().getByRole("table", { name: strings.response.headers })).getAllByRole("row").slice(1);
    expect(headerRows.map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent!.trim()))).toEqual([
      ["content-type", "application/json"],
      ["x-request-id", "abc-123"],
    ]);

    await fireEvent.click(cookiesTab);
    const cookieRows = within(panel().getByRole("table", { name: strings.response.cookies })).getAllByRole("row").slice(1);
    expect(cookieRows).toHaveLength(1);
    expect(cookieRows[0].textContent).toContain("sid");
    expect(cookieRows[0].textContent).toContain("s3cr3t");
    expect(cookieRows[0].textContent).toContain("api.test");
    expect(cookieRows[0].textContent).toContain(strings.response.session);
  });

  it("shows no count for a response without cookies", async () => {
    await openRequest({ onSend: async () => answered() });
    await clickSend();
    await panel().findByText("200 OK");

    const cookiesTab = panel().getByRole("tab", { name: /^Cookies/ });
    expect(cookiesTab.textContent!.trim()).toBe("Cookies");
    await fireEvent.click(cookiesTab);
    expect(panel().getByText(strings.response.noCookies)).toBeTruthy();
  });

  it("counts the redirects and lists them on a click", async () => {
    await openRequest({
      onSend: async () =>
        answered({
          redirects: [
            { status: 301, method: "GET", url: "http://api.test/users", location: "https://api.test/users" },
            { status: 302, method: "POST", url: "https://api.test/users", location: "/v2/users" },
          ],
        }),
    });
    await clickSend();

    const toggle = await panel().findByRole("button", { name: strings.response.redirects(2) });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(panel().queryByText("/v2/users")).toBeNull();

    await fireEvent.click(toggle);

    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    const hops = within(panel().getByRole("list", { name: strings.response.redirectList })).getAllByRole("listitem");
    expect(hops).toHaveLength(2);
    expect(hops[0].textContent).toContain("301");
    expect(hops[0].textContent).toContain("http://api.test/users");
    // The method of a hop has the colour of the method, as everywhere.
    const methods = hops.map((hop) => hop.querySelector(".method")!);
    expect(methods.map((method) => method.textContent)).toEqual(["GET", "POST"]);
    expect(methods[0].classList.contains("method-get")).toBe(true);
    expect(methods[1].classList.contains("method-post")).toBe(true);
    expect(hops[1].textContent).toContain("/v2/users");
  });

  it("does not mention redirects when there were none", async () => {
    await openRequest({ onSend: async () => answered() });
    await clickSend();
    await panel().findByText("200 OK");

    expect(panel().queryByRole("button", { name: strings.response.redirects(0) })).toBeNull();
  });

  it("shows a cancelled send as a plain message, not as an error", async () => {
    await openRequest({ onSend: async () => failed({ kind: "cancelled" }) });

    await clickSend();

    const text = await panel().findByText(strings.response.errors.cancelled);
    expect(text.classList.contains("error")).toBe(false);
  });

  it("shows a connection error with the message of the core", async () => {
    await openRequest({ onSend: async () => failed({ kind: "connect", message: "tcp connect error: refused (os error 10061)" }) });

    await clickSend();

    const text = await panel().findByText(strings.response.errors.connect);
    expect(text.classList.contains("error")).toBe(true);
    expect(panel().getByText("tcp connect error: refused (os error 10061)")).toBeTruthy();
    // Not a failure of the command: no notification.
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it.each([
    ["timeout", strings.response.errors.timeout],
    ["tls", strings.response.errors.tls],
    ["invalid_url", strings.response.errors.invalid_url],
    ["too_many_redirects", strings.response.errors.too_many_redirects],
    ["io", strings.response.errors.io],
    ["other", strings.response.errors.other],
  ] as const)("names the error %s", async (kind, text) => {
    await openRequest({ onSend: async () => failed({ kind, message: "details" }) });

    await clickSend();

    expect((await panel().findByText(text)).classList.contains("error")).toBe(true);
  });

  it("names the form fields whose files have to be chosen again", async () => {
    await openRequest({ onSend: async () => failed({ kind: "unconfirmed_files", message: "files", fields: ["avatar", "cv"] }) });

    await clickSend();

    expect(await panel().findByText(`${strings.response.errors.unconfirmed_files}: avatar, cv`)).toBeTruthy();
  });

  it("shows the scripts warning above a response and above an error", async () => {
    const scripts: Warning[] = [{ kind: "scripts_not_allowed", items: ["Demo API"] }];
    const text = strings.response.warnings.scripts_not_allowed(["Demo API"]);
    expect(text).toContain("Demo API");
    const api = await openRequest({ onSend: async () => answered({}, scripts) });

    await clickSend();
    await panel().findByText("200 OK");
    expect(panel().getByText(text)).toBeTruthy();

    api.onSend = async () => failed({ kind: "connect", message: "refused" }, scripts);
    await clickSend();
    await panel().findByText(strings.response.errors.connect);
    expect(panel().getByText(text)).toBeTruthy();
  });

  it("shows the other warnings of a send", async () => {
    await openRequest({
      onSend: async () =>
        answered({}, [
          { kind: "unresolved_variables", items: ["host", "token"] },
          { kind: "body_pruned", items: ["GET"] },
          { kind: "unsupported_auth", items: ["digest"] },
        ]),
    });

    await clickSend();
    await panel().findByText("200 OK");

    expect(panel().getByText(strings.response.warnings.unresolved_variables(["host", "token"]))).toBeTruthy();
    expect(panel().getByText(strings.response.warnings.body_pruned(["GET"]))).toBeTruthy();
    expect(panel().getByText(strings.response.warnings.unsupported_auth(["digest"]))).toBeTruthy();
  });

  it("replaces the response with the one of the next send and releases the previous body", async () => {
    const api = await openRequest({
      onSend: async () => answered(),
      bodies: { b1: utf8.encode('{"a":1}'), b2: utf8.encode('{"b":2}') },
    });
    await clickSend();
    await waitFor(() => expect(bodyText().value).toBe('{"a":1}'));

    api.onSend = async () => answered({ status: 201, status_text: "Created", body: { id: "b2", total: 7, truncated: false, json: true } });
    await clickSend();

    expect(await panel().findByText("201 Created")).toBeTruthy();
    await waitFor(() => expect(bodyText().value).toBe('{"b":2}'));
    expect(api.released).toEqual(["b1"]);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows a notification when the command itself fails", async () => {
    await openRequest({ onSend: () => Promise.reject("request not found") });

    await clickSend();

    expect((await screen.findByRole("alert")).textContent).toContain("request not found");
    expect(sendButton().disabled).toBe(false);
    expect(panel().getByText(strings.response.empty)).toBeTruthy();
  });

  it("does not send a request without an address", async () => {
    const onSend = vi.fn<SendHandler>(async () => answered());
    await openRequest({ url: "", onSend });

    expect(sendButton().disabled).toBe(true);
    const event = await press(CTRL_ENTER);

    expect(event.defaultPrevented).toBe(true);
    expect(onSend).not.toHaveBeenCalled();

    await fireEvent.input(screen.getByRole("textbox", { name: strings.editor.url }), { target: { value: "https://api.test" } });
    expect(sendButton().disabled).toBe(false);
  });

  describe("copying the body", () => {
    /** Puts a clipboard that answers with `writeText` in place of the one jsdom lacks. */
    function stubClipboard(writeText: (text: string) => Promise<void>) {
      const spy = vi.fn(writeText);
      Object.defineProperty(navigator, "clipboard", { value: { writeText: spy }, configurable: true });
      return spy;
    }

    function copyButton(): HTMLElement {
      return panel().getByRole("button", { name: strings.response.copy });
    }

    /** Sends the request and waits until the body of the answer is on the screen. */
    async function sendAndShow(text: string): Promise<void> {
      await clickSend();
      await waitFor(() => expect(bodyText().value).toBe(text));
    }

    afterEach(() => {
      Reflect.deleteProperty(navigator, "clipboard");
    });

    it("has an icon button with a name and the same tooltip", async () => {
      await openRequest({ onSend: async () => answered() });
      await sendAndShow('{"a":1}');

      expect(copyButton().title).toBe(strings.response.copy);
      expect(copyButton().textContent!.trim()).toBe("");
      expect(copyButton().querySelector("svg")).not.toBeNull();
    });

    it("copies the body in the shown view and says so", async () => {
      const api = await openRequest({ onSend: async () => answered() });
      const copyResponseBody = vi
        .spyOn(api, "copyResponseBody")
        .mockImplementation(async (_id, pretty) => (pretty ? '{\n  "a": 1\n}' : '{"a":1}'));
      const writeText = stubClipboard(async () => {});
      await sendAndShow('{"a":1}');

      // A JSON body is shown as Pretty first.
      await fireEvent.click(copyButton());
      await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
      expect(copyResponseBody).toHaveBeenLastCalledWith("b1", true);
      expect(writeText).toHaveBeenLastCalledWith('{\n  "a": 1\n}');
      expect((await screen.findByRole("status")).textContent).toContain(strings.response.copied);
      expect(screen.queryByRole("alert")).toBeNull();

      await fireEvent.click(panel().getByRole("button", { name: strings.response.raw }));
      await fireEvent.click(copyButton());
      await waitFor(() => expect(writeText).toHaveBeenCalledTimes(2));
      expect(copyResponseBody).toHaveBeenLastCalledWith("b1", false);
      expect(writeText).toHaveBeenLastCalledWith('{"a":1}');
    });

    it("copies the text the core gives for the stored body", async () => {
      await openRequest({
        onSend: async () => answered({ body: { id: "b1", total: 5, truncated: false, json: false } }),
        bodies: { b1: utf8.encode("hello") },
      });
      const writeText = stubClipboard(async () => {});
      await sendAndShow("hello");

      await fireEvent.click(copyButton());

      await waitFor(() => expect(writeText).toHaveBeenCalledWith("hello"));
    });

    it("shows an error when the clipboard refuses", async () => {
      await openRequest({ onSend: async () => answered() });
      const writeText = stubClipboard(() => Promise.reject(new Error("NotAllowedError")));
      await sendAndShow('{"a":1}');

      await fireEvent.click(copyButton());

      expect((await screen.findByRole("alert")).textContent).toContain(strings.response.copyFailed);
      expect(writeText).toHaveBeenCalledTimes(1);
      expect(toasts.items.some((toast) => toast.text === strings.response.copied)).toBe(false);
    });

    it("shows an error when there is no clipboard at all", async () => {
      await openRequest({ onSend: async () => answered() });
      await sendAndShow('{"a":1}');

      await fireEvent.click(copyButton());

      expect((await screen.findByRole("alert")).textContent).toContain(strings.response.copyFailed);
    });

    it("shows the message of the command when the body cannot be read", async () => {
      const api = await openRequest({ onSend: async () => answered() });
      vi.spyOn(api, "copyResponseBody").mockRejectedValue("response body not found: b1");
      const writeText = stubClipboard(async () => {});
      await sendAndShow('{"a":1}');

      await fireEvent.click(copyButton());

      expect((await screen.findByRole("alert")).textContent).toContain("response body not found: b1");
      expect(writeText).not.toHaveBeenCalled();
    });

    it("offers nothing to copy for a response without a body", async () => {
      const empty = { id: "b1", total: 0, truncated: false, json: false };
      await openRequest({ onSend: async () => answered({ status: 204, status_text: "No Content", decoded_size: 0, body: empty }) });
      await clickSend();
      await panel().findByText("204 No Content");

      expect(panel().queryByRole("button", { name: strings.response.copy })).toBeNull();
    });
  });

  describe("saving the body to a file", () => {
    const PATH = "C:\\out\\response.json";

    function saveButtons(): HTMLElement[] {
      return panel().queryAllByRole("button", { name: strings.response.save });
    }

    /** Lets the commands that were started end and their results be handled. */
    async function settle() {
      for (let turn = 0; turn < 10; turn++) await Promise.resolve();
    }

    it("has an icon button next to Copy that saves the body by its id and says where the file is", async () => {
      const api = await openRequest({ onSend: async () => answered() });
      const save = vi.spyOn(api, "saveResponseBody").mockResolvedValue(PATH);
      await clickSend();
      await panel().findByText("200 OK");

      expect(saveButtons()).toHaveLength(1);
      const [button] = saveButtons();
      expect(button.title).toBe(strings.response.save);
      expect(button.textContent!.trim()).toBe("");
      expect(button.querySelector("svg")).not.toBeNull();

      await fireEvent.click(button);

      await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
      expect(save).toHaveBeenCalledWith("b1");
      expect((await screen.findByRole("status")).textContent).toContain(`Сохранено: ${PATH}`);
      expect(screen.queryByRole("alert")).toBeNull();
    });

    it("says nothing when the window that asks where to save was closed", async () => {
      const api = await openRequest({ onSend: async () => answered() });
      const save = vi.spyOn(api, "saveResponseBody").mockResolvedValue(null);
      await clickSend();
      await panel().findByText("200 OK");

      await fireEvent.click(saveButtons()[0]);
      await waitFor(() => expect(save).toHaveBeenCalledWith("b1"));
      await settle();

      expect(toasts.items).toEqual([]);
    });

    it("shows the message of the command when the file cannot be written", async () => {
      const api = await openRequest({ onSend: async () => answered() });
      vi.spyOn(api, "saveResponseBody").mockRejectedValue("access denied: C:\\out");
      await clickSend();
      await panel().findByText("200 OK");

      await fireEvent.click(saveButtons()[0]);

      expect((await screen.findByRole("alert")).textContent).toContain("access denied: C:\\out");
      expect(toasts.items.some((toast) => toast.text.startsWith("Сохранено"))).toBe(false);
    });

    it("offers nothing to save for a response without a body", async () => {
      const empty = { id: "b1", total: 0, truncated: false, json: false };
      await openRequest({ onSend: async () => answered({ status: 204, status_text: "No Content", decoded_size: 0, body: empty }) });
      await clickSend();
      await panel().findByText("204 No Content");

      expect(saveButtons()).toHaveLength(0);
    });

    it("has a text button in the note of a truncated body as well", async () => {
      const total = 5 * 1024 * 1024;
      const api = await openRequest({
        onSend: async () => answered({ decoded_size: total, body: { id: "b1", total, truncated: true, json: false } }),
        bodies: { b1: utf8.encode("beginning") },
      });
      const save = vi.spyOn(api, "saveResponseBody").mockResolvedValue(PATH);
      await clickSend();
      const note = await panel().findByText(strings.response.truncated(formatBytes(total)));

      expect(saveButtons()).toHaveLength(2);
      const inNote = within(note.closest(".strip") as HTMLElement).getByRole("button", { name: strings.response.save });
      expect(inNote.textContent!.trim()).toBe(strings.response.save);

      await fireEvent.click(inNote);

      await waitFor(() => expect(save).toHaveBeenCalledWith("b1"));
      expect((await screen.findByRole("status")).textContent).toContain(`Сохранено: ${PATH}`);
    });

    it("has no button in the body of a response that is shown whole", async () => {
      await openRequest({ onSend: async () => answered() });
      await clickSend();
      await waitFor(() => expect(bodyText().value).toBe('{"a":1}'));

      expect(saveButtons()).toHaveLength(1);
    });
  });

  describe("search in the body", () => {
    it("a new value of findSignal shows Body instead of Headers and opens the search in its editor", async () => {
      const api = createFakeApi({ bodies: { b1: utf8.encode('{"a":1}') } });
      const sending = new Sending();
      sending.result = answered();
      const { rerender } = render(ResponsePanel, { props: { sending }, context: new Map([["api", api]]) });
      const editor = () => document.querySelector<HTMLTextAreaElement>("textarea")!;
      await waitFor(() => expect(editor()?.value).toBe('{"a":1}'));
      await fireEvent.click(screen.getByRole("tab", { name: /Headers/ }));
      expect(screen.getByRole("tab", { name: /Headers/ }).getAttribute("aria-selected")).toBe("true");
      expect(editor().hasAttribute("data-search-open")).toBe(false);

      await rerender({ findSignal: 1 });

      await waitFor(() => expect(editor().hasAttribute("data-search-open")).toBe(true));
      expect(screen.getByRole("tab", { name: /Body/ }).getAttribute("aria-selected")).toBe("true");
      expect(screen.getByRole("tab", { name: /Headers/ }).getAttribute("aria-selected")).toBe("false");
      expect(editor().closest("[role=tabpanel]")!.hasAttribute("hidden")).toBe(false);
    });

    it("does nothing for a response without a body", async () => {
      const api = createFakeApi();
      const sending = new Sending();
      sending.result = answered({ status: 204, decoded_size: 0, body: { id: "b1", total: 0, truncated: false, json: false } });
      const { rerender } = render(ResponsePanel, { props: { sending }, context: new Map([["api", api]]) });
      await fireEvent.click(screen.getByRole("tab", { name: /Headers/ }));

      await rerender({ findSignal: 1 });
      for (let turn = 0; turn < 10; turn++) await Promise.resolve();

      expect(screen.getByRole("tab", { name: /Headers/ }).getAttribute("aria-selected")).toBe("true");
    });
  });

  it("releases the body of the response when its tab is closed", async () => {
    const api = await openRequest({ onSend: async () => answered() });
    await clickSend();
    await panel().findByText("200 OK");

    await press({ key: "w", code: "KeyW", ctrlKey: true });

    expect(api.released).toEqual(["b1"]);
    expect(screen.queryByText("200 OK")).toBeNull();
  });
});
