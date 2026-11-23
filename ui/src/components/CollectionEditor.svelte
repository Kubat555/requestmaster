<script lang="ts">
  import { getContext } from "svelte";
  import AuthEditor from "./AuthEditor.svelte";
  import DescriptionField from "./DescriptionField.svelte";
  import KeyValueTable from "./KeyValueTable.svelte";
  import ScriptsEditor from "./ScriptsEditor.svelte";
  import type { Api } from "../lib/api";
  import { slideIndicator } from "../lib/slideIndicator";
  import { strings } from "../lib/strings";
  import type { Tab } from "../lib/tabs.svelte";
  import { toasts } from "../lib/toasts.svelte";
  import { filledScripts, type CollectionDraft, type VariableRow } from "../lib/types";

  interface Props {
    /** The tab of the collection; the editors change its draft. */
    tab: Tab<CollectionDraft>;
  }

  let { tab }: Props = $props();

  const api = getContext<Api>("api");

  type Section = "variables" | "auth" | "scripts" | "description";
  const SECTIONS: Section[] = ["variables", "auth", "scripts", "description"];

  const id = $props.id();
  let section = $state<Section>("variables");
  /**
   * The scripts have been shown. Their editor is created when the section is first shown and then only hidden,
   * so that the code editor is not loaded before it is needed and keeps its state afterwards.
   */
  let scriptsShown = $state(false);
  /**
   * The scripts of the collection may run; `null` — not read yet. Kept by the core apart from the content of
   * the collection: it is not a part of the draft and acts at once.
   */
  let scriptsAllowed = $state<boolean | null>(null);

  /** Number next to the name of a section: enabled variables, scripts that are not empty; 0 — no number. */
  const counts = $derived<Record<Section, number>>({
    variables: tab.draft.variables.filter((variable) => variable.enabled).length,
    auth: 0,
    scripts: filledScripts(tab.draft.scripts),
    description: 0,
  });

  function show(next: Section) {
    section = next;
    if (next !== "scripts" || scriptsShown) return;
    scriptsShown = true;
    api.scriptsAllowed(tab.nodeId).then(
      (allowed) => (scriptsAllowed = allowed),
      (error) => toasts.error(String(error)),
    );
  }

  /** Gives or takes back the consent at once; a refusal of the command puts the mark back. */
  async function allowScripts(allowed: boolean) {
    scriptsAllowed = allowed;
    try {
      await api.setScriptsAllowed(tab.nodeId, allowed);
    } catch (error) {
      scriptsAllowed = !allowed;
      toasts.error(String(error));
    }
  }

  function newVariable(): VariableRow {
    return { key: "", value: "", secret: false, enabled: true, postman_extra: {} };
  }
</script>

<div class="collection-editor">
  <div class="section-tabs" role="tablist" aria-label={strings.collection.sections} {@attach slideIndicator}>
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
      <div class="scripts">
        <div class="consent">
          <label class="choice">
            <input
              type="checkbox"
              checked={scriptsAllowed ?? false}
              disabled={scriptsAllowed === null}
              aria-describedby="{id}-allow-note"
              onchange={(event) => allowScripts(event.currentTarget.checked)}
            />
            {strings.collection.allowScripts}
          </label>
          <span class="note" id="{id}-allow-note">{strings.collection.allowScriptsNote}</span>
        </div>
        <div class="code">
          <ScriptsEditor bind:scripts={tab.draft.scripts} />
        </div>
      </div>
    </div>
  {/if}
  <div class="panel scroll" role="tabpanel" id="{id}-panel-description" aria-labelledby="{id}-tab-description" hidden={section !== "description"}>
    <DescriptionField bind:value={tab.draft.description} />
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

  /* The consent above the editor of the scripts, which takes the rest of the height. */
  .scripts {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .consent {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: var(--space);
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
    border-bottom: 1px solid var(--border);
  }

  .choice {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 2);
  }

  .choice input {
    margin: 0;
    accent-color: var(--accent);
  }

  .note {
    color: var(--text-muted);
  }

  .code {
    flex: 1;
    min-height: 0;
  }
</style>
