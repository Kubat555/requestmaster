import { describe, expect, it } from "vitest";
import type { VariableChange, VariableRow } from "./types";
import { applyToRows } from "./variableSync";

function row(key: string, value: string, more: Partial<VariableRow> = {}): VariableRow {
  return { key, value, secret: false, enabled: true, postman_extra: {}, ...more };
}

function off(key: string, value: string): VariableRow {
  return row(key, value, { enabled: false });
}

/** `b`: secret, with keys of Postman the editor does not show. */
function secretB(): VariableRow {
  return row("b", "3", { secret: true, postman_extra: { id: "x", type: "secret" } });
}

/** The list the table of the core starts from: `a=1`; `a=2` switched off; the secret `b=3`; `a=4`. */
function initial(): VariableRow[] {
  return [row("a", "1"), off("a", "2"), secretB(), row("a", "4")];
}

function change(op: VariableChange["op"], key = "", value: string | null = null): VariableChange {
  return { scope: "environment", target_id: "env", op, key, value };
}

/** The rows after one change. */
function after(rows: VariableRow[], one: VariableChange): VariableRow[] {
  applyToRows(rows, one);
  return rows;
}

describe("applyToRows", () => {
  // The table of `one_rule_for_set_unset_clear` of the core (crates/rqm-runner/tests/variable_changes.rs).
  it("rule_matches_the_core", () => {
    expect(after(initial(), change("set", "a", "9"))).toEqual([row("a", "1"), off("a", "2"), secretB(), row("a", "9")]);
    expect(after(initial(), change("set", "b", "t"))).toEqual([
      row("a", "1"),
      off("a", "2"),
      { ...secretB(), value: "t" },
      row("a", "4"),
    ]);
    expect(after(initial(), change("set", "n", "v"))).toEqual([...initial(), row("n", "v")]);
    expect(after([off("d", "0")], change("set", "d", "1"))).toEqual([off("d", "0"), row("d", "1")]);
    expect(after(initial(), change("unset", "a"))).toEqual([off("a", "2"), secretB()]);
    expect(after(initial(), change("clear"))).toEqual([off("a", "2")]);
  });

  it("untouched_rows_keep_their_identity", () => {
    const rows = initial();
    const [first, disabled, secret, last] = rows;

    applyToRows(rows, change("set", "a", "9"));
    applyToRows(rows, change("set", "n", "v"));
    // The same objects in the same array: the editor keeps what it knows about a row (a shown secret).
    expect(rows.slice(0, 4).every((item, index) => item === [first, disabled, secret, last][index])).toBe(true);

    applyToRows(rows, change("unset", "a"));
    expect(rows[0]).toBe(disabled);
    expect(rows[1]).toBe(secret);
  });
});
