// Sending the request of a tab: whether a send is running and the result the response panel shows.

import type { Api } from "./api";
import { history } from "./history.svelte";
import type { Tab } from "./tabs.svelte";
import { toasts } from "./toasts.svelte";
import type { RequestDraft, SendResult } from "./types";

/** What a send needs of a request tab. */
type Sendable = Pick<Tab<RequestDraft>, "nodeId" | "draft" | "sending">;

/** The result shown after the user cancelled: the answer of the core is not waited for. */
function cancelledResult(): SendResult {
  return { response: null, error: { kind: "cancelled", message: "", fields: [] }, warnings: [] };
}

/**
 * A plain copy of the draft to hand to a command. Generic, because the type `$state.snapshot` computes for
 * the nested JSON of a request is too deep for the compiler.
 */
function plainCopy<T>(value: T): T {
  return $state.snapshot(value) as T;
}

function report(error: unknown): void {
  toasts.error(String(error));
}

/** Deletes the stored body of a response nobody shows any more. */
function releaseBody(api: Api, result: SendResult | null): void {
  if (result?.response) api.releaseBody(result.response.body.id).catch(report);
}

/** The send state of one tab. */
export class Sending {
  /** Whether a send is running. */
  state = $state<"idle" | "sending">("idle");
  /** Outcome of the last send that was not dropped; `null` before the first one. Replaced as a whole. */
  result = $state.raw<SendResult | null>(null);
  /** Id of the send whose result will be taken: the last one started; `null` — none, or it was cancelled. */
  #awaited: string | null = null;
  /** The api of the last send: closing the tab releases the stored body through it. */
  #api: Api | null = null;

  /**
   * Sends the draft of the tab, saved or not, and waits for the outcome. A send started while another one
   * runs takes its place: only the result of the last send is kept, any other is dropped and its body
   * released. A failure of the command itself is shown as a notification and leaves the previous result.
   * Whatever the command ends with, the shell has written the send to the history by then, so the list of the
   * history is read again — also for a result that is dropped.
   */
  async send(api: Api, tab: Sendable): Promise<void> {
    const sendId = crypto.randomUUID();
    this.#awaited = sendId;
    this.#api = api;
    this.state = "sending";

    let result: SendResult;
    try {
      // Not waited for: the history has nothing to do with what the send shows.
      result = await api.sendRequest(sendId, tab.nodeId, plainCopy(tab.draft)).finally(() => void history.reload(api));
    } catch (error) {
      // A send that was cancelled or replaced is of no interest any more, and neither is its failure.
      if (this.#awaited !== sendId) return;
      this.#awaited = null;
      this.state = "idle";
      report(error);
      return;
    }

    if (this.#awaited !== sendId) {
      releaseBody(api, result);
      return;
    }
    this.#awaited = null;
    this.#show(api, result);
    this.state = "idle";
  }

  /**
   * Stops the running send. The tab is idle with a "cancelled" result at once; whatever the send returns
   * later is dropped. Does nothing when no send is running. The history is not read here: the shell writes the
   * cancelled send when its command returns, and {@link send} reads the history then.
   */
  cancel(api: Api): void {
    const sendId = this.#awaited;
    if (sendId === null) return;
    this.#awaited = null;
    this.state = "idle";
    this.#show(api, cancelledResult());
    api.cancelSend(sendId).catch(report);
  }

  /** The tab is being closed: stops the running send and releases the body of the shown response. */
  dispose(): void {
    const api = this.#api;
    // Nothing was ever sent from this tab.
    if (!api) return;
    const sendId = this.#awaited;
    if (sendId !== null) {
      this.#awaited = null;
      this.state = "idle";
      api.cancelSend(sendId).catch(report);
    }
    this.#show(api, null);
  }

  /** Replaces the shown result; the body of the previous response is not needed any more. */
  #show(api: Api, result: SendResult | null): void {
    releaseBody(api, this.result);
    this.result = result;
  }
}

/** Whether the window may start a send of the tab: none is running and the address is not empty. */
export function canSend(tab: Sendable): boolean {
  return tab.sending.state === "idle" && tab.draft.url.trim() !== "";
}
