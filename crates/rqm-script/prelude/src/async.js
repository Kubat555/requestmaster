// Timers, and the list of the promises that were rejected and are not handled yet. The functions of the
// timers live here: Rust keeps only the number, the moment and the period of a timer (`src/host.rs`), and
// the engine calls `fire` when a timer is due (`src/engine.rs`).

/**
 * Puts `setTimeout` and its kin into the global scope. Returns `later(fn)`: calls `fn` when the script
 * has nothing left to run at once — like a timer without a delay, an exception of `fn` is an error of the script.
 */
export function installAsync(host, safe) {
  /** Number of a timer → `{ fn, args, repeats }`. */
  const timers = { __proto__: null };

  /** The number of the new timer; nothing when `fn` is not a function — there is no timer then. */
  function add(fn, delay, repeats, args) {
    if (typeof fn !== "function") return undefined;
    let millis = safe.Number(delay);
    if (!(millis > 0)) millis = 0;
    const number = host.timer_add(millis, repeats);
    timers[number] = { fn, args, repeats };
    return number;
  }

  /** Only a timer this table knows goes to the primitive: any other number is nothing to clear. */
  function clear(number) {
    if (typeof number !== "number" || timers[number] === undefined) return;
    delete timers[number];
    host.timer_clear(number);
  }

  globalThis.setTimeout = function setTimeout(fn, delay, ...args) {
    return add(fn, delay, false, args);
  };
  globalThis.setInterval = function setInterval(fn, delay, ...args) {
    return add(fn, delay, true, args);
  };
  globalThis.setImmediate = function setImmediate(fn, ...args) {
    return add(fn, 0, false, args);
  };
  globalThis.clearTimeout = function clearTimeout(number) {
    clear(number);
  };
  globalThis.clearInterval = function clearInterval(number) {
    clear(number);
  };
  globalThis.clearImmediate = function clearImmediate(number) {
    clear(number);
  };

  // The engine calls this when the timer of that number is due. An exception of the function goes back to
  // the engine: it is the error of the script.
  host.fire = function (number) {
    const timer = timers[number];
    if (timer === undefined) return;
    if (!timer.repeats) delete timers[number];
    safe.apply(timer.fn, undefined, timer.args);
  };

  // Promise → the reason it was rejected with, while it has no handler. The methods are saved: the script
  // may replace those of `Map.prototype`.
  const rejected = new Map();
  const put = Map.prototype.set;
  const drop = Map.prototype.delete;
  const each = Map.prototype.forEach;

  // The rejection tracker of the engine calls this: for a promise rejected with no handler, and once more
  // when a handler is attached to it later.
  host.track = function (promise, reason, handled) {
    if (handled) safe.apply(drop, rejected, [promise]);
    else safe.apply(put, rejected, [promise, reason]);
  };

  // `{ reason }` of the first promise that is still not handled; nothing when there is none.
  host.unhandled = function () {
    let found;
    safe.apply(each, rejected, [
      function (reason) {
        if (found === undefined) found = { reason };
      },
    ]);
    return found;
  };

  return function later(fn) {
    add(fn, 0, false, []);
  };
}
