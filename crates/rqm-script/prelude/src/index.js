// Entry of the prelude: everything a script sees besides the language itself. Loaded into the context of
// every script before its text.
//
// The script shares the realm with this code and may replace any built-in object, so:
// - every limit is checked by the Rust primitive, never here;
// - the primitives are taken off the global object first thing and are only ever called directly
//   (`host.log(x)`): never through `.call / .apply / .bind`, never passed as a callback, never put into an
//   object the script can reach;
// - the built-in functions this code needs are saved below, before the text of the script runs.
import { installConsole } from "./console.js";
import { createVariables } from "./variables.js";

const host = globalThis.__host;
delete globalThis.__host;

const apply = Reflect.apply;
const toWellFormed = String.prototype.toWellFormed;
const sliceString = String.prototype.slice;

const safe = {
  String,
  stringify: JSON.stringify,
  parse: JSON.parse,
  isArray: Array.isArray,
  keys: Object.keys,
  /** Every string that goes to a primitive passes through here: a lone surrogate becomes U+FFFD. */
  wellFormed(text) {
    return apply(toWellFormed, text, []);
  },
  slice(text, start, end) {
    return apply(sliceString, text, [start, end]);
  },
};

installConsole(host, safe);

const info = safe.parse(host.info());
info.iteration = 0;
info.iterationCount = 1;

globalThis.pm = { info, ...createVariables(host, safe) };
