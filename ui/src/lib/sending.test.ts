import { afterEach, describe, expect, it, vi } from "vitest";
import { observeSends, type SendObserver } from "./sending.svelte";
import { Tab, Tabs } from "./tabs.svelte";
import { toasts } from "./toasts.svelte";
import { createFakeApi, noScriptResults, type FakeApi } from "./testing/fakeApi";
import { normalizeContent, type RequestContent, type RequestDraft, type ResponseDto, type SendResult } from "./types";

/** A response whose body is stored under `bodyId`. */
function response(bodyId: string): ResponseDto {
  return {
    status: 200,
    status_text: "OK",
    http_version: "HTTP/1.1",
    headers: [],
    cookies: [],
    redirects: [],
    ttfb_ms: 10,
    total_ms: 20,
    wire_size: 2,
    decoded_size: 2,
    decode_error: null,
    sent: { method: "GET", url: "https://a.test", headers: [] },
    body: { id: bodyId, total: 2, truncated: false, json: true },
  };
}

function answered(bodyId: string): SendResult {
  return { response: response(bodyId), error: null, warnings: [], ...noScriptResults() };
}

/** One call of `sendRequest` that the test answers when it wants. */
interface Call {
  sendId: string;
  requestId: string;
  draft: RequestContent | null;
  resolve(result: SendResult): void;
  reject(error: string): void;
}

/** A fake api whose sends stay unanswered until the test resolves them, and a request tab. */
function setup() {
  const api: FakeApi = createFakeApi();
  const calls: Call[] = [];
  api.onSend = (sendId, requestId, draft) =>
    new Promise<SendResult>((resolve, reject) => calls.push({ sendId, requestId, draft, resolve, reject }));
  const tab = new Tab<RequestDraft>(
    { id: "req", kind: "request", name: "A" },
    normalizeContent("request", { method: "GET", url: "https://a.test", query: [{ key: "page", value: "1" }] }),
  );
  return { api, calls, tab, sending: tab.sending };
}

afterEach(() => {
  toasts.items = [];
});

