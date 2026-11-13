<script lang="ts">
  import CookiesView from "./CookiesView.svelte";
  import HeadersView from "./HeadersView.svelte";
  import ResponseBody from "./ResponseBody.svelte";
  import ResponseStatus from "./ResponseStatus.svelte";
  import SendWarnings from "./SendWarnings.svelte";
  import type { Sending } from "../lib/sending.svelte";
  import { strings } from "../lib/strings";

  interface Props {
    /** The send state of the tab whose response is shown. */
    sending: Sending;
  }

  let { sending }: Props = $props();

  type Section = "body" | "headers" | "cookies";
  const SECTIONS: Section[] = ["body", "headers", "cookies"];
  /** A send that takes less does not replace what the panel shows with the waiting indicator. */
  const INDICATOR_DELAY_MS = 150;

  const id = $props.id();
  /** Stays as chosen when the next response arrives. */
  let section = $state<Section>("body");
  /** A send has been running for longer than the delay. */
  let waiting = $state(false);

  $effect(() => {
    if (sending.state !== "sending") return;
    const timer = setTimeout(() => (waiting = true), INDICATOR_DELAY_MS);
    return () => {
      clearTimeout(timer);
      waiting = false;
    };
  });

  const result = $derived(sending.result);
  const response = $derived(result?.response ?? null);
  const error = $derived(result?.error ?? null);

  /** Number of rows next to the name of a section; 0 — no number. */
  const counts = $derived<Record<Section, number>>({
    body: 0,
    headers: response?.headers.length ?? 0,
    cookies: response?.cookies.length ?? 0,
  });

  /** What went wrong, by the kind of the error; the fields whose files are missing are named. */
  const errorText = $derived.by(() => {
    if (!error) return "";
    const text = strings.response.errors[error.kind];
    return error.kind === "unconfirmed_files" ? `${text}: ${error.fields.join(", ")}` : text;
  });
</script>

<div class="response-panel">
  {#if waiting}
    <p class="note" role="status">{strings.response.sending}</p>
  {:else if !result}
    <p class="note">{strings.response.empty}</p>
  {:else}
    <SendWarnings warnings={result.warnings} />
    {#if error}
      <div class="failure">
        <!-- A cancel is what the user asked for, so it is not shown as an error. -->
        <p class="reason" class:error={error.kind !== "cancelled"}>{errorText}</p>
        {#if error.message}
          <pre class="message">{error.message}</pre>
        {/if}
      </div>
    {:else if response}
      <ResponseStatus {response} />

      <div class="sections" role="tablist" aria-label={strings.response.sections}>
        {#each SECTIONS as name (name)}
          <button
            type="button"
            role="tab"
            id="{id}-tab-{name}"
            aria-selected={section === name}
            aria-controls="{id}-panel-{name}"
            class:active={section === name}
            onclick={() => (section = name)}
          >
            {strings.response[name]}
            {#if counts[name] > 0}
              <span class="count">{counts[name]}</span>
            {/if}
          </button>
        {/each}
      </div>

      <!-- The body stays while another section is shown, so that it is not fetched again. -->
      <div class="panel" role="tabpanel" id="{id}-panel-body" aria-labelledby="{id}-tab-body" hidden={section !== "body"}>
        {#key response.body.id}
          <ResponseBody body={response.body} decodeError={response.decode_error} />
        {/key}
      </div>
      {#if section === "headers"}
        <div class="panel scroll" role="tabpanel" id="{id}-panel-headers" aria-labelledby="{id}-tab-headers">
          <HeadersView headers={response.headers} />
        </div>
      {:else if section === "cookies"}
        <div class="panel scroll" role="tabpanel" id="{id}-panel-cookies" aria-labelledby="{id}-tab-cookies">
          <CookiesView cookies={response.cookies} />
        </div>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .response-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .note {
    margin: 0;
    padding: calc(var(--space) * 3);
    color: var(--text-muted);
  }

  .failure {
    padding: calc(var(--space) * 3);
    overflow: auto;
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
</style>
