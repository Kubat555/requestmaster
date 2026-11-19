import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import ResponsePanel from "./ResponsePanel.svelte";
import Sidebar from "./Sidebar.svelte";
import { icons } from "../lib/icons";
import { motion } from "../lib/motion.svelte";
import { Sending } from "../lib/sending.svelte";
import { strings } from "../lib/strings";
import { tabs } from "../lib/tabs.svelte";
import { createFakeApi, noScriptResults, type SendHandler } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import { normalizeContent, type MotionLevel, type RequestDraft, type ResponseDto, type SendResult } from "../lib/types";
import { Workspace } from "../lib/workspace.svelte";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const utf8 = new TextEncoder();

/** A `200 OK` response whose body is stored under `id`. */
function answered(id = "b1"): SendResult {
  const response: ResponseDto = {
    status: 200,
    status_text: "OK",
    http_version: "HTTP/1.1",
    headers: [["content-type", "application/json"]],
    cookies: [],
    redirects: [],
    ttfb_ms: 40,
    total_ms: 120,
    wire_size: 7,
    decoded_size: 7,
    decode_error: null,
    sent: { method: "GET", url: "https://api.example.test/users", headers: [] },
    body: { id, total: 7, truncated: false, json: true },
  };
  return { response, error: null, warnings: [], ...noScriptResults() };
}

/** Renders the window at the motion level and opens the request "List users" of the default tree in a tab. */
async function openRequest(level: MotionLevel, onSend?: SendHandler) {
  motion.choose(level);
  const api = createFakeApi({ onSend, bodies: { b1: utf8.encode('{"a":1}'), b2: utf8.encode('{"a":2}') } });
  render(App, { context: new Map([["api", api]]) });
  await screen.findByRole("treeitem", { name: /Demo API/ });
  await tabs.open(api, "req-list-users");
  await screen.findByRole("tab", { name: /List users/ });
  return api;
}

/** The button that sends the request of the shown tab, and stops the send while one is running. */
function sendButton(): HTMLButtonElement {
  return document.querySelector<HTMLButtonElement>("[data-tab-panel]:not([hidden]) button.send")!;
}

/** The response part of the window. */
function responsePanel(): HTMLElement {
  return screen.getByRole("region", { name: strings.response.label }).querySelector<HTMLElement>(".response-panel")!;
}

// The tabs, the notifications and the motion level of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
  motion.choose("normal");
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("the Send button at the rich motion level", () => {
  /** The rings on the button. */
  function ripples(): HTMLElement[] {
    return [...sendButton().querySelectorAll<HTMLElement>(".ripple")];
  }

  function placeButton() {
    sendButton().getBoundingClientRect = () => new DOMRect(200, 50, 90, 32);
  }

  it("spreads a ring from where the pointer pressed, and the click sends as always", async () => {
    const onSend = vi.fn<SendHandler>(async () => answered());
    await openRequest("rich", onSend);
    placeButton();

    await fireEvent.pointerDown(sendButton(), { clientX: 230, clientY: 60 });

    expect(ripples()).toHaveLength(1);
    expect(ripples()[0].style.getPropertyValue("--ripple-x")).toBe("30px");
    expect(ripples()[0].style.getPropertyValue("--ripple-y")).toBe("10px");
    expect(ripples()[0].getAttribute("aria-hidden")).toBe("true");
    // The ring is only shown: the press alone sends nothing.
    expect(onSend).not.toHaveBeenCalled();

    // The click of that press: no second ring.
    await fireEvent.click(sendButton(), { detail: 1 });
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(ripples()).toHaveLength(1);
  });

  it("spreads the ring of a press of the keyboard from the middle", async () => {
    const onSend = vi.fn<SendHandler>(async () => answered());
    await openRequest("rich", onSend);

    // A click made by Enter or Space comes without a press of the pointer and without a count of clicks.
    await fireEvent.click(sendButton(), { detail: 0 });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(ripples()).toHaveLength(1);
    expect(ripples()[0].classList.contains("centred")).toBe(true);
    expect(ripples()[0].style.getPropertyValue("--ripple-x")).toBe("");
  });

  it("takes every ring away when its animation is over or cut short", async () => {
    await openRequest("rich");
    placeButton();
    await fireEvent.pointerDown(sendButton(), { clientX: 210, clientY: 60 });
    await fireEvent.pointerDown(sendButton(), { clientX: 250, clientY: 60 });
    const [first, second] = ripples();
    expect(second.style.getPropertyValue("--ripple-x")).toBe("50px");

    await fireEvent.animationEnd(first);
    expect(ripples()).toEqual([second]);

    await fireEvent(second, new Event("animationcancel"));
    expect(ripples()).toEqual([]);
  });

  it("does not answer a press while it cannot send", async () => {
    await openRequest("rich");
    await fireEvent.input(screen.getByRole("textbox", { name: strings.editor.url }), { target: { value: "" } });
    expect(sendButton().disabled).toBe(true);

    await fireEvent.pointerDown(sendButton(), { clientX: 230, clientY: 60 });

    expect(ripples()).toEqual([]);
  });

  it.each(["normal", "off"] as const)("has no rings at the level %j", async (level) => {
    const onSend = vi.fn<SendHandler>(async () => answered());
    await openRequest(level, onSend);

    await fireEvent.pointerDown(sendButton(), { clientX: 230, clientY: 60 });
    await fireEvent.click(sendButton(), { detail: 0 });

    expect(ripples()).toEqual([]);
    expect(onSend).toHaveBeenCalledTimes(1);
  });
});

