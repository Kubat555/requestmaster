// What the window remembers between two launches: its layout with the open tabs, and the unsaved drafts of the
// tabs of nodes. Read once at the start; after that every change is written down a little later, and at once
// when the window is closed.

import { flushSync } from "svelte";
import type { Api } from "./api";
import { layout } from "./layout.svelte";
import { tabs, type Tab } from "./tabs.svelte";
import { toasts } from "./toasts.svelte";
import type { NodeContent, SavedTab, SessionDto, WindowState } from "./types";

/** How long after the last change the state of the window is written. */
export const WINDOW_SAVE_DELAY_MS = 500;
/** How long after the last edit the draft of a tab is written. */
export const DRAFT_SAVE_DELAY_MS = 1000;

/** Width of the sidebar in pixels: the value of a new window and the limits. */
export const SIDEBAR_DEFAULT = 280;
export const SIDEBAR_MIN = 200;
export const SIDEBAR_MAX = 480;

/** What a database without a remembered window gives. */
function emptySession(): SessionDto {
  return {
    window: { sidebar_width: SIDEBAR_DEFAULT, placement: "auto", split: 50, tabs: [], active: null },
    drafts: [],
    measure: false,
  };
}

/** A remembered width of the sidebar within the limits; anything that is not a number is the default. */
function sidebarWidthOf(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return SIDEBAR_DEFAULT;
  return Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, value));
}

/** The text two states of the window are compared by: the keys always go in the same order. */
function windowKey({ sidebar_width, placement, split, tabs: saved, active }: WindowState): string {
  return JSON.stringify([sidebar_width, placement, split, saved.map(({ kind, id }) => [kind, id]), active]);
}

export class Session {
  /** Width of the sidebar in pixels, `SIDEBAR_MIN`..`SIDEBAR_MAX`. */
  sidebarWidth = $state(SIDEBAR_DEFAULT);

  /** What was read at the start; `null` — nothing was read yet. */
  #loaded: SessionDto | null = null;

  /**
   * Whether the program was started to be measured, as the core told at the start; `false` until
   * {@link start} has read it and when the reading failed.
   */
  get measure(): boolean {
    return this.#loaded?.measure ?? false;
  }

  /** Changes with every {@link stop}: what was begun before it must not go on after it. */
  #run = 0;
  /** Stops watching for changes; `null` — the watching has not begun. */
  #unwatch: (() => void) | null = null;

  /** {@link windowKey} of the state the storage has; `null` — not known, the next write tells. */
  #windowWritten: string | null = null;
  /** Counts down to the write of the state of the window. */
  #windowTimer: ReturnType<typeof setTimeout> | null = null;

  /** The open tabs of nodes whose drafts are watched, by the id of the node, with what stops the watching. */
  #watched = new Map<string, { tab: Tab; stop: () => void }>();
  /** Ids of the nodes whose draft is in the storage, or may be: it was written or restored. */
  #stored = new Set<string>();
  /** Ids of the nodes whose tab has edits the storage does not have. */
  #unwritten = new Set<string>();
  /** Ids of the nodes whose draft could not be deleted from the storage; tried again when the window is closed. */
  #undeleted = new Set<string>();
  /** Count down to the write of a draft, by the id of the node. */
  #draftTimers = new Map<string, ReturnType<typeof setTimeout>>();

  /** Every command runs after the previous one has ended, so that the storage gets them in the order of the calls. */
  #queue: Promise<unknown> = Promise.resolve();
  /** The writes that are running; none of them rejects. */
  #running = new Set<Promise<unknown>>();

