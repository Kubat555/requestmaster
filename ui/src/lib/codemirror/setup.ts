// What every code editor of the window is made of. The list is the `basicSetup` of the `codemirror` package
// with the fold gutter given the chevrons of the interface and without the key bindings of the linter (there is
// no linter). Next to it — the pieces that differ between editors: the Tab key and the indentation.

import {
  acceptCompletion,
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
} from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import {
  bracketMatching,
  defaultHighlightStyle,
  foldGutter,
  foldKeymap,
  indentOnInput,
  indentUnit,
  syntaxHighlighting,
} from "@codemirror/language";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { EditorState, type Extension } from "@codemirror/state";
import {
  crosshairCursor,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
} from "@codemirror/view";
import { icons } from "../icons";
import { indentColumns } from "./indent";
import { indentGuides } from "./indentGuides";

const SVG = "http://www.w3.org/2000/svg";

/** The attributes `Icon.svelte` draws an icon with. */
const ICON_ATTRIBUTES = {
  width: "16",
  height: "16",
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  "stroke-width": "1.6",
  "stroke-linecap": "round",
  "stroke-linejoin": "round",
  "aria-hidden": "true",
};

/**
 * The marker of the fold gutter: the chevron of the icon set, pointing down at a block that is open and right
 * at a folded one. Its colour and place are set by the theme of the editor (`.cm-fold-marker`).
 */
export function foldMarker(open: boolean): HTMLElement {
  const marker = document.createElement("span");
  marker.className = "cm-fold-marker";
  const svg = marker.appendChild(document.createElementNS(SVG, "svg"));
  for (const [name, value] of Object.entries(ICON_ATTRIBUTES)) svg.setAttribute(name, value);
  for (const d of icons[open ? "chevron-down" : "chevron-right"]) {
    svg.appendChild(document.createElementNS(SVG, "path")).setAttribute("d", d);
  }
  return marker;
}

/** The extensions every code editor has, whatever it shows. */
export const baseSetup: Extension = [
  lineNumbers(),
  highlightActiveLineGutter(),
  highlightSpecialChars(),
  history(),
  foldGutter({ markerDOM: foldMarker }),
  drawSelection(),
  dropCursor(),
  EditorState.allowMultipleSelections.of(true),
  indentOnInput(),
  syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
  bracketMatching(),
  closeBrackets(),
  autocompletion(),
  rectangularSelection(),
  crosshairCursor(),
  highlightActiveLine(),
  highlightSelectionMatches(),
  keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...searchKeymap, ...historyKeymap, ...foldKeymap, ...completionKeymap]),
];

/**
 * Tab for an editor whose text can be changed: it takes the chosen option of an open autocompletion list,
 * otherwise indents the line of the caret or the selected lines; Shift+Tab takes one level of indentation back.
 * Escape and then Tab still leaves the editor. Without this extension Tab moves the focus on.
 */
export const tabIndentation: Extension = keymap.of([{ key: "Tab", run: acceptCompletion }, indentWithTab]);

/**
 * The indentation of an editor: `unit` (spaces or a tab) is one level for Enter, Tab and re-indentation; with
 * `guides` every level of a line also gets a vertical line.
 */
export function indentation(unit: string, guides: boolean): Extension {
  return [indentUnit.of(unit), guides ? indentGuides(indentColumns(unit)) : []];
}
