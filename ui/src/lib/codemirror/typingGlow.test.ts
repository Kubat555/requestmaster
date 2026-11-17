import { history, redo, undo } from "@codemirror/commands";
import { Compartment, EditorSelection, EditorState, Transaction, type TransactionSpec } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import tokens from "../../styles/tokens.css?raw";
import {
  contentReplaced,
  copiedRanges,
  expireGlow,
  GLOW_LIFETIME_MS,
  GLOW_MAX_CHARS,
  glowField,
  pulseCopied,
  typingGlow,
} from "./typingGlow";

/** A mark of the glow as a test sees it: the range, the class and when it appeared. */
interface Mark {
  from: number;
  to: number;
  class: string;
  born: number;
}

function marks(state: EditorState): Mark[] {
  const found: Mark[] = [];
  const cursor = state.field(glowField).marks.iter();
  for (; cursor.value; cursor.next()) {
    found.push({ from: cursor.from, to: cursor.to, class: cursor.value.spec.class, born: cursor.value.spec.born });
  }
  return found;
}

/** The ranges of the marks with the text they cover, for a short check. */
function glowing(state: EditorState): string[] {
  return marks(state).map((mark) => state.sliceDoc(mark.from, mark.to));
}

function flashing(state: EditorState): boolean {
  return state.field(glowField).flash !== null;
}

function stateOf(doc: string, on = true): EditorState {
  return EditorState.create({ doc, extensions: [history(), EditorState.allowMultipleSelections.of(true), typingGlow(on)] });
}

/** The state after a transaction made at the moment `time`. */
function at(state: EditorState, time: number, spec: TransactionSpec): EditorState {
  return state.update({ ...spec, annotations: Transaction.time.of(time) }).state;
}

/** Types or pastes `text` at `pos`, the way the editor reports it (`userEvent`). */
function insert(state: EditorState, time: number, pos: number, text: string, userEvent = "input.type"): EditorState {
  return at(state, time, { changes: { from: pos, insert: text }, userEvent });
}

