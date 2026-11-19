// `require(name)`: the few modules of the Postman sandbox that are here. Every other name — a module for
// later of the table (`api.json`) or anything else — is "not found": no file is ever read.

/**
 * `modules` — name → what gives the module, asked on every call. `text` turns the name into a string.
 */
export function createRequire(text, modules) {
  const names = [];
  const givers = [];
  for (const name in modules) {
    names[names.length] = name;
    givers[givers.length] = modules[name];
  }
  return function require(name) {
    const wanted = text(name);
    for (let i = 0; i < names.length; i++) {
      if (names[i] === wanted) return givers[i]();
    }
    throw new Error("Cannot find module '" + wanted + "' (not supported in RequestMaster)");
  };
}
