// `pm.response`. The body stays on the Rust side until a script asks for it: the primitive reads it, checks
// its size and turns it into text (`src/host.rs`).
import { PairList } from "./lists.js";

const TOO_LARGE = "response body is larger than 10 MB and is not available to scripts";

/** Every object the factory below has made: by this an assertion tells a response from anything else. */
const responses = new WeakSet();

export function isResponse(value) {
  return typeof value === "object" && value !== null && responses.has(value);
}

/**
 * `undefined` when the scripts run before the request. `chai()` gives chai with the assertions of a
 * response and loads it on first use.
 */
export function createResponse(host, safe, chai) {
  const described = host.response();
  if (typeof described !== "string") return undefined;
  const data = safe.parse(described);

  // Read on first use and kept for this script.
  let asked = false;
  let text;

  /** The text of the body; `undefined` — it is larger than the limit. Throws when it cannot be read. */
  function body() {
    if (!asked) {
      const read = host.response_body();
      text = typeof read === "string" ? read : undefined;
      asked = true;
    }
    return text;
  }

  function bodyText() {
    const read = body();
    if (read === undefined) throw new Error(TOO_LARGE);
    return read;
  }

  const api = {
    code: data.code,
    status: data.status,
    responseTime: data.time,
    responseSize: data.size,
    headers: new PairList(data.headers),
    reason() {
      return data.status;
    },
    text() {
      return bodyText();
    },
    json() {
      const read = bodyText();
      try {
        return safe.parse(read);
      } catch (cause) {
        const error = new Error(cause && typeof cause.message === "string" ? cause.message : safe.String(cause));
        error.name = "JSONError";
        throw error;
      }
    },
  };

  // `pm.response.to.have.status(200)`. Not enumerable: printing the response does not load chai.
  Object.defineProperty(api, "to", {
    configurable: true,
    enumerable: false,
    get() {
      return chai().expect(api).to;
    },
  });
  responses.add(api);

  return { api, data, body };
}
