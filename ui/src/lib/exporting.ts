// What every export to a file and every choice of files does around its command.

import type { Api } from "./api";
import { strings } from "./strings";
import { toasts } from "./toasts.svelte";

/**
 * Runs an export command and tells where the file is. `null` — the user closed the window that asks where to
 * save: not an error, and nothing is told. A failure is shown as a notification.
 */
export async function exportToFile(run: () => Promise<string | null>): Promise<void> {
  try {
    const path = await run();
    if (path !== null) toasts.info(strings.export.saved(path));
  } catch (error) {
    toasts.error(String(error));
  }
}

/**
 * Asks for files with the system window (`multiple` — more than one may be chosen). Empty — the user closed the
 * window, or the command failed; a failure is shown as a notification.
 */
export async function pickFiles(api: Api, multiple: boolean): Promise<string[]> {
  try {
    return await api.pickFiles(multiple);
  } catch (error) {
    toasts.error(String(error));
    return [];
  }
}