describe("the arrival of a response at the rich motion level", () => {
  it("lets every new response rise into the panel once", async () => {
    const onSend = vi.fn<SendHandler>(async () => answered("b1"));
    await openRequest("rich", onSend);

    await fireEvent.click(sendButton());
    await within(responsePanel()).findByText("200 OK");
    expect(responsePanel().classList.contains("arrived")).toBe(true);

    // An animation of something inside the panel does not end it; its own does.
    await fireEvent.animationEnd(within(responsePanel()).getByText("200 OK"));
    expect(responsePanel().classList.contains("arrived")).toBe(true);
    await fireEvent.animationEnd(responsePanel());
    expect(responsePanel().classList.contains("arrived")).toBe(false);

    onSend.mockImplementation(async () => answered("b2"));
    await fireEvent.click(sendButton());
    await waitFor(() => expect(responsePanel().classList.contains("arrived")).toBe(true));
  });

  it.each(["normal", "off"] as const)("shows the response at once at the level %j", async (level) => {
    await openRequest(level, async () => answered());

    await fireEvent.click(sendButton());
    await within(responsePanel()).findByText("200 OK");

    expect(responsePanel().classList.contains("arrived")).toBe(false);
  });

  describe("the waiting indicator", () => {
    /** The panel of a send that stays unanswered, 150 ms after its start: the indicator is on the screen. */
    function waitingIndicator(level: MotionLevel): HTMLElement {
      motion.choose(level);
      vi.useFakeTimers();
      const api = createFakeApi({ onSend: () => new Promise<SendResult>(() => {}) });
      const sending = new Sending();
      render(ResponsePanel, { props: { sending }, context: new Map([["api", api]]) });
      const draft: RequestDraft = normalizeContent("request", { method: "GET", url: "https://api.test/users" });
      void sending.send(api, { nodeId: "req", draft, sending });
      flushSync();
      vi.advanceTimersByTime(150);
      flushSync();
      return screen.getByRole("status", { name: strings.response.sending });
    }

    it("is the two arrows of the application instead of the ring; the line and the time stay", () => {
      const indicator = waitingIndicator("rich");

      const arrows = indicator.querySelector("svg.exchange")!;
      expect([...arrows.querySelectorAll("path")].map((path) => path.getAttribute("d"))).toEqual([...icons.send]);
      expect(arrows.getAttribute("aria-hidden")).toBe("true");
      expect(indicator.querySelector(".spinner")).toBeNull();
      expect(indicator.querySelector(".progress")).not.toBeNull();
      expect(indicator.querySelector(".elapsed")!.textContent).toBe(`0,1 ${strings.units.seconds}`);
      expect([...indicator.children].every((part) => part.getAttribute("aria-hidden") === "true")).toBe(true);
      expect(indicator.querySelector("[style]")).toBeNull();
    });

    it.each(["normal", "off"] as const)("is the ring at the level %j", (level) => {
      const indicator = waitingIndicator(level);

      expect(indicator.querySelector(".spinner.lg")).not.toBeNull();
      expect(indicator.querySelector("svg.exchange")).toBeNull();
    });
  });
});

