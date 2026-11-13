<script lang="ts" module>
  /** Bytes that are not UTF-8 become the replacement character instead of an exception. */
  const decoder = new TextDecoder("utf-8", { fatal: false });
</script>

<script lang="ts">
  import { getContext, untrack } from "svelte";
  import CodeEditor from "./CodeEditor.svelte";
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
  }

  let { body, decodeError }: Props = $props();

  const api = getContext<Api>("api");

  /** Re-indented by the core; only a JSON body has this view, and it is the first one shown. */
  let pretty = $state(untrack(() => body.json));
  /** The text on the screen and the view it was read in; `null` until the body is read for the first time. */
  let shown = $state.raw<{ text: string; pretty: boolean } | null>(null);

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
      <p class="strip">{strings.response.truncated(formatBytes(body.total))}</p>
    {/if}
    {#if decodeError !== null}
      <p class="strip">{strings.response.decodeError}</p>
    {/if}
    <!-- The text appears at once, without an animation. -->
    {#if shown}
      <div class="code">
        <CodeEditor value={shown.text} language={shown.pretty ? "json" : "text"} readonly label={strings.response.bodyText} />
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
    padding: calc(var(--space) * 2);
  }

  .bar button {
    height: var(--row-height);
    padding: 0 calc(var(--space) * 3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--text-muted);
    cursor: pointer;
  }

  .bar button:first-child {
    border-radius: var(--radius) 0 0 var(--radius);
  }

  .bar button:last-child {
    border-left: none;
    border-radius: 0 var(--radius) var(--radius) 0;
  }

  .bar button[aria-pressed="true"] {
    background: var(--bg);
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

  .code {
    flex: 1;
    min-height: 120px;
    border-top: 1px solid var(--border);
  }
</style>
