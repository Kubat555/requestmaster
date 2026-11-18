// `pm.request`, the old global `request` and `pm.execution`. The request itself lives on the Rust side
// (`src/host.rs`): it goes from script to script there, and the primitives refuse to change it once it has
// been sent. In a test script this code changes only its own copy.
import { PairList, cutAt } from "./lists.js";

// How the host lays the body out. The numbers are the ones `src/host.rs` expects.
const NONE = 0;
const RAW = 1;
const URLENCODED = 2;
const FORMDATA = 3;
const OTHER = 4;

/** The level of a warning in the console. */
const WARN = 2;

const SCHEME = /^[a-z][a-z0-9+.-]*:\/\//i;
const PORT = /:(\d*|\{\{[^{}]*\}\})$/;

/** `a=1&b=2` as `[[key, value], …]`; the value of a piece without `=` is `null`. Nothing is decoded. */
function parseQuery(query) {
  const pairs = [];
  if (typeof query !== "string") return pairs;
  const pieces = query.split("&");
  for (let i = 0; i < pieces.length; i++) {
    if (pieces[i] === "") continue;
    const cut = cutAt(pieces[i], "=");
    pairs[pairs.length] = [cut[0], cut[1] === undefined ? null : cut[1]];
  }
  return pairs;
}

/** `[{ key, value, disabled }, …]`, or one such object, as `[[key, value, disabled], …]`. */
function listPairs(list, isArray) {
  const items = isArray(list) ? list : [list];
  const pairs = [];
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item === null || typeof item !== "object") continue;
    pairs[pairs.length] = [item.key, item.value, !!item.disabled];
  }
  return pairs;
}

/** The part of an address before its `?`, without the scheme: `[host with port, path]`. */
function hostAndPath(base) {
  const scheme = SCHEME.exec(base);
  const rest = scheme === null ? base : base.slice(scheme[0].length);
  const slash = rest.indexOf("/");
  return slash < 0 ? [rest, ""] : [rest.slice(0, slash), rest.slice(slash)];
}

/**
 * `info` — `pm.info` before the script could touch it. Returns `{ api(), legacy() }`: `pm.request` (one
 * object per script, made on first use) and what the old global `request` holds. The host is asked for the
 * request once per script, and only when the script reads one of the two.
 */
