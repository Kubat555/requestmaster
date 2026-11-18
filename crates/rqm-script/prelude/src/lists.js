// A list of `{ key, value }` pairs: the headers of a response.

/** Sets an own property; an assignment would not for a key like `__proto__`. */
export function put(object, key, value) {
  Object.defineProperty(object, key, { value, writable: true, enumerable: true, configurable: true });
}

const lower = (name) => String(name).toLowerCase();

export class PairList {
  /** `pairs` — `[[key, value], …]` in their order. */
  constructor(pairs) {
    this.members = [];
    for (let i = 0; i < pairs.length; i++) {
      this.members[i] = { key: pairs[i][0], value: pairs[i][1] };
    }
  }

  /** The value of the last pair with the name; the case of the name does not matter. */
  get(name) {
    const wanted = lower(name);
    for (let i = this.members.length - 1; i >= 0; i--) {
      if (lower(this.members[i].key) === wanted) return this.members[i].value;
    }
    return undefined;
  }

  has(name) {
    const wanted = lower(name);
    for (let i = 0; i < this.members.length; i++) {
      if (lower(this.members[i].key) === wanted) return true;
    }
    return false;
  }

  /** A copy: changing it does not change the list. */
  all() {
    const copy = [];
    for (let i = 0; i < this.members.length; i++) {
      copy[i] = { key: this.members[i].key, value: this.members[i].value };
    }
    return copy;
  }

  each(fn) {
    for (let i = 0; i < this.members.length; i++) fn(this.members[i], i);
  }

  count() {
    return this.members.length;
  }

  /** Keys in lower case; a name that repeats gives an array of its values. */
  toObject() {
    const object = {};
    for (let i = 0; i < this.members.length; i++) {
      const key = lower(this.members[i].key);
      const value = this.members[i].value;
      if (!Object.hasOwn(object, key)) {
        put(object, key, value);
      } else if (Array.isArray(object[key])) {
        object[key][object[key].length] = value;
      } else {
        put(object, key, [object[key], value]);
      }
    }
    return object;
  }
}