describe("Sending", () => {
  it("is idle and has no result before the first send", () => {
    const { sending } = setup();
    expect(sending.state).toBe("idle");
    expect(sending.result).toBeNull();
  });

  it("sends the draft, not the stored content", async () => {
    const { api, calls, tab, sending } = setup();
    tab.draft.url = "https://changed.test";
    tab.draft.method = "POST";

    const sent = sending.send(api, tab);

    expect(calls).toHaveLength(1);
    expect(calls[0].requestId).toBe("req");
    expect(calls[0].sendId).not.toBe("");
    expect(calls[0].draft).toEqual({
      method: "POST",
      url: "https://changed.test",
      query: [{ key: "page", value: "1", enabled: true, postman_extra: {} }],
      path_variables: [],
      headers: [],
      body: { mode: "none" },
      auth: null,
      scripts: { pre_request: "", test: "" },
      settings: { follow_redirects: null, max_redirects: null, strict_ssl: null, disable_body_pruning: null },
      disabled_auto_headers: [],
    });
    // The stored content is not touched and the tab stays changed.
    expect(tab.saved.url).toBe("https://a.test");
    expect(tab.dirty).toBe(true);

    calls[0].resolve(answered("b1"));
    await sent;
  });

  it("is sending while the request runs and idle with the result after it", async () => {
    const { api, calls, tab, sending } = setup();

    const sent = sending.send(api, tab);
    expect(sending.state).toBe("sending");
    expect(sending.result).toBeNull();

    const result = answered("b1");
    calls[0].resolve(result);
    await sent;

    expect(sending.state).toBe("idle");
    expect(sending.result).toEqual(result);
    expect(api.released).toEqual([]);
  });

  it("remembers when the running send started; a send that replaces it starts the count anew", () => {
    const { api, tab, sending } = setup();
    const now = vi.spyOn(performance, "now");

    now.mockReturnValue(1000);
    void sending.send(api, tab);
    expect(sending.startedAt).toBe(1000);

    now.mockReturnValue(1700);
    void sending.send(api, tab);
    expect(sending.startedAt).toBe(1700);

    now.mockRestore();
  });

  it("gives every send an id of its own", async () => {
    const { api, calls, tab, sending } = setup();
    const first = sending.send(api, tab);
    calls[0].resolve(answered("b1"));
    await first;
    const second = sending.send(api, tab);
    calls[1].resolve(answered("b2"));
    await second;

    expect(calls[0].sendId).not.toBe(calls[1].sendId);
  });

  it("cancel calls cancelSend with the id of the send and is idle with a cancelled result at once", () => {
    const { api, calls, tab, sending } = setup();
    void sending.send(api, tab);

    sending.cancel(api);

    expect(sending.state).toBe("idle");
    expect(sending.result?.response).toBeNull();
    expect(sending.result?.error?.kind).toBe("cancelled");
    expect(api.cancelled).toEqual([calls[0].sendId]);
  });

  it("the cancelled result has nothing of the scripts: the answer of the core is not waited for", () => {
    const { api, tab, sending } = setup();
    void sending.send(api, tab);

    sending.cancel(api);

    // Written out, not taken from a helper: these are the keys the panels of the window read.
    expect(sending.result).toEqual({
      response: null,
      error: { kind: "cancelled", message: "", fields: [] },
      warnings: [],
      tests: [],
      console: [],
      console_truncated: false,
      script_errors: [],
      variable_changes: [],
    });
  });

  it("drops a response that arrives after the cancel and releases its body", async () => {
    const { api, calls, tab, sending } = setup();
    const sent = sending.send(api, tab);
    sending.cancel(api);

    calls[0].resolve(answered("late"));
    await sent;

    expect(sending.state).toBe("idle");
    expect(sending.result?.error?.kind).toBe("cancelled");
    expect(sending.result?.response).toBeNull();
    expect(api.released).toEqual(["late"]);
  });

  it("keeps the cancelled result when the core reports the cancel later", async () => {
    const { api, calls, tab, sending } = setup();
    const sent = sending.send(api, tab);
    sending.cancel(api);
    const shown = sending.result;

    calls[0].resolve({ response: null, error: { kind: "cancelled", message: "cancelled", fields: [] }, warnings: [], ...noScriptResults() });
    await sent;

    expect(sending.result).toBe(shown);
    expect(api.released).toEqual([]);
  });

  it("cancel does nothing when no send is running", () => {
    const { api, sending } = setup();
    sending.cancel(api);
    expect(sending.result).toBeNull();
    expect(api.cancelled).toEqual([]);
  });

  it("of two sends in a row keeps the second result; the first is dropped and its body released", async () => {
    const { api, calls, tab, sending } = setup();
    const first = sending.send(api, tab);
    const second = sending.send(api, tab);
    expect(calls).toHaveLength(2);

    calls[0].resolve(answered("b1"));
    await first;
    // The first result is not the last send of the tab.
    expect(api.released).toEqual(["b1"]);
    expect(sending.result).toBeNull();
    expect(sending.state).toBe("sending");

    calls[1].resolve(answered("b2"));
    await second;
    expect(sending.state).toBe("idle");
    expect(sending.result?.response?.body.id).toBe("b2");
    expect(api.released).toEqual(["b1"]);
  });

  it("keeps the second result when the first one arrives after it", async () => {
    const { api, calls, tab, sending } = setup();
    const first = sending.send(api, tab);
    const second = sending.send(api, tab);

    calls[1].resolve(answered("b2"));
    await second;
    calls[0].resolve(answered("b1"));
    await first;

    expect(sending.state).toBe("idle");
    expect(sending.result?.response?.body.id).toBe("b2");
    expect(api.released).toEqual(["b1"]);
  });

  it("releases the body of the previous response when a new one arrives", async () => {
    const { api, calls, tab, sending } = setup();
    const first = sending.send(api, tab);
    calls[0].resolve(answered("b1"));
    await first;
    expect(api.released).toEqual([]);

    const second = sending.send(api, tab);
    // The previous response stays until the new one is here.
    expect(sending.result?.response?.body.id).toBe("b1");
    expect(api.released).toEqual([]);
    calls[1].resolve(answered("b2"));
    await second;

    expect(api.released).toEqual(["b1"]);
    expect(sending.result?.response?.body.id).toBe("b2");
  });

  it("releases the body of the previous response when the new result is an error", async () => {
    const { api, calls, tab, sending } = setup();
    const first = sending.send(api, tab);
    calls[0].resolve(answered("b1"));
    await first;

    const second = sending.send(api, tab);
    calls[1].resolve({ response: null, error: { kind: "connect", message: "refused", fields: [] }, warnings: [], ...noScriptResults() });
    await second;

    expect(api.released).toEqual(["b1"]);
    expect(sending.result?.error?.kind).toBe("connect");
  });

  it("is idle and shows a notification when the command itself fails", async () => {
    const { api, calls, tab, sending } = setup();
    const sent = sending.send(api, tab);

    calls[0].reject("request not found");
    await sent;

    expect(sending.state).toBe("idle");
    expect(sending.result).toBeNull();
    expect(toasts.items.map((toast) => [toast.kind, toast.text])).toEqual([["error", "request not found"]]);
  });

  it("releases the body of the response when the tab is closed", async () => {
    const api = createFakeApi({ onSend: async () => answered("b1") });
    const tabs = new Tabs();
    await tabs.open(api, "req-list-users");
    const tab = tabs.active as Tab<RequestDraft>;
    await tab.sending.send(api, tab);
    expect(api.released).toEqual([]);

    tabs.close(tab.id);

    expect(api.released).toEqual(["b1"]);
  });

  it("stops the send of a tab that is closed and releases the body of its late response", async () => {
    const { api, calls } = setup();
    const tabs = new Tabs();
    await tabs.open(api, "req-list-users");
    const tab = tabs.active as Tab<RequestDraft>;
    const sent = tab.sending.send(api, tab);

    tabs.close(tab.id);
    expect(api.cancelled).toEqual([calls[0].sendId]);

    calls[0].resolve(answered("late"));
    await sent;
    expect(api.released).toEqual(["late"]);
  });
});

