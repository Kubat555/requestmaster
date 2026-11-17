<script lang="ts">
  import { getContext, untrack } from "svelte";
  import AuthEditor from "./AuthEditor.svelte";
  import BodyEditor from "./BodyEditor.svelte";
  import KeyValueTable from "./KeyValueTable.svelte";
  import RequestSettingsEditor from "./RequestSettingsEditor.svelte";
  import ScriptsEditor from "./ScriptsEditor.svelte";
  import UrlBar from "./UrlBar.svelte";
  import type { Api } from "../lib/api";
  import { canSend } from "../lib/sending.svelte";
  import { slideIndicator } from "../lib/slideIndicator";
  import { strings } from "../lib/strings";
  import { tabs, type Tab } from "../lib/tabs.svelte";
  import { filledScripts, type ParamRow, type RequestDraft } from "../lib/types";
  import { provideVariableScope, VariableScope } from "../lib/variableScope.svelte";
  import { requestTokenNames } from "../lib/variables";

  interface Props {
    /** The tab of the request; the editors change its draft. */
    tab: Tab<RequestDraft>;
  }

  let { tab }: Props = $props();

  const api = getContext<Api>("api");

  /** The variables `{{name}}` resolves to in this request; the fields below mark their tokens by it. */
  const scope = provideVariableScope(new VariableScope(untrack(() => tab.nodeId)));

  // Read while the tab is shown, and again after something has changed variables; a hidden tab reads when shown.
  $effect(() => {
    if (tabs.active === tab && scope.stale) untrack(() => void scope.refresh(api));
  });

  /**
   * Names of the variables the request uses that are not found; none while the variables are not read. Counted
   * here, from the draft: a variable of an inherited auth or a nested `{{a{{b}}}}` is not seen, so the warning
   * after a send may name more.
   */
  const unresolved = $derived(scope.loaded ? requestTokenNames(tab.draft).filter((name) => scope.lookup(name) === null) : []);

  type Section = "params" | "headers" | "body" | "auth" | "scripts" | "settings";
  const SECTIONS: Section[] = ["params", "headers", "body", "auth", "scripts", "settings"];

  const id = $props.id();
  let section = $state<Section>("params");
  /**
   * Sections that have been shown. A section is created when it is first shown and then only hidden, so that
   * it keeps its state (the undo history of the body, for one) and the code editor is not loaded before it
   * is needed.
   */
  let visited = $state<Section[]>(["params"]);

  function enabled(rows: ParamRow[]): number {
    return rows.filter((row) => row.enabled).length;
  }

  /**
   * Number next to the name of a section: enabled rows, scripts that are not empty, settings that are set;
   * 0 — no number.
   */
  const counts = $derived<Record<Section, number>>({
    params: enabled(tab.draft.query) + enabled(tab.draft.path_variables),
    headers: enabled(tab.draft.headers),
    body: 0,
    auth: 0,
    scripts: filledScripts(tab.draft.scripts),
    settings: Object.values(tab.draft.settings).filter((setting) => setting !== null).length,
  });

  function show(next: Section) {
    section = next;
    if (!visited.includes(next)) visited.push(next);
  }

  function newRow(): ParamRow {
    return { key: "", value: "", enabled: true, postman_extra: {} };
  }
</script>

<div class="request-editor">
  <!-- What is sent is the draft: saving first is not needed. -->
  <UrlBar
    bind:method={tab.draft.method}
    bind:url={tab.draft.url}
    sending={tab.sending.state === "sending"}
    canSend={canSend(tab)}
    {unresolved}
    onsend={() => void tab.sending.send(api, tab)}
    oncancel={() => tab.sending.cancel(api)}
  />

  <div class="section-tabs" role="tablist" aria-label={strings.editor.sections} {@attach slideIndicator}>
    {#each SECTIONS as name (name)}
      <button
        type="button"
        role="tab"
        id="{id}-tab-{name}"
        aria-selected={section === name}
        aria-controls="{id}-panel-{name}"
        onclick={() => show(name)}
      >
        {strings.editor[name]}
        {#if counts[name] > 0}
          <span class="count">{counts[name]}</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="panel scroll" role="tabpanel" id="{id}-panel-params" aria-labelledby="{id}-tab-params" hidden={section !== "params"}>
    <KeyValueTable bind:rows={tab.draft.query} label={strings.editor.query} create={newRow} variables />
    <!-- Path variables come with the request; without any the block would be an empty table. The key is the
         `:name` of the address and is not substituted, so only the values mark variables. -->
    {#if tab.draft.path_variables.length > 0}
      <h3>{strings.editor.pathVariables}</h3>
      <KeyValueTable bind:rows={tab.draft.path_variables} label={strings.editor.pathVariables} create={newRow} variables="value" />
    {/if}
  </div>
  {#if visited.includes("headers")}
    <div class="panel scroll" role="tabpanel" id="{id}-panel-headers" aria-labelledby="{id}-tab-headers" hidden={section !== "headers"}>
      <KeyValueTable bind:rows={tab.draft.headers} label={strings.editor.headers} create={newRow} variables />
    </div>
  {/if}
  {#if visited.includes("body")}
    <div class="panel" role="tabpanel" id="{id}-panel-body" aria-labelledby="{id}-tab-body" hidden={section !== "body"}>
      <BodyEditor bind:body={tab.draft.body} />
    </div>
  {/if}
  {#if visited.includes("auth")}
    <div class="panel scroll" role="tabpanel" id="{id}-panel-auth" aria-labelledby="{id}-tab-auth" hidden={section !== "auth"}>
      <AuthEditor bind:auth={tab.draft.auth} inherit />
    </div>
  {/if}
  {#if visited.includes("scripts")}
    <div class="panel" role="tabpanel" id="{id}-panel-scripts" aria-labelledby="{id}-tab-scripts" hidden={section !== "scripts"}>
      <ScriptsEditor bind:scripts={tab.draft.scripts} />
    </div>
  {/if}
  {#if visited.includes("settings")}
    <div class="panel scroll" role="tabpanel" id="{id}-panel-settings" aria-labelledby="{id}-tab-settings" hidden={section !== "settings"}>
      <RequestSettingsEditor bind:settings={tab.draft.settings} />
    </div>
  {/if}
</div>

<style>
  .request-editor {
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

  h3 {
    margin: calc(var(--space) * 4) 0 calc(var(--space) * 2);
    color: var(--text-muted);
    font-size: var(--font-size);
    font-weight: 600;
  }
</style>
