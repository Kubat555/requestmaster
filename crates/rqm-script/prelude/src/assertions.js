// The assertions of `pm.response.to.*`: a chai plugin of our own with the names and the failure texts of
// `chai-postman` 2.0.1. Part of the chai bundle, not of the prelude: it never sees a primitive of the host.

const CODE = "expected response to have status code #{exp} but got #{act}";
const NOT_CODE = "expected response to not have status code #{act}";
const REASON = "expected response to have status reason #{exp} but got #{act}";
const NOT_REASON = "expected response to not have status reason #{act}";
const CLASS = "expected response code to be #{exp}XX but found #{act}";
const NOT_CLASS = "expected response code to not be #{exp}XX";

/** Property → the first digit of the code. */
const CLASSES = [
  ["info", 1],
  ["success", 2],
  ["redirection", 3],
  ["clientError", 4],
  ["serverError", 5],
];

/** Property → the code and the reason phrase. `notAcceptable` is "Acceptable" in Postman too. */
const NAMED = [
  ["accepted", 202, "Accepted"],
  ["withoutContent", 204, "No Content"],
  ["badRequest", 400, "Bad Request"],
  ["unauthorised", 401, "Unauthorised"],
  ["unauthorized", 401, "Unauthorized"],
  ["forbidden", 403, "Forbidden"],
  ["notFound", 404, "Not Found"],
  ["notAcceptable", 406, "Acceptable"],
  ["rateLimited", 429, "Too Many Requests"],
];

const hasOwn = Object.prototype.hasOwnProperty;

/** A path as lodash reads it: names separated by dots, indexes and quoted names in brackets. */
function segments(path) {
  const found = [];
  let name = "";
  let at = 0;
  while (at < path.length) {
    const character = path[at];
    const close = character === "[" ? path.indexOf("]", at) : -1;
    if (character === ".") {
      if (name !== "") found.push(name);
      name = "";
      at++;
    } else if (close > at) {
      if (name !== "") found.push(name);
      name = "";
      let key = path.slice(at + 1, close);
      const quote = key[0];
      if (key.length > 1 && (quote === '"' || quote === "'") && key[key.length - 1] === quote) {
        key = key.slice(1, -1);
      }
      found.push(key);
      at = close + 1;
    } else {
      name += character;
      at++;
    }
  }
  if (name !== "") found.push(name);
  return found;
}

/** `{ found, value }` of what lies at the path. A key that holds a dot itself is looked at first, as in lodash. */
function lookup(root, path) {
  if (root !== null && root !== undefined && hasOwn.call(Object(root), path)) {
    return { found: true, value: root[path] };
  }
  const keys = segments(path);
  if (keys.length === 0) return { found: false, value: undefined };
  let value = root;
  for (const key of keys) {
    if (value === null || value === undefined || !hasOwn.call(Object(value), key)) {
      return { found: false, value: undefined };
    }
    value = value[key];
  }
  return { found: true, value };
}