describe("typing glow: the state", () => {
  it("marks a typed letter, and only it", () => {
    let state = stateOf("ac");
    expect(marks(state)).toEqual([]);

    state = insert(state, 1000, 1, "b");

    expect(state.doc.toString()).toBe("abc");
    expect(marks(state)).toEqual([{ from: 1, to: 2, class: "cm-typed-glow", born: 1000 }]);
    expect(flashing(state)).toBe(false);
  });

  it("marks every letter typed after it with its own time", () => {
    let state = stateOf("");
    state = insert(state, 1000, 0, "a");
    state = insert(state, 1040, 1, "b");
    state = insert(state, 1080, 2, "c");

    expect(marks(state)).toEqual([
      { from: 0, to: 1, class: "cm-typed-glow", born: 1000 },
      { from: 1, to: 2, class: "cm-typed-glow", born: 1040 },
      { from: 2, to: 3, class: "cm-typed-glow", born: 1080 },
    ]);
  });

  it("marks a pasted text and a taken completion as a whole", () => {
    let state = stateOf("{}");
    state = insert(state, 1000, 1, '"pasted": 1', "input.paste");
    expect(glowing(state)).toEqual(['"pasted": 1']);

    state = at(state, 1100, { changes: { from: 12, to: 12, insert: "{{token}}" }, userEvent: "input.complete" });
    expect(glowing(state)).toEqual(['"pasted": 1', "{{token}}"]);
  });

  it("marks what replaced a selection, and every place of several carets", () => {
    let state = stateOf("one two one");
    state = at(state, 1000, { changes: { from: 4, to: 7, insert: "2" }, userEvent: "input.type" });
    expect(state.doc.toString()).toBe("one 2 one");
    expect(glowing(state)).toEqual(["2"]);

    state = stateOf("a\nb\nc");
    state = at(state, 1000, {
      changes: [
        { from: 1, insert: "!" },
        { from: 3, insert: "!" },
        { from: 5, insert: "!" },
      ],
      userEvent: "input.type",
    });
    expect(state.doc.toString()).toBe("a!\nb!\nc!");
    expect(marks(state).map(({ from, to }) => [from, to])).toEqual([
      [1, 2],
      [4, 5],
      [7, 8],
    ]);
  });

  it("does not mark what was not typed: a text set from outside, a deletion, Enter, a moved line, a composition", () => {
    let state = stateOf("abc");
    state = at(state, 1000, { changes: { from: 3, insert: "d" } });
    state = at(state, 1010, { changes: { from: 0, to: 1 }, userEvent: "delete.backward" });
    state = at(state, 1020, { changes: { from: 3, insert: "\n  " }, userEvent: "input" });
    state = at(state, 1030, { changes: { from: 0, insert: "x" }, userEvent: "move.line" });
    // A text that is still being composed keeps its DOM to itself.
    state = at(state, 1040, { changes: { from: 0, insert: "か" }, userEvent: "input.type.compose" });
    state = at(state, 1050, { selection: { anchor: 1 } });

    expect(marks(state)).toEqual([]);
    expect(flashing(state)).toBe(false);
  });

  it("moves the marks with the text when something changes before them, and drops a mark whose text is deleted", () => {
    let state = stateOf("hello");
    state = insert(state, 1000, 5, "!");
    expect(marks(state).map(({ from, to }) => [from, to])).toEqual([[5, 6]]);

    // A change before the mark that is not typed: the mark follows its letter.
    state = at(state, 1010, { changes: { from: 0, insert: ">> " } });
    expect(marks(state).map(({ from, to }) => [from, to])).toEqual([[8, 9]]);
    expect(glowing(state)).toEqual(["!"]);

    state = at(state, 1020, { changes: { from: 0, to: 3 }, userEvent: "delete.selection" });
    expect(glowing(state)).toEqual(["!"]);

    state = at(state, 1030, { changes: { from: 5, to: 6 }, userEvent: "delete.backward" });
    expect(marks(state)).toEqual([]);
    expect(state.field(glowField).marks.size).toBe(0);
  });

  it("removes the marks that are old enough and keeps the younger ones", () => {
    let state = stateOf("");
    state = insert(state, 1000, 0, "a");
    state = insert(state, 1300, 1, "b");
    state = insert(state, 1600, 2, "c");

    state = at(state, 1900, { effects: expireGlow.of(1300) });
    expect(marks(state)).toEqual([{ from: 2, to: 3, class: "cm-typed-glow", born: 1600 }]);

    state = at(state, 2200, { effects: expireGlow.of(1600) });
    expect(marks(state)).toEqual([]);
    expect(state.field(glowField).marks.size).toBe(0);
  });

  it("does not grow while a key is held: what is older than the lifetime is swept away", () => {
    let state = stateOf("");
    let most = 0;
    // 30 letters a second for ten seconds, swept the way the editor sweeps: once in a lifetime.
    for (let index = 0, swept = 0; index < 300; index++) {
      const time = index * 33;
      if (time - swept >= GLOW_LIFETIME_MS) {
        state = at(state, time, { effects: expireGlow.of(time - GLOW_LIFETIME_MS) });
        swept = time;
      }
      state = insert(state, time, index, "x");
      most = Math.max(most, state.field(glowField).marks.size);
    }

    expect(state.doc.length).toBe(300);
    // Two lifetimes of letters at the most, whatever the length of the text.
    expect(most).toBeLessThanOrEqual(Math.ceil((2 * GLOW_LIFETIME_MS) / 33) + 1);
  });

  it("flashes the text once instead of marking a long insert", () => {
    expect(GLOW_MAX_CHARS).toBe(2000);
    let state = stateOf("");
    state = insert(state, 1000, 0, "x".repeat(GLOW_MAX_CHARS), "input.paste");
    expect(marks(state).map(({ from, to }) => [from, to])).toEqual([[0, GLOW_MAX_CHARS]]);
    expect(flashing(state)).toBe(false);

    state = stateOf("");
    state = insert(state, 1000, 0, "x".repeat(GLOW_MAX_CHARS + 1), "input.paste");
    expect(marks(state)).toEqual([]);
    expect(flashing(state)).toBe(true);

    // Many short pieces that are long together are a long insert as well.
    state = stateOf("a\n".repeat(700));
    state = at(state, 1000, {
      changes: Array.from({ length: 700 }, (_, line) => ({ from: line * 2, insert: "xyz" })),
      userEvent: "input.type",
    });
    expect(marks(state)).toEqual([]);
    expect(flashing(state)).toBe(true);
  });

  it("the flash goes away with the sweep, and not before its time", () => {
    let state = stateOf("");
    state = insert(state, 1000, 0, "x".repeat(GLOW_MAX_CHARS + 1), "input.paste");

    state = at(state, 1500, { effects: expireGlow.of(900) });
    expect(flashing(state)).toBe(true);

    state = at(state, 1700, { effects: expireGlow.of(1000) });
    expect(flashing(state)).toBe(false);
  });

  it("flashes when the editor says its whole text was replaced, whatever its length", () => {
    let state = stateOf('{"a":1}');
    state = at(state, 1000, { changes: { from: 0, to: 7, insert: '{\n  "a": 1\n}' }, effects: contentReplaced.of(null) });

    expect(marks(state)).toEqual([]);
    expect(flashing(state)).toBe(true);
  });

  it("marks what undo and redo bring back", () => {
    let state = stateOf("keep ");
    state = insert(state, 1000, 5, "this");
    state = at(state, 3000, { effects: expireGlow.of(2000) });
    state = at(state, 3100, { changes: { from: 5, to: 9 }, userEvent: "delete.selection" });
    expect(state.doc.toString()).toBe("keep ");

    let restored: Transaction | undefined;
    undo({ state, dispatch: (transaction) => (restored = transaction) });
    expect(restored!.isUserEvent("undo")).toBe(true);
    state = restored!.state;
    expect(state.doc.toString()).toBe("keep this");
    expect(glowing(state)).toEqual(["this"]);

    // Undoing the typing itself takes the text away: nothing is left to glow.
    undo({ state, dispatch: (transaction) => (restored = transaction) });
    state = restored!.state;
    expect(state.doc.toString()).toBe("keep ");
    expect(marks(state)).toEqual([]);

    redo({ state, dispatch: (transaction) => (restored = transaction) });
    expect(restored!.isUserEvent("redo")).toBe(true);
    state = restored!.state;
    expect(state.doc.toString()).toBe("keep this");
    expect(glowing(state)).toEqual(["this"]);
  });

  it("gives the editor its decorations, and the class of the flash", () => {
    let state = stateOf("");
    state = insert(state, 1000, 0, "abc", "input.paste");

    const sets = state.facet(EditorView.decorations).filter((set) => typeof set !== "function");
    expect(sets).toContain(state.field(glowField).marks);
    expect(state.facet(EditorView.editorAttributes)).not.toContainEqual({ class: "cm-glow-flash" });

    state = at(state, 1100, { effects: contentReplaced.of(null) });
    expect(state.facet(EditorView.editorAttributes)).toContainEqual({ class: "cm-glow-flash" });
  });
});

