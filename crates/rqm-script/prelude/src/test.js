// `pm.test`. What a test is cut to, how many tests a job may record and how much text they may hold is the
// business of the primitive.

/** Twice `TEST_NAME_CHARS` and `TEST_ERROR_CHARS`, in UTF-16 units: a rough cut, as for a line of the console. */
const NAME_UNITS = 2000;
const ERROR_UNITS = 20000;

const NO_DONE = "done() was not called";
const NO_SETTLE = "the promise of the test was not settled";

/** `{ test, record, finish }`: `pm.test`, the recording of one result, and what ends the tests of a script. */
export function createTest(host, safe, pm) {
  /** Tests this script has recorded. */
  let recorded = 0;
  /** Tests that end later: `{ close, late }`. Lives here — Rust keeps no JavaScript value. */
  const waiting = [];

  function cut(text, units) {
    return safe.wellFormed(text.length > units ? safe.slice(text, 0, units) : text);
  }

  /** "{name}: {message}" for an object with a message, otherwise what `String` gives. */
  function describe(thrown) {
    try {
      if (typeof thrown === "object" && thrown !== null && thrown.message !== undefined) {
        const name = thrown.name;
        const shown = name === undefined || name === null || name === "" ? "Error" : safe.String(name);
        return shown + ": " + safe.String(thrown.message);
      }
      return safe.String(thrown);
    } catch (error) {
      return "[unprintable]";
    }
  }

  function record(name, passed, skipped, error) {
    const title = cut(safe.String(name), NAME_UNITS);
    if (error === undefined) {
      host.test(title, passed, skipped);
    } else {
      host.test(title, passed, skipped, cut(error, ERROR_UNITS));
    }
    recorded++;
  }

  /**
   * A test whose result comes later. `close(error)` records it once; `late` is the error it gets when the
   * script ends first. `refused` — what the primitive threw when the result was recorded.
   */
  function later(name, late) {
    const entry = {
      late,
      open: true,
      refused: undefined,
      close(error) {
        if (!entry.open) return;
        entry.open = false;
        try {
          record(name, error === undefined, false, error);
        } catch (thrown) {
          entry.refused = thrown;
          throw thrown;
        }
      },
    };
    waiting[waiting.length] = entry;
    return entry;
  }

  /** A test that takes `done`: recorded when `done(error)` is called, or when the function throws. */
  function withDone(name, fn) {
    const entry = later(name, NO_DONE);
    let thrown;
    let threw = false;
    try {
      fn(function done(error) {
        entry.close(error === undefined || error === null ? undefined : describe(error));
      });
    } catch (caught) {
      thrown = caught;
      threw = true;
    }
    // A refusal of the primitive (too many tests) is an error of the script, as for a plain test.
    if (entry.refused !== undefined) throw entry.refused;
    if (threw) entry.close(describe(thrown));
  }

  function test(name, fn) {
    if (typeof fn !== "function") {
      record(name, true, false, undefined);
      return pm;
    }
    if (fn.length > 0) {
      withDone(name, fn);
      return pm;
    }
    let entry;
    let error;
    try {
      const returned = fn();
      if (returned !== null && typeof returned === "object" && typeof returned.then === "function") {
        // Recorded when the promise is settled.
        const pending = later(name, NO_SETTLE);
        entry = pending;
        returned.then(
          function () {
            pending.close(undefined);
          },
          function (reason) {
            pending.close(describe(reason));
          },
        );
        return pm;
      }
    } catch (thrown) {
      error = describe(thrown);
    }
    if (entry === undefined) record(name, error === undefined, false, error);
    else entry.close(error);
    return pm;
  }

  test.skip = function (name) {
    record(name, true, true, undefined);
    return pm;
  };

  test.index = function () {
    return recorded;
  };

  /** The script has ended: the tests that are still waiting fail. */
  function finish() {
    for (let i = 0; i < waiting.length; i++) {
      const entry = waiting[i];
      if (entry.open) entry.close(entry.late);
    }
  }

  return { test, record, finish };
}
