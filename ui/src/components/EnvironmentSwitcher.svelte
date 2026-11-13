<script lang="ts">
  import { getContext } from "svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  const api = getContext<Api>("api");

  /** Value of the option that stands for "no environment"; an environment id is never empty. */
  const NONE = "";

  function onChange(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    void workspace.setEnvironment(api, value === NONE ? null : value);
  }
</script>

<select class="switcher" aria-label={strings.environment.label} value={workspace.activeEnvironmentId ?? NONE} onchange={onChange}>
  <option value={NONE}>{strings.environment.none}</option>
  {#each workspace.environments as environment (environment.id)}
    <option value={environment.id}>{environment.name}</option>
  {/each}
</select>

<style>
  .switcher {
    flex: none;
    height: var(--row-height);
    max-width: 240px;
    margin: 0 calc(var(--space) * 2);
    padding: 0 calc(var(--space) * 2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
</style>
