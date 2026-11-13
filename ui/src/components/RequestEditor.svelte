<script lang="ts">
  import { getContext } from "svelte";
  import AuthEditor from "./AuthEditor.svelte";
  import BodyEditor from "./BodyEditor.svelte";
  import KeyValueTable from "./KeyValueTable.svelte";
  import UrlBar from "./UrlBar.svelte";
  import type { Api } from "../lib/api";
  import { canSend } from "../lib/sending.svelte";
  import { strings } from "../lib/strings";
  import type { Tab } from "../lib/tabs.svelte";
  import type { ParamRow, RequestDraft } from "../lib/types";

  interface Props {
    /** The tab of the request; the editors change its draft. */
    tab: Tab<RequestDraft>;
  }

  let { tab }: Props = $props();

  const api = getContext<Api>("api");

  type Section = "params" | "headers" | "body" | "auth";
  const SECTIONS: Section[] = ["params", "headers", "body", "auth"];

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

  /** Number of enabled rows next to the name of a section; 0 — no number. */
  const counts = $derived<Record<Section, number>>({
    params: enabled(tab.draft.query) + enabled(tab.draft.path_variables),
    headers: enabled(tab.draft.headers),
    body: 0,
    auth: 0,
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
    onsend={() => void tab.sending.send(api, tab)}
    oncancel={() => tab.sending.cancel(api)}
  />

  <div class="sections" role="tablist" aria-label={strings.editor.sections}>
    {#each SECTIONS as name (name)}
      <button
        type="button"
        role="tab"
        id="{id}-tab-{name}"
        aria-selected={section === name}
        aria-controls="{id}-panel-{name}"
        class:active={section === name}
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
    <KeyValueTable bind:rows={tab.draft.query} label={strings.editor.query} create={newRow} />
    <!-- Path variables come with the request; without any the block would be an empty table. -->
    {#if tab.draft.path_variables.length > 0}
      <h3>{strings.editor.pathVariables}</h3>
      <KeyValueTable bind:rows={tab.draft.path_variables} label={strings.editor.pathVariables} create={newRow} />
    {/if}
  </div>
  {#if visited.includes("headers")}
    <div class="panel scroll" role="tabpanel" id="{id}-panel-headers" aria-labelledby="{id}-tab-headers" hidden={section !== "headers"}>
      <KeyValueTable bind:rows={tab.draft.headers} label={strings.editor.headers} create={newRow} />
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
</div>

<style>
  .request-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .sections {
    flex: none;
    display: flex;
    padding: 0 calc(var(--space) * 2);
    border-bottom: 1px solid var(--border);
  }

  .sections button {
    height: var(--row-height);
    padding: 0 calc(var(--space) * 3);
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .sections button:hover {
    color: var(--text);
  }

  .sections button.active {
    border-bottom-color: var(--accent);
    color: var(--text);
  }

  .count {
    margin-left: var(--space);
    color: var(--accent);
    font-size: 11px;
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
