<script lang="ts">
  import AuthEditor from "./AuthEditor.svelte";
  import DescriptionField from "./DescriptionField.svelte";
  import ScriptsEditor from "./ScriptsEditor.svelte";
  import { strings } from "../lib/strings";
  import type { Tab } from "../lib/tabs.svelte";
  import { filledScripts, type FolderDraft } from "../lib/types";

  interface Props {
    /** The tab of the folder; the editors change its draft. */
    tab: Tab<FolderDraft>;
  }

  let { tab }: Props = $props();

  type Section = "auth" | "scripts" | "description";
  const SECTIONS: Section[] = ["auth", "scripts", "description"];
  /** Names of the sections; the description is called the same as in a collection. */
  const LABELS: Record<Section, string> = {
    auth: strings.folder.auth,
    scripts: strings.folder.scripts,
    description: strings.collection.description,
  };

  const id = $props.id();
  let section = $state<Section>("auth");
  /**
   * The scripts have been shown. Their editor is created when the section is first shown and then only hidden,
   * so that the code editor is not loaded before it is needed and keeps its state afterwards.
   */
  let scriptsShown = $state(false);

  /** Number next to the name of a section: scripts that are not empty; 0 — no number. */
  const counts = $derived<Record<Section, number>>({
    auth: 0,
    scripts: filledScripts(tab.draft.scripts),
    description: 0,
  });

  function show(next: Section) {
    section = next;
    if (next === "scripts") scriptsShown = true;
  }
</script>

<!-- What a folder gives to the requests inside it: the authorization and the scripts; and its description. It has no variables. -->
<div class="folder-editor">
  <div class="section-tabs" role="tablist" aria-label={strings.folder.sections}>
    {#each SECTIONS as name (name)}
      <button
        type="button"
        role="tab"
        id="{id}-tab-{name}"
        aria-selected={section === name}
        aria-controls="{id}-panel-{name}"
        onclick={() => show(name)}
      >
        {LABELS[name]}
        {#if counts[name] > 0}
          <span class="count">{counts[name]}</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="panel scroll" role="tabpanel" id="{id}-panel-auth" aria-labelledby="{id}-tab-auth" hidden={section !== "auth"}>
    <!-- A folder always has a parent to inherit from: a collection or another folder. -->
    <AuthEditor bind:auth={tab.draft.auth} inherit />
  </div>
  {#if scriptsShown}
    <div class="panel" role="tabpanel" id="{id}-panel-scripts" aria-labelledby="{id}-tab-scripts" hidden={section !== "scripts"}>
      <ScriptsEditor bind:scripts={tab.draft.scripts} />
    </div>
  {/if}
  <div class="panel scroll" role="tabpanel" id="{id}-panel-description" aria-labelledby="{id}-tab-description" hidden={section !== "description"}>
    <DescriptionField bind:value={tab.draft.description} />
  </div>
</div>

<style>
  .folder-editor {
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
</style>
