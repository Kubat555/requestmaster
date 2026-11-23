// Sends that answer with what a script changed in the variables, for component tests.

import { Tab } from "../tabs.svelte";
import { normalizeContent, type RequestDraft, type SendResult, type VariableChange } from "../types";
import { noScriptResults } from "./fakeApi";

/** The result of a send whose scripts made `changes` and whose request was not sent after them. */
export function resultWithChanges(changes: VariableChange[]): SendResult {
  return {
    response: null,
    error: { kind: "skipped", message: "", fields: [] },
    warnings: [],
    ...noScriptResults(),
    variable_changes: changes,
  };
}

/** A request tab to send from; it is not among the tabs of the window. */
export function requestTab(): Tab<RequestDraft> {
  return new Tab<RequestDraft>(
    { id: "sender", kind: "request", name: "Sender" },
    normalizeContent("request", { method: "GET", url: "https://a.test" }),
  );
}