describe("the copy button of a response at the rich motion level", () => {
  function copyButton(): HTMLElement {
    return within(responsePanel()).getByRole("button", { name: strings.response.copy });
  }

  /** The paths of the icon of the button. */
  function icon(): string[] {
    return [...copyButton().querySelectorAll("path")].map((path) => path.getAttribute("d") ?? "");
  }

  async function sendAndWait(level: MotionLevel, writeText: (text: string) => Promise<void>) {
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    await openRequest(level, async () => answered());
    await fireEvent.click(sendButton());
    await within(responsePanel()).findByText("200 OK");
  }

  afterEach(() => {
    Reflect.deleteProperty(navigator, "clipboard");
  });

  it("turns into a check mark for a second after a copy, and keeps its name", async () => {
    await sendAndWait("rich", async () => {});
    expect(icon()).toEqual([...icons.copy]);
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });

    await fireEvent.click(copyButton());
    // The copy goes through the core and the clipboard; the clock of the test stands still meanwhile.
    for (let turn = 0; turn < 20; turn++) await Promise.resolve();
    flushSync();
    expect(icon()).toEqual([...icons.check]);
    expect(copyButton().classList.contains("copied")).toBe(true);
    expect(copyButton().getAttribute("aria-label")).toBe(strings.response.copy);

    vi.advanceTimersByTime(999);
    flushSync();
    expect(icon()).toEqual([...icons.check]);
    vi.advanceTimersByTime(1);
    flushSync();
    expect(icon()).toEqual([...icons.copy]);
    expect(copyButton().classList.contains("copied")).toBe(false);
  });

  it("stays a copy button when the clipboard refuses", async () => {
    await sendAndWait("rich", () => Promise.reject(new Error("denied")));

    await fireEvent.click(copyButton());
    await screen.findByText(strings.response.copyFailed);

    expect(icon()).toEqual([...icons.copy]);
    expect(copyButton().classList.contains("copied")).toBe(false);
  });

  it("stays a copy button at the normal level: the notification tells about the copy", async () => {
    await sendAndWait("normal", async () => {});

    await fireEvent.click(copyButton());
    await screen.findByText(strings.response.copied);

    expect(icon()).toEqual([...icons.copy]);
    expect(copyButton().classList.contains("copied")).toBe(false);
  });
});

describe("the dialog of the settings at the rich motion level", () => {
  async function openSettings(level: MotionLevel) {
    motion.choose(level);
    render(App, { context: new Map([["api", createFakeApi()]]) });
    await fireEvent.click(screen.getByRole("button", { name: strings.settings.button }));
    return screen.findByRole<HTMLInputElement>("textbox", { name: strings.settings.timeout });
  }

  async function save() {
    await fireEvent.click(screen.getByRole("button", { name: strings.settings.save }));
  }

  it("shakes the field whose value was refused, once", async () => {
    const timeout = await openSettings("rich");
    const frame = timeout.closest<HTMLElement>("[data-pulse-frame]")!;

    await fireEvent.input(timeout, { target: { value: "abc" } });
    await save();

    await waitFor(() => expect(frame.classList.contains("shake")).toBe(true));
    await fireEvent.animationEnd(frame);
    expect(frame.classList.contains("shake")).toBe(false);
    expect(timeout.getAttribute("aria-invalid")).toBe("true");
  });

  it("does not shake a field at the normal level", async () => {
    const timeout = await openSettings("normal");

    await fireEvent.input(timeout, { target: { value: "abc" } });
    await save();
    await screen.findByText(strings.settings.timeoutInvalid);

    expect(document.querySelector(".shake")).toBeNull();
  });
});

describe("the rows of tabs at the rich motion level", () => {
  it("gives every row of sections and every segmented choice one sliding mark", async () => {
    await openRequest("rich");

    for (const name of [strings.editor.sections, strings.sidebar.sections]) {
      const row = screen.getByRole("tablist", { name });
      await waitFor(() => expect(row.querySelectorAll(".slide-indicator")).toHaveLength(1));
      expect(row.querySelector(".slide-indicator")!.getAttribute("aria-hidden")).toBe("true");
    }
    // The tabs are counted as before: the mark is not one of them.
    expect(within(screen.getByRole("tablist", { name: strings.editor.sections })).getAllByRole("tab")).toHaveLength(6);
  });

  it("has no marks at the normal level", async () => {
    await openRequest("normal");

    expect(document.querySelector(".slide-indicator")).toBeNull();
  });
});

