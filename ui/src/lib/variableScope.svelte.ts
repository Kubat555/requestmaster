// The variables a `{{name}}` can resolve to in the editor of one node, as the core tells them, and the signal
// that makes every editor read them again.

import type { Api } from "./api";
import { toasts } from "./toasts.svelte";
import type { ScopeVariable } from "./types";

/** Grows every time something that changes variables has happened. */
let changes = $state(0);

/**
 * Something that changes variables has happened: the active environment was chosen, an environment or the
 * global variables were saved or deleted, a collection was saved, a node was moved. Every {@link VariableScope}
 * is stale from now on.
 */
export function bumpVariables(): void {
  changes += 1;
}

/**
 * The variables in the scope of the node `nodeId`. The editor of the node puts it into the context under the key
 * `"variables"` and calls {@link refresh} while its tab is shown and the scope is {@link stale}.
 */
export class VariableScope {
  readonly nodeId: string;
  /** The variables as the last reading returned them; empty until the first one and after a failed one. */
  variables = $state.raw<ScopeVariable[]>([]);
  /** The last reading succeeded: a name that is not among {@link variables} is not found. */
  loaded = $state(false);
  readonly #byName = $derived(new Map(this.variables.map((variable) => [variable.name, variable])));
  /** Value of `changes` when the last reading began; -1 — nothing was read yet. */
  #readAt = $state(-1);
  /** Number of the last reading: the answer of an earlier one is dropped. */
  #reading = 0;

  constructor(nodeId: string) {
    this.nodeId = nodeId;
  }

  /** Nothing was read yet, or {@link bumpVariables} was called after the last reading began. */
  get stale(): boolean {
    return this.#readAt !== changes;
  }

  /** The variable `{{name}}` resolves to; `null` — there is none, or the variables are not read. */
  lookup(name: string): ScopeVariable | null {
    return this.#byName.get(name) ?? null;
  }

  /**
   * Reads the variables from the core. Of several readings that overlap only the answer of the last one is
   * taken. A failure is shown as a notification and leaves no variables: the tokens lose the colours of their
   * scopes and nothing is called "not found"; the next reading is after the next {@link bumpVariables}.
   */
  async refresh(api: Api): Promise<void> {
    const reading = ++this.#reading;
    this.#readAt = changes;
    let variables: ScopeVariable[];
    try {
      variables = await api.variablesInScope(this.nodeId);
    } catch (error) {
      if (reading !== this.#reading) return;
      this.variables = [];
      this.loaded = false;
      toasts.error(String(error));
      return;
    }
    if (reading !== this.#reading) return;
    this.variables = variables;
    this.loaded = true;
  }
}
