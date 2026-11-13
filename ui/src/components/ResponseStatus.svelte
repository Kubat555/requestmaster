<script lang="ts">
  import Button from "./ui/Button.svelte";
  import { formatBytes, formatMs, statusClass } from "../lib/format";
  import { strings } from "../lib/strings";
  import type { ResponseDto } from "../lib/types";

  interface Props {
    response: ResponseDto;
    /** The user asked to copy the body; the owner knows the view it is shown in. */
    oncopy: () => void;
  }

  let { response, oncopy }: Props = $props();

  const id = $props.id();
  /** Whether the list of the followed redirects is shown. */
  let hopsShown = $state(false);

  /** The code with the phrase of the server; HTTP/2 has no phrase. */
  const status = $derived(`${response.status} ${response.status_text}`.trim());
</script>

<!-- The status line of a response: code, time, size and the redirects that led to it. -->
<div class="response-status">
  <div class="line">
    <!-- The colour repeats what the code says. -->
    <span class={["status", statusClass(response.status)]}>{status}</span>
    <span title={strings.response.ttfb(formatMs(response.ttfb_ms))}>{formatMs(response.total_ms)}</span>
    <span title={response.wire_size === response.decoded_size ? undefined : strings.response.wireSize(formatBytes(response.wire_size))}>
      {formatBytes(response.decoded_size)}
    </span>
    {#if response.redirects.length > 0}
      <button type="button" class="redirects" aria-expanded={hopsShown} aria-controls="{id}-hops" onclick={() => (hopsShown = !hopsShown)}>
        {strings.response.redirects(response.redirects.length)}
      </button>
    {/if}
    <!-- A response without a body has nothing to copy. -->
    {#if response.body.total > 0}
      <span class="copy">
        <Button variant="ghost" size="sm" icon="copy" onclick={oncopy}>{strings.response.copy}</Button>
      </span>
    {/if}
  </div>
  {#if hopsShown && response.redirects.length > 0}
    <ol class="hops" id="{id}-hops" aria-label={strings.response.redirectList}>
      {#each response.redirects as hop, index (index)}
        <li>
          <span class={["status", statusClass(hop.status)]}>{hop.status}</span>
          {hop.method}
          {hop.url}
          <span aria-hidden="true">→</span>
          {hop.location}
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .response-status {
    flex: none;
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: calc(var(--space) * 4);
    min-height: var(--control-height-sm);
    color: var(--text-muted);
  }

  .status {
    font-family: var(--font-mono);
    font-weight: 600;
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
  .copy {
    margin-left: auto;
  }

  .redirects {
    height: var(--control-height-sm);
    padding: 0 calc(var(--space) * 2);
    background: transparent;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
    cursor: pointer;
  }

  .redirects:hover {
    color: var(--text);
  }

  .hops {
    margin: calc(var(--space) * 2) 0 0;
    padding-left: calc(var(--space) * 6);
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }
</style>
