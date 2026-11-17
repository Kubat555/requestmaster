<script lang="ts" module>
  /** Bytes that are not UTF-8 become the replacement character instead of an exception. */
  const decoder = new TextDecoder("utf-8", { fatal: false });
</script>

<script lang="ts">
  import { getContext } from "svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import Button from "./ui/Button.svelte";
  import type { Api } from "../lib/api";
  import { formatBytes } from "../lib/format";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
  import type { ResponseDto } from "../lib/types";

  interface Props {
    /** The stored body of the response. The component is made anew for another body. */
    body: ResponseDto["body"];
    /** Why the body could not be unpacked; then it is shown as it was received. */
    decodeError: string | null;
    /** The view: re-indented by the core, or as received. Only a JSON body has the first one. */
    pretty: boolean;
    /** The user asked to save the body to a file. */
    onsave: () => void;
  }

  let { body, decodeError, pretty = $bindable(), onsave }: Props = $props();

  const api = getContext<Api>("api");

  /** The text on the screen and the view it was read in; `null` until the body is read for the first time. */
  let shown = $state.raw<{ text: string; pretty: boolean } | null>(null);

  /** The editor with the text, once the body is read. */
  let editor = $state<CodeEditor>();
  /** The search was asked for before the body was read; it opens with the editor. */
  let searchAsked = $state(false);

  /** Opens the search panel of the editor with the text; a response without a body has none. */
  export function openSearch(): void {
    searchAsked = true;
  }

  $effect(() => {
    if (!searchAsked || !editor) return;
    searchAsked = false;
    editor.openSearch();
  });

  $effect(() => {
    if (body.total === 0) return;
    // An answer for a view that is not the chosen one any more is not shown.
    let stale = false;
    const view = pretty;
    api.responseBody(body.id, view).then(
      (bytes) => {
        if (!stale) shown = { text: decoder.decode(bytes), pretty: view };
      },
      (error) => {
        if (!stale) toasts.error(String(error));
      },
    );
    return () => {
      stale = true;
    };
  });
</script>

{#if body.total === 0}
  <p class="note">{strings.response.noBody}</p>
{:else}
  <div class="response-body">
    <div class="bar" role="group" aria-label={strings.response.view}>
      <button type="button" aria-pressed={pretty} disabled={!body.json} onclick={() => (pretty = true)}>
        {strings.response.pretty}
      </button>
      <button type="button" aria-pressed={!pretty} onclick={() => (pretty = false)}>{strings.response.raw}</button>
    </div>
    {#if body.truncated}
      <div class="strip truncated">
        <span>{strings.response.truncated(formatBytes(body.total))}</span>
        <Button size="sm" onclick={onsave}>{strings.response.save}</Button>
      </div>
    {/if}
    {#if decodeError !== null}
      <p class="strip">{strings.response.decodeError}</p>
    {/if}
    <!-- The text appears at once, without an animation. -->
    {#if shown}
      <div class="code">
        <CodeEditor bind:this={editor} value={shown.text} language={shown.pretty ? "json" : "text"} readonly label={strings.response.bodyText} />
      </div>
    {/if}
  </div>
{/if}

<style>
  .response-body {
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

  .bar {
    flex: none;
    display: flex;
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
  }

  /* Two buttons under one border, like `Segmented`; they stay toggle buttons because Pretty may be unavailable. */
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
    margin: 0 calc(var(--space) * 2) calc(var(--space) * 2);
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    background: var(--surface-2);
    border-left: 3px solid var(--status-client);
    border-radius: var(--radius);
  }

  .truncated {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: calc(var(--space) * 2);
  }

  .code {
    flex: 1;
    min-height: 120px;
    border-top: 1px solid var(--border);
  }
</style>
