// The open tabs of the window. The tab of a request or of a collection holds the stored content of its node and
// the copy the user edits; the tab of an environment holds its variables and saves every edit by itself.

import type { Api } from "./api";
import { Sending } from "./sending.svelte";
import { strings } from "./strings";
import { toasts } from "./toasts.svelte";
import {
  GLOBALS_ID,
  normalizeContent,
  type CollectionDraft,
  type EnvironmentDto,
  type NodeDto,
  type RequestDraft,
  type VariableRow,
} from "./types";
import { workspace } from "./workspace.svelte";

/** What a tab shows. A tab of the history comes with the history itself. */
export type TabKind = "request" | "collection" | "environment" | "history";

/** What every tab has, whatever it shows. */
export interface TabLike {
  /** Unique among the open tabs: the id of the node, or `env:` + the id of the environment. */
  readonly id: string;
  readonly kind: TabKind;
  /** Id of the node of a request or a collection; `null` — the tab is not of a node. */
  readonly nodeId: string | null;
  name: string;
  /** Whether the tab has edits that only Ctrl+S saves. */
  readonly dirty: boolean;
  /** What Ctrl+S does on the tab; resolves to whether everything of the tab is in the storage now. */
  save(api: Api): Promise<boolean>;
  /** The tab is being removed: stops what it has started. */
  dispose(): void;
}

/** Content the tab of a node can hold. */
export type TabContent = RequestDraft | CollectionDraft;

/** Whether two JSON-like values are the same; the order of object keys does not matter. */
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((item, index) => deepEqual(item, b[index]));
  }
  const left = a as Record<string, unknown>;
  const right = b as Record<string, unknown>;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every((key) => key in right && deepEqual(left[key], right[key]));
}

/** The tab of a node: a request or a collection. */
export class Tab<C extends TabContent = TabContent> implements TabLike {
  /** Id of the tab; the same as the id of its node, because a node has at most one tab. */
  readonly id: string;
  readonly nodeId: string;
  readonly kind: "request" | "collection";
  /** Name of the node; follows a renaming in the tree. */
  name: string;
  /** Content as it is in the storage, normalized. Replaced as a whole, never edited. */
  saved: C;
  /** The copy the editors change. */
  draft: C;
  /** Whether `draft` differs from `saved`. */
  readonly dirty: boolean;
  /** The send of the request and its result; a collection tab never sends. */
  readonly sending = new Sending();

  /** `content` is the normalized content of the node; the tab keeps copies of it. */
  constructor(node: { id: string; kind: "request" | "collection"; name: string }, content: C) {
    this.id = node.id;
    this.nodeId = node.id;
    this.kind = node.kind;
    this.name = $state(node.name);
    this.saved = $state.raw(structuredClone(content));
    this.draft = $state(structuredClone(content));
    this.dirty = $derived(!deepEqual(this.draft, this.saved));
  }

  /**
   * Writes the draft to the storage and re-reads the tree, so that it shows the saved method. Returns whether
   * the content was saved; a failure is shown as a notification and leaves the draft as it is.
   */
  async save(api: Api): Promise<boolean> {
    // What is sent is what becomes `saved`: an edit made while the command runs leaves the tab changed.
    const content = $state.snapshot(this.draft) as C;
    try {
      await api.saveNodeContent(this.nodeId, content);
    } catch (error) {
      toasts.error(String(error));
      return false;
    }
    this.saved = content;
    await workspace.reloadTree(api);
    return true;
  }

  /** Throws the edits away: the draft is the stored content again. */
  revert(): void {
    this.draft = structuredClone(this.saved);
  }

  /** Stops the running send and releases the stored body of the response. */
  dispose(): void {
    this.sending.dispose();
  }
}

/** How long after the last edit the variables of an environment are saved. */
const SAVE_DELAY_MS = 400;

/** Id of the tab of the environment. */
export function environmentTabId(environmentId: string): string {
  return `env:${environmentId}`;
}

