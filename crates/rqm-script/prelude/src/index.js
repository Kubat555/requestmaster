// Entry of the prelude: everything a script sees besides the language itself. Loaded into the context of
// every script before its text.
//
// The script shares the realm with this code and may replace any built-in object, so:
// - every limit is checked by the Rust primitive, never here;
// - the primitives are taken off the global object first thing and are only ever called directly
//   (`host.log(x)`): never through `.call / .apply / .bind`, never passed as a callback, never put into an
//   object the script can reach;
// - the built-in functions this code needs are saved below, before the text of the script runs.
import { installAsync } from "./async.js";
import { installConsole } from "./console.js";
import {
  installLegacyRequest,
  installLegacyResponse,
  installLegacyTests,
  installLegacyVariables,
} from "./legacy.js";
import { createExecution, createRequest } from "./request.js";
import { createRequire } from "./require.js";
import { createResponse, isResponse, makeResponse } from "./response.js";
import { createSendRequest } from "./send-request.js";
import { installStubs, withoutStubs } from "./stubs.js";
import { createTest } from "./test.js";
import { createVariables } from "./variables.js";

const host = globalThis.__host;
delete globalThis.__host;

const apply = Reflect.apply;
const toWellFormed = String.prototype.toWellFormed;
const sliceString = String.prototype.slice;
// The functions of the engine itself: `require('atob')` gives them whatever the script did to the globals.
const engineAtob = globalThis.atob;
const engineBtoa = globalThis.btoa;

const safe = {
  String,
  Number,
  Error,
  Promise,
  apply,
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
const later = installAsync(host, safe);

const info = safe.parse(host.info());
info.iteration = 0;
info.iterationCount = 1;

const pm = { info, ...createVariables(host, safe) };

/**
 * chai with the assertions of a response. Its bundle is loaded into the context when a script first reads
 * `pm.expect` or `pm.response.to`: it leaves one function under a global name, taken away here at once.
 */
let chai;
function loadChai() {
  if (chai === undefined) {
    // A library asks the global object what is there (`Buffer`): the stubs keep silent while it loads.
    chai = withoutStubs(function () {
      host.load_bundle("chai");
      const install = globalThis.__rqm_chai;
      delete globalThis.__rqm_chai;
      if (typeof install !== "function") throw new Error("chai is not available");
      return install(isResponse);
    });
  }
  return chai;
}

/** lodash 4.18.1: `_` and `require('lodash')`. Loaded like chai, on first use. */
let lodash;
let lodashLoading = false;
function loadLodash() {
  if (lodash === undefined) {
    lodashLoading = true;
    let loaded;
    try {
      loaded = withoutStubs(function () {
        host.load_bundle("lodash");
        const left = globalThis.__rqm_lodash;
        delete globalThis.__rqm_lodash;
        return left;
      });
    } finally {
      lodashLoading = false;
    }
    if (typeof loaded !== "function") throw new Error("lodash is not available");
    lodash = loaded;
  }
  return lodash;
}

// The global `_`. A getter with a setter, as the old global names are: a script may put its own value
// under the name. lodash itself reads the name while it loads (what it would give back in `noConflict`):
// there is nothing under it then.
Object.defineProperty(globalThis, "_", {
  configurable: true,
  enumerable: true,
  get() {
    return lodashLoading ? undefined : loadLodash();
  },
  set(value) {
    Object.defineProperty(globalThis, "_", { value, writable: true, enumerable: true, configurable: true });
  },
});

Object.defineProperty(pm, "expect", {
  configurable: true,
  enumerable: true,
  get() {
    return loadChai().expect;
  },
});

// Only the scripts that run after the request have a response.
const response = createResponse(host, safe, loadChai);
if (response !== undefined) {
  pm.response = response.api;
  installLegacyResponse(response);
}

// `pm.request` is made when a script first reads it: the host is not asked for the request before that.
const request = createRequest(host, safe, info);
Object.defineProperty(pm, "request", {
  configurable: true,
  enumerable: true,
  get() {
    return request.api();
  },
});
installLegacyRequest(request);
pm.execution = createExecution(host, info);
installLegacyVariables(safe, pm, response);

// The answer to a request of the script is a response of the same factory; its body stays on the Rust side
// until the script asks for it.
pm.sendRequest = createSendRequest(host, safe, pm, later, function (data, number) {
  return makeResponse(safe, loadChai, data, function () {
    return host.answer_body(number);
  }).api;
});

const tests = createTest(host, safe, pm);
pm.test = tests.test;

// Both belong to a collection run: here they are empty.
pm.iterationData = {
  get() {},
  has() {
    return false;
  },
  toObject() {
    return {};
  },
};
pm.visualizer = {
  set() {},
  clear() {},
};

globalThis.require = createRequire(safe.String, {
  lodash: loadLodash,
  chai: loadChai,
  atob() {
    return engineAtob;
  },
  btoa() {
    return engineBtoa;
  },
});
const collectLegacyTests = installLegacyTests(safe, tests.record);

// The engine calls this when the script has ended — its text, its requests and its timers — with an exception
// too (not after a timeout, a memory limit or a cancel): first the tests of `pm.test` that are still waiting, then the pairs of `tests`.
// The object of the primitives is out of the reach of the script, so the function is too.
host.finish = function () {
  tests.finish();
  collectLegacyTests();
};

globalThis.pm = pm;
// After everything above is in place: the names for later go onto the objects that exist.
installStubs(info.eventName);
