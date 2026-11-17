import { describe, expect, it } from "vitest";
import { editBetween, glowPieces, glowsAfter, MAX_FIELD_GLOWS, type FieldGlow } from "./fieldGlow";

describe("editBetween", () => {
  it("finds a typed letter at the caret", () => {
    expect(editBetween("helo", "hello", 4)).toEqual({ from: 3, removed: 0, inserted: 1 });
    expect(editBetween("", "a", 1)).toEqual({ from: 0, removed: 0, inserted: 1 });
    expect(editBetween("abc", "abcd", 4)).toEqual({ from: 3, removed: 0, inserted: 1 });
  });

  it("tells a letter typed among the same letters by the caret", () => {
    expect(editBetween("aaa", "aaaa", 1)).toEqual({ from: 0, removed: 0, inserted: 1 });
    expect(editBetween("aaa", "aaaa", 3)).toEqual({ from: 2, removed: 0, inserted: 1 });
    expect(editBetween("aaa", "aaaa", 4)).toEqual({ from: 3, removed: 0, inserted: 1 });
  });

  it("finds a paste over a selection", () => {
    expect(editBetween("http://old.test/x", "http://new.example/x", 18)).toEqual({ from: 7, removed: 8, inserted: 11 });
  });

  it("finds a deletion: nothing is inserted", () => {
    expect(editBetween("hello", "helo", 3)).toEqual({ from: 3, removed: 1, inserted: 0 });
    expect(editBetween("hello", "", 0)).toEqual({ from: 0, removed: 5, inserted: 0 });
  });

  it("falls back to the common ends of the texts when the caret is not after the new letters", () => {
    expect(editBetween("one three", "one two three", 0)).toEqual({ from: 3, removed: 0, inserted: 4 });
    expect(editBetween("abc", "abc", 1)).toEqual({ from: 1, removed: 0, inserted: 0 });
    expect(editBetween("abc", "xyz", 99)).toEqual({ from: 0, removed: 3, inserted: 3 });
  });
});

describe("glowsAfter", () => {
  const glow = (id: number, from: number, to: number): FieldGlow => ({ id, from, to });

  it("adds the new letters as a piece of their own, in the order of the text", () => {
    expect(glowsAfter([], { from: 2, removed: 0, inserted: 1 }, 1)).toEqual([glow(1, 2, 3)]);
    expect(glowsAfter([glow(1, 2, 3)], { from: 3, removed: 0, inserted: 1 }, 2)).toEqual([glow(1, 2, 3), glow(2, 3, 4)]);
    // Typed before a piece: the piece moves, the new one stands first.
    expect(glowsAfter([glow(1, 2, 3)], { from: 0, removed: 0, inserted: 2 }, 2)).toEqual([glow(2, 0, 2), glow(1, 4, 5)]);
    // Typed right before a piece, at its first letter.
    expect(glowsAfter([glow(1, 2, 3)], { from: 2, removed: 0, inserted: 1 }, 2)).toEqual([glow(2, 2, 3), glow(1, 3, 4)]);
  });

  it("adds nothing for a deletion and moves what stands after it", () => {
    expect(glowsAfter([glow(1, 0, 1), glow(2, 5, 6)], { from: 2, removed: 2, inserted: 0 }, 3)).toEqual([glow(1, 0, 1), glow(2, 3, 4)]);
  });

  it("drops a piece whose letters are deleted and keeps the start of one that is cut", () => {
    expect(glowsAfter([glow(1, 2, 4)], { from: 2, removed: 2, inserted: 0 }, 2)).toEqual([]);
    expect(glowsAfter([glow(1, 2, 6)], { from: 4, removed: 5, inserted: 0 }, 2)).toEqual([glow(1, 2, 4)]);
    // Replaced by new letters: those glow instead.
    expect(glowsAfter([glow(1, 2, 4)], { from: 1, removed: 4, inserted: 2 }, 2)).toEqual([glow(2, 1, 3)]);
  });

  it("lets letters typed inside a piece glow with it", () => {
    expect(glowsAfter([glow(1, 2, 8)], { from: 5, removed: 0, inserted: 1 }, 2)).toEqual([glow(1, 2, 9)]);
  });

  it("never holds more than the limit: the oldest pieces stop", () => {
    let glows: FieldGlow[] = [];
    for (let index = 0; index < MAX_FIELD_GLOWS + 10; index++) {
      glows = glowsAfter(glows, { from: index, removed: 0, inserted: 1 }, index + 1);
    }

    expect(glows).toHaveLength(MAX_FIELD_GLOWS);
    expect(glows[0]).toEqual(glow(11, 10, 11));
    expect(glows.at(-1)).toEqual(glow(MAX_FIELD_GLOWS + 10, MAX_FIELD_GLOWS + 9, MAX_FIELD_GLOWS + 10));
  });
});

describe("glowPieces", () => {
  it("is nothing for a text without glowing letters", () => {
    expect(glowPieces("hello", 0, [])).toBeNull();
    expect(glowPieces("hello", 10, [{ id: 1, from: 0, to: 10 }])).toBeNull();
    expect(glowPieces("hello", 0, [{ id: 1, from: 5, to: 9 }])).toBeNull();
  });

  it("cuts the text at the edges of the glowing letters", () => {
    expect(
      glowPieces("hello world", 0, [
        { id: 1, from: 4, to: 5 },
        { id: 2, from: 5, to: 6 },
      ]),
    ).toEqual([
      { key: "t0", text: "hell", glow: null },
      { key: "g1", text: "o", glow: 1 },
      { key: "g2", text: " ", glow: 2 },
      { key: "t3", text: "world", glow: null },
    ]);
  });

  it("takes of a piece only what lies in the text: the text may start anywhere in the field", () => {
    // The field is `ab{{host}}cd`, this text is the token; the piece covers `b{{h`.
    expect(glowPieces("{{host}}", 2, [{ id: 7, from: 1, to: 5 }])).toEqual([
      { key: "g7", text: "{{h", glow: 7 },
      { key: "t1", text: "ost}}", glow: null },
    ]);
    expect(glowPieces("ab", 0, [{ id: 7, from: 1, to: 5 }])).toEqual([
      { key: "t0", text: "a", glow: null },
      { key: "g7", text: "b", glow: 7 },
    ]);
  });

  it("puts every letter of the text into exactly one piece", () => {
    const glows = [
      { id: 1, from: 0, to: 2 },
      { id: 2, from: 3, to: 4 },
      { id: 3, from: 9, to: 20 },
    ];
    expect(
      glowPieces("0123456789", 0, glows)!
        .map((piece) => piece.text)
        .join(""),
    ).toBe("0123456789");
  });
});