describe("typing glow: the copied text", () => {
  it("pulses the selected ranges that are copied", () => {
    let state = stateOf("one two three");
    state = at(state, 1000, { selection: EditorSelection.create([EditorSelection.range(0, 3), EditorSelection.range(8, 13)]) });

    expect(copiedRanges(state)).toEqual([
      { from: 0, to: 3 },
      { from: 8, to: 13 },
    ]);
    state = at(state, 1010, { effects: pulseCopied.of(copiedRanges(state)) });

    expect(marks(state)).toEqual([
      { from: 0, to: 3, class: "cm-copied", born: 1010 },
      { from: 8, to: 13, class: "cm-copied", born: 1010 },
    ]);
    expect(state.doc.toString()).toBe("one two three");
    expect(state.selection.ranges.map(({ from, to }) => [from, to])).toEqual([
      [0, 3],
      [8, 13],
    ]);
  });

  it("pulses the line of the caret when nothing is selected: that is what the editor copies", () => {
    let state = stateOf("first\nsecond\n\nlast");
    state = at(state, 1000, { selection: { anchor: 8 } });
    expect(copiedRanges(state)).toEqual([{ from: 6, to: 12 }]);

    // An empty line has nothing to show.
    state = at(state, 1010, { selection: { anchor: 13 } });
    expect(copiedRanges(state)).toEqual([]);
    state = at(state, 1020, { effects: pulseCopied.of(copiedRanges(state)) });
    expect(marks(state)).toEqual([]);
  });

  it("the pulse is swept away like the glow of a letter", () => {
    let state = stateOf("copy me");
    state = at(state, 1000, { effects: pulseCopied.of([{ from: 0, to: 4 }]) });
    state = insert(state, 1200, 7, "!");
    expect(marks(state).map((mark) => mark.class)).toEqual(["cm-copied", "cm-typed-glow"]);

    state = at(state, 1700, { effects: expireGlow.of(1000) });
    expect(marks(state).map((mark) => mark.class)).toEqual(["cm-typed-glow"]);
  });
});