export function createRequest(host, safe, info) {
  // Only a script that runs before the request changes it; later the primitives refuse.
  const writable = info.eventName === "prerequest";
  const id = info.requestId;
  const name = info.requestName;

  /** `[method, url, headers, mode, raw, fields, other]`, see `Primitives::request` in `src/host.rs`. */
  let loaded;
  function data() {
    if (loaded === undefined) loaded = safe.parse(host.req_get());
    return loaded;
  }

  /** Every text of the request goes to a primitive through here. */
  function text(value) {
    return safe.wellFormed(value === undefined || value === null ? "" : safe.String(value));
  }

  function warn(message) {
    host.log(WARN, safe.wellFormed(message));
  }

  /** The pairs of a list as a primitive takes them: a key, a value and "is disabled" for each. */
  function flat(list) {
    const parts = [];
    for (let i = 0; i < list.members.length; i++) {
      const member = list.members[i];
      parts[parts.length] = text(member.key);
      parts[parts.length] = text(member.value);
      parts[parts.length] = member.disabled === true;
    }
    return parts;
  }

  function build() {
    const got = data();

    // ---- method ----
    let method = got[0];

    // ---- address: an object over one string ----
    let raw = got[1];
    /** The address before its `?`, and its `#fragment` (with the `#`) or nothing. */
    let base = "";
    let fragment = "";
    const query = new PairList([], {
      exact: true,
      divider: "=",
      // An edit of the list rewrites the query in the string; the fragment stays at the end.
      changed() {
        let out = base;
        let first = true;
        for (let i = 0; i < query.members.length; i++) {
          const member = query.members[i];
          if (member.disabled) continue;
          out += (first ? "?" : "&") + safe.String(member.key);
          if (member.value !== null && member.value !== undefined) out += "=" + safe.String(member.value);
          first = false;
        }
        raw = out + fragment;
        if (writable) host.req_url(text(raw));
      },
    });

    function parse() {
      const hash = cutAt(raw, "#");
      const question = cutAt(hash[0], "?");
      base = question[0];
      fragment = hash[1] === undefined ? "" : "#" + hash[1];
      query.reset(parseQuery(question[1]));
    }
    parse();

    function queryString() {
      const found = cutAt(cutAt(raw, "#")[0], "?")[1];
      return found === undefined ? "" : found;
    }

    function path() {
      const found = hostAndPath(base)[1];
      return found === "" ? "/" : found;
    }

    const url = {
      query,
      toString() {
        return raw;
      },
      getRaw() {
        return raw;
      },
      /** Without the scheme and the port. */
      getHost() {
        return hostAndPath(base)[0].replace(PORT, "");
      },
      getPath() {
        return path();
      },
      /** Without the `?`. */
      getQueryString() {
        return queryString();
      },
      getPathWithQuery() {
        const found = queryString();
        return found === "" ? path() : path() + "?" + found;
      },
      /** `'a=1&b=2'` or `[{ key, value }, …]`. */
      addQueryParams(params) {
        const pairs = typeof params === "string" ? parseQuery(params) : listPairs(params, safe.isArray);
        for (let i = 0; i < pairs.length; i++) query.add({ key: pairs[i][0], value: pairs[i][1] });
      },
      /** A name or an array of names. */
      removeQueryParams(names) {
        const list = safe.isArray(names) ? names : [names];
        for (let i = 0; i < list.length; i++) {
          const item = list[i];
          query.remove(item !== null && typeof item === "object" ? item.key : item);
        }
      },
    };

    // ---- headers ----
    const headers = new PairList(got[2], {
      changed() {
        if (writable) host.req_headers(flat(headers));
      },
    });

    // ---- body ----
    let mode = got[3];
    let rawBody = got[4];
    const other = got[6];

    function modeName() {
      if (mode === RAW) return "raw";
      if (mode === URLENCODED) return "urlencoded";
      if (mode === FORMDATA) return "formdata";
      return mode === OTHER ? other : undefined;
    }

    /** A form-data, file or GraphQL body: a script sees its mode and cannot edit it. */
    function mayEdit() {
      if (mode !== FORMDATA && mode !== OTHER) return true;
      warn("pm.request.body: a " + modeName() + " body cannot be changed from a script");
      return false;
    }

    function writeBody() {
      if (!writable) return;
      if (mode === RAW) host.req_body(RAW, [text(rawBody)]);
      else host.req_body(URLENCODED, flat(urlencoded));
    }

    const urlencoded = new PairList(mode === URLENCODED ? got[5] : [], {
      exact: true,
      divider: "=",
      allow: mayEdit,
      // The body the script wrote last is the body: an edit of the fields makes it urlencoded.
      changed() {
        mode = URLENCODED;
        rawBody = null;
        writeBody();
      },
    });

    function setRaw(value) {
      mode = RAW;
      rawBody = value === undefined || value === null ? "" : safe.String(value);
      urlencoded.reset([]);
      writeBody();
    }

    const body = {
      urlencoded,
      /** `raw`, `urlencoded`, `formdata`, `file`, `graphql`; `undefined` — the request has no body. */
      get mode() {
        return modeName();
      },
      get raw() {
        return mode === RAW ? rawBody : undefined;
      },
      set raw(value) {
        if (mayEdit()) setRaw(value);
      },
      /** The text of a raw body; the enabled fields of an urlencoded one as `a=1&b=2`; otherwise nothing. */
      toString() {
        if (mode === RAW) return rawBody;
        let out = "";
        if (mode !== URLENCODED) return out;
        for (let i = 0; i < urlencoded.members.length; i++) {
          const member = urlencoded.members[i];
          if (member.disabled) continue;
          if (out !== "") out += "&";
          out += text(member.key) + "=" + text(member.value);
        }
        return out;
      },
      isEmpty() {
        if (mode === NONE) return true;
        if (mode === RAW) return rawBody === "";
        return mode === URLENCODED && urlencoded.count() === 0;
      },
      /** A string — a raw body; `{ mode: 'raw', raw }`; `{ mode: 'urlencoded', urlencoded }`. Replaces any body. */
      update(next) {
        if (typeof next === "string") {
          setRaw(next);
          return;
        }
        const wanted = next === null || typeof next !== "object" ? undefined : next.mode;
        if (wanted === "raw") {
          setRaw(next.raw);
        } else if (wanted === "urlencoded") {
          const fields = next.urlencoded;
          urlencoded.reset(typeof fields === "string" ? parseQuery(fields) : listPairs(fields, safe.isArray));
          mode = URLENCODED;
          rawBody = null;
          writeBody();
        } else {
          warn("pm.request.body: mode '" + safe.String(wanted) + "' cannot be set from a script");
        }
      },
    };

    const api = {
      headers,
      body,
      addHeader(item) {
        headers.add(item);
      },
      removeHeader(header) {
        headers.remove(header);
      },
      upsertHeader(item) {
        headers.upsert(item);
      },
    };
    Object.defineProperty(api, "method", {
      configurable: true,
      enumerable: true,
      get() {
        return method;
      },
      set(value) {
        method = safe.String(value);
        if (writable) host.req_method(text(method));
      },
    });
    // The object of the address is one per script: an assignment changes what it holds.
    Object.defineProperty(api, "url", {
      configurable: true,
      enumerable: true,
      get() {
        return url;
      },
      set(value) {
        raw = safe.String(value);
        parse();
        if (writable) host.req_url(text(raw));
      },
    });
    return api;
  }

  let api;
  return {
    api() {
      if (api === undefined) api = build();
      return api;
    },
    /**
     * The old `request`: the request as this script got it. `headers` — the enabled ones, keys in lower
     * case; `data` — the text of a raw body, the enabled fields of an urlencoded or a form-data one as an
     * object, otherwise `{}`. A name that repeats gives an array of its values.
     */
    legacy() {
      const got = data();
      let payload = {};
      if (got[3] === RAW) payload = got[4];
      else if (got[3] === URLENCODED || got[3] === FORMDATA) payload = new PairList(got[5], { exact: true }).toObject();
      return {
        id,
        name,
        method: got[0],
        url: got[1],
        headers: new PairList(got[2]).toObject(),
        data: payload,
      };
    },
  };
}

/** `pm.execution`. `skipRequest` exists only before the request, as in Postman. */
export function createExecution(host, info) {
  const execution = {
    /** Only a collection run has a next request. */
    setNextRequest() {},
  };
  if (info.eventName === "prerequest") {
    // The primitive always throws: the script ends here, and that is not an error.
    execution.skipRequest = function skipRequest() {
      host.skip_request();
    };
  }
  return execution;
}
