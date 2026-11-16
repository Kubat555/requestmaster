// The list of the history section of the sidebar: the sends the shell has written down, the newest first.

import type { Api } from "./api";
import { toasts } from "./toasts.svelte";
import type { HistoryItem } from "./types";

/** How many entries are read at a time. */
export const HISTORY_PAGE = 100;

/** The history of one window. The application uses {@link history}; a test may create its own. */
export class History {
  /** The entries read so far, the newest first. Replaced as a whole. */
  items = $state.raw<HistoryItem[]>([]);
  /** Whether the storage may hold entries older than the last one of {@link items}. */
  hasMore = $state(false);
  /** Number of the last {@link reload} or {@link clear}: the answer to an earlier {@link reload} is not taken. */
  #generation = 0;

  /**
   * Reads the newest entries again: when the section is opened and after every send. Older entries that were
   * read with {@link loadMore} stay in the list. A failure is shown as a notification and leaves the list as it
   * was.
   */
  async reload(api: Api): Promise<void> {
    const generation = ++this.#generation;
    let page: HistoryItem[];
    try {
      page = await api.historyList(HISTORY_PAGE, null);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    if (generation !== this.#generation) return;
    const last = page.at(-1);
    // What was read beyond the first page goes on after it; a page that is not full is the whole history.
    const older = last && page.length === HISTORY_PAGE ? this.items.filter((item) => item.id < last.id) : [];
    this.hasMore = older.length > 0 ? this.hasMore : page.length === HISTORY_PAGE;
    this.items = [...page, ...older];
  }

  /**
   * Reads the entries older than the last one of the list and puts them after it. A failure is shown as a
   * notification and leaves the list as it was.
   */
  async loadMore(api: Api): Promise<void> {
    const last = this.items.at(-1);
    if (!last) return;
    let page: HistoryItem[];
    try {
      page = await api.historyList(HISTORY_PAGE, last.id);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    // The list does not end where it did: the history was cleared meanwhile, or the same entries were asked
    // for twice and are already here.
    if (this.items.at(-1)?.id !== last.id) return;
    this.items = [...this.items, ...page];
    this.hasMore = page.length === HISTORY_PAGE;
  }

  /**
   * Deletes the whole history and tells whether it is gone. A failure is shown as a notification and leaves
   * the list as it was.
   */
  async clear(api: Api): Promise<boolean> {
    try {
      await api.historyClear();
    } catch (error) {
      toasts.error(String(error));
      return false;
    }
    this.#generation++;
    this.items = [];
    this.hasMore = false;
    return true;
  }
}

/** The history of the application window. */
export const history = new History();
