// `pm.globals`, `pm.collectionVariables`, `pm.environment`, `pm.variables`. The lists themselves, the rules
// of set / unset / clear, the limits and the log of changes live in Rust (`src/variables.rs`).

// What a primitive reads or changes. The numbers are the ones `src/host.rs` expects.
const GLOBALS = 0;
const COLLECTION = 1;
const ENVIRONMENT = 2;
const LOCAL = 3;
/** Read only: local, then environment, collection, globals. */
const EFFECTIVE = 4;

export function createVariables(host, safe) {
  const key = (name) => safe.wellFormed(safe.String(name));

  function replace(view, input) {
    if (typeof input === "string") return host.var_replace(view, safe.wellFormed(input));
    if (safe.isArray(input)) {
      const copy = [];
      for (let i = 0; i < input.length; i++) copy[i] = replace(view, input[i]);
      return copy;
    }
    if (typeof input === "object" && input !== null) {
      const copy = {};
      const names = safe.keys(input);
      for (let i = 0; i < names.length; i++) copy[names[i]] = replace(view, input[names[i]]);
      return copy;
    }
    return input;
  }

  /** `target` — the list `set / unset / clear` change; `view` — what `get / has / toObject / replaceIn` read. */
  function scope(target, view) {
    return {
      /** What `set` was given within this send, otherwise the stored text. */
      get(name) {
        const wanted = key(name);
        const typed = host.var_typed(view, wanted);
        return typeof typed === "string" ? safe.parse(typed) : host.var_text(view, wanted);
      },
      has(name) {
        return host.var_has(view, key(name));
      },
      set(name, value) {
        const text = value === undefined ? "" : safe.String(value);
        let typed;
        if (typeof value !== "string") {
          try {
            typed = safe.stringify(value);
          } catch (error) {
            typed = undefined;
          }
        }
        if (typeof typed === "string") {
          host.var_set(target, key(name), safe.wellFormed(text), safe.wellFormed(typed));
        } else {
          host.var_set(target, key(name), safe.wellFormed(text));
        }
      },
      unset(name) {
        host.var_unset(target, key(name));
      },
      clear() {
        host.var_clear(target);
      },
      toObject() {
        // [[key, text, typed or null], …] from the widest scope to the narrowest: a later entry wins.
        const entries = safe.parse(host.var_list(view));
        const object = {};
        for (let i = 0; i < entries.length; i++) {
          const typed = entries[i][2];
          object[entries[i][0]] = typeof typed === "string" ? safe.parse(typed) : entries[i][1];
        }
        return object;
      },
      replaceIn(input) {
        return replace(view, input);
      },
    };
  }

  const environment = scope(ENVIRONMENT, ENVIRONMENT);
  environment.name = host.env_name();

  return {
    globals: scope(GLOBALS, GLOBALS),
    collectionVariables: scope(COLLECTION, COLLECTION),
    environment,
    variables: scope(LOCAL, EFFECTIVE),
  };
}
