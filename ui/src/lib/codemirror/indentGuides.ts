// Indent guides of the code editor: a thin vertical line at every indentation level of a line. The lines are a
// background of the line element, chosen by classes — the policy of the window allows no style attributes.

import { type Extension, type Text } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";

/** The deepest level that still gets a guide. */
export const MAX_GUIDES = 12;

/** How many lines around an empty one are looked at for the block it stands in. */
const SCAN_LIMIT = 500;

/**
 * Number of whole indentation levels (`step` columns each) before the text of the line, at most `MAX_GUIDES`.
 * `null` for a line without text: its guides are the ones of the lines around it.
 */
export function guideDepth(line: string, step: number, tabSize: number): number | null {
  let columns = 0;
  for (const char of line) {
    if (char === " ") columns++;
    else if (char === "\t") columns += tabSize - (columns % tabSize);
    else return Math.min(Math.floor(columns / step), MAX_GUIDES);
  }
  return null;
}

/**
 * The lines of the visible ranges, each once and in order: where the line starts and how many guides it has.
 * An empty line continues the block around it: it has the guides of the deeper one of its nearest lines with text.
 */
export function visibleGuides(
  doc: Text,
  ranges: readonly { from: number; to: number }[],
  step: number,
  tabSize: number,
): { from: number; depth: number }[] {
  /** The depth of the nearest line with text above (`direction` -1) or below (1) the line; 0 when there is none. */
  function neighbour(number: number, direction: 1 | -1): number {
    for (let passed = 1; passed <= SCAN_LIMIT; passed++) {
      const at = number + direction * passed;
      if (at < 1 || at > doc.lines) break;
      const depth = guideDepth(doc.line(at).text, step, tabSize);
      if (depth !== null) return depth;
    }
    return 0;
  }

  const lines: { from: number; depth: number }[] = [];
  let done = 0;
  /** The depth of the run of empty lines being passed; `null` outside one. */
  let run: number | null = null;
  for (const range of ranges) {
    const last = doc.lineAt(range.to).number;
    for (let number = Math.max(doc.lineAt(range.from).number, done + 1); number <= last; number++) {
      const line = doc.line(number);
      let depth = guideDepth(line.text, step, tabSize);
      if (depth === null) depth = run ??= Math.max(neighbour(number, -1), neighbour(number, 1));
      else run = null;
      lines.push({ from: line.from, depth });
      done = number;
    }
  }
  return lines;
}

/** The depths that have guides. */
const DEPTHS = Array.from({ length: MAX_GUIDES }, (_, index) => index + 1);

/** The decoration of a line with guides, by its depth. */
const lineOf = new Map(DEPTHS.map((depth) => [depth, Decoration.line({ class: `cm-guides cm-guides-${depth}` })]));

/** The steps (columns of one level) the guides can be drawn with: the widths an indentation unit may have. */
const STEPS = [2, 4];

const GUIDE = "var(--indent-guide)";

/**
 * The guides are a striped background of the line: it starts where the text does, is as wide as the indentation
 * and takes no room, so the text is where it would be without them and they scroll with it.
 */
const styles = EditorView.baseTheme({
  ".cm-guides": {
    backgroundImage: `repeating-linear-gradient(to right, ${GUIDE} 0, ${GUIDE} 1px, transparent 1px, transparent var(--cm-guide-step))`,
    backgroundRepeat: "no-repeat",
    backgroundOrigin: "content-box",
  },
  ...Object.fromEntries(STEPS.map((step) => [`&.cm-guide-step-${step}`, { "--cm-guide-step": `${step}ch` }])),
  ...Object.fromEntries(
    DEPTHS.map((depth) => [`.cm-guides-${depth}`, { backgroundSize: `calc(var(--cm-guide-step) * ${depth}) 100%` }]),
  ),
});

/**
 * Indent guides for an editor whose indentation level is `step` columns wide (2 or 4). Only the lines on the
 * screen are decorated.
 */
export function indentGuides(step: number): Extension {
  function build(view: EditorView): DecorationSet {
    const lines = visibleGuides(view.state.doc, view.visibleRanges, step, view.state.tabSize);
    return Decoration.set(lines.flatMap((line) => lineOf.get(line.depth)?.range(line.from) ?? []));
  }

  const guides = ViewPlugin.define(
    (view) => {
      const plugin = {
        decorations: build(view),
        update(update: ViewUpdate) {
          if (update.docChanged || update.viewportChanged) plugin.decorations = build(update.view);
        },
      };
      return plugin;
    },
    { decorations: (plugin) => plugin.decorations },
  );

  return [guides, styles, EditorView.editorAttributes.of({ class: `cm-guide-step-${step}` })];
}
