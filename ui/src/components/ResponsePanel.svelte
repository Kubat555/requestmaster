<script lang="ts">
  import { getContext, tick } from "svelte";
  import CookiesView from "./CookiesView.svelte";
  import HeadersView from "./HeadersView.svelte";
  import ResponseBody from "./ResponseBody.svelte";
  import ResponseStatus from "./ResponseStatus.svelte";
  import SendWarnings from "./SendWarnings.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import type { Api } from "../lib/api";
  import { exportToFile } from "../lib/exporting";
  import type { Sending } from "../lib/sending.svelte";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";

  interface Props {
    /** The send state of the tab whose response is shown. */
    sending: Sending;
    /** A new value shows the body of the response and opens the search in it. */
    findSignal?: number;
  }

  let { sending, findSignal }: Props = $props();

  const api = getContext<Api>("api");

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

  /**
   * The view of the body, chosen in `ResponseBody` and used here to copy what is shown. Every response starts
   * with Pretty when its body is JSON.
   */
  let pretty = $derived(response?.body.json ?? false);

  /**
   * Puts the body into the clipboard as it is shown. The text comes from the core; a failure of the command
   * is shown with its message, a clipboard that is missing or refuses — with a message of its own.
   */
  async function copyBody() {
    if (!response) return;
    let text: string;
    try {
      text = await api.copyResponseBody(response.body.id, pretty);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      toasts.info(strings.response.copied);
    } catch {
      toasts.error(strings.response.copyFailed);
    }
  }

  /** Writes the body, as received and unpacked, to a file the user chooses, and tells where the file is. */
  function saveBody() {
    if (!response) return;
    const bodyId = response.body.id;
    void exportToFile(() => api.saveResponseBody(bodyId));
  }

  /** The view of the body on the screen. */
  let bodyView = $state<ResponseBody>();
  /** The value of `findSignal` that was answered last. */
  let answeredFind: number | undefined;

  $effect(() => {
    if (findSignal === undefined || findSignal === answeredFind) return;
    answeredFind = findSignal;
    void openSearch();
  });

  /** Shows the body and opens the search in it; a response without a body has nothing to search in. */
  async function openSearch() {
    if (!response || response.body.total === 0) return;
    section = "body";
    // The search field takes the focus, and it cannot while the body is still hidden.
    await tick();
    bodyView?.openSearch();
  }

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
    <EmptyState icon="send" title={strings.response.emptyTitle} text={strings.response.empty} />
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
      <ResponseStatus {response} oncopy={copyBody} onsave={saveBody} />

      <div class="section-tabs" role="tablist" aria-label={strings.response.sections}>
        {#each SECTIONS as name (name)}
          <button
            type="button"
            role="tab"
            id="{id}-tab-{name}"
            aria-selected={section === name}
            aria-controls="{id}-panel-{name}"
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
          <ResponseBody bind:this={bodyView} body={response.body} decodeError={response.decode_error} bind:pretty onsave={saveBody} />
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

  .panel {
    flex: 1;
    min-height: 0;
  }

  .scroll {
    padding: calc(var(--space) * 2);
    overflow: auto;
  }
</style>
