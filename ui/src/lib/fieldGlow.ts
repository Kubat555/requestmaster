// The glow of the letters just typed into a one-line field that draws its own text (`VarInput.svelte`), at the
// rich motion level: which letters an edit brought, where the letters that still glow are after it, and how
// the text of the field is cut into glowing and plain pieces. The code editor has its own (codemirror/typingGlow.ts).

/** Letters that glow: a piece of the text of the field. The id stays the same while the piece moves with the text. */
export interface FieldGlow {
  id: number;
  from: number;
  to: number;
}

/** The most pieces that glow at once; with more, the oldest stop. */
export const MAX_FIELD_GLOWS = 64;

/** What an edit did to a text: `removed` letters at `from` gave way to `inserted` new ones. */
export interface FieldEdit {
  from: number;
  removed: number;
  inserted: number;
}

function commonPrefix(a: string, b: string, limit: number): number {
  let length = 0;
  while (length < limit && a.charCodeAt(length) === b.charCodeAt(length)) length++;
  return length;
}

function commonSuffix(a: string, b: string, limit: number): number {
  let length = 0;
  while (length < limit && a.charCodeAt(a.length - 1 - length) === b.charCodeAt(b.length - 1 - length)) length++;
  return length;
}

/**
 * The edit that turned `before` into `after`. Typing and pasting leave the caret right after the new letters,
 * so everything after `caret` is the old text — that tells a letter typed among the same letters from its
 * neighbours. When the caret is somewhere else, the edit is what lies between the common ends of the two texts.
 */
export function editBetween(before: string, after: string, caret: number): FieldEdit {
  let tail = after.length - caret;
  if (tail < 0 || tail > before.length || after.slice(caret) !== before.slice(before.length - tail)) {
    tail = commonSuffix(before, after, Math.min(before.length, after.length));
  }
  const from = commonPrefix(before, after, Math.min(before.length, after.length) - tail);
  return { from, removed: before.length - tail - from, inserted: after.length - tail - from };
}

/**
 * The glowing pieces after the edit, in the order of the text. A piece before the edit stays, one after it
 * moves, one that lost letters keeps what is left before the edit; letters put inside a piece join it. The
 * new letters are a piece of their own with the id `nextId`.
 */
export function glowsAfter(glows: readonly FieldGlow[], edit: FieldEdit, nextId: number): FieldGlow[] {
  const { from, removed, inserted } = edit;
  const end = from + removed;
  const shift = inserted - removed;
  const kept: FieldGlow[] = [];
  let joined = false;
  for (const glow of glows) {
    if (glow.to <= from) {
      kept.push(glow);
    } else if (glow.from >= end) {
      kept.push({ id: glow.id, from: glow.from + shift, to: glow.to + shift });
    } else if (removed === 0) {
      // The edit is inside the piece: the new letters glow with it.
      kept.push({ id: glow.id, from: glow.from, to: glow.to + inserted });
      joined = true;
    } else if (glow.from < from) {
      kept.push({ id: glow.id, from: glow.from, to: from });
    }
  }
  if (inserted > 0 && !joined) {
    const at = kept.findIndex((glow) => glow.from >= from);
    kept.splice(at === -1 ? kept.length : at, 0, { id: nextId, from, to: from + inserted });
  }
  // The oldest pieces have the smallest ids.
  while (kept.length > MAX_FIELD_GLOWS) {
    kept.splice(kept.indexOf(kept.reduce((oldest, glow) => (glow.id < oldest.id ? glow : oldest))), 1);
  }
  return kept;
}

/** A piece of a text as it is drawn: plain (`glow` is `null`) or glowing; the key tells the pieces of one text apart. */
export interface GlowPiece {
  key: string;
  text: string;
  glow: number | null;
}

/**
 * Cuts the text that starts at `start` of the field into plain and glowing pieces; `null` when none of it
 * glows — it is then drawn as one text, the way a field without the glow draws it.
 */
export function glowPieces(text: string, start: number, glows: readonly FieldGlow[]): GlowPiece[] | null {
  const end = start + text.length;
  const pieces: GlowPiece[] = [];
  let at = start;
  const plain = (to: number) => pieces.push({ key: `t${pieces.length}`, text: text.slice(at - start, to - start), glow: null });
  for (const glow of glows) {
    if (glow.to <= at) continue;
    if (glow.from >= end) break;
    const from = Math.max(glow.from, at);
    const to = Math.min(glow.to, end);
    if (from > at) plain(from);
    pieces.push({ key: `g${glow.id}`, text: text.slice(from - start, to - start), glow: glow.id });
    at = to;
  }
  if (pieces.length === 0) return null;
  if (at < end) plain(end);
  return pieces;
}
