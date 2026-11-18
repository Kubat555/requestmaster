// A list of `{ key, value }` pairs: the headers of a response and of the request, the parameters of its query,
// the fields of its urlencoded body.

/** Sets an own property; an assignment would not for a key like `__proto__`. */
export function put(object, key, value) {
  Object.defineProperty(object, key, { value, writable: true, enumerable: true, configurable: true });
}

const lower = (name) => String(name).toLowerCase();

/** `text` cut at the first `divider`: `[before, after]`; `after` is `undefined` when there is no divider. */
export function cutAt(text, divider) {
  const at = text.indexOf(divider);
  return at < 0 ? [text, undefined] : [text.slice(0, at), text.slice(at + divider.length)];
}

export class PairList {
  /**
   * `pairs` — `[[key, value, disabled], …]` in their order (`disabled` may be left out). `options`:
   * - `exact` — names are compared with their case (parameters); otherwise without (headers);
   * - `divider` — what stands between the name and the value when a pair is given as one string;
   * - `allow()` — asked before every edit: `false` — the edit is not made;
   * - `changed()` — called after every edit.
   * `get`, `has`, `count` and `toObject` do not see a disabled pair; `all` and `each` give every pair.
   */
  constructor(pairs, options) {
    this.members = [];
    // Not enumerable: printing the list shows its pairs only.
    Object.defineProperty(this, "options", { value: options || {}, writable: false, enumerable: false });
    this.reset(pairs);
  }

  /** Replaces the pairs; this is not an edit of the script, so nobody is told. */
  reset(pairs) {
    this.members.length = 0;
    for (let i = 0; i < pairs.length; i++) {
      const member = { key: pairs[i][0], value: pairs[i][1] };
      if (pairs[i][2]) member.disabled = true;
      this.members[i] = member;
    }
  }

  same(key, wanted) {
    return this.options.exact ? String(key) === wanted : lower(key) === wanted;
  }

  wanted(name) {
    return this.options.exact ? String(name) : lower(name);
  }

  /** The value of the last pair with the name. */
  get(name) {
    const wanted = this.wanted(name);
    for (let i = this.members.length - 1; i >= 0; i--) {
      const member = this.members[i];
      if (!member.disabled && this.same(member.key, wanted)) return member.value;
    }
    return undefined;
  }

  has(name) {
    const wanted = this.wanted(name);
    for (let i = 0; i < this.members.length; i++) {
      const member = this.members[i];
      if (!member.disabled && this.same(member.key, wanted)) return true;
    }
    return false;
  }

  /** A copy: changing it does not change the list. */
  all() {
    const copy = [];
    for (let i = 0; i < this.members.length; i++) {
      const member = { key: this.members[i].key, value: this.members[i].value };
      if (this.members[i].disabled) member.disabled = true;
      copy[i] = member;
    }
    return copy;
  }

  each(fn) {
    for (let i = 0; i < this.members.length; i++) fn(this.members[i], i);
  }

  count() {
    let count = 0;
    for (let i = 0; i < this.members.length; i++) {
      if (!this.members[i].disabled) count++;
    }
    return count;
  }

  /** Keys in lower case unless the names are exact; a name that repeats gives an array of its values. */
  toObject() {
    const object = {};
    for (let i = 0; i < this.members.length; i++) {
      if (this.members[i].disabled) continue;
      const key = this.options.exact ? String(this.members[i].key) : lower(this.members[i].key);
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

  /** `{ key, value }` of what a script gives as a pair: an object, or one string with the divider. */
  pair(item) {
    if (typeof item === "string") {
      const cut = cutAt(item, this.options.divider || ":");
      const value = cut[1] === undefined ? "" : cut[1];
      return this.options.divider === undefined ? { key: cut[0].trim(), value: value.trim() } : { key: cut[0], value };
    }
    if (item === null || typeof item !== "object") return undefined;
    const pair = { key: item.key, value: item.value };
    if (item.disabled) pair.disabled = true;
    return pair;
  }

  /** Makes the edit when it is allowed, then tells of it. */
  edit(change) {
    const options = this.options;
    if (typeof options.allow === "function" && !options.allow()) return;
    change();
    if (typeof options.changed === "function") options.changed();
  }

  /** Adds a pair at the end: `{ key, value }` or `'Name: value'`. */
  add(item) {
    const pair = this.pair(item);
    if (pair === undefined) return;
    this.edit(() => {
      this.members[this.members.length] = pair;
    });
  }

  /** Changes the value of the last pair with the name that is not disabled, or adds the pair. */
  upsert(item) {
    const pair = this.pair(item);
    if (pair === undefined) return;
    this.edit(() => {
      const wanted = this.wanted(pair.key);
      for (let i = this.members.length - 1; i >= 0; i--) {
        const member = this.members[i];
        if (!member.disabled && this.same(member.key, wanted)) {
          member.value = pair.value;
          return;
        }
      }
      this.members[this.members.length] = pair;
    });
  }

  /** Removes every pair with the name, the disabled ones too. */
  remove(name) {
    this.edit(() => {
      const wanted = this.wanted(name);
      let kept = 0;
      for (let i = 0; i < this.members.length; i++) {
        if (!this.same(this.members[i].key, wanted)) this.members[kept++] = this.members[i];
      }
      this.members.length = kept;
    });
  }
}
