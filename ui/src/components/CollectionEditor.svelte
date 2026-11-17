<script lang="ts">
  import AuthEditor from "./AuthEditor.svelte";
  import KeyValueTable from "./KeyValueTable.svelte";
  import ScriptsEditor from "./ScriptsEditor.svelte";
  import { strings } from "../lib/strings";
  import type { Tab } from "../lib/tabs.svelte";
  import type { CollectionDraft, VariableRow } from "../lib/types";

  interface Props {
    /** The tab of the collection; the editors change its draft. */
    tab: Tab<CollectionDraft>;
  }

  let { tab }: Props = $props();

  type Section = "variables" | "auth" | "scripts" | "description";
  const SECTIONS: Section[] = ["variables", "auth", "scripts", "description"];

  const id = $props.id();
  let section = $state<Section>("variables");
  /**
   * The scripts have been shown. Their editor is created when the section is first shown and then only hidden,
   * so that the code editor is not loaded before it is needed and keeps its state afterwards.
   */
  let scriptsShown = $state(false);

  /** Number next to the name of a section: enabled variables, scripts that are not empty; 0 — no number. */
  const counts = $derived<Record<Section, number>>({
    variables: tab.draft.variables.filter((variable) => variable.enabled).length,
    auth: 0,
    scripts: Object.values(tab.draft.scripts).filter((script) => script.trim() !== "").length,
    description: 0,
  });

  function show(next: Section) {
    section = next;
    if (next === "scripts") scriptsShown = true;
  }

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
        onclick={() => show(name)}
      >
        {strings.collection[name]}
        {#if counts[name] > 0}
          <span class="count">{counts[name]}</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="panel scroll" role="tabpanel" id="{id}-panel-variables" aria-labelledby="{id}-tab-variables" hidden={section !== "variables"}>
    <KeyValueTable
      bind:rows={tab.draft.variables}
      label={strings.collection.variables}
      keyLabel={strings.collection.name}
      create={newVariable}
      secret={(variable) => variable.secret}
    />
  </div>
  <div class="panel scroll" role="tabpanel" id="{id}-panel-auth" aria-labelledby="{id}-tab-auth" hidden={section !== "auth"}>
    <!-- A collection has no parent to inherit from. -->
    <AuthEditor bind:auth={tab.draft.auth} inherit={false} />
  </div>
  {#if scriptsShown}
    <div class="panel" role="tabpanel" id="{id}-panel-scripts" aria-labelledby="{id}-tab-scripts" hidden={section !== "scripts"}>
      <ScriptsEditor bind:scripts={tab.draft.scripts} />
    </div>
  {/if}
  <div class="panel scroll" role="tabpanel" id="{id}-panel-description" aria-labelledby="{id}-tab-description" hidden={section !== "description"}>
    <!-- The source text as it is kept: Markdown is not rendered. -->
    <textarea class="description" aria-label={strings.collection.description} spellcheck="false" bind:value={tab.draft.description}></textarea>
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
  }

  .scroll {
    padding: calc(var(--space) * 2);
    overflow: auto;
  }
  .description {
    display: block;
    width: 100%;
    height: 100%;
    padding: calc(var(--space) * 2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    resize: none;
  }
</style>
