import { afterEach, describe, expect, it, vi } from "vitest";
import { GLOBAL_HOTKEYS, installHotkeys, SHORTCUTS, type GlobalHotkey, type HotkeyActions } from "./hotkeys";
import { strings } from "./strings";

/** Every action of the window, each a spy. Typed as the whole interface: a new action must be added here. */
function spies(): { [Action in keyof HotkeyActions]: ReturnType<typeof vi.fn<() => void>> } {
  return { save: vi.fn(), closeTab: vi.fn(), focusUrl: vi.fn(), send: vi.fn(), find: vi.fn(), shortcuts: vi.fn() };
}

/** The key event of the combination, as a keyboard sends it. */
function eventOf({ key, ctrl }: GlobalHotkey): KeyboardEvent {
  const named = key.length > 1;
  return new KeyboardEvent("keydown", {
    key: named ? key[0].toUpperCase() + key.slice(1) : key,
    code: named ? key[0].toUpperCase() + key.slice(1) : `Key${key.toUpperCase()}`,
    ctrlKey: ctrl,
    bubbles: true,
    cancelable: true,
  });
}

/** Every row of the list, whatever its group. */
const rows = SHORTCUTS.flatMap((group) => group.shortcuts);

let remove: (() => void) | undefined;

afterEach(() => {
  remove?.();
  remove = undefined;
  document.body.replaceChildren();
});

describe("installHotkeys", () => {
  it("has a combination for every action of the window, and each one does its own action only", () => {
    const actions = spies();
    remove = installHotkeys(actions);
    const names = Object.keys(actions) as (keyof HotkeyActions)[];

    expect(GLOBAL_HOTKEYS.map((hotkey) => hotkey.action).sort()).toEqual([...names].sort());
    for (const hotkey of GLOBAL_HOTKEYS) {
      const event = eventOf(hotkey);
      document.body.dispatchEvent(event);

      expect(event.defaultPrevented, hotkey.key).toBe(true);
      expect(actions[hotkey.action], hotkey.key).toHaveBeenCalledTimes(1);
    }
    for (const name of names) expect(actions[name], name).toHaveBeenCalledTimes(1);
  });

  it("opens the list of the combinations with F1 and cancels the help of the webview", () => {
    const actions = spies();
    remove = installHotkeys(actions);
    const event = new KeyboardEvent("keydown", { key: "F1", code: "F1", bubbles: true, cancelable: true });

    document.body.dispatchEvent(event);

    expect(actions.shortcuts).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);
  });

  it("leaves a combination with another modifier alone", () => {
    const actions = spies();
    remove = installHotkeys(actions);
    for (const modifier of ["altKey", "shiftKey", "metaKey"] as const) {
      for (const init of [
        { key: "F1", code: "F1" },
        { key: "s", code: "KeyS", ctrlKey: true },
      ]) {
        const event = new KeyboardEvent("keydown", { ...init, [modifier]: true, bubbles: true, cancelable: true });
        document.body.dispatchEvent(event);
        expect(event.defaultPrevented).toBe(false);
      }
    }
    // The letter without Ctrl is typed, not a combination.
    const typed = new KeyboardEvent("keydown", { key: "s", code: "KeyS", bubbles: true, cancelable: true });
    document.body.dispatchEvent(typed);

    expect(typed.defaultPrevented).toBe(false);
    for (const action of Object.values(actions)) expect(action).not.toHaveBeenCalled();
  });

  it("cancels a combination and does nothing while a modal dialog is open", () => {
    const actions = spies();
    remove = installHotkeys(actions);
    document.body.appendChild(document.createElement("div")).setAttribute("aria-modal", "true");

    for (const hotkey of GLOBAL_HOTKEYS) {
      const event = eventOf(hotkey);
      document.body.dispatchEvent(event);
      expect(event.defaultPrevented).toBe(true);
    }

    for (const action of Object.values(actions)) expect(action).not.toHaveBeenCalled();
  });
});

describe("SHORTCUTS", () => {
  it("lists every combination the window answers, with the keys and the text it is defined with", () => {
    for (const hotkey of GLOBAL_HOTKEYS) {
      const listed = rows.filter((row) => row.keys === hotkey.keys);

      expect(listed, hotkey.key).toHaveLength(1);
      expect(listed[0].text).toBe(hotkey.text);
    }
  });

  it("names the keys of a combination of the window as the listener reads them", () => {
    for (const { key, ctrl, keys } of GLOBAL_HOTKEYS) {
      expect(keys).toEqual([ctrl ? ["Ctrl", key[0].toUpperCase() + key.slice(1)] : [key.toUpperCase()]]);
    }
  });

  it("has the five groups, and every row has keys and a text", () => {
    const groups = strings.shortcuts.groups;

    expect(SHORTCUTS.map((group) => group.title)).toEqual([groups.request, groups.tabs, groups.tree, groups.editor, groups.window]);
    for (const row of rows) {
      expect(row.text).not.toBe("");
      expect(row.keys.length).toBeGreaterThan(0);
      for (const combination of row.keys) {
        expect(combination.length).toBeGreaterThan(0);
        for (const key of combination) expect(key).not.toBe("");
      }
    }
  });
});
