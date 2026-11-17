import { acceptCompletion } from "@codemirror/autocomplete";
import { indentWithTab } from "@codemirror/commands";
import { json } from "@codemirror/lang-json";
import { getIndentUnit } from "@codemirror/language";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, keymap, runScopeHandlers } from "@codemirror/view";
import { afterEach, describe, expect, it } from "vitest";
import { icons } from "../icons";
import { baseSetup, foldMarker, indentation, tabIndentation } from "./setup";

let view: EditorView | undefined;

afterEach(() => {
  view?.destroy();
  view = undefined;
});

/** An editor in the page; `|` in the text is the caret, two of them are the ends of the selection. */
function editor(marked: string, extensions: Extension): EditorView {
  view?.destroy();
  const parts = marked.split("|");
  const anchor = parts[0].length;
  const head = parts.length > 2 ? anchor + parts[1].length : anchor;
  view = new EditorView({
    parent: document.body,
    state: EditorState.create({ doc: parts.join(""), selection: { anchor, head }, extensions }),
  });
  return view;
}

/** Presses the key in the editor; tells whether a binding of the editor took it. */
function press(target: EditorView, key: string, shiftKey = false): boolean {
  return runScopeHandlers(target, new KeyboardEvent("keydown", { key, shiftKey }), "editor");
}

describe("foldMarker", () => {
  it("draws the chevron of the icon set: down for an open block, right for a folded one", () => {
    for (const [open, name] of [
      [true, "chevron-down"],
      [false, "chevron-right"],
    ] as const) {
      const marker = foldMarker(open);
      expect(marker.className).toBe("cm-fold-marker");
      const svg = marker.querySelector("svg")!;
      expect(svg.namespaceURI).toBe("http://www.w3.org/2000/svg");
      expect(svg.getAttribute("aria-hidden")).toBe("true");
      expect([...svg.querySelectorAll("path")].map((path) => path.getAttribute("d"))).toEqual([...icons[name]]);
      expect(marker.textContent).toBe("");
      // The policy of the window drops style attributes.
      expect(marker.hasAttribute("style")).toBe(false);
      expect(marker.querySelector("[style]")).toBeNull();
    }
  });
});

describe("baseSetup", () => {
  it("has exactly one fold gutter and one gutter of line numbers", () => {
    const shown = editor('{\n  "a": [\n    1\n  ]\n}', [baseSetup, json()]);
    expect(shown.dom.querySelectorAll(".cm-foldGutter").length).toBe(1);
    expect(shown.dom.querySelectorAll(".cm-lineNumbers").length).toBe(1);
  });

  it("leaves Tab to the window", () => {
    const shown = editor('{\n  "a": 1|\n}', [baseSetup, json()]);
    expect(press(shown, "Tab")).toBe(false);
    expect(press(shown, "Tab", true)).toBe(false);
    expect(shown.state.doc.toString()).toBe('{\n  "a": 1\n}');
  });

  it("leaves Escape unhandled when nothing of the editor is open, so that Tab after it leaves the editor", () => {
    const shown = editor('{\n  "a": 1|\n}', [baseSetup, tabIndentation, json()]);
    expect(press(shown, "Escape")).toBe(false);
  });
});

describe("tabIndentation", () => {
  it("indents the line with Tab and takes the indentation back with Shift+Tab", () => {
    const shown = editor('{\n"a": 1|\n}', [baseSetup, tabIndentation, indentation("    ", false)]);
    expect(press(shown, "Tab")).toBe(true);
    expect(shown.state.doc.toString()).toBe('{\n    "a": 1\n}');
    expect(press(shown, "Tab", true)).toBe(true);
    expect(shown.state.doc.toString()).toBe('{\n"a": 1\n}');
  });

  it("indents every selected line", () => {
    const shown = editor("a\n|b\nc|\nd", [baseSetup, tabIndentation, indentation("  ", false)]);
    press(shown, "Tab");
    expect(shown.state.doc.toString()).toBe("a\n  b\n  c\nd");
    press(shown, "Tab", true);
    expect(shown.state.doc.toString()).toBe("a\nb\nc\nd");
  });

  it("changes nothing in a text that is read only", () => {
    const shown = editor("a|", [baseSetup, tabIndentation, EditorState.readOnly.of(true)]);
    expect(press(shown, "Tab")).toBe(false);
    expect(shown.state.doc.toString()).toBe("a");
  });

  // jsdom has no layout, so the list of the autocompletion never opens in a test: the order of the bindings is
  // what gives Tab to an open list first.
  it("offers Tab to the list of the autocompletion before indenting", () => {
    const bindings = EditorState.create({ extensions: tabIndentation }).facet(keymap).flat();
    expect(bindings.filter((binding) => binding.key === "Tab")).toEqual([{ key: "Tab", run: acceptCompletion }, indentWithTab]);
  });
});

describe("indentation", () => {
  it("sets the unit of the editor", () => {
    const columns = (unit: string) => getIndentUnit(EditorState.create({ extensions: indentation(unit, false) }));
    expect(columns("  ")).toBe(2);
    expect(columns("    ")).toBe(4);
    expect(columns("\t")).toBe(4);
  });

  it("marks the editor with the step of the guides only when they are asked for", () => {
    expect(editor("a", indentation("    ", true)).dom.classList.contains("cm-guide-step-4")).toBe(true);
    expect(editor("a", indentation("\t", true)).dom.classList.contains("cm-guide-step-4")).toBe(true);
    expect(editor("a", indentation("  ", true)).dom.classList.contains("cm-guide-step-2")).toBe(true);
    expect(editor("a", indentation("  ", false)).dom.className).not.toContain("cm-guide-step");
  });

  it("puts the classes of the guides on the indented lines, without style attributes", () => {
    const shown = editor('{\n  "a": [\n    1\n  ]\n}', indentation("  ", true));
    const classes = [...shown.contentDOM.querySelectorAll(".cm-line")].map((line) =>
      [...line.classList].filter((name) => name.startsWith("cm-guides")).join(" "),
    );
    expect(classes).toEqual(["", "cm-guides cm-guides-1", "cm-guides cm-guides-2", "cm-guides cm-guides-1", ""]);
    expect(shown.contentDOM.querySelector(".cm-line[style]")).toBeNull();
  });
});
