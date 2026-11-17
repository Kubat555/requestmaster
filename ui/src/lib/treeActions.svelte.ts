// What the menu of a row of the tree does, and the two questions it may lead to: about a deletion and about
// exporting a collection with unsaved edits. The tree keeps the rows, the keyboard and the place of the menu.

import type { TreeAction } from "../components/TreeMenu.svelte";
import type { Api } from "./api";
import { exportToFile } from "./exporting";
import { painted } from "./paint";
import { tabs, type AnyTab } from "./tabs.svelte";
import type { TreeNode } from "./tree";
import type { Workspace } from "./workspace.svelte";

/** The menu actions, the delete question and the export question of the tree of one workspace. */
export class TreeActions {
  /** The node the user asked to delete, until the question is answered; `null` — no question on the screen. */
  deleting = $state.raw<TreeNode | null>(null);
  /** The deletion the user confirmed is running: the question waits for it and cannot be answered again. */
  deletingBusy = $state(false);
  /**
   * The collection the user asked to export while its open tabs have unsaved edits, until the question is
   * answered; `null` — no question on the screen.
   */
  exporting = $state.raw<TreeNode | null>(null);

  readonly #workspace: Workspace;

  constructor(workspace: Workspace) {
    this.#workspace = workspace;
  }

  /**
   * Does what the item of the menu says with the node. Resolves to the id of the row the keyboard goes to
   * afterwards; `null` — the focus stays where it is.
   */
  async run(api: Api, node: TreeNode, action: TreeAction): Promise<string | null> {
    const workspace = this.#workspace;
    switch (action) {
      case "open":
        workspace.select(node.id);
        workspace.onOpen(node.id);
        return null;
      case "new_request":
        await workspace.createNode(api, node.id, "request");
        return null;
      case "new_folder":
        await workspace.createNode(api, node.id, "folder");
        return null;
      case "rename":
        workspace.renamingId = node.id;
        return null;
      case "duplicate":
        return workspace.duplicateNode(api, node.id);
      case "export":
        // The file is made of what the storage has, so the user decides about the edits it does not have.
        if (this.unsavedTabs(node.id).length > 0) this.exporting = node;
        else await exportToFile(() => api.exportCollection(node.id));
        return null;
      case "delete":
        this.deleting = node;
        return null;
    }
  }

  /**
   * The user confirmed the deletion. The question stays on the screen, marked as busy, until the command has
   * ended. The tabs of everything deleted are closed without a question about their unsaved edits — there is
   * nothing left to save them to; closing stops their sends and releases their responses. Resolves to the ids
   * of the deleted nodes; `null` — there was no question, a deletion is already running, or the deletion
   * failed (already shown as a notification; the question stays).
   */
  async confirmDelete(api: Api): Promise<string[] | null> {
    const node = this.deleting;
    if (!node || this.deletingBusy) return null;
    this.deletingBusy = true;
    // The question shows that the deletion runs before it starts: a big folder takes a while, and what
    // follows the command — the tree read again, the tabs closed — holds the window until it is done.
    await painted();
    const removed = await this.#workspace.deleteNode(api, node.id);
    this.deletingBusy = false;
    // A failure leaves the question on the screen, to try again or to cancel.
    if (removed === null) return null;
    this.deleting = null;
    tabs.closeNodes(removed);
    return removed;
  }

  /** The question about the deletion is dismissed; not while the deletion is running. */
  cancelDelete(): void {
    if (!this.deletingBusy) this.deleting = null;
  }

  /**
   * The question about the unsaved edits is answered: `save` — the edited tabs are saved first, otherwise the
   * file gets the stored version and the edits stay in their tabs. A tab that cannot be saved stops the export;
   * its failure is already shown as a notification.
   */
  async confirmExport(api: Api, save: boolean): Promise<void> {
    const node = this.exporting;
    this.exporting = null;
    if (!node) return;
    if (save) {
      for (const tab of this.unsavedTabs(node.id)) {
        if (!(await tab.save(api))) return;
      }
    }
    await exportToFile(() => api.exportCollection(node.id));
  }

  /** The open tabs of the collection and of everything inside it that have unsaved edits. */
  unsavedTabs(collectionId: string): AnyTab[] {
    const ids = new Set(this.#workspace.subtree(collectionId).map((row) => row.id));
    return tabs.list.filter((tab) => tab.dirty && tab.nodeId !== null && ids.has(tab.nodeId));
  }
}
