// The key combinations of the application. One listener on the window catches them in the capture phase, so
// that neither the code editor nor the webview itself gets them first. Next to them — the guard that keeps the
// right-click menu of the webview away from everything but text fields.

import { strings } from "./strings";

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
  /** Ctrl+F pressed outside a code editor: search in the body of the response of the shown tab. */
  find(): void;
  /** F1: show the list of the key combinations. */
  shortcuts(): void;
}

/** One row of the list of the key combinations. */
export interface Shortcut {
  /**
   * The combinations of the row, each as the keys pressed together. Several of them are other ways to do the
   * same, or — with {@link then} — presses that follow one another.
   */
  readonly keys: readonly (readonly string[])[];
  /** The combinations are pressed one after another, not instead of one another. */
  readonly then?: boolean;
  /** What it does. */
  readonly text: string;
}

/** The rows of the list about one part of the window. */
export interface ShortcutGroup {
  readonly title: string;
  readonly shortcuts: readonly Shortcut[];
}

/** A combination the listener of the window answers, wherever the focus is. */
export interface GlobalHotkey extends Shortcut {
  /** The key as {@link letter} names it. */
  readonly key: string;
  /** Whether Ctrl is held; no other modifier ever is. */
  readonly ctrl: boolean;
  readonly action: keyof HotkeyActions;
}

const text = strings.shortcuts;
const UP = "↑";
const DOWN = "↓";
const LEFT = "←";
const RIGHT = "→";
/** The three ways to ask for a menu: the pointer and the two of the keyboard. */
const MENU_KEYS = [[text.keys.rightClick], ["Shift", "F10"], [text.keys.menuKey]];

/** Every combination {@link installHotkeys} answers. The list the user sees takes its rows from here. */
export const GLOBAL_HOTKEYS: readonly GlobalHotkey[] = [
  // Caught by the window, or the code editor would first put an empty line into the body.
  { key: "enter", ctrl: true, action: "send", keys: [["Ctrl", "Enter"]], text: text.send },
  { key: "s", ctrl: true, action: "save", keys: [["Ctrl", "S"]], text: text.save },
  { key: "l", ctrl: true, action: "focusUrl", keys: [["Ctrl", "L"]], text: text.focusUrl },
  // Cancelled even when there is nothing to search in, or the webview would show its own search bar.
  { key: "f", ctrl: true, action: "find", keys: [["Ctrl", "F"]], text: text.find },
  { key: "w", ctrl: true, action: "closeTab", keys: [["Ctrl", "W"]], text: text.closeTab },
  // Cancelled, or the webview would open its help page.
  { key: "f1", ctrl: false, action: "shortcuts", keys: [["F1"]], text: text.shortcuts },
];

/** The row of the list for the combination of the window that does `action`. */
function globalShortcut(action: keyof HotkeyActions): Shortcut {
  const { keys, text: description } = GLOBAL_HOTKEYS.find((hotkey) => hotkey.action === action)!;
  return { keys, text: description };
}

/**
 * Everything the keyboard and the mouse do beyond the obvious, by the part of the window; shown by the dialog
 * of the key combinations. The combinations of the window come from {@link GLOBAL_HOTKEYS}; the rest is
 * answered by the tab bar (`TabBar.svelte`), the tree (`Tree.svelte`), the code editor
 * (`codemirror/setup.ts`), the fields with variables (`VarInput.svelte`), and the dialogs and menus.
 */
export const SHORTCUTS: readonly ShortcutGroup[] = [
  {
    title: text.groups.request,
    shortcuts: [globalShortcut("send"), globalShortcut("save"), globalShortcut("focusUrl"), globalShortcut("find")],
  },
  {
    title: text.groups.tabs,
    shortcuts: [
      globalShortcut("closeTab"),
      { keys: [[LEFT], [RIGHT]], text: text.tabNeighbour },
      { keys: [[text.keys.middleClick]], text: text.tabMiddleClick },
      { keys: [[text.keys.wheel]], text: text.tabWheel },
      { keys: MENU_KEYS, text: text.tabMenu },
    ],
  },
  {
    title: text.groups.tree,
    shortcuts: [
      { keys: [[UP], [DOWN]], text: text.treeMove },
      { keys: [[RIGHT]], text: text.treeExpand },
      { keys: [[LEFT]], text: text.treeCollapse },
      { keys: [["Enter"]], text: text.treeOpen },
      { keys: [["F2"]], text: text.treeRename },
      { keys: [["Delete"]], text: text.treeDelete },
      {
        keys: [
          ["Alt", UP],
          ["Alt", DOWN],
          ["Alt", LEFT],
          ["Alt", RIGHT],
        ],
        text: text.treeReorder,
      },
      { keys: MENU_KEYS, text: text.treeMenu },
    ],
  },
  {
    title: text.groups.editor,
    shortcuts: [
      { keys: [["Tab"]], text: text.editorIndent },
      { keys: [["Shift", "Tab"]], text: text.editorOutdent },
      { keys: [["Esc"], ["Tab"]], then: true, text: text.editorLeave },
      { keys: [["Ctrl", text.keys.space]], text: text.editorVariables },
      { keys: [["Ctrl", "Z"]], text: text.editorUndo },
      { keys: [["Ctrl", "Y"]], text: text.editorRedo },
      { keys: [["Ctrl", "F"]], text: text.editorFind },
    ],
  },
  {
    title: text.groups.window,
    shortcuts: [globalShortcut("shortcuts"), { keys: [["Esc"]], text: text.dismiss }],
  },
];

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
  function handler(event: KeyboardEvent) {
    // F5 and Ctrl+R (with Shift too) reload the page of the webview, which would drop the responses and the
    // edits that are not written down yet.
    const reload = event.key === "F5" || (event.ctrlKey && !event.altKey && letter(event) === "r");
    // AltGr arrives as Ctrl+Alt and types a character, so Alt rules a combination out.
    const hotkey =
      event.altKey || event.shiftKey || event.metaKey
        ? undefined
        : GLOBAL_HOTKEYS.find(({ key, ctrl }) => ctrl === event.ctrlKey && key === letter(event));
    if (!reload && !hotkey) return;
    // Inside a code editor Ctrl+F is its own: it opens the search panel of that editor.
    if (hotkey?.action === "find" && event.target instanceof Element && event.target.closest(".cm-editor") !== null) return;

    event.preventDefault();
    event.stopPropagation();
    if (hotkey && document.querySelector('[aria-modal="true"]') === null) actions[hotkey.action]();
  }

  window.addEventListener("keydown", handler, { capture: true });
  return () => window.removeEventListener("keydown", handler, { capture: true });
}

/** Where the menu of the webview stays: text fields and the text of the code editor need it for copy and paste. */
const KEEPS_CONTEXT_MENU = 'input, textarea, .cm-content, [contenteditable]:not([contenteditable="false"])';

/**
 * Cancels the right-click menu of the webview everywhere except text fields, and returns the function that
 * stops it. The menu has a "Reload" item, and a reload of the page drops the responses and the edits that are
 * not written down yet.
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
