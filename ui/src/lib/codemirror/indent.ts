// The indentation unit of a text, found in the text itself: Enter, Tab and the indent guides of the code editor
// follow the text that is shown, not a fixed setting.

/** The unit of a text that tells nothing about its indentation; the one Beautify writes. */
const DEFAULT_UNIT = "  ";

/** Columns a tab takes: the tab size of CodeMirror, which the editor does not change. */
const TAB_COLUMNS = 4;

/**
 * The indentation unit of the text: a tab when most of its indented lines start with tabs, otherwise four or two
 * spaces — by the smallest run of leading spaces. Lines of whitespace only and runs of an odd length (the
 * alignment of a block comment, for one) tell nothing. Two spaces when no line is indented.
 */
export function detectIndentUnit(text: string): string {
  let tabbed = 0;
  let spaced = 0;
  let smallest = Infinity;
  for (const match of text.matchAll(/^(?:\t+| +)(?=[^\s])/gm)) {
    const run = match[0];
    if (run.startsWith("\t")) {
      tabbed++;
    } else if (run.length % 2 === 0) {
      spaced++;
      smallest = Math.min(smallest, run.length);
    }
  }
  if (tabbed > spaced) return "\t";
  if (spaced === 0) return DEFAULT_UNIT;
  return smallest % 4 === 0 ? "    " : DEFAULT_UNIT;
}

/** Columns one level of the unit takes on the screen. */
export function indentColumns(unit: string): number {
  return unit === "\t" ? TAB_COLUMNS : unit.length;
}
