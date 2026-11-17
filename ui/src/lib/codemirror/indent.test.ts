import { insertNewlineAndIndent } from "@codemirror/commands";
import { json } from "@codemirror/lang-json";
import { indentUnit } from "@codemirror/language";
import { EditorState, type Transaction } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { detectIndentUnit, indentColumns } from "./indent";

describe("detectIndentUnit", () => {
  it("is two spaces for a text without indented lines", () => {
    expect(detectIndentUnit("")).toBe("  ");
    expect(detectIndentUnit('{"a":1}')).toBe("  ");
    expect(detectIndentUnit("one\ntwo\n\n")).toBe("  ");
  });

  it("is the smallest run of leading spaces", () => {
    expect(detectIndentUnit('{\n  "a": [\n    1\n  ]\n}')).toBe("  ");
    expect(detectIndentUnit('{\n    "a": [\n        1\n    ]\n}')).toBe("    ");
    // The shallowest line may come after deeper ones.
    expect(detectIndentUnit("a\n        b\n    c")).toBe("    ");
    expect(detectIndentUnit("a\n      b")).toBe("  ");
    expect(detectIndentUnit("a\n        b")).toBe("    ");
  });

  it("is a tab when the lines are indented with tabs", () => {
    expect(detectIndentUnit('{\n\t"a": [\n\t\t1\n\t]\n}')).toBe("\t");
    // A stray tab does not outvote the spaces.
    expect(detectIndentUnit("a\n  b\n  c\n\td")).toBe("  ");
  });

  it("does not take the alignment of a comment or an empty indented line for a level", () => {
    expect(detectIndentUnit("/**\n * text\n */\nfunction f() {\n    return 1;\n}")).toBe("    ");
    expect(detectIndentUnit("a\n  \n    b")).toBe("    ");
    expect(detectIndentUnit("a\r\n    b\r\n")).toBe("    ");
  });
});

describe("indentColumns", () => {
  it("is the width of the unit, a tab being four columns", () => {
    expect(indentColumns("  ")).toBe(2);
    expect(indentColumns("    ")).toBe(4);
    expect(indentColumns("\t")).toBe(4);
  });
});

/** The text and the caret after Enter is pressed at `|` of the JSON text, the unit detected from the text. */
function enter(marked: string): string {
  const caret = marked.indexOf("|");
  const doc = marked.replace("|", "");
  let state = EditorState.create({
    doc,
    selection: { anchor: caret },
    extensions: [json(), indentUnit.of(detectIndentUnit(doc))],
  });
  insertNewlineAndIndent({ state, dispatch: (transaction: Transaction) => (state = transaction.state) });
  const head = state.selection.main.head;
  return `${state.sliceDoc(0, head)}|${state.sliceDoc(head)}`;
}

describe("Enter in JSON", () => {
  it("keeps the indentation of the block", () => {
    expect(enter('{\n  "a": 1,|\n  "b": 2\n}')).toBe('{\n  "a": 1,\n  |\n  "b": 2\n}');
    expect(enter('{\n    "a": 1,|\n    "b": 2\n}')).toBe('{\n    "a": 1,\n    |\n    "b": 2\n}');
  });

  it("goes one level deeper after an opening bracket, by the unit of the text", () => {
    expect(enter('{\n  "a": {|\n}')).toBe('{\n  "a": {\n    |\n}');
    expect(enter('{\n    "a": {|\n}')).toBe('{\n    "a": {\n        |\n}');
    expect(enter('{\n    "a": [|\n}')).toBe('{\n    "a": [\n        |\n}');
    expect(enter('{\n\t"a": {|\n}')).toBe('{\n\t"a": {\n\t\t|\n}');
  });

  it("puts the closing bracket on a line of its own between a pair", () => {
    expect(enter('{\n    "a": {|}\n}')).toBe('{\n    "a": {\n        |\n    }\n}');
    expect(enter("[|]")).toBe("[\n  |\n]");
  });
});
