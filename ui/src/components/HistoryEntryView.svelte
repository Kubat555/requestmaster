<script lang="ts">
  import { getContext, onMount } from "svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import HeadersView from "./HeadersView.svelte";
  import SendWarnings from "./SendWarnings.svelte";
  import Button from "./ui/Button.svelte";
  import type { Api } from "../lib/api";
  import { formatBytes, formatClock, formatDate, formatMs, methodClass, statusClass } from "../lib/format";
  import { strings } from "../lib/strings";
  import { tabs, type HistoryTab } from "../lib/tabs.svelte";

  interface Props {
    /** The tab of the history entry to show. The entry never changes. */
    tab: HistoryTab;
  }

  let { tab }: Props = $props();

  const api = getContext<Api>("api");

  const id = $props.id();
  const entry = $derived(tab.entry);
  const sentAt = $derived(`${formatDate(entry.sent_at)} ${formatClock(entry.sent_at)}`);
  /** The request the entry was sent from can be opened while it is in the tree. */
  const canOpen = $derived(entry.node_exists && entry.node_id !== null);

  /** How a body is shown: `pretty` is its text re-indented by the core, `null` — the body is not JSON. */
  interface BodyView {
    pretty: string | null;
    mode: "pretty" | "raw";
  }
  const views = $state<Record<"request" | "response", BodyView>>({
    request: { pretty: null, mode: "raw" },
    response: { pretty: null, mode: "raw" },
  });

  // Each body is given to the core once, when the entry is opened: a body it can re-indent is JSON and is
  // shown as Pretty; any other one has only Raw.
  onMount(() => {
    const bodies = { request: tab.entry.request_body, response: tab.entry.response_body };
    for (const part of ["request", "response"] as const) {
      const text = bodies[part];
      if (!text) continue;
      api.formatJson(text).then(
        (pretty) => (views[part] = { pretty, mode: "pretty" }),
        () => {},
      );
    }
  });

  function openRequest() {
    if (entry.node_id !== null) void tabs.open(api, entry.node_id);
  }
</script>

