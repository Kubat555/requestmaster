// `pm.test`. What a test is cut to, how many tests a job may record and how much text they may hold is the
// business of the primitive.

/** Twice `TEST_NAME_CHARS` and `TEST_ERROR_CHARS`, in UTF-16 units: a rough cut, as for a line of the console. */
const NAME_UNITS = 2000;
const ERROR_UNITS = 20000;

export function createTest(host, safe, pm) {
  /** Tests this script has recorded. */
  let recorded = 0;

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

  function test(name, fn) {
    let error;
    if (typeof fn === "function") {
      try {
        fn();
      } catch (thrown) {
        error = describe(thrown);
      }
    }
    record(name, error === undefined, false, error);
    return pm;
  }

  test.skip = function (name) {
    record(name, true, true, undefined);
    return pm;
  };

  test.index = function () {
    return recorded;
  };

  return test;
}
