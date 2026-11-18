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
