// What is in Postman and not here yet: every name of the table (`api.json`) with the status `unsupported`
// throws `<name> is not supported in RequestMaster` — a function when called, a property when read.
//
// A stub is not enumerable: printing an object and `Object.keys` do not come across it. No primitive of the
// host is used here. Whatever runs after the text of the script has started (`stubResponse` for the answer
// to `pm.sendRequest`, `stubRequest`) uses only what was saved when this file loaded.
import api from "./api.json";

const SafeError = Error;
const defineProperty = Object.defineProperty;

/** `[parent, last, kind, name, phase]` of every name for later: `pm.vault` → parent `pm`, last `vault`. */
const LATER = [];
for (const member of api.members) {
  if (member.status !== "unsupported") continue;
  const cut = member.name.lastIndexOf(".");
  LATER.push([
    cut < 0 ? "" : member.name.slice(0, cut),
    member.name.slice(cut + 1),
    member.kind,
    member.name,
    member.phase,
  ]);
}

/**
 * Above zero while the prelude itself looks at objects that carry stubs — the console prints one
 * (`JSON.stringify` calls `toJSON`), a library loads and asks the global object for `Buffer`. A property stub
 * then reads as `undefined`, and a function stub gives back the object it was called on.
 */
let quiet = 0;

/** Runs `run()` with the stubs silent. */
export function withoutStubs(run) {
  quiet++;
  try {
    return run();
  } finally {
    quiet--;
  }
}

function message(name) {
  return name + " is not supported in RequestMaster";
}

function stub(object, last, kind, name) {
  if (kind === "function") {
    defineProperty(object, last, {
      configurable: true,
      enumerable: false,
      writable: true,
      value: function () {
        if (quiet > 0) return this;
        throw new SafeError(message(name));
      },
    });
  } else {
    defineProperty(object, last, {
      configurable: true,
      enumerable: false,
      get() {
        if (quiet > 0) return undefined;
        throw new SafeError(message(name));
      },
    });
  }
}

/** Puts on `object` the stubs of the names that stand right under `parent` in the table. */
function stubMembers(object, parent) {
  for (let i = 0; i < LATER.length; i++) {
    const entry = LATER[i];
    if (entry[0] === parent) stub(object, entry[1], entry[2], entry[3]);
  }
}

/** The members of a response for later: called for every object the response factory makes. */
export function stubResponse(response) {
  stubMembers(response, "pm.response");
}

/** The members of `pm.request` and of its address for later: called when the request object is made. */
export function stubRequest(request, url) {
  stubMembers(request, "pm.request");
  stubMembers(url, "pm.request.url");
}

function under(name, parent) {
  return name === parent || name.indexOf(parent + ".") === 0;
}

/**
 * Everything else for later, on the objects the prelude has built: called once, before the text of the
 * script. `phase` — `pm.info.eventName`; a name the table gives another phase alone is left out. An object
 * that is not there yet is made (not enumerable).
 */
export function installStubs(phase) {
  for (let i = 0; i < LATER.length; i++) {
    const [parent, last, kind, name, own] = LATER[i];
    if (under(parent, "pm.response") || under(parent, "pm.request")) continue;
    if (own !== undefined && own !== phase) continue;
    let object = globalThis;
    if (parent !== "") {
      const parts = parent.split(".");
      for (let at = 0; at < parts.length; at++) {
        let next = object[parts[at]];
        if (next === undefined || next === null) {
          next = {};
          defineProperty(object, parts[at], { configurable: true, enumerable: false, writable: true, value: next });
        }
        object = next;
      }
    }
    stub(object, last, kind, name);
  }
}
