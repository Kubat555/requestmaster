// The glow of the code editor at the rich motion level: letters that were just typed, pasted or brought back by
// undo appear with a glow of the accent colour that fades; a long insert flashes the whole text softly once
// instead; a copied selection pulses once. Everything is a class on a mark or on the editor — the policy of the
// window allows no style attributes — and every mark is taken away again a little later. The work of a
// transaction is in proportion to what it inserted: the text is never scanned.

import { StateEffect, StateField, Transaction, type EditorState, type Extension } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";

/**
 * How long a mark stays, ms: the glow of a letter (`--dur-glow` of tokens.css) and the pulse and the flash
 * (`--dur-5`) have ended by then, so taking the mark away changes nothing on the screen.
 */
export const GLOW_LIFETIME_MS = 600;

/** The longest insert whose letters glow; a longer one flashes the text of the editor once. */
export const GLOW_MAX_CHARS = 2000;

/** A piece of the text. */
interface Range {
  from: number;
  to: number;
}

/** Takes away everything that appeared at this moment or before it (the time of a transaction, ms). */
export const expireGlow = StateEffect.define<number>();

/** The editor replaced its whole text with one that came from outside (Beautify, a revert): the text flashes. */
export const contentReplaced = StateEffect.define<null>();

/** These ranges were copied: they pulse once. Sent without a change of the text, so the ranges need no mapping. */
export const pulseCopied = StateEffect.define<readonly Range[]>();

/** What the user did to the text that makes the new letters glow; a composition keeps its DOM to itself. */
const GLOWING_EVENTS = ["input.type", "input.paste", "input.complete", "undo", "redo"];
const COMPOSING = "input.type.compose";

function glows(transaction: Transaction): boolean {
  return !transaction.isUserEvent(COMPOSING) && GLOWING_EVENTS.some((event) => transaction.isUserEvent(event));
}

/** The mark of the letters that appeared at the moment `born`, with the class that draws them. */
function markOf(className: string, born: number): Decoration {
  return Decoration.mark({ class: className, born });
}

/** What glows: the marks of the letters and of the copied ranges, and when the flash of the text began. */
interface Glow {
  marks: DecorationSet;
  /** The time of the transaction that made the whole text flash; `null` — it does not. */
  flash: number | null;
}

export const glowField = StateField.define<Glow>({
  create: () => ({ marks: Decoration.none, flash: null }),

  update(glow, transaction) {
    let { marks, flash } = glow;
    const time = transaction.annotation(Transaction.time) ?? 0;
    if (transaction.docChanged) marks = marks.map(transaction.changes);

    for (const effect of transaction.effects) {
      if (effect.is(expireGlow)) {
        const cutoff = effect.value;
        marks = marks.update({ filter: (_from, _to, mark) => mark.spec.born > cutoff });
        if (flash !== null && flash <= cutoff) flash = null;
      } else if (effect.is(contentReplaced)) {
        flash = time;
      } else if (effect.is(pulseCopied)) {
        const copied = markOf("cm-copied", time);
        marks = marks.update({ add: effect.value.filter(({ from, to }) => to > from).map(({ from, to }) => copied.range(from, to)) });
      }
    }

    if (transaction.docChanged && glows(transaction)) {
      const inserted: Range[] = [];
      let length = 0;
      transaction.changes.iterChangedRanges((_fromA, _toA, from, to) => {
        if (to === from) return;
        inserted.push({ from, to });
        length += to - from;
      });
      if (length > GLOW_MAX_CHARS) {
        flash = time;
      } else if (inserted.length > 0) {
        const typed = markOf("cm-typed-glow", time);
        marks = marks.update({ add: inserted.map(({ from, to }) => typed.range(from, to)) });
      }
    }

    return marks === glow.marks && flash === glow.flash ? glow : { marks, flash };
  },

  provide: (field) => [
    EditorView.decorations.from(field, (glow) => glow.marks),
    EditorView.editorAttributes.from(field, (glow): Record<string, string> => (glow.flash === null ? {} : { class: "cm-glow-flash" })),
  ],
});

/**
 * What a copy takes: the selected ranges, or — the editor copies whole lines when nothing is selected — the
 * lines of the carets that have text.
 */
export function copiedRanges(state: EditorState): Range[] {
  const selected = state.selection.ranges.filter((range) => !range.empty);
  if (selected.length > 0) return selected.map(({ from, to }) => ({ from, to }));
  const lines = new Map<number, Range>();
  for (const { head } of state.selection.ranges) {
    const line = state.doc.lineAt(head);
    if (line.length > 0) lines.set(line.from, { from: line.from, to: line.to });
  }
  return [...lines.values()].sort((a, b) => a.from - b.from);
}

/**
 * Takes the marks away when their time has passed: one timer at a time, started when something begins to glow
 * and again while something still does. Typing however fast adds no timers, and a mark lives at most two
 * lifetimes, so the set of marks does not grow while a key is held.
 */
const sweeper = ViewPlugin.define((view) => {
  let timer: ReturnType<typeof setTimeout> | null = null;

  const sweep = () => {
    timer = null;
    // The update that follows starts the next wait if something still glows.
    view.dispatch({ effects: expireGlow.of(Date.now() - GLOW_LIFETIME_MS) });
  };

  const wait = (state: EditorState) => {
    const { marks, flash } = state.field(glowField);
    if (timer === null && (marks.size > 0 || flash !== null)) timer = setTimeout(sweep, GLOW_LIFETIME_MS);
  };

  return {
    update: (update: ViewUpdate) => wait(update.state),
    destroy() {
      if (timer !== null) clearTimeout(timer);
    },
  };
});

/**
 * The copy is only seen here, never handled: the event goes on to the editor, which puts the text into the
 * clipboard itself. Nothing is read from the clipboard.
 */
const copyPulse = EditorView.domEventHandlers({
  copy(_event, view) {
    const ranges = copiedRanges(view.state);
    if (ranges.length > 0) view.dispatch({ effects: pulseCopied.of(ranges) });
    return false;
  },
});

/** Where and how long the glow is drawn; all of it under the rich level of the root element. */
const RICH = ':root[data-motion="rich"] &';
const GLOW_ANIMATION = "cm-typed-glow var(--dur-glow) ease-out";

const styles = EditorView.baseTheme({
  // A highlighted token inside the mark has a colour of its own, a mark inside a token inherits one: both
  // start from the accent and come to their colour.
  [`${RICH} .cm-typed-glow, ${RICH} .cm-typed-glow span`]: { animation: GLOW_ANIMATION },
  [`${RICH} .cm-copied`]: { borderRadius: "2px", animation: "cm-copied var(--dur-5) ease-out" },
  [`${RICH}.cm-glow-flash .cm-content`]: { animation: "cm-glow-flash var(--dur-5) ease-out" },
  "@keyframes cm-typed-glow": {
    from: { color: "var(--accent)", textShadow: "var(--glow), var(--glow)" },
  },
  "@keyframes cm-copied": {
    from: {
      backgroundColor: "color-mix(in srgb, var(--accent) 45%, transparent)",
      boxShadow: "0 0 0 1px var(--accent), var(--glow)",
    },
  },
  "@keyframes cm-glow-flash": {
    from: { opacity: "0.55", textShadow: "var(--glow)" },
  },
});

/**
 * The glow of an editor, or nothing when it is `off`: the editor keeps it in a compartment and switches it with
 * the motion level of the window.
 */
export function typingGlow(on: boolean): Extension {
  return on ? [glowField, sweeper, copyPulse, styles] : [];
}
