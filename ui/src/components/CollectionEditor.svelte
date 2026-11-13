<script lang="ts">
  import AuthEditor from "./AuthEditor.svelte";
  import KeyValueTable from "./KeyValueTable.svelte";
  import { strings } from "../lib/strings";
  import type { Tab } from "../lib/tabs.svelte";
  import type { CollectionDraft, VariableRow } from "../lib/types";

  interface Props {
    /** The tab of the collection; the editors change its draft. */
    tab: Tab<CollectionDraft>;
  }

  let { tab }: Props = $props();

  type Section = "variables" | "auth";
  const SECTIONS: Section[] = ["variables", "auth"];

  const id = $props.id();
  let section = $state<Section>("variables");

  /** Number of enabled variables next to the name of the section; 0 — no number. */
  const enabledVariables = $derived(tab.draft.variables.filter((variable) => variable.enabled).length);

  function newVariable(): VariableRow {
    return { key: "", value: "", secret: false, enabled: true, postman_extra: {} };
  }
</script>

<div class="collection-editor">
  <div class="section-tabs" role="tablist" aria-label={strings.collection.sections}>
    {#each SECTIONS as name (name)}
      <button
        type="button"
        role="tab"
        id="{id}-tab-{name}"
        aria-selected={section === name}
        aria-controls="{id}-panel-{name}"
        onclick={() => (section = name)}
      >
        {strings.collection[name]}
        {#if name === "variables" && enabledVariables > 0}
          <span class="count">{enabledVariables}</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="panel" role="tabpanel" id="{id}-panel-variables" aria-labelledby="{id}-tab-variables" hidden={section !== "variables"}>
    <KeyValueTable
      bind:rows={tab.draft.variables}
      label={strings.collection.variables}
      keyLabel={strings.collection.name}
      create={newVariable}
      secret={(variable) => variable.secret}
    />
  </div>
  <div class="panel" role="tabpanel" id="{id}-panel-auth" aria-labelledby="{id}-tab-auth" hidden={section !== "auth"}>
    <!-- A collection has no parent to inherit from. -->
    <AuthEditor bind:auth={tab.draft.auth} inherit={false} />
  </div>
</div>

<style>
  .collection-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .panel {
    flex: 1;
    min-height: 0;
    padding: calc(var(--space) * 2);
    overflow: auto;
  }
</style>