describe("observers of sends", () => {
  let stop: (() => void) | undefined;

  /** Registers an observer that writes what happens to `log`; the settled results are kept in `settled`. */
  function observe(log: string[], before: SendObserver["before"] = async () => {}) {
    const settled: SendResult[] = [];
    stop = observeSends({
      before: async (api) => {
        log.push("before");
        await before(api);
      },
      settled: (_api, result) => {
        log.push("settled");
        settled.push(result);
      },
    });
    return settled;
  }

  afterEach(() => {
    stop?.();
    stop = undefined;
  });

  it("observer_runs_before_and_after_a_send", async () => {
    const { api, calls, tab, sending } = setup();
    const log: string[] = [];
    const onSend = api.onSend;
    api.onSend = (...args) => {
      log.push("sendRequest");
      return onSend(...args);
    };
    const settled = observe(log);

    const sent = sending.send(api, tab);
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    const result = answered("b1");
    calls[0].resolve(result);
    await sent;

    expect(log).toEqual(["before", "sendRequest", "settled"]);
    expect(settled).toEqual([result]);
    expect(sending.result).toEqual(result);

    // A removed observer hears nothing more.
    stop!();
    const again = sending.send(api, tab);
    calls[1].resolve(answered("b2"));
    await again;
    expect(log).toHaveLength(4);
    expect(log[3]).toBe("sendRequest");
  });

  it("settled_is_called_for_a_cancelled_send", async () => {
    const { api, calls, tab, sending } = setup();
    const settled = observe([]);
    const sent = sending.send(api, tab);
    await vi.waitFor(() => expect(calls).toHaveLength(1));

    sending.cancel(api);
    const late = answered("late");
    calls[0].resolve(late);
    await sent;

    expect(settled).toEqual([late]);
    expect(sending.result?.error?.kind).toBe("cancelled");
    expect(api.released).toEqual(["late"]);
  });

  it("settled_is_called_for_a_replaced_send", async () => {
    const { api, calls, tab, sending } = setup();
    const settled = observe([]);
    const first = sending.send(api, tab);
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    const second = sending.send(api, tab);
    await vi.waitFor(() => expect(calls).toHaveLength(2));

    const dropped = answered("b1");
    const kept = answered("b2");
    calls[0].resolve(dropped);
    await first;
    expect(settled).toEqual([dropped]);

    calls[1].resolve(kept);
    await second;
    expect(settled).toEqual([dropped, kept]);
    expect(sending.result).toEqual(kept);
    expect(api.released).toEqual(["b1"]);
  });

  it("failing_before_does_not_stop_the_send", async () => {
    const { api, calls, tab, sending } = setup();
    const settled = observe([], () => Promise.reject("flush failed"));

    const sent = sending.send(api, tab);
    await vi.waitFor(() => expect(calls).toHaveLength(1));
    const result = answered("b1");
    calls[0].resolve(result);
    await sent;

    expect(settled).toEqual([result]);
    expect(sending.result).toEqual(result);
  });
});
