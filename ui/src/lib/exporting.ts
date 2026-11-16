// What every export to a file does once its command was called.

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
