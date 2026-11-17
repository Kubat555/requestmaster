import { Text } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { guideDepth, MAX_GUIDES, visibleGuides } from "./indentGuides";

describe("guideDepth", () => {
  it("counts the levels of the leading whitespace", () => {
    expect(guideDepth('"a": 1', 2, 4)).toBe(0);
    expect(guideDepth('  "a": 1', 2, 4)).toBe(1);
    expect(guideDepth('    "a": 1', 2, 4)).toBe(2);
    expect(guideDepth('    "a": 1', 4, 4)).toBe(1);
    // A level that is not complete has no guide.
    expect(guideDepth("   x", 2, 4)).toBe(1);
    expect(guideDepth("   x", 4, 4)).toBe(0);
  });

  it("takes a tab to the next tab stop", () => {
    expect(guideDepth("\t\tx", 4, 4)).toBe(2);
    expect(guideDepth("  \tx", 4, 4)).toBe(1);
    expect(guideDepth("\tx", 2, 4)).toBe(2);
  });

  it("has no depth of its own for a line without text", () => {
    expect(guideDepth("", 2, 4)).toBeNull();
    expect(guideDepth("    ", 2, 4)).toBeNull();
    expect(guideDepth("\t", 4, 4)).toBeNull();
  });

  it("is capped", () => {
    expect(guideDepth(`${" ".repeat(100)}x`, 2, 4)).toBe(MAX_GUIDES);
  });
});

/** The depths of the lines of the visible part of the text (all of it unless `ranges` are given). */
function depths(text: string, step = 2): number[] {
  const doc = Text.of(text.split("\n"));
  return visibleGuides(doc, [{ from: 0, to: doc.length }], step, 4).map((line) => line.depth);
}

describe("visibleGuides", () => {
  it("gives the depth of every visible line", () => {
    expect(depths('{\n  "a": [\n    1\n  ]\n}')).toEqual([0, 1, 2, 1, 0]);
    expect(depths('{\n    "a": [\n        1\n    ]\n}', 4)).toEqual([0, 1, 2, 1, 0]);
  });

  it("continues the guides of the block through its empty lines", () => {
    expect(depths('{\n  "a": 1,\n\n  "b": 2\n}')).toEqual([0, 1, 1, 1, 0]);
    // Right after the opening bracket and right before the closing one.
    expect(depths('{\n\n  "a": 1\n}')).toEqual([0, 1, 1, 0]);
    expect(depths('{\n  "a": 1\n\n\n}')).toEqual([0, 1, 1, 1, 0]);
    // A line of spaces only is an empty one.
    expect(depths('{\n  "a": [\n      \n    1\n  ]\n}')).toEqual([0, 1, 2, 2, 1, 0]);
    expect(depths("\n\na\n\n")).toEqual([0, 0, 0, 0, 0]);
  });

  it("looks at the lines around the visible part for an empty line at its edge", () => {
    const doc = Text.of('{\n  "a": [\n\n\n    1\n  ]\n}'.split("\n"));
    const third = doc.line(3);
    const fourth = doc.line(4);
    expect(visibleGuides(doc, [{ from: third.from, to: fourth.to }], 2, 4)).toEqual([
      { from: third.from, depth: 2 },
      { from: fourth.from, depth: 2 },
    ]);
  });

  it("gives a line once when two visible ranges meet in it", () => {
    const doc = Text.of("a\n  b\n  c".split("\n"));
    const second = doc.line(2);
    const ranges = [
      { from: 0, to: second.from + 1 },
      { from: second.from + 2, to: doc.length },
    ];
    expect(visibleGuides(doc, ranges, 2, 4).map((line) => line.from)).toEqual([0, second.from, doc.line(3).from]);
  });
});
