// The open tabs of the window: each one holds the stored content of a request or a collection and the copy
// the user edits.

import type { Api } from "./api";
import { Sending } from "./sending.svelte";
import { toasts } from "./toasts.svelte";
import { normalizeContent, type CollectionDraft, type NodeDto, type RequestDraft } from "./types";
import { workspace } from "./workspace.svelte";

/** Content a tab can hold. */
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

/** One open tab: a request or a collection. */
export class Tab<C extends TabContent = TabContent> {
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
}

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
  list = $state<Tab[]>([]);
  #activeId = $state<string | null>(null);

  /** The tab on the screen; `null` when none is open. */
  get active(): Tab | null {
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

  /** Shows the tab. */
  activate(tabId: string): void {
    if (this.list.some((tab) => tab.id === tabId)) this.#activeId = tabId;
  }

  /**
   * Removes the tab with whatever its draft holds; asking about unsaved edits is the caller's job. Its
   * running send is stopped and the stored body of its response is released. When the shown tab is closed,
   * the one that takes its place is shown, or the previous one after the last.
   */
  close(tabId: string): void {
    const index = this.list.findIndex((tab) => tab.id === tabId);
    if (index < 0) return;
    this.list[index].sending.dispose();
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
    for (const tab of this.list.filter((candidate) => nodeIds.includes(candidate.nodeId))) this.close(tab.id);
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