describe("the tree at the rich motion level", () => {
  /** Renders the sidebar over the default tree with only the collection expanded. */
  async function renderTree(level: MotionLevel) {
    motion.choose(level);
    const api = createFakeApi();
    const workspace = new Workspace();
    await workspace.load(api);
    workspace.expand(["col-1"]);
    const { container } = render(Sidebar, { props: { workspace }, context: new Map([["api", api]]) });
    const rowOf = (id: string) => container.querySelector<HTMLElement>(`[role="treeitem"][data-id="${id}"]`)!;
    const fresh = () => [...container.querySelectorAll<HTMLElement>('[role="treeitem"].fresh')].map((row) => row.dataset.id);
    return { workspace, container, rowOf, fresh };
  }

  it("fades in the rows of a node that is expanded, and only them", async () => {
    const { workspace, rowOf, fresh } = await renderTree("rich");
    // The rows the tree starts with do not fade.
    expect(fresh()).toEqual([]);

    workspace.toggle("fold-users");
    await waitFor(() => expect(fresh()).toEqual(["req-list-users", "req-create-user"]));

    // Their fade is over: the mark is gone, so that scrolling them back into view does not play it again.
    await fireEvent.animationEnd(rowOf("req-list-users"));
    expect(fresh()).toEqual([]);

    // Collapsing takes the rows away at once, and nothing fades.
    workspace.toggle("fold-users");
    await waitFor(() => expect(rowOf("req-list-users")).toBeNull());
    expect(fresh()).toEqual([]);
  });

  it("does not fade the rows a filter brings", async () => {
    const { workspace, fresh, rowOf } = await renderTree("rich");

    workspace.filter = "order";
    await waitFor(() => expect(rowOf("req-get-order")).not.toBeNull());
    expect(fresh()).toEqual([]);

    workspace.filter = "";
    await waitFor(() => expect(rowOf("req-get-order")).toBeNull());
    expect(fresh()).toEqual([]);
  });

  it("fades nothing at the normal level", async () => {
    const { workspace, rowOf, fresh } = await renderTree("normal");

    workspace.toggle("fold-users");
    await waitFor(() => expect(rowOf("req-list-users")).not.toBeNull());

    expect(fresh()).toEqual([]);
  });

  it("marks the row that is being dragged, until the drag is over", async () => {
    const { rowOf, container } = await renderTree("rich");
    const dataTransfer = { types: [] as string[], effectAllowed: "", setData: vi.fn(), getData: vi.fn() };

    await fireEvent(rowOf("fold-users"), createEvent.dragStart(rowOf("fold-users"), { dataTransfer }));
    expect(rowOf("fold-users").classList.contains("dragging")).toBe(true);
    expect(container.querySelectorAll(".dragging")).toHaveLength(1);

    await fireEvent(rowOf("fold-users"), createEvent.dragEnd(rowOf("fold-users")));
    expect(container.querySelector(".dragging")).toBeNull();
  });
});

describe("the notifications at the rich motion level", () => {
  function region(): HTMLElement {
    return screen.getByRole("region", { name: strings.toasts.label });
  }

  /** The notifications a screen reader is told about. */
  function live(): string[] {
    return within(region())
      .queryAllByRole("status")
      .map((toast) => toast.textContent?.trim() ?? "");
  }

  function leaving(): HTMLElement[] {
    return [...region().querySelectorAll<HTMLElement>(".toast.leaving")];
  }

  it("lets a notification whose time is over fade away as a picture nobody can reach", async () => {
    motion.choose("rich");
    render(App, { context: new Map([["api", createFakeApi()]]) });
    toasts.info("first");
    toasts.info("second");
    await waitFor(() => expect(live()).toEqual(["first", "second"]));

    // The time of the first one is over: it leaves the list at once.
    toasts.items = toasts.items.slice(1);
    await waitFor(() => expect(live()).toEqual(["second"]));

    expect(leaving().map((toast) => toast.textContent?.trim())).toEqual(["first"]);
    const [ghost] = leaving();
    expect(ghost.getAttribute("aria-hidden")).toBe("true");
    expect(ghost.hasAttribute("inert")).toBe(true);
    expect(ghost.hasAttribute("role")).toBe(false);
    expect(ghost.matches("button, a, input, [tabindex]") || ghost.querySelector("button, a, input, [tabindex]")).toBeFalsy();
    // It keeps its place above the one that stays.
    expect([...region().children].map((toast) => toast.textContent?.trim())).toEqual(["first", "second"]);

    await fireEvent.animationEnd(ghost);
    expect(leaving()).toEqual([]);
    expect([...region().children].map((toast) => toast.textContent?.trim())).toEqual(["second"]);
  });

  it.each(["normal", "off"] as const)("takes a notification away at once at the level %j", async (level) => {
    motion.choose(level);
    render(App, { context: new Map([["api", createFakeApi()]]) });
    toasts.info("first");
    await waitFor(() => expect(live()).toEqual(["first"]));

    toasts.items = [];
    await waitFor(() => expect(live()).toEqual([]));

    expect(region().children).toHaveLength(0);
  });
});
