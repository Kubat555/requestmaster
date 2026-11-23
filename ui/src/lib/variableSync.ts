// What the scripts of a send changed in the variables, brought to the editors that are open: the core has
// written the changes to the storage already, and an editor that goes on showing the old rows would write
// them back with its next save.

import { observeSends } from "./sending.svelte";
import { tabs } from "./tabs.svelte";
import { GLOBALS_ID, type VariableChange, type VariableRow } from "./types";
import { bumpVariables } from "./variableScope.svelte";
import { workspace } from "./workspace.svelte";

/** Removes the rows that match, in place: the rows that stay are the same objects. */
function removeRows(rows: VariableRow[], matches: (row: VariableRow) => boolean): void {
  for (let index = rows.length - 1; index >= 0; index -= 1) {
    if (matches(rows[index])) rows.splice(index, 1);
  }
}

/** The rule of the core (`apply_to_variables`) for the rows of an open editor; changes `rows` in place. */
export function applyToRows(rows: VariableRow[], change: VariableChange): void {
  switch (change.op) {
    case "set": {
      const value = change.value ?? "";
      for (let index = rows.length - 1; index >= 0; index -= 1) {
        if (rows[index].enabled && rows[index].key === change.key) {
          rows[index].value = value;
          return;
        }
      }
      rows.push({ key: change.key, value, secret: false, enabled: true, postman_extra: {} });
      return;
    }
    case "unset":
      removeRows(rows, (row) => row.enabled && row.key === change.key);
      return;
    case "clear":
      removeRows(rows, (row) => row.enabled);
      return;
  }
}

/** The changes by the set they were made in, each group in the order of the changes. */
function byTarget(changes: VariableChange[]): Map<string, VariableChange[]> {
  const groups = new Map<string, VariableChange[]>();
  for (const change of changes) {
    const group = groups.get(change.target_id);
    if (group) group.push(change);
    else groups.set(change.target_id, [change]);
  }
  return groups;
}

/** Keeps the open editors in step with what scripts change; the returned function stops it. */
export function installVariableSync(): () => void {
  return observeSends({
    // A script sees what is on the screen: the edits that wait for their save are written first.
    async before(api) {
      for (const id of [workspace.activeEnvironmentId, GLOBALS_ID]) {
        if (id !== null) await tabs.environment(id)?.flush(api);
      }
    },
    settled(api, result) {
      const changes = result.variable_changes;
      if (changes.length === 0) return;
      for (const [targetId, group] of byTarget(changes)) {
        if (group[0].scope === "collection") {
          const tab = tabs.list.find((candidate) => candidate.kind === "collection" && candidate.nodeId === targetId);
          if (tab?.kind === "collection") tab.applyVariableChanges(group);
        } else {
          tabs.environment(targetId)?.applyChanges(api, group);
        }
      }
      // Whatever is open, the requests resolve `{{name}}` by the changed variables from now on.
      bumpVariables();
    },
  });
}
