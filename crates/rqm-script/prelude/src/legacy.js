// The names scripts written for old Postman use: global variables that are worked out only when read.
import { put } from "./lists.js";

/**
 * A global the first read works out. It is a getter with a setter, so that a script can also assign to the
 * name without declaring it: the script is not strict, and with a getter alone the assignment would do nothing.
 */
export function lazyGlobal(name, compute) {
  function settle(value) {
    Object.defineProperty(globalThis, name, { value, writable: true, enumerable: true, configurable: true });
  }
  Object.defineProperty(globalThis, name, {
    configurable: true,
    enumerable: true,
    get() {
      const value = compute();
      settle(value);
      return value;
    },
    set(value) {
      settle(value);
    },
  });
}

/**
 * The global `tests` of old scripts: `tests["name"] = value`. Returns what turns its pairs into tests when
 * the script has ended — the object is read then, so a script may also put another object under the name.
 */
export function installLegacyTests(safe, record) {
  globalThis.tests = {};
  return function collect() {
    const results = globalThis.tests;
    if (results === null || typeof results !== "object") return;
    const names = safe.keys(results);
    for (let i = 0; i < names.length; i++) {
      const value = results[names[i]];
      if (value) {
        record(names[i], true, false, undefined);
      } else {
        // Only a primitive is falsy: `String` cannot throw here.
        const shown = value === "" ? "''" : safe.String(value);
        record(names[i], false, false, "AssertionError: expected " + shown + " to be truthy");
      }
    }
  };
}

/** `responseBody`, `responseCode`, `responseHeaders`, `responseTime`. */
export function installLegacyResponse(response) {
  const data = response.data;
  // `undefined` for a body over the limit.
  lazyGlobal("responseBody", () => response.body());
  lazyGlobal("responseCode", () => ({ code: data.code, name: data.status, detail: data.status }));
  lazyGlobal("responseHeaders", () => {
    // The name as it came; of a name that repeats, the last value.
    const headers = {};
    for (let i = 0; i < data.headers.length; i++) put(headers, data.headers[i][0], data.headers[i][1]);
    return headers;
  });
  lazyGlobal("responseTime", () => data.time);
}

/** The old `request`: worked out from the request as the script got it. */
export function installLegacyRequest(request) {
  lazyGlobal("request", () => request.legacy());
}

/**
 * `environment`, `globals`, `data`, `iteration` and the object `postman`. `response` — as `createResponse`
 * gives it; `undefined` before the request.
 */
export function installLegacyVariables(safe, pm, response) {
  /**
   * `toObject()` of a scope at the moment the snapshot is first asked for. The object stays here: a script
   * that puts its own value under the global name does not change what `postman.set…` writes to.
   */
  function snapshot(name, scope) {
    let object;
    function current() {
      if (object === undefined) object = scope.toObject();
      return object;
    }
    lazyGlobal(name, current);
    return current;
  }
  const environment = snapshot("environment", pm.environment);
  const globals = snapshot("globals", pm.globals);
  lazyGlobal("data", () => ({}));
  lazyGlobal("iteration", () => 0);

  /** As `postman-sandbox` 6.9.0 does: `toString()` for a truthy value and for `false`; the rest as it is. */
  function stored(value) {
    return (value === false || value) && typeof value.toString === "function" ? value.toString() : value;
  }

  // Each changes the scope and the old object of this script.
  function setter(scope, object) {
    return function (key, value) {
      const kept = stored(value);
      scope.set(key, kept);
      put(object(), key, kept);
    };
  }
  function unsetter(scope, object) {
    return function (key) {
      scope.unset(key);
      delete object()[key];
    };
  }
  function clearer(scope, object) {
    return function () {
      scope.clear();
      const kept = object();
      const names = safe.keys(kept);
      for (let i = 0; i < names.length; i++) delete kept[names[i]];
    };
  }

  globalThis.postman = {
    setEnvironmentVariable: setter(pm.environment, environment),
    getEnvironmentVariable(key) {
      return pm.environment.get(key);
    },
    clearEnvironmentVariable: unsetter(pm.environment, environment),
    clearEnvironmentVariables: clearer(pm.environment, environment),
    setGlobalVariable: setter(pm.globals, globals),
    getGlobalVariable(key) {
      return pm.globals.get(key);
    },
    clearGlobalVariable: unsetter(pm.globals, globals),
    clearGlobalVariables: clearer(pm.globals, globals),
    /** The case of the name does not matter; before the request there is no response. */
    getResponseHeader(name) {
      return response === undefined ? undefined : response.api.headers.get(name);
    },
    /** Only a collection run has a next request. */
    setNextRequest() {},
  };
}
