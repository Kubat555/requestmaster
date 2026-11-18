// `pm.response`. The body stays on the Rust side until a script asks for it: the primitive reads it, checks
// its size and turns it into text (`src/host.rs`).
import { PairList } from "./lists.js";

const TOO_LARGE = "response body is larger than 10 MB and is not available to scripts";

/** `undefined` when the scripts run before the request. */
export function createResponse(host, safe) {
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

  return { api, data, body };
}