describe("typing glow: on and off", () => {
  it("is nothing at all unless it is switched on", () => {
    let state = stateOf("abc", false);
    state = insert(state, 1000, 3, "d");
    state = at(state, 1010, { effects: [contentReplaced.of(null), pulseCopied.of([{ from: 0, to: 2 }])] });

    expect(state.field(glowField, false)).toBeUndefined();
    expect(state.facet(EditorView.decorations)).toEqual([]);
    expect(state.facet(EditorView.editorAttributes)).toEqual([]);
  });

  it("is switched on and off in an open editor through its compartment", () => {
    const slot = new Compartment();
    let state = EditorState.create({ doc: "abc", extensions: slot.of(typingGlow(false)) });

    state = at(state, 1000, { effects: slot.reconfigure(typingGlow(true)) });
    state = insert(state, 1010, 3, "d");
    expect(glowing(state)).toEqual(["d"]);

    // Switched off, nothing is left behind.
    state = at(state, 1020, { effects: slot.reconfigure(typingGlow(false)) });
    expect(state.field(glowField, false)).toBeUndefined();
    expect(state.facet(EditorView.decorations)).toEqual([]);

    state = at(state, 1030, { effects: slot.reconfigure(typingGlow(true)) });
    expect(marks(state)).toEqual([]);
  });

  it("the mark of a letter lives longer than its glow lasts", () => {
    const glow = /--dur-glow:\s*(\d+)ms/.exec(tokens);
    expect(glow).not.toBeNull();
    expect(Number(glow![1])).toBeLessThanOrEqual(GLOW_LIFETIME_MS);
    const flash = /--dur-5:\s*(\d+)ms/.exec(tokens);
    expect(Number(flash![1])).toBeLessThanOrEqual(GLOW_LIFETIME_MS);
  });
});

