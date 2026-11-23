// The names of the sandbox API in a script editor: offered from the static list of `scriptApi.ts`, by the path
// typed before the caret. Nothing of the script is run or read beyond that path.

import type { Completion, CompletionContext, CompletionResult } from "@codemirror/autocomplete";
import { SCRIPT_API } from "../scriptApi";

/** A dotted path that ends at the caret: `pm`, `pm.`, `pm.environment.s`. */
const TYPED = /[\w$]+(?:\.[\w$]*)*$/;

/**
 * Completion source of a script editor: the members of the sandbox API after a dot, its top-level names
 * while one is typed. `null` — nothing to offer.
 */
export function scriptCompletions(context: CompletionContext): CompletionResult | null {
  const line = context.state.doc.lineAt(context.pos);
  const typed = TYPED.exec(line.text.slice(0, context.pos - line.from))?.[0] ?? "";
  // Without a dot the list is for a name being typed, or for the one who asked for it (Ctrl+Space).
  if (typed === "" && !context.explicit) return null;

  // The last dot parts the path from the start of the name; `base` is what a full name has before that name.
  const base = typed.slice(0, typed.lastIndexOf(".") + 1);
  const start = typed.slice(base.length);

  const options = new Map<string, Completion>();
  for (const item of SCRIPT_API) {
    if (!item.name.startsWith(base)) continue;
    const rest = item.name.slice(base.length);
    const end = rest.indexOf(".");
    const label = end < 0 ? rest : rest.slice(0, end);
    if (!label.startsWith(start)) continue;
    const option = options.get(label) ?? { label };
    if (end < 0) {
      // The name itself. One that also goes on (`pm.test` and `pm.test.skip`) stays a namespace.
      option.type ??= item.kind;
      if (item.hint) option.detail = item.hint;
    } else {
      option.type = "namespace";
    }
    options.set(label, option);
  }
  if (options.size === 0) return null;
  return { from: context.pos - start.length, options: [...options.values()] };
}
