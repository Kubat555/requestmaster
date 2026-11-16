// State shared by the parts of the window: the tree of the sidebar and the environments of the switcher.

import { SvelteSet } from "svelte/reactivity";
import type { Api } from "./api";
import { strings } from "./strings";
import { toasts } from "./toasts.svelte";
import type { EnvSummary, NodeKind, TreeRow } from "./types";

/** The state of one window. The application uses {@link workspace}; a test creates its own. */
export class Workspace {
  /** All nodes of the tree, as the `tree` command returned them. */
  rows = $state<TreeRow[]>([]);
  /** Ids of the expanded collections and folders. */
  expanded = new SvelteSet<string>();
  /** Text of the filter field of the sidebar. */
  filter = $state("");
  /** Id of the node highlighted in the tree. */
  selectedId = $state<string | null>(null);
  /**
   * Id of the row of the tree that had the keyboard focus last. A click on a folder does not select it, so the
   * commands of the tree act on this row, not on the selected one.
   */
  focusedId = $state<string | null>(null);
  /** Id of the node whose row shows a field instead of the name; `null` — nothing is being renamed. */
  renamingId = $state<string | null>(null);
  /** Environments of the switcher, by name. */
  environments = $state<EnvSummary[]>([]);
  /** Id of the active environment; `null` — none. */
  activeEnvironmentId = $state<string | null>(null);
  /** Id of the environment whose row of the list shows a field instead of the name; `null` — none. */
  renamingEnvironmentId = $state<string | null>(null);
  /** Called when the user opens a request or a collection in the tree. */
  onOpen: (id: string) => void = () => {};

  /**
   * Reads the tree, the environments and the active environment; what is expanded and selected stays.
   * A failure is shown as a notification and leaves the state as it was.
   */
  async load(api: Api): Promise<void> {
    try {
      const [rows, environments, activeEnvironmentId] = await Promise.all([
        api.tree(),
        api.environments(),
        api.activeEnvironment(),
      ]);
      this.rows = rows;
      this.environments = environments;
      this.activeEnvironmentId = activeEnvironmentId;
    } catch (error) {
      toasts.error(String(error));
    }
  }

  /**
   * Reads the tree again, e.g. after a saved request may have changed its method. A failure is shown as a
   * notification and leaves the tree as it was.
   */
  async reloadTree(api: Api): Promise<void> {
    try {
      this.rows = await api.tree();
    } catch (error) {
      toasts.error(String(error));
    }
  }

  /**
   * Reads the environments and the active one again, after an environment was created, renamed or deleted.
   * A failure is shown as a notification and leaves them as they were.
   */
  async reloadEnvironments(api: Api): Promise<void> {
    try {
      const [environments, activeEnvironmentId] = await Promise.all([api.environments(), api.activeEnvironment()]);
      this.environments = environments;
      this.activeEnvironmentId = activeEnvironmentId;
    } catch (error) {
      toasts.error(String(error));
    }
  }

  /**
   * Creates an environment with the default name and resolves to its id; its row of the list starts in the
   * renaming mode. A failure is shown as a notification and resolves to `null`.
   */
  async createEnvironment(api: Api): Promise<string | null> {
    let id: string;
    try {
      id = await api.createEnvironment(strings.environment.newName);
    } catch (error) {
      toasts.error(String(error));
      return null;
    }
    await this.reloadEnvironments(api);
    this.renamingEnvironmentId = id;
    return id;
  }

  /**
   * Renames the environment, keeping the variables the storage has, and resolves to the name it has now.
   * A failure is shown as a notification, leaves the environment as it was and resolves to `null`.
   */
  async renameEnvironment(api: Api, id: string, name: string): Promise<string | null> {
    try {
      // There is no command for the name alone: it is saved together with the variables.
      const { variables } = await api.getEnvironment(id);
      await api.saveEnvironment(id, name, variables);
    } catch (error) {
      toasts.error(String(error));
      return null;
    }
    await this.reloadEnvironments(api);
    return this.environments.find((environment) => environment.id === id)?.name ?? name;
  }

  /**
   * Deletes the environment and tells whether it is gone; a deleted active environment leaves none chosen.
   * A failure is shown as a notification and leaves everything as it was.
   */
  async deleteEnvironment(api: Api, id: string): Promise<boolean> {
    try {
      await api.deleteEnvironment(id);
    } catch (error) {
      toasts.error(String(error));
      return false;
    }
    // The core has cleared the choice too; this holds even when the re-reading below fails.
    if (this.activeEnvironmentId === id) this.activeEnvironmentId = null;
    if (this.renamingEnvironmentId === id) this.renamingEnvironmentId = null;
    await this.reloadEnvironments(api);
    return true;
  }