describe("typing glow: in an editor", () => {
  let view: EditorView | undefined;
  /** How many times the editor swept its marks. The editor has timers of its own, so the sweeps are counted. */
  let sweeps = 0;

  const countSweeps = EditorView.updateListener.of((update) => {
    for (const transaction of update.transactions) {
      if (transaction.effects.some((effect) => effect.is(expireGlow))) sweeps++;
    }
  });

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(10_000);
    sweeps = 0;
  });

  afterEach(() => {
    view?.destroy();
    view = undefined;
    vi.useRealTimers();
  });

  function editor(doc: string, on = true): EditorView {
    view = new EditorView({ parent: document.body, state: EditorState.create({ doc, extensions: [typingGlow(on), countSweeps] }) });
    return view;
  }

  function type(target: EditorView, text: string) {
    target.dispatch({ changes: { from: target.state.doc.length, insert: text }, userEvent: "input.type" });
  }

  it("draws a typed letter in a span with the class, without a style attribute", () => {
    const target = editor("ab");
    type(target, "c");

    const spans = target.contentDOM.querySelectorAll(".cm-typed-glow");
    expect([...spans].map((span) => span.textContent)).toEqual(["c"]);
    expect(target.contentDOM.querySelector("[style]")).toBeNull();
  });

  it("writes its styles under the rich level of the root element only", () => {
    editor("");
    // The editor writes its styles into a style tag, a rule a line.
    const css = [...document.querySelectorAll("style")].map((style) => style.textContent ?? "").join("\n");
    const rules = css.split("\n").filter((line) => /cm-typed-glow|cm-copied|cm-glow-flash/.test(line));

    expect(rules.length).toBeGreaterThan(3);
    for (const rule of rules) {
      const selectors = rule.slice(0, rule.indexOf("{")).split(",");
      const rich = selectors.every((selector) => selector.trim().startsWith(':root[data-motion="rich"] '));
      expect(rule.startsWith("@keyframes") || rich, rule).toBe(true);
    }
    expect(css).toMatch(/@keyframes cm-typed-glow \{\s*from \{[^}]*text-shadow/);
  });

  it("takes the marks away by itself a lifetime later, and leaves nothing behind", () => {
    const target = editor("");
    type(target, "a");
    vi.advanceTimersByTime(200);
    type(target, "b");
    expect(target.state.field(glowField).marks.size).toBe(2);

    // The sweep comes a lifetime after the first letter and takes what is old enough by then.
    vi.advanceTimersByTime(GLOW_LIFETIME_MS - 200);
    expect(glowing(target.state)).toEqual(["b"]);
    expect(sweeps).toBe(1);

    vi.advanceTimersByTime(GLOW_LIFETIME_MS);
    expect(target.state.field(glowField).marks.size).toBe(0);
    expect(target.contentDOM.querySelector(".cm-typed-glow")).toBeNull();
    expect(target.state.doc.toString()).toBe("ab");
    expect(sweeps).toBe(2);

    // Nothing glows, so nothing waits.
    vi.advanceTimersByTime(GLOW_LIFETIME_MS * 10);
    expect(sweeps).toBe(2);
  });

  it("sweeps once in a lifetime however fast the letters come", () => {
    const target = editor("");
    for (let index = 0; index < 20; index++) {
      type(target, "x");
      vi.advanceTimersByTime(10);
    }
    expect(sweeps).toBe(0);

    vi.advanceTimersByTime(GLOW_LIFETIME_MS - 200);
    expect(sweeps).toBe(1);
    vi.advanceTimersByTime(GLOW_LIFETIME_MS);
    expect(sweeps).toBe(2);
    expect(target.state.field(glowField).marks.size).toBe(0);
  });

  it("takes the flash of a long paste away the same way", () => {
    const target = editor("");
    target.dispatch({ changes: { from: 0, insert: "x".repeat(GLOW_MAX_CHARS + 1) }, userEvent: "input.paste" });
    expect(target.dom.classList.contains("cm-glow-flash")).toBe(true);
    expect(target.contentDOM.querySelector(".cm-typed-glow")).toBeNull();

    vi.advanceTimersByTime(GLOW_LIFETIME_MS);
    expect(target.dom.classList.contains("cm-glow-flash")).toBe(false);
    vi.advanceTimersByTime(GLOW_LIFETIME_MS * 10);
    expect(sweeps).toBe(1);
  });

  it("pulses the selection on a copy without touching the event or the clipboard", () => {
    const target = editor("copy this text");
    target.dispatch({ selection: { anchor: 5, head: 9 } });
    const event = new Event("copy", { bubbles: true, cancelable: true });
    const clipboardData = { getData: vi.fn(), setData: vi.fn(), clearData: vi.fn() };
    Object.defineProperty(event, "clipboardData", { value: clipboardData });

    target.contentDOM.dispatchEvent(event);

    expect(marks(target.state).map(({ from, to, class: name }) => [from, to, name])).toEqual([[5, 9, "cm-copied"]]);
    expect([...target.contentDOM.querySelectorAll(".cm-copied")].map((span) => span.textContent)).toEqual(["this"]);
    // The pulse is only shown: what is copied is the business of the editor itself.
    expect(clipboardData.getData).not.toHaveBeenCalled();
    expect(target.state.selection.main.from).toBe(5);
    expect(target.state.selection.main.to).toBe(9);
    expect(target.state.doc.toString()).toBe("copy this text");

    vi.advanceTimersByTime(GLOW_LIFETIME_MS);
    expect(target.contentDOM.querySelector(".cm-copied")).toBeNull();
  });

  it("stops waiting when the glow is switched off or the editor is closed", () => {
    const slot = new Compartment();
    view = new EditorView({
      parent: document.body,
      state: EditorState.create({ doc: "", extensions: [slot.of(typingGlow(true)), countSweeps] }),
    });
    type(view, "a");

    view.dispatch({ effects: slot.reconfigure(typingGlow(false)) });
    expect(view.contentDOM.querySelector(".cm-typed-glow")).toBeNull();
    vi.advanceTimersByTime(GLOW_LIFETIME_MS * 3);
    expect(sweeps).toBe(0);

    view.dispatch({ effects: slot.reconfigure(typingGlow(true)) });
    type(view, "b");
    const dispatch = vi.spyOn(view, "dispatch");
    view.destroy();
    vi.advanceTimersByTime(GLOW_LIFETIME_MS * 3);
    expect(dispatch).not.toHaveBeenCalled();
    expect(sweeps).toBe(0);
    view = undefined;
  });

  it("does nothing in an editor where it is off", () => {
    const target = editor("abc", false);
    type(target, "d");
    target.contentDOM.dispatchEvent(new Event("copy", { bubbles: true, cancelable: true }));
    vi.advanceTimersByTime(GLOW_LIFETIME_MS * 3);

    expect(target.contentDOM.querySelector(".cm-typed-glow, .cm-copied")).toBeNull();
    expect(sweeps).toBe(0);
  });
});
