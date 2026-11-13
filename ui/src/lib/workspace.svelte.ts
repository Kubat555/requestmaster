// State shared by the parts of the window: the tree of the sidebar and the environments of the switcher.

import { SvelteSet } from "svelte/reactivity";
import type { Api } from "./api";
import { toasts } from "./toasts.svelte";
import type { EnvSummary, TreeRow } from "./types";

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
  /** Environments of the switcher, by name. */
  environments = $state<EnvSummary[]>([]);
  /** Id of the active environment; `null` — none. */
  activeEnvironmentId = $state<string | null>(null);
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
