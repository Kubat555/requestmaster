// Short notifications in the corner of the window: errors of the commands and plain messages.

/** One notification on the screen. */
export interface Toast {
  id: number;
  kind: "error" | "info";
  text: string;
}

/** How long a notification stays on the screen. */
const LIFETIME_MS = 6000;

class Toasts {
  /** Notifications on the screen, oldest first. */
  items = $state<Toast[]>([]);
  #nextId = 1;

  /** Shows an error, e.g. the message of a failed command. */
  error(text: string): void {
    this.#show("error", text);
  }

  /** Shows a plain message. */
  info(text: string): void {
    this.#show("info", text);
  }

  #show(kind: Toast["kind"], text: string): void {
    const id = this.#nextId++;
    this.items.push({ id, kind, text });
    setTimeout(() => {
      this.items = this.items.filter((toast) => toast.id !== id);
    }, LIFETIME_MS);
  }
}

/** The notifications of the window; `Toasts.svelte` renders them. */
export const toasts = new Toasts();
