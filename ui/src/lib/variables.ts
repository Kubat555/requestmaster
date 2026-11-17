// `{{variables}}` in the text of a field: where the tokens are, what to offer while one is typed, which names a
// request uses. Marking only — nothing is substituted here: the values and the scopes come from the core.

import { strings } from "./strings";
import type { Auth, DraftBody, ParamRow, RequestDraft, ScopeVariable } from "./types";

/** A `{{name}}` in a text. */
export interface VarToken {
  /** UTF-16 offset of the first brace. */
  from: number;
  /** UTF-16 offset right after the last brace. */
  to: number;
  /** What stands between the braces: any characters, spaces included; may be empty. */
  name: string;
}

function isBrace(char: string | undefined): boolean {
  return char === "{" || char === "}";
}

/**
 * The `{{name}}` tokens the core replaces in its first pass: innermost, left to right. The rule is the one of
 * `Resolver::pass` in `rqm-vars`: a token starts with `{{`, its name goes up to the first brace, and it is a
 * token only when `}}` follows the name at once; otherwise the search goes on from the next character.
 */
export function findTokens(text: string): VarToken[] {
  const tokens: VarToken[] = [];
  let index = 0;
  while (index + 1 < text.length) {
    if (text[index] !== "{" || text[index + 1] !== "{") {
      index += 1;
      continue;
    }
    const nameStart = index + 2;
    let nameEnd = nameStart;
    while (nameEnd < text.length && !isBrace(text[nameEnd])) nameEnd += 1;
    if (text[nameEnd] !== "}" || text[nameEnd + 1] !== "}") {
      // Not a token at this place; an inner `{{` may still start one.
      index += 1;
      continue;
    }
    tokens.push({ from: index, to: nameEnd + 2, name: text.slice(nameStart, nameEnd) });
    index = nameEnd + 2;
  }
  return tokens;
}

/**
 * The name being typed at the caret: `from` — where it starts, right after `{{`; `prefix` — its part before the
 * caret. `null` — the caret is not inside a `{{`.
 */
export function completionAt(text: string, caret: number): { from: number; prefix: string } | null {
  let from = caret;
  while (from > 0 && !isBrace(text[from - 1])) from -= 1;
  if (from < 2 || text[from - 1] !== "{" || text[from - 2] !== "{") return null;
  return { from, prefix: text.slice(from, caret) };
}

/**
 * Puts the chosen name in place of the one being typed (`from` — of {@link completionAt}) and closes the token.
 * A token that is already closed gets its whole name replaced and no second `}}`. `caret` of the result stands
 * right after the token.
 */
export function applyCompletion(text: string, caret: number, from: number, name: string): { text: string; caret: number } {
  let nameEnd = caret;
  while (nameEnd < text.length && !isBrace(text[nameEnd])) nameEnd += 1;
  const closed = text.startsWith("}}", nameEnd);
  // Without the closing braces what follows the caret is not a part of the name: it stays.
  const rest = closed ? text.slice(nameEnd + 2) : text.slice(caret);
  const start = `${text.slice(0, from)}${name}}}`;
  return { text: start + rest, caret: start.length };
}

/** How many variables the list of the autocompletion shows at most. */
const SUGGEST_LIMIT = 50;

/**
 * The variables to offer for the typed beginning of a name: first the names that start with it, then the ones
 * that contain it, each group in the order of `variables`; the case does not matter. At most 50.
 */
export function suggest(variables: ScopeVariable[], prefix: string): ScopeVariable[] {
  const wanted = prefix.toLowerCase();
  const starting: ScopeVariable[] = [];
  const containing: ScopeVariable[] = [];
  for (const variable of variables) {
    const at = variable.name.toLowerCase().indexOf(wanted);
    if (at === 0) starting.push(variable);
    else if (at > 0) containing.push(variable);
  }
  return [...starting, ...containing].slice(0, SUGGEST_LIMIT);
}

/** Keys and values of the rows that are sent. */
function rowTexts(rows: ParamRow[]): string[] {
  return rows.filter((row) => row.enabled).flatMap((row) => [row.key, row.value]);
}

function bodyTexts(body: DraftBody): string[] {
  switch (body.mode) {
    case "raw":
      return [body.data.text];
    case "url_encoded":
      return rowTexts(body.data);
    case "form_data":
      // The key of every part that is sent and the text of a text one; never the path of a file.
      return body.data
        .filter((part) => part.enabled !== false)
        .flatMap((part) => ("text" in part.value ? [part.key, part.value.text] : [part.key]));
    case "graphql":
      return [body.data.query, body.data.variables];
    default:
      return [];
  }
}

function authTexts(auth: Auth | null): string[] {
  switch (auth?.type) {
    case "basic":
      return [auth.username, auth.password];
    case "bearer":
      return [auth.token];
    case "oauth2_token":
      return [auth.token, auth.header_prefix];
    case "api_key":
      return [auth.key, auth.value];
    default:
      return [];
  }
}

/**
 * Names used where the core substitutes for this request: enabled rows only. Without repeats, in the order of
 * appearance. The method, the rows that are switched off, the keys of the path variables and the paths of the
 * files are not looked at; neither is an inherited auth: the request does not hold it.
 */
export function requestTokenNames(draft: RequestDraft): string[] {
  const texts = [
    draft.url,
    ...rowTexts(draft.query),
    ...rowTexts(draft.headers),
    ...draft.path_variables.filter((row) => row.enabled).map((row) => row.value),
    ...bodyTexts(draft.body),
    ...authTexts(draft.auth),
  ];
  const names = new Set<string>();
  for (const text of texts) {
    for (const token of findTokens(text)) names.add(token.name);
  }
  return [...names];
}

/**
 * What the hint and the list of the autocompletion say about a variable: its scope in words and its value.
 * A secret value is never shown: a mask stands for it. `null` — there is no such variable.
 */
export function describeVariable(variable: ScopeVariable | null): { scope: string; value: string } {
  const text = strings.variables;
  if (variable === null) return { scope: text.notFound, value: "" };
  const scope = text.scopes[variable.scope];
  if (variable.secret) return { scope, value: text.secretMask };
  if (variable.scope === "dynamic") return { scope, value: text.dynamicValue };
  // Without a value and not dynamic: the core hides only secrets, so this is shown as one.
  if (variable.value === null) return { scope, value: text.secretMask };
  return { scope, value: variable.value === "" ? text.emptyValue : variable.value };
}