export function createAssertions(isResponse) {
  return function (chai, utils) {
    const Assertion = chai.Assertion;

    /** The response under the assertion; anything else fails, whether the assertion is negated or not. */
    function response(assertion) {
      const value = assertion._obj;
      new Assertion(value).assert(
        isResponse(value),
        "expecting a postman response object but got #{this}",
        "not expecting a postman response object",
      );
      return value;
    }

    function statusCode(assertion, code) {
      const target = response(assertion);
      assertion.assert(target.code === Number(code), CODE, NOT_CODE, Number(code), target.code);
    }

    function statusReason(assertion, reason) {
      const actual = response(assertion).reason();
      assertion.assert(actual === String(reason), REASON, NOT_REASON, String(reason), actual);
    }

    function statusClass(assertion, digit) {
      const code = response(assertion).code;
      assertion.assert(Math.floor(code / 100) === digit, CLASS, NOT_CLASS, digit, code);
    }

    /** The reason phrase first, without regard to case, then the code. */
    function named(assertion, code, reason) {
      const target = response(assertion);
      const wanted = reason.toUpperCase();
      const actual = String(target.reason()).toUpperCase();
      assertion.assert(actual === wanted, REASON, NOT_REASON, wanted, actual);
      assertion.assert(Number(target.code) === code, CODE, NOT_CODE, code, Number(target.code));
    }

    function withBody(assertion) {
      const text = response(assertion).text();
      assertion.assert(
        Boolean(text),
        "expected response to have content in body",
        "expected response to not have content in body",
      );
    }

    /** `{ value }` of the parsed body, or `{ error }` — the text of why it is not JSON. */
    function parse(target) {
      try {
        return { value: target.json() };
      } catch (thrown) {
        const failed = thrown !== null && typeof thrown === "object" ? thrown : {};
        return { error: String(failed.name) + ": " + String(failed.message) };
      }
    }

    function validJson(assertion, parsed) {
      assertion.assert(
        parsed.error === undefined,
        "expected response body to be a valid json but got error " + parsed.error,
        "expected response body not to be a valid json",
      );
    }

    /** The parsed body for an assertion about what is in it: a body that is not JSON fails first. */
    function jsonOf(target) {
      const parsed = parse(target);
      validJson(new Assertion(target), parsed);
      return parsed.value;
    }

    Assertion.addMethod("statusCode", function (code) {
      statusCode(this, code);
    });
    Assertion.addMethod("statusReason", function (reason) {
      statusReason(this, reason);
    });
    Assertion.addMethod("status", function (codeOrReason) {
      if (typeof codeOrReason === "number") statusCode(this, codeOrReason);
      else statusReason(this, codeOrReason);
    });
    Assertion.addMethod("statusCodeClass", function (digit) {
      statusClass(this, digit);
    });
    for (const [name, digit] of CLASSES) {
      Assertion.addProperty(name, function () {
        statusClass(this, digit);
      });
    }
    Assertion.addProperty("error", function () {
      const code = response(this).code;
      const digit = Math.floor(code / 100);
      this.assert(
        digit === 4 || digit === 5,
        "expected response code to be 4XX or 5XX but found #{act}",
        "expected response code to not be 4XX or 5XX",
        null,
        code,
      );
    });
    for (const [name, code, reason] of NAMED) {
      Assertion.addProperty(name, function () {
        named(this, code, reason);
      });
    }
    // On anything but a response `ok` stays the one of chai.
    Assertion.overwriteProperty("ok", function (base) {
      return function () {
        if (!isResponse(this._obj)) return base.call(this);
        named(this, 200, "OK");
        return undefined;
      };
    });

    Assertion.addMethod("header", function (name, value) {
      const target = response(this);
      const key = String(name);
      const present = target.headers.has(key);
      if (arguments.length < 2) {
        this.assert(
          present,
          "expected response to have header with key '" + key + "'",
          "expected response to not have header with key '" + key + "'",
        );
        return;
      }
      // Asked with a value, the header has to be there whether the assertion is negated or not: a negated
      // one then says that its value is another.
      new Assertion(target).assert(present, "expected response to have header with key '" + key + "'", "");
      const actual = target.headers.get(key);
      this.assert(
        actual === value,
        "expected '" + key + "' response header to be #{exp} but got #{act}",
        "expected '" + key + "' response header to not be #{act}",
        value,
        actual,
      );
    });

    Assertion.addProperty("withBody", function () {
      withBody(this);
    });
    Assertion.addProperty("json", function () {
      validJson(this, parse(response(this)));
    });
    Assertion.addMethod("body", function (content) {
      const target = response(this);
      if (arguments.length === 0) {
        withBody(this);
      } else if (content instanceof RegExp) {
        const text = target.text();
        this.assert(
          content.exec(text) !== null,
          "expected response body text to match #{exp} but got #{act}",
          "expected response body text to not match #{exp}",
          content,
          text,
        );
      } else if (content !== null && typeof content === "object") {
        const json = jsonOf(target);
        this.assert(
          utils.eql(json, content),
          "expected response body json to equal #{exp} but got #{act}",
          "expected response body json to not equal #{exp} but got #{act}",
          content,
          json,
        );
      } else {
        const text = target.text();
        this.assert(
          text === content,
          "expected response body to equal #{exp} but got #{act}",
          "expected response body to not equal #{exp}",
          content,
          text,
        );
      }
    });
    Assertion.addMethod("jsonBody", function (path, value) {
      const target = response(this);
      if (arguments.length === 0) {
        validJson(this, parse(target));
        return;
      }
      const json = jsonOf(target);
      const key = String(path);
      const at = lookup(json, key);
      if (arguments.length === 1) {
        this.assert(
          at.found,
          "expected #{act} in response to contain property #{exp}",
          "expected #{act} in response to not contain property #{exp}",
          key,
          json,
        );
        return;
      }
      this.assert(
        utils.eql(at.value, value),
        'expected response body json at "' + key + '" to contain #{exp} but got #{act}',
        'expected response body json at "' + key + '" to not contain #{exp} but got #{act}',
        value,
        at.value,
      );
    });
  };
}
