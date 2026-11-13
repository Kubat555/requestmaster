// The key combinations of the application. One listener on the window catches them in the capture phase, so
// that neither the code editor nor the webview itself gets them first. Next to them — the guard that keeps the
// right-click menu of the webview away from everything but text fields.

/** What the combinations do. */
export interface HotkeyActions {
  /** Ctrl+S: save the shown tab. */
  save(): void;
  /** Ctrl+W: close the shown tab. */
  closeTab(): void;
  /** Ctrl+L: move the focus to the URL field. */
  focusUrl(): void;
  /** Ctrl+Enter: send the request of the shown tab. */
  send(): void;
}

/**
 * The pressed key in lower case: the Latin letter of a letter key, the name of any other one ("enter"). The
 * letter is taken from the physical key, so that a combination works on any keyboard layout (on the Russian
 * one Ctrl+S arrives with the key "ы").
 */
function letter(event: KeyboardEvent): string {
  return (event.code.startsWith("Key") ? event.code.slice(3) : event.key).toLowerCase();
}

/**
 * Starts listening for the combinations and returns the function that stops it. A combination of the
 * application never reaches other listeners and has no default action. While a modal dialog is open the
 * combinations do nothing: the window behind the dialog must not change.
 */
export function installHotkeys(actions: HotkeyActions): () => void {
  const byKey: Record<string, () => void> = {
    s: actions.save,
    w: actions.closeTab,
    l: actions.focusUrl,
    // Caught here, or the code editor would first put an empty line into the body.
    enter: actions.send,
  };

  function handler(event: KeyboardEvent) {
    // F5 and Ctrl+R (with Shift too) reload the page of the webview, which would drop every unsaved draft.
    const reload = event.key === "F5" || (event.ctrlKey && !event.altKey && letter(event) === "r");
    // AltGr arrives as Ctrl+Alt and types a character, so Alt rules a combination out.
    const action = event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey ? byKey[letter(event)] : undefined;
    if (!reload && !action) return;

    event.preventDefault();
    event.stopPropagation();
    if (action && document.querySelector('[aria-modal="true"]') === null) action();
  }

  window.addEventListener("keydown", handler, { capture: true });
  return () => window.removeEventListener("keydown", handler, { capture: true });
}

/** Where the menu of the webview stays: text fields and the text of the code editor need it for copy and paste. */
const KEEPS_CONTEXT_MENU = 'input, textarea, .cm-content, [contenteditable]:not([contenteditable="false"])';

/**
 * Cancels the right-click menu of the webview everywhere except text fields, and returns the function that
 * stops it. The menu has a "Reload" item, and a reload of the page drops every unsaved draft.
 */
export function installContextMenuGuard(): () => void {
  function handler(event: MouseEvent) {
    const target = event.target;
    if (target instanceof Element && target.closest(KEEPS_CONTEXT_MENU) !== null) return;
    event.preventDefault();
  }

  window.addEventListener("contextmenu", handler);
  return () => window.removeEventListener("contextmenu", handler);
}
