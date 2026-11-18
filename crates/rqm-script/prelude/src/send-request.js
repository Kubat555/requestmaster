// `pm.sendRequest`. Lays the request of the script out for the primitive; how many requests a script may
// make, how large one may be and how much may wait in the queue is the business of the primitive
// (`src/host.rs`). The engine sends the requests one at a time and calls `answer` with what came back.
//
// Nothing is substituted here: the address, the headers and the body go out as the script gave them, as in
// Postman. A script that wants `{{variables}}` replaced calls `pm.variables.replaceIn`.

// How the body is laid out. The numbers are the ones `src/host.rs` expects.
const NONE = 0;
const RAW = 1;
const URLENCODED = 2;
const FORMDATA = 3;

/** The level of a warning in the console. */
const WARN = 2;

const NO_FILES = "uploading files from scripts is not allowed";
const TOO_LARGE = "pm.sendRequest: the request is larger than 10 MB";
const NOTHING = "sendrequest: nothing to request";

/**
 * `later(fn)` calls `fn` when the script has nothing left to run at once; `makeAnswer(data, number)` builds
 * the response object of an answer, of the same factory as `pm.response`.
 */
export function createSendRequest(host, safe, pm, later, makeAnswer) {
  /** Number of a request → the function that takes its answer: `(error)` or `(null, response)`. */
  const waiting = { __proto__: null };

  /** Every text of a request goes to the primitive through here. */
  function text(value) {
    return safe.wellFormed(value === undefined || value === null ? "" : safe.String(value));
  }

  function warn(message) {
    host.log(WARN, safe.wellFormed(message));
  }

  /**
   * `[{ key, value, disabled }, …]`: the pairs that are not disabled, added to `parts`. With `files` a part
   * that is a file is left out; returns whether there was one.
   */
  function addPairs(parts, list, files) {
    let cut = false;
    for (let i = 0; i < list.length; i++) {
      const item = list[i];
      if (item === null || typeof item !== "object" || item.disabled) continue;
      if (files && (item.type === "file" || (item.src !== undefined && item.src !== null))) {
        cut = true;
        continue;
      }
      parts[parts.length] = text(item.key);
      parts[parts.length] = text(item.value);
    }
    return cut;
  }

  /** `Name: value` lines of headers, or `a=1&b=2` of a body, added to `parts`. */
  function addLines(parts, source, separator, divider, trim) {
    const lines = source.split(separator);
    for (let i = 0; i < lines.length; i++) {
      const line = safe.String(lines[i]);
      const at = line.indexOf(divider);
      let key = at < 0 ? line : line.slice(0, at);
      let value = at < 0 ? "" : line.slice(at + 1);
      if (trim) {
        key = key.trim();
        value = value.trim();
      }
      if (key === "" && value === "") continue;
      parts[parts.length] = text(key);
      parts[parts.length] = text(value);
    }
  }

  function addHeaders(parts, header) {
    if (typeof header === "string") {
      addLines(parts, header, "\n", ":", true);
    } else if (safe.isArray(header)) {
      addPairs(parts, header, false);
    } else if (header !== null && typeof header === "object") {
      const names = safe.keys(header);
      for (let i = 0; i < names.length; i++) {
        parts[parts.length] = text(names[i]);
        parts[parts.length] = text(header[names[i]]);
      }
    }
  }

  /** Adds the body to `parts` and returns how it is laid out. A file never gets there. */
  function addBody(parts, body) {
    if (body === null || typeof body !== "object") return NONE;
    const mode = body.mode;
    if (mode === undefined || mode === null) return NONE;
    if (mode === "raw") {
      const raw = typeof body.raw === "string" ? body.raw : safe.stringify(body.raw);
      if (typeof raw !== "string") return NONE;
      parts[parts.length] = text(raw);
      return RAW;
    }
    if (mode === "urlencoded") {
      const fields = body.urlencoded;
      if (typeof fields === "string") addLines(parts, fields, "&", "=", false);
      else if (safe.isArray(fields)) addPairs(parts, fields, false);
      return URLENCODED;
    }
    if (mode === "formdata") {
      if (safe.isArray(body.formdata) && addPairs(parts, body.formdata, true)) warn(NO_FILES);
      return FORMDATA;
    }
    if (mode === "file") {
      warn(NO_FILES);
      return NONE;
    }
    warn("pm.sendRequest: body mode '" + safe.slice(safe.String(mode), 0, 100) + "' is not supported");
    return NONE;
  }

  function sendRequest(request, callback) {
    // Without a callback the answer settles a promise.
    let done = callback;
    let outcome = pm;
    if (typeof callback !== "function") {
      outcome = new safe.Promise(function (resolve, reject) {
        done = function (error, response) {
          if (error) reject(error);
          else resolve(response);
        };
      });
    }
    /** An error that is not a network one reaches the script the same way, and not before `sendRequest` returns. */
    function refuse(message) {
      later(function () {
        done(new safe.Error(message));
      });
      return outcome;
    }

    const address = typeof request === "string";
    const options = !address && request !== null && typeof request === "object" ? request : {};
    let url = address ? request : options.url;
    if (url !== null && typeof url === "object") url = url.raw;
    if (typeof url !== "string" || url === "") return refuse(NOTHING);

    const method = options.method;
    const parts = [method === undefined || method === null ? "GET" : text(method), text(url)];
    addHeaders(parts, options.header);
    const headers = (parts.length - 2) / 2;
    const mode = addBody(parts, options.body);

    // 0 — the request is too large; the call over the limit of calls throws.
    const number = host.request(parts, headers, mode);
    if (number === 0) return refuse(TOO_LARGE);
    waiting[number] = done;
    return outcome;
  }

  // The engine calls this with the answer to the request of that number: `failure` — the text of the network
  // error, otherwise `described` — the JSON of the response. An exception of the callback goes back to the
  // engine: it is the error of the script.
  host.answer = function (number, failure, described) {
    const done = waiting[number];
    if (done === undefined) return;
    delete waiting[number];
    if (typeof failure === "string") done(new safe.Error(failure));
    else done(null, makeAnswer(safe.parse(described), number));
  };

  return sendRequest;
}