{#snippet body(text: string, truncated: boolean, view: BodyView, label: string)}
  {@const pretty = view.mode === "pretty" ? view.pretty : null}
  <div class="body">
    <div class="bar" role="group" aria-label={strings.response.view}>
      <button type="button" aria-pressed={view.mode === "pretty"} disabled={view.pretty === null} onclick={() => (view.mode = "pretty")}>
        {strings.response.pretty}
      </button>
      <button type="button" aria-pressed={view.mode === "raw"} onclick={() => (view.mode = "raw")}>{strings.response.raw}</button>
    </div>
    {#if truncated}
      <p class="strip">{strings.history.truncated}</p>
    {/if}
    <div class="code">
      <CodeEditor value={pretty ?? text} language={pretty === null ? "text" : "json"} readonly {label} />
    </div>
  </div>
{/snippet}

<!-- A send as the history keeps it, to read: what was sent on the left, what came back on the right. -->
<div class="history-entry">
  <header class="summary">
    <span class="sent">{sentAt}</span>
    <span class={["method", methodClass(entry.method)]}>{entry.method}</span>
    <span class="url">{entry.url}</span>
    <!-- The colour repeats what the code says. -->
    {#if entry.status !== null}
      <span class={["status", statusClass(entry.status)]}>{entry.status}</span>
    {/if}
    {#if entry.total_ms !== null}
      <span>{formatMs(entry.total_ms)}</span>
    {/if}
    {#if entry.size !== null}
      <span>{formatBytes(entry.size)}</span>
    {/if}
    <span class="open">
      <Button size="sm" disabled={!canOpen} title={canOpen ? undefined : strings.history.requestDeleted} onclick={openRequest}>
        {strings.history.openRequest}
      </Button>
    </span>
  </header>
  <SendWarnings warnings={entry.warnings} />
  <div class="panes">
    <section class="pane" aria-labelledby="{id}-request">
      <h3 id="{id}-request">{strings.history.request}</h3>
      <div class="headers">
        <HeadersView headers={entry.request_headers} />
      </div>
      {#if !entry.request_body}
        <p class="note">{strings.body.empty}</p>
      {:else}
        {@render body(entry.request_body, entry.request_body_truncated, views.request, strings.history.requestBody)}
      {/if}
    </section>
    <section class="pane" aria-labelledby="{id}-response">
      <h3 id="{id}-response">{strings.history.response}</h3>
      {#if entry.error_kind !== null}
        <div class="failure">
          <!-- A cancel is what the user asked for, so it is not shown as an error. -->
          <p class="reason" class:error={entry.error_kind !== "cancelled"}>{strings.response.errors[entry.error_kind]}</p>
          {#if entry.error_message}
            <pre class="message">{entry.error_message}</pre>
          {/if}
        </div>
      {:else}
        <div class="headers">
          <HeadersView headers={entry.response_headers} />
        </div>
        {#if entry.response_body_binary}
          <p class="strip">{strings.history.binary}</p>
        {:else if !entry.response_body}
          <p class="note">{strings.response.noBody}</p>
        {:else}
          {@render body(entry.response_body, entry.response_body_truncated, views.response, strings.response.bodyText)}
        {/if}
      {/if}
    </section>
  </div>
</div>

<style>
  .history-entry {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .summary {
    flex: none;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: calc(var(--space) * 4);
    min-height: var(--tab-height);
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
  }

  .method,
  .status {
    font-family: var(--font-mono);
    font-weight: 600;
  }

  .url {
    flex: 1;
    min-width: 0;
    color: var(--text);
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .ok {
    color: var(--status-ok);
  }

  .redirect {
    color: var(--status-redirect);
  }

  .client {
    color: var(--status-client);
  }

  .server {
    color: var(--status-server);
  }

  /* At the right edge of the line. */
  .open {
    margin-left: auto;
  }

  /* The request on the left, the response on the right. */
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template: minmax(0, 1fr) / minmax(0, 1fr) minmax(0, 1fr);
  }

  .pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
  }

  .pane + .pane {
    border-left: 1px solid var(--border);
  }

  h3 {
    flex: none;
    margin: 0;
    padding: calc(var(--space) * 2) calc(var(--space) * 3) 0;
    font-size: var(--font-size);
    font-weight: 600;
  }

  /* The table takes what it needs, up to a part of the height: the body under it stays on the screen. */
  .headers {
    flex: none;
    max-height: 40%;
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
    overflow: auto;
  }

  .note {
    margin: 0;
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
    color: var(--text-muted);
  }

  .failure {
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
  }

  .reason {
    margin: 0;
    font-weight: 600;
  }

  .reason.error {
    color: var(--danger);
  }

  .message {
    margin: calc(var(--space) * 2) 0 0;
    color: var(--text-muted);
    font-family: var(--font-mono);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 200px;
  }

  .bar {
    flex: none;
    display: flex;
    padding: 0 calc(var(--space) * 3) calc(var(--space) * 2);
  }

  /* Two buttons under one border, as in the response panel; toggle buttons, because Pretty may be unavailable. */
  .bar button {
    height: var(--control-height-sm);
    padding: 0 calc(var(--space) * 3);
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text-muted);
    cursor: pointer;
    transition:
      background-color var(--dur-2) var(--ease),
      color var(--dur-2) var(--ease);
  }

  .bar button:hover:enabled {
    color: var(--text);
  }

  .bar button:first-child {
    border-radius: var(--radius) 0 0 var(--radius);
  }

  .bar button:last-child {
    border-left: none;
    border-radius: 0 var(--radius) var(--radius) 0;
  }

  .bar button[aria-pressed="true"] {
    background: var(--surface-2);
    color: var(--text);
  }

  .bar button:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .strip {
    flex: none;
    margin: 0 calc(var(--space) * 3) calc(var(--space) * 2);
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    background: var(--surface-2);
    border-left: 3px solid var(--status-client);
    border-radius: var(--radius);
  }

  .code {
    flex: 1;
    min-height: 120px;
    border-top: 1px solid var(--border);
  }
</style>
