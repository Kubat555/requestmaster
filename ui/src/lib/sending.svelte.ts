// Sending the request of a tab: whether a send is running and the result the response panel shows.

import type { Api } from "./api";
import { history } from "./history.svelte";
import type { Tab } from "./tabs.svelte";
import { toasts } from "./toasts.svelte";
import type { RequestDraft, SendResult } from "./types";

/** What a send needs of a request tab. */
type Sendable = Pick<Tab<RequestDraft>, "nodeId" | "draft" | "sending">;

/**
 * The result shown after the user cancelled: the answer of the core is not waited for, so there is nothing
 * of the scripts in it.
 */
function cancelledResult(): SendResult {
  return {
    response: null,
    error: { kind: "cancelled", message: "", fields: [] },
    warnings: [],
    tests: [],
    console: [],
    console_truncated: false,
    script_errors: [],
    variable_changes: [],
  };
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

/** Something that follows every send of the window, whatever tab it is of. */
export interface SendObserver {
  /** Before the command of a send is called; the send waits for it. A failure does not stop the send. */
  before(api: Api): Promise<void>;
  /** The command returned a result — also one the tab drops (the send was cancelled or replaced). */
  settled(api: Api, result: SendResult): void;
}

const observers = new Set<SendObserver>();

/** Registers an observer of every send of the window; the returned function removes it. */
export function observeSends(observer: SendObserver): () => void {
  observers.add(observer);
  return () => void observers.delete(observer);
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
  /** When the running send started, by `performance.now()`; of the last send when none is running. */
  startedAt = 0;
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
   *
   * The observers of the sends ({@link observeSends}) are waited for before the command and are told its
   * result before the tab decides whether it is the awaited one: what the scripts of a dropped send changed
   * in the variables is written by the core all the same. A send that was cancelled or replaced while the
   * observers were waited for does not call the command at all. A command that fails has no result to tell.
   */
  async send(api: Api, tab: Sendable): Promise<void> {
    const sendId = crypto.randomUUID();
    this.#awaited = sendId;
    this.#api = api;
    this.startedAt = performance.now();
    this.state = "sending";

    for (const observer of [...observers]) {
      try {
        await observer.before(api);
      } catch (error) {
        report(error);
      }
    }
    // Cancelled or replaced meanwhile: nothing went to the core yet, and nothing has to.
    if (this.#awaited !== sendId) return;

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

    for (const observer of [...observers]) {
      try {
        observer.settled(api, result);
      } catch (error) {
        report(error);
      }
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