/**
 * The tab of an environment or of the global variables. There is no draft: the editor changes `variables`,
 * tells the tab with {@link edited}, and the tab saves them by itself a little later. So the tab is never
 * `dirty`; a save that failed is told by {@link failed}.
 */
export class EnvironmentTab implements TabLike {
  readonly id: string;
  readonly kind = "environment";
  readonly nodeId = null;
  /** Id of the environment; `globals` for the global variables. */
  readonly environmentId: string;
  /** Name of the environment; follows a renaming in the list. */
  name: string;
  /** The variables, edited in place by the editor. */
  variables: VariableRow[];
  readonly dirty = false;
  /** The last save failed: the storage does not have what the editor shows. */
  failed: boolean;
  /** Counts down to the save after the last edit. */
  #timer: ReturnType<typeof setTimeout> | null = null;
  /** There are edits the storage does not have: made after the last save began, or left by a failed one. */
  #unsaved = false;
  /** The saving that is running; everyone who asks for a save meanwhile waits for the same one. */
  #flushing: Promise<boolean> | null = null;

  constructor(environment: EnvironmentDto) {
    this.id = environmentTabId(environment.id);
    this.environmentId = environment.id;
    // The set of global variables has no name of its own to show.
    this.name = $state(environment.id === GLOBALS_ID ? strings.environment.globals : environment.name);
    this.variables = $state(structuredClone(environment.variables));
    this.failed = $state(false);
  }

  /** The editor changed a variable: the save starts when nothing more is changed for a while. */
  edited(api: Api): void {
    this.#unsaved = true;
    this.suspend();
    this.#timer = setTimeout(() => void this.flush(api), SAVE_DELAY_MS);
  }

  /**
   * Does the pending save now instead of waiting for its time. Resolves to whether the storage has every edit;
   * `true` at once when there was nothing to save. A failure is shown as a notification and sets {@link failed}:
   * the edits stay unsaved, and the next edit or the next call tries again.
   */
  flush(api: Api): Promise<boolean> {
    this.suspend();
    // One saving at a time: what is edited while a command runs is written by the same saving, after it.
    this.#flushing ??= this.#writeAll(api).finally(() => (this.#flushing = null));
    return this.#flushing;
  }

  /** Ctrl+S has nothing of its own to do here: the pending save is done now. */
  save(api: Api): Promise<boolean> {
    return this.flush(api);
  }

  /**
   * Stops the countdown to the save; the edits stay unsaved until {@link flush} or the next edit. For the time
   * a command that may make the save pointless is running: the deletion of the environment.
   */
  suspend(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
  }

  /**
   * Reads the variables from the storage again: something other than the editor changed them — an import of
   * global variables. Call {@link flush} before that change: an edit that is not saved by now is dropped, or the
   * next save would write the old set over the changed one. A failure to read is shown as a notification and
   * leaves the variables as they are.
   */
  async reload(api: Api): Promise<void> {
    let environment: EnvironmentDto;
    try {
      environment = await api.getEnvironment(this.environmentId);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    this.suspend();
    this.#unsaved = false;
    this.failed = false;
    this.variables = environment.variables;
  }

  /** Drops the pending save: the tab is removed with whatever is not saved. */
  dispose(): void {
    this.suspend();
    this.#unsaved = false;
  }

  /** Writes the variables until the storage has every edit; stops at the first failure. */
  async #writeAll(api: Api): Promise<boolean> {
    while (this.#unsaved) {
      // What is sent is what gets saved: an edit made while the command runs is unsaved again.
      this.#unsaved = false;
      // Through `unknown`: the type of a snapshot of nested JSON (`postman_extra`) is too deep to compute.
      const variables = $state.snapshot<unknown>(this.variables) as VariableRow[];
      try {
        await api.saveEnvironment(this.environmentId, this.name, variables);
      } catch (error) {
        this.#unsaved = true;
        this.failed = true;
        toasts.error(String(error));
        return false;
      }
      this.failed = false;
    }
    return true;
  }
}

/** Any tab of the window. */
export type AnyTab = Tab | EnvironmentTab;