  /**
   * Reads what was remembered and applies the layout: the placement of the response, the share of the editor and
   * the width of the sidebar. Called before the window is drawn, so that it is drawn as it was left. Never
   * rejects: a failure to read is shown as a notification and gives the defaults. Reads once; {@link stop}
   * makes the next call read again.
   */
  async start(api: Api): Promise<void> {
    if (this.#loaded) return;
    const run = this.#run;
    let loaded: SessionDto;
    try {
      loaded = await api.loadSession();
    } catch (error) {
      toasts.error(String(error));
      loaded = emptySession();
    }
    if (run !== this.#run || this.#loaded) return;
    this.#loaded = loaded;
    layout.apply(loaded.window);
    this.sidebarWidth = sidebarWidthOf(loaded.window.sidebar_width);
  }

  /**
   * Does {@link start} if it was not done, opens the remembered tabs with their drafts, and begins to watch
   * for changes: from now on they are written down by themselves.
   */
  async open(api: Api): Promise<void> {
    const run = this.#run;
    await this.start(api);
    if (run !== this.#run || this.#unwatch || !this.#loaded) return;
    const { window, drafts } = this.#loaded;
    const contents = new Map(drafts.map((draft) => [draft.node_id, draft.content]));
    await tabs.restore(api, window.tabs, contents, window.active);
    if (run !== this.#run || this.#unwatch) return;

    for (const id of contents.keys()) {
      if (tabs.list.some((tab) => tab.nodeId === id)) this.#stored.add(id);
    }
    // Compared with what was read, not with what is on the screen now: a tab the user opened meanwhile, a value
    // that was brought within its limits, a tab that was left out — all of that is a change to write down.
    this.#windowWritten = windowKey(window);
    this.#unwatch = $effect.root(() => {
      $effect(() => this.#watchWindow(api));
      $effect(() => this.#watchTabs(api));
    });
  }

  /**
   * Writes now everything that was waiting for its time: the drafts of the changed tabs and the state of the
   * window. Resolves to whether the storage has all of it; `false` — something was not written. Shows no
   * notification: the caller decides what to do. Before {@link open} has ended there is nothing to write.
   */
  async flush(api: Api): Promise<boolean> {
    if (!this.#unwatch) return true;
    // The changes made just now are seen first.
    flushSync();
    await Promise.all([...this.#running]);
    let written = true;
    for (const tab of [...tabs.list]) {
      if (tab.kind === "environment" || tab.kind === "history" || !this.#unwritten.has(tab.nodeId)) continue;
      written = (await this.#writeDraft(api, tab)) === null && written;
    }
    for (const id of [...this.#undeleted]) written = (await this.#deleteDraft(api, id)) && written;
    if (windowKey(this.#windowState()) !== this.#windowWritten) written = (await this.#writeWindow(api)) === null && written;
    return written;
  }

  /**
   * Stops watching and drops everything that was waiting for its time, without writing it. The next
   * {@link start} reads the storage again.
   */
  stop(): void {
    this.#run++;
    this.#unwatch?.();
    this.#unwatch = null;
    this.#loaded = null;
    if (this.#windowTimer !== null) clearTimeout(this.#windowTimer);
    this.#windowTimer = null;
    this.#windowWritten = null;
    for (const timer of this.#draftTimers.values()) clearTimeout(timer);
    this.#draftTimers.clear();
    for (const watched of this.#watched.values()) watched.stop();
    this.#watched.clear();
    this.#stored.clear();
    this.#unwritten.clear();
    this.#undeleted.clear();
  }

  /** The state of the window as it is remembered. A tab of a history entry is not: its entry may be gone by the next launch. */
  #windowState(): WindowState {
    const saved: SavedTab[] = [];
    for (const tab of tabs.list) {
      if (tab.kind === "environment") saved.push({ kind: "environment", id: tab.environmentId });
      else if (tab.kind !== "history") saved.push({ kind: "node", id: tab.nodeId });
    }
    const active = tabs.active;
    return {
      sidebar_width: this.sidebarWidth,
      placement: layout.placement,
      split: layout.split,
      tabs: saved,
      active: active && active.kind !== "history" ? active.id : null,
    };
  }

  /** Runs on every change of the state of the window: the write starts when nothing more changes for a while. */
  #watchWindow(api: Api): void {
    const changed = windowKey(this.#windowState()) !== this.#windowWritten;
    if (this.#windowTimer !== null) clearTimeout(this.#windowTimer);
    this.#windowTimer = null;
    if (!changed) return;
    this.#windowTimer = setTimeout(() => {
      this.#windowTimer = null;
      void this.#notify(this.#writeWindow(api));
    }, WINDOW_SAVE_DELAY_MS);
  }

  /** Writes the state of the window; resolves to the error, `null` when it is written. */
  #writeWindow(api: Api): Promise<string | null> {
    if (this.#windowTimer !== null) clearTimeout(this.#windowTimer);
    this.#windowTimer = null;
    const state = this.#windowState();
    const key = windowKey(state);
    // What is sent is what the storage has afterwards: a change made while the command runs is seen as one.
    this.#windowWritten = key;
    return this.#command(
      () => api.saveWindowState(state),
      () => {
        if (this.#windowWritten === key) this.#windowWritten = null;
      },
    );
  }

  /**
   * Runs when a tab is opened or closed: watches the draft of every tab of a node, and deletes the stored draft
   * of a tab that was closed — with or without saving, its edits are not wanted any more.
   */
  #watchTabs(api: Api): void {
    const open = tabs.list.filter((tab): tab is Tab => tab.nodeId !== null);
    for (const [id, watched] of this.#watched) {
      if (open.includes(watched.tab)) continue;
      watched.stop();
      this.#watched.delete(id);
      void this.#deleteDraft(api, id);
    }
    for (const tab of open) {
      if (this.#watched.has(tab.nodeId)) continue;
      // An effect of its own, not a child of this one: it lives as long as the tab, whatever happens to the others.
      const stop = $effect.root(() => {
        let first = true;
        $effect(() => {
          this.#watchDraft(api, tab, first);
          first = false;
        });
      });
      this.#watched.set(tab.nodeId, { tab, stop });
    }
  }

  /**
   * Runs on every edit of the draft of the tab. While the tab is changed, its draft is written when nothing
   * more is edited for a while; when it stops being changed (Ctrl+S, the edits were put back), the stored
   * draft is deleted at once. `first` — the tab has just got its watching, nothing was edited since.
   */
  #watchDraft(api: Api, tab: Tab, first: boolean): void {
    const id = tab.nodeId;
    if (!tab.dirty) {
      void this.#deleteDraft(api, id);
      return;
    }
    // Reads every field of the draft, so that an edit of any of them comes here.
    $state.snapshot<unknown>(tab.draft);
    // A restored tab is changed from the start, and its draft is what the storage has.
    if (first && this.#stored.has(id)) return;
    this.#unwritten.add(id);
    this.#cancelDraftTimer(id);
    this.#draftTimers.set(
      id,
      setTimeout(() => {
        this.#draftTimers.delete(id);
        void this.#notify(this.#writeDraft(api, tab));
      }, DRAFT_SAVE_DELAY_MS),
    );
  }

  /** Writes the draft of the tab; resolves to the error, `null` when it is written or there is nothing to write. */
  #writeDraft(api: Api, tab: Tab): Promise<string | null> {
    const id = tab.nodeId;
    this.#cancelDraftTimer(id);
    if (!tab.dirty || !tabs.list.includes(tab)) return Promise.resolve(null);
    // Through `unknown`: the type of a snapshot of nested JSON (`postman_extra`) is too deep to compute.
    const content = $state.snapshot<unknown>(tab.draft) as NodeContent;
    // What is sent is what the storage has afterwards: an edit made while the command runs is unwritten again.
    this.#unwritten.delete(id);
    this.#undeleted.delete(id);
    this.#stored.add(id);
    return this.#command(
      () => api.saveDraft(id, content),
      () => this.#unwritten.add(id),
    );
  }

  /**
   * Deletes the draft of the node from the storage, if it may be there, and drops its write that was waiting.
   * Resolves to whether the storage has no such draft now. A failure is not told: the deletion is tried again
   * when the window is closed, and a draft that stays is only shown once more at the next launch.
   */
  async #deleteDraft(api: Api, id: string): Promise<boolean> {
    this.#cancelDraftTimer(id);
    this.#unwritten.delete(id);
    if (!this.#stored.delete(id) && !this.#undeleted.has(id)) return true;
    this.#undeleted.delete(id);
    const error = await this.#command(
      () => api.saveDraft(id, null),
      () => this.#undeleted.add(id),
    );
    return error === null;
  }

  #cancelDraftTimer(id: string): void {
    const timer = this.#draftTimers.get(id);
    if (timer !== undefined) clearTimeout(timer);
    this.#draftTimers.delete(id);
  }

  /**
   * Runs the command after the ones started before it. Resolves to the error as text, `null` when the command
   * was done; `failed` is called before that, unless the session was stopped meanwhile.
   */
  #command(command: () => Promise<void>, failed: () => void): Promise<string | null> {
    const run = this.#run;
    const result = this.#queue.then(command).then(
      () => null,
      (error) => {
        if (run === this.#run) failed();
        return String(error);
      },
    );
    this.#queue = result;
    this.#running.add(result);
    void result.then(() => this.#running.delete(result));
    return result;
  }

  /** A write started by a timer that failed is shown as a notification; the next change tries again. */
  async #notify(write: Promise<string | null>): Promise<void> {
    const error = await write;
    if (error !== null) toasts.error(error);
  }
}

/** What the application window remembers. */
export const session = new Session();