  /**
   * Where the "+" button creates: the row that had the focus last, without it the selected node, without
   * that the first collection; `null` while there are no collections.
   */
  get createTarget(): TreeRow | null {
    const collections = this.rows.filter((row) => row.kind === "collection");
    return (
      this.#row(this.focusedId) ??
      this.#row(this.selectedId) ??
      collections.sort((a, b) => a.sort_order - b.sort_order)[0] ??
      null
    );
  }

  /**
   * Creates a node with the default name of its kind at the end of `parentId` (`null` — a collection) and
   * resolves to its id. The new node is selected, its ancestors are expanded, its row starts in the renaming
   * mode, and a request is opened. A failure is shown as a notification, leaves the tree as it was and
   * resolves to `null`.
   */
  async createNode(api: Api, parentId: string | null, kind: NodeKind): Promise<string | null> {
    let id: string;
    try {
      id = await api.createNode(parentId, kind, strings.tree.newNode[kind]);
    } catch (error) {
      toasts.error(String(error));
      return null;
    }
    await this.reloadTree(api);
    // A filter would hide a row whose name does not match it, and the field for the name with it.
    this.filter = "";
    for (let row = this.#row(id); row?.parent_id; row = this.#row(row.parent_id)) this.expanded.add(row.parent_id);
    this.selectedId = id;
    this.renamingId = id;
    if (kind === "request") this.onOpen(id);
    return id;
  }

  /**
   * Renames the node and re-reads the tree; resolves to the name the node has now. A failure is shown as a
   * notification, leaves the tree as it was and resolves to `null`.
   */
  async renameNode(api: Api, id: string, name: string): Promise<string | null> {
    try {
      await api.renameNode(id, name);
    } catch (error) {
      toasts.error(String(error));
      return null;
    }
    await this.reloadTree(api);
    return this.#row(id)?.name ?? name;
  }

  /**
   * Copies the node with everything inside right under the original and selects the copy; resolves to its
   * id. A failure is shown as a notification, leaves the tree as it was and resolves to `null`.
   */
  async duplicateNode(api: Api, id: string): Promise<string | null> {
    let copyId: string;
    try {
      copyId = await api.duplicateNode(id);
    } catch (error) {
      toasts.error(String(error));
      return null;
    }
    await this.reloadTree(api);
    this.selectedId = copyId;
    return copyId;
  }

  /**
   * Deletes the node with everything inside and resolves to the ids of all deleted nodes, so that the caller
   * closes their tabs. A failure is shown as a notification, leaves the tree as it was and resolves to `null`.
   */
  async deleteNode(api: Api, id: string): Promise<string[] | null> {
    const removed = this.subtree(id).map((row) => row.id);
    try {
      await api.deleteNode(id);
    } catch (error) {
      toasts.error(String(error));
      return null;
    }
    await this.reloadTree(api);
    for (const removedId of removed) this.expanded.delete(removedId);
    if (this.selectedId !== null && removed.includes(this.selectedId)) this.selectedId = null;
    if (this.focusedId !== null && removed.includes(this.focusedId)) this.focusedId = null;
    if (this.renamingId !== null && removed.includes(this.renamingId)) this.renamingId = null;
    return removed;
  }

  /** The node and everything under it at any depth; empty when there is no such node. */
  subtree(id: string): TreeRow[] {
    const root = this.#row(id);
    if (!root) return [];
    // The children of every node, found in one pass over the rows instead of one pass per node of the subtree.
    const children = new Map<string, TreeRow[]>();
    for (const row of this.rows) {
      if (row.parent_id === null) continue;
      const siblings = children.get(row.parent_id);
      if (siblings) siblings.push(row);
      else children.set(row.parent_id, [row]);
    }
    const found = [root];
    const collect = (parentId: string) => {
      for (const child of children.get(parentId) ?? []) {
        found.push(child);
        collect(child.id);
      }
    };
    collect(id);
    return found;
  }

  #row(id: string | null): TreeRow | undefined {
    return this.rows.find((row) => row.id === id);
  }

  /** Expands a collapsed node and collapses an expanded one. */
  toggle(id: string): void {
    if (!this.expanded.delete(id)) this.expanded.add(id);
  }

  /** Expands the nodes, e.g. the roots of the collections just imported. */
  expand(ids: string[]): void {
    for (const id of ids) this.expanded.add(id);
  }

  /** Highlights the node in the tree. */
  select(id: string): void {
    this.selectedId = id;
  }

  /**
   * Makes the environment active; `null` clears the choice. The switcher shows the new choice at once;
   * a failure is shown as a notification and brings the previous choice back.
   */
  async setEnvironment(api: Api, id: string | null): Promise<void> {
    const previous = this.activeEnvironmentId;
    this.activeEnvironmentId = id;
    try {
      await api.setActiveEnvironment(id);
    } catch (error) {
      toasts.error(String(error));
      // A later choice made while this one was being saved is not undone.
      if (this.activeEnvironmentId === id) this.activeEnvironmentId = previous;
    }
  }
}

/** The state of the application window. */
export const workspace = new Workspace();