/** Makes the tab of a node; a folder has no tab. */
function createTab(node: NodeDto): Tab | null {
  switch (node.kind) {
    case "request":
      return new Tab({ id: node.id, kind: node.kind, name: node.name }, normalizeContent("request", node.content));
    case "collection":
      return new Tab({ id: node.id, kind: node.kind, name: node.name }, normalizeContent("collection", node.content));
    case "folder":
      return null;
  }
}

/** The tabs of one window. The application uses {@link tabs}; a test may create its own. */
export class Tabs {
  /** Open tabs, in the order of the tab bar. */
  list = $state<AnyTab[]>([]);
  #activeId = $state<string | null>(null);

  /** The tab on the screen; `null` when none is open. */
  get active(): AnyTab | null {
    return this.list.find((tab) => tab.id === this.#activeId) ?? null;
  }

  /**
   * Opens the node in a tab and shows it. A node that is already open is only shown; a folder is not opened.
   * A failure to read the node is shown as a notification.
   */
  async open(api: Api, nodeId: string): Promise<void> {
    if (this.#show(nodeId)) return;
    let node: NodeDto;
    try {
      node = await api.getNode(nodeId);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    // The same node may have been opened while this one was being read.
    if (this.#show(nodeId)) return;
    const tab = createTab(node);
    if (!tab) return;
    this.list.push(tab);
    this.#activeId = tab.id;
  }

  /**
   * Opens the environment (`globals` — the global variables) in a tab and shows it; one that is already open is
   * only shown. A failure to read the environment is shown as a notification.
   */
  async openEnvironment(api: Api, environmentId: string): Promise<void> {
    const tabId = environmentTabId(environmentId);
    if (this.activate(tabId)) return;
    let environment: EnvironmentDto;
    try {
      environment = await api.getEnvironment(environmentId);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    // The same environment may have been opened while this one was being read.
    if (this.activate(tabId)) return;
    this.list.push(new EnvironmentTab(environment));
    this.#activeId = tabId;
  }

  /** The tab of the environment, if it is open. */
  environment(environmentId: string): EnvironmentTab | null {
    const tab = this.list.find((candidate) => candidate.id === environmentTabId(environmentId));
    return tab?.kind === "environment" ? tab : null;
  }

  /** Shows the tab; tells whether there is such a tab. */
  activate(tabId: string): boolean {
    const open = this.list.some((tab) => tab.id === tabId);
    if (open) this.#activeId = tabId;
    return open;
  }

  /**
   * Removes the tab with whatever it holds; asking about unsaved edits is the caller's job. The running send
   * of a request is stopped and the stored body of its response is released; the pending save of an
   * environment is dropped. When the shown tab is closed,
   * the one that takes its place is shown, or the previous one after the last.
   */
  close(tabId: string): void {
    const index = this.list.findIndex((tab) => tab.id === tabId);
    if (index < 0) return;
    this.list[index].dispose();
    this.list.splice(index, 1);
    if (this.#activeId === tabId) this.#activeId = (this.list[index] ?? this.list[index - 1])?.id ?? null;
  }

  /** The node was renamed: its tab, if open, shows the new name; the draft is not touched. */
  rename(nodeId: string, name: string): void {
    const tab = this.list.find((candidate) => candidate.nodeId === nodeId);
    if (tab) tab.name = name;
  }

  /**
   * The nodes were deleted: their tabs are closed with whatever their drafts hold, as {@link close} does —
   * nothing is left to save them to.
   */
  closeNodes(nodeIds: string[]): void {
    for (const tab of this.list.filter((candidate) => candidate.nodeId !== null && nodeIds.includes(candidate.nodeId))) this.close(tab.id);
  }

  /** Shows the tab of the node if it is open. */
  #show(nodeId: string): boolean {
    const open = this.list.find((tab) => tab.nodeId === nodeId);
    if (open) this.#activeId = open.id;
    return open !== undefined;
  }
}

/** The tabs of the application window. */
export const tabs = new Tabs();
