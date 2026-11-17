// `{{variables}}` in the code editor: the tokens are coloured by their scopes, the one under the pointer is told
// about, names are offered while one is typed. Marking only, as in the one-line fields (`VarInput.svelte`): the
// values and the scopes come from the core. The colours are the tokens of `tokens.css`, set by classes.

import { pickedCompletion, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { EditorState, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, hoverTooltip, type DecorationSet, type Tooltip } from "@codemirror/view";
import type { ScopeVariable } from "../types";
import { applyCompletion, completionAt, describeVariable, findTokens, suggest } from "../variables";

/** Mark decorations for the tokens of the text: `cm-var cm-var-<scope>` or `cm-var cm-var-unresolved`. */
export function variableMarks(
  text: string,
  lookup: (name: string) => ScopeVariable | null,
): { from: number; to: number; class: string }[] {
  return findTokens(text).map((token) => {
    const variable = lookup(token.name);
    return { from: token.from, to: token.to, class: `cm-var cm-var-${variable ? variable.scope : "unresolved"}` };
  });
}

/** The scope in words and the value, as the list of a one-line field shows them next to the name. */
function detailOf(variable: ScopeVariable): string {
  const described = describeVariable(variable);
  return `${described.scope} · ${described.value}`;
}

/**
 * Puts the name in place of the one being typed (`from`–`to`) and closes the token; the `}}` that already stand
 * after the name — the ones `closeBrackets` has put — are not doubled.
 */
function insertName(view: EditorView, completion: Completion, from: number, to: number): void {
  const line = view.state.doc.lineAt(to);
  const result = applyCompletion(line.text, to - line.from, from - line.from, completion.label);
  // What stands after the new caret is the end of the line as it was: only the part before it is replaced.
  const kept = result.text.length - result.caret;
  view.dispatch({
    changes: { from, to: line.to - kept, insert: `${completion.label}}}` },
    selection: { anchor: line.from + result.caret },
    scrollIntoView: true,
    userEvent: "input.complete",
    annotations: pickedCompletion.of(completion),
  });
}

/** Completion source: options after `{{`; `null` elsewhere. */
export function variableCompletions(variables: ScopeVariable[]): (context: CompletionContext) => CompletionResult | null {
  return (context) => {
    const line = context.state.doc.lineAt(context.pos);
    const typed = completionAt(line.text, context.pos - line.from);
    if (!typed) return null;
    const items = suggest(variables, typed.prefix);
    if (items.length === 0) return null;
    return {
      from: line.from + typed.from,
      options: items.map((item) => ({ label: item.name, detail: detailOf(item), apply: insertName })),
      // The order and the choice are the ones of `suggest`, the same as in a one-line field.
      filter: false,
    };
  };
}

/** The hint about a token: the same texts as `VarTooltip.svelte`. */
function hintOf(name: string, variable: ScopeVariable | null): HTMLElement {
  const described = describeVariable(variable);
  const hint = document.createElement("div");
  hint.className = "cm-var-hint";
  const head = hint.appendChild(document.createElement("div"));
  head.className = "cm-var-hint-head";
  const title = head.appendChild(document.createElement("span"));
  title.className = "cm-var-hint-name";
  title.textContent = name;
  const scope = head.appendChild(document.createElement("span"));
  scope.className = `cm-var-hint-scope cm-var-${variable ? variable.scope : "unresolved"}`;
  scope.textContent = described.scope;
  if (described.value !== "") {
    const value = hint.appendChild(document.createElement("div"));
    value.className = "cm-var-hint-value";
    value.textContent = described.value;
  }
  return hint;
}

const SCOPE_COLOURS = {
  "cm-var-global": "var(--var-global)",
  "cm-var-collection": "var(--var-collection)",
  "cm-var-environment": "var(--var-environment)",
  "cm-var-dynamic": "var(--var-dynamic)",
  "cm-var-unresolved": "var(--danger)",
};

/**
 * A token may be cut into pieces by the highlighting of the language, each piece with a colour of its own (red
 * for `{{id}}` that stands in JSON without quotes): the rule names the pieces too, so the colour of the scope wins.
 */
const styles = EditorView.baseTheme({
  ...Object.fromEntries(
    Object.entries(SCOPE_COLOURS).map(([name, color]) => [`.cm-var.${name}, .cm-var.${name} span`, { color }]),
  ),
  ".cm-var.cm-var-unresolved": { textDecoration: "underline", textDecorationColor: "var(--danger)" },
  ".cm-var-hint": { maxWidth: "420px", padding: "calc(var(--space) * 1.5) calc(var(--space) * 2)" },
  ".cm-var-hint-head": { display: "flex", alignItems: "baseline", gap: "calc(var(--space) * 2)" },
  ".cm-var-hint-name, .cm-var-hint-value": { fontFamily: "var(--font-mono)", overflowWrap: "anywhere" },
  ".cm-var-hint-name": { fontWeight: "600" },
  ".cm-var-hint-scope": { flex: "none" },
  ...Object.fromEntries(
    Object.entries(SCOPE_COLOURS).map(([name, color]) => [`.cm-var-hint-scope.${name}`, { color }]),
  ),
  // A long value is cut, not the editor covered with it.
  ".cm-var-hint-value": { marginTop: "var(--space)", maxHeight: "120px", overflow: "hidden", whiteSpace: "pre-wrap" },
});

/** The extensions of an editor with `variables`: the variables a `{{name}}` of its text can resolve to. */
export function variableSupport(variables: ScopeVariable[]): Extension {
  const byName = new Map(variables.map((variable) => [variable.name, variable]));
  const lookup = (name: string): ScopeVariable | null => byName.get(name) ?? null;

  function marksOf(state: EditorState): DecorationSet {
    const marks = variableMarks(state.doc.toString(), lookup);
    return Decoration.set(marks.map((mark) => Decoration.mark({ class: mark.class }).range(mark.from, mark.to)));
  }

  const marks = StateField.define<DecorationSet>({
    create: marksOf,
    update: (value, transaction) => (transaction.docChanged ? marksOf(transaction.state) : value),
    provide: (field) => EditorView.decorations.from(field),
  });

  const hint = hoverTooltip((view, pos): Tooltip | null => {
    let found: Tooltip | null = null;
    view.state.field(marks).between(pos, pos, (from, to) => {
      const name = view.state.sliceDoc(from + 2, to - 2);
      found = { pos: from, end: to, create: () => ({ dom: hintOf(name, lookup(name)) }) };
      return false;
    });
    return found;
  });

  // One source for the whole life of the extension: the autocompletion tells its sources apart by identity.
  const data = [{ autocomplete: variableCompletions(variables) }];

  return [marks, hint, styles, EditorState.languageData.of(() => data)];
}
