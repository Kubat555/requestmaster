import { CompletionContext, type Completion } from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import apiTable from "../../../../crates/rqm-script/prelude/src/api.json?raw";
import { SCRIPT_API } from "../scriptApi";
import { scriptCompletions } from "./scriptCompletions";

/** What the source answers for `text` with the caret at its end. */
function complete(text: string, explicit = false) {
  const state = EditorState.create({ doc: text });
  return scriptCompletions(new CompletionContext(state, text.length, explicit));
}

/** The options for `text`, by their labels; an empty list when nothing is offered. */
function options(text: string, explicit = false): Map<string, Completion> {
  return new Map((complete(text, explicit)?.options ?? []).map((option) => [option.label, option]));
}

describe("scriptCompletions", () => {
  it("list_matches_the_table_of_the_core", () => {
    const table = JSON.parse(apiTable) as { members: { name: string; kind: string; status: string }[] };
    const supported = table.members
      .filter((member) => member.status === "v1" || member.status === "noop")
      .map((member) => `${member.name} ${member.kind}`);
    const listed = SCRIPT_API.map((item) => `${item.name} ${item.kind}`);

    expect(supported.length).toBeGreaterThan(0);
    expect(new Set(listed).size).toBe(listed.length);
    expect([...listed].sort()).toEqual([...supported].sort());
  });

  it("members_after_a_dot", () => {
    const members = complete("pm.");
    expect(members?.from).toBe(3);
    const labels = members?.options.map((option) => option.label) ?? [];
    for (const name of ["environment", "test", "response", "sendRequest"]) expect(labels).toContain(name);

    const typed = complete("pm.environment.s");
    expect(typed?.from).toBe("pm.environment.".length);
    expect(typed?.options).toEqual([{ label: "set", type: "function", detail: "set(key, value)" }]);

    expect(options("pm.response.to.have.").has("status")).toBe(true);
    expect(options("postman.").has("setEnvironmentVariable")).toBe(true);
    // A segment that goes on is a namespace, not a member.
    expect(options("pm.").get("environment")?.type).toBe("namespace");
  });

  it("top_level_names", () => {
    const typed = options("p");
    expect(typed.has("pm")).toBe(true);
    expect(typed.has("postman")).toBe(true);

    expect(complete("")).toBeNull();
    const asked = options("", true);
    for (const name of ["pm", "console", "tests", "responseBody"]) expect(asked.has(name)).toBe(true);

    expect(complete("foo.")).toBeNull();
    expect(complete("x = 1")).toBeNull();
  });

  it("unsupported_names_are_not_offered", () => {
    const typed = options("pm.va");
    expect(typed.has("variables")).toBe(true);
    expect(typed.has("vault")).toBe(false);
  });
});
