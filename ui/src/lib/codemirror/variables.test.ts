import { CompletionContext, type Completion } from "@codemirror/autocomplete";
import { EditorState, type TransactionSpec } from "@codemirror/state";
import { EditorView, type DecorationSet } from "@codemirror/view";
import { describe, expect, it } from "vitest";
import { strings } from "../strings";
import type { ScopeVariable } from "../types";
import { variableCompletions, variableMarks, variableSupport } from "./variables";

const SECRET_VALUE = "s3cret-value";

function variable(name: string, more: Partial<ScopeVariable> = {}): ScopeVariable {
  return { name, scope: "environment", value: "v", secret: false, ...more };
}

const VARIABLES: ScopeVariable[] = [
  variable("id", { scope: "collection", value: "7" }),
  variable("token", { value: null, secret: true }),
  variable("tokenUrl", { value: "https://auth.test/token" }),
  variable("host", { scope: "global" }),
];

function lookup(name: string): ScopeVariable | null {
  return VARIABLES.find((item) => item.name === name) ?? null;
}

/** What the source offers for the text with the caret at `pos`. */
function complete(text: string, pos: number) {
  const state = EditorState.create({ doc: text });
  return variableCompletions(VARIABLES)(new CompletionContext(state, pos, false));
}

/** Chooses the option `label` offered at `pos` of the text; gives the text and the caret after it. */
function choose(text: string, pos: number, label: string): { text: string; caret: number } {
  let state = EditorState.create({ doc: text });
  const result = variableCompletions(VARIABLES)(new CompletionContext(state, pos, false))!;
  const option = result.options.find((item) => item.label === label)!;
  const view = {
    get state() {
      return state;
    },
    dispatch(spec: TransactionSpec) {
      state = state.update(spec).state;
    },
  } as unknown as EditorView;
  (option.apply as (view: EditorView, completion: Completion, from: number, to: number) => void)(view, option, result.from, pos);
  return { text: state.doc.toString(), caret: state.selection.main.head };
}

describe("variableMarks", () => {
  it("marks a found token with its scope and one that is not found as unresolved, with exact edges", () => {
    expect(variableMarks('{"id": {{id}}, "x": "{{nope}}"}', lookup)).toEqual([
      { from: 7, to: 13, class: "cm-var cm-var-collection" },
      { from: 21, to: 29, class: "cm-var cm-var-unresolved" },
    ]);
  });

  it("marks nothing in a text without tokens", () => {
    expect(variableMarks('{"id": 1}', lookup)).toEqual([]);
  });
});

describe("variableCompletions", () => {
  it("offers the variables that fit the name typed after `{{`, from the start of the name", () => {
    const result = complete("{{to", 4)!;

    expect(result.from).toBe(2);
    expect(result.options.map((option) => option.label)).toEqual(["token", "tokenUrl"]);
  });

  it("offers nothing outside a `{{`", () => {
    expect(complete("abc", 3)).toBeNull();
  });

  it("offers nothing when no name fits", () => {
    expect(complete("{{zzz", 5)).toBeNull();
  });

  it("finds the name on the line of the caret", () => {
    const result = complete('{\n  "a": "{{ho', 14)!;

    expect(result.from).toBe(12);
    expect(result.options.map((option) => option.label)).toEqual(["host"]);
  });

  it("shows a mask instead of the value of a secret variable", () => {
    const secret: ScopeVariable[] = [variable("token", { value: SECRET_VALUE, secret: true })];
    const state = EditorState.create({ doc: "{{to" });
    const result = variableCompletions(secret)(new CompletionContext(state, 4, false))!;

    expect(result.options).toHaveLength(1);
    expect(result.options[0].detail).toContain(strings.variables.secretMask);
    expect(JSON.stringify(result.options)).not.toContain(SECRET_VALUE);
  });

  it("says the scope and the value of a variable that is not secret", () => {
    const option = complete("{{tokenU", 8)!.options[0];

    expect(option.detail).toContain(strings.variables.scopes.environment);
    expect(option.detail).toContain("https://auth.test/token");
  });

  it("closes the token it puts in", () => {
    expect(choose("{{to", 4, "tokenUrl")).toEqual({ text: "{{tokenUrl}}", caret: 12 });
  });

  it("does not double the braces that are already there", () => {
    expect(choose("{{}}", 2, "host")).toEqual({ text: "{{host}}", caret: 8 });
    expect(choose('"{{to}}" x', 5, "token")).toEqual({ text: '"{{token}}" x', caret: 10 });
  });
});

describe("variableSupport", () => {
  /** The classes of the marks the state of an editor draws, with their edges. */
  function drawn(state: EditorState): { from: number; to: number; class: string }[] {
    const found: { from: number; to: number; class: string }[] = [];
    for (const source of state.facet(EditorView.decorations)) {
      (source as DecorationSet).between(0, state.doc.length, (from, to, mark) => {
        found.push({ from, to, class: mark.spec.class });
      });
    }
    return found;
  }

  it("marks the tokens of the text and marks them anew when the text changes", () => {
    const state = EditorState.create({ doc: "{{id}} {{nope}}", extensions: variableSupport(VARIABLES) });

    expect(drawn(state)).toEqual([
      { from: 0, to: 6, class: "cm-var cm-var-collection" },
      { from: 7, to: 15, class: "cm-var cm-var-unresolved" },
    ]);

    const changed = state.update({ changes: { from: 0, to: 6, insert: "x {{host}}" } }).state;

    expect(drawn(changed)).toEqual([
      { from: 2, to: 10, class: "cm-var cm-var-global" },
      { from: 11, to: 19, class: "cm-var cm-var-unresolved" },
    ]);
  });

  it("gives the autocompletion one and the same source every time it asks", () => {
    const state = EditorState.create({ doc: "{{to", extensions: variableSupport(VARIABLES) });

    const sources = state.languageDataAt("autocomplete", 4);

    expect(sources).toHaveLength(1);
    expect(state.languageDataAt("autocomplete", 4)[0]).toBe(sources[0]);
  });
});
