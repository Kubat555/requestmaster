// `console.log / info / warn / error / debug`: turns the arguments into one line of text. The exact cut of
// the line and the limits of the console are the business of the primitive.

/** Twice `CONSOLE_LINE_CHARS`, in UTF-16 units: a rough cut, so that no text of any length crosses the border. */
const LINE_UNITS = 20000;

const LOG = 0;
const INFO = 1;
const WARN = 2;
const ERROR = 3;
const DEBUG = 4;

export function installConsole(host, safe) {
  /** `JSON.stringify` where an object that contains itself is written as "[Circular]". */
  function json(value) {
    const ancestors = [];
    let depth = 0;
    return safe.stringify(value, function (key, item) {
      if (typeof item !== "object" || item === null) return item;
      while (depth > 0 && ancestors[depth - 1] !== this) depth--;
      for (let i = 0; i < depth; i++) {
        if (ancestors[i] === item) return "[Circular]";
      }
      ancestors[depth++] = item;
      return item;
    });
  }

  function format(value) {
    if (typeof value === "string") return value;
    if (value === undefined) return "undefined";
    if (typeof value === "function") {
      const name = typeof value.name === "string" && value.name !== "" ? value.name : "anonymous";
      return "[Function: " + name + "]";
    }
    if (typeof value === "bigint") return safe.String(value) + "n";
    if (typeof value === "object" && value !== null) {
      const name = value.name;
      const message = value.message;
      if (typeof name === "string" && typeof message === "string") return name + ": " + message;
    }
    const text = json(value);
    return typeof text === "string" ? text : safe.String(value);
  }

  function part(value) {
    try {
      return format(value);
    } catch (error) {
      try {
        return safe.String(value);
      } catch (again) {
        return "[unprintable]";
      }
    }
  }

  function print(level, values) {
    let line = "";
    for (let i = 0; i < values.length; i++) {
      if (i > 0) line += " ";
      line += part(values[i]);
      if (line.length > LINE_UNITS) {
        line = safe.slice(line, 0, LINE_UNITS);
        break;
      }
    }
    host.log(level, safe.wellFormed(line));
  }

  globalThis.console = {
    log(...values) {
      print(LOG, values);
    },
    info(...values) {
      print(INFO, values);
    },
    warn(...values) {
      print(WARN, values);
    },
    error(...values) {
      print(ERROR, values);
    },
    debug(...values) {
      print(DEBUG, values);
    },
    clear() {},
  };
}
