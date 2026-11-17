<script lang="ts">
  import { onDestroy } from "svelte";
  import IconButton from "./ui/IconButton.svelte";
  import { formatBytes, formatMs, methodClass, statusClass } from "../lib/format";
  import { motion } from "../lib/motion.svelte";
  import { strings } from "../lib/strings";
  import type { ResponseDto } from "../lib/types";

  interface Props {
    response: ResponseDto;
    /** The user asked to copy the body; the owner knows the view it is shown in and tells whether it is copied. */
    oncopy: () => Promise<boolean>;
    /** The user asked to save the body to a file. */
    onsave: () => void;
  }

  let { response, oncopy, onsave }: Props = $props();

  const id = $props.id();
  /** Whether the list of the followed redirects is shown. */
  let hopsShown = $state(false);

  /**
   * The response whose status has already appeared. The status of a new response drops into its place once;
   * the mark of that is taken away when it has, so that showing the panel again does not repeat it.
   */
  let appeared = $state.raw<ResponseDto | null>(null);

  /** The code with the phrase of the server; HTTP/2 has no phrase. */
  const status = $derived(`${response.status} ${response.status_text}`.trim());

  /** How long the copy button shows a check mark after a copy, at the rich motion level. */
  const COPIED_MS = 1000;
  /** The body has just been copied: the button says so with its icon. */
  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function copy() {
    if (!(await oncopy()) || !motion.rich) return;
    copied = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = false), COPIED_MS);
  }

  onDestroy(() => clearTimeout(copiedTimer));
</script>

<!-- The status line of a response: code, time, size and the redirects that led to it. -->
<div class="response-status">
  <div class="line">
    <!-- The colour repeats what the code says. -->
    <span
      class={["status", statusClass(response.status), appeared !== response && "pop-in"]}
      onanimationend={() => (appeared = response)}
    >
      {status}
    </span>
    <span title={strings.response.ttfb(formatMs(response.ttfb_ms))}>{formatMs(response.total_ms)}</span>
    <span title={response.wire_size === response.decoded_size ? undefined : strings.response.wireSize(formatBytes(response.wire_size))}>
      {formatBytes(response.decoded_size)}
    </span>
    {#if response.redirects.length > 0}
      <button type="button" class="redirects" aria-expanded={hopsShown} aria-controls="{id}-hops" onclick={() => (hopsShown = !hopsShown)}>
        {strings.response.redirects(response.redirects.length)}
      </button>
    {/if}
    <!-- A response without a body has nothing to copy or to save. -->
    {#if response.body.total > 0}
      <span class="actions">
        <IconButton
          icon={copied ? "check" : "copy"}
          class={copied ? "copied" : undefined}
          size="sm"
          label={strings.response.copy}
          onclick={copy}
        />
        <IconButton icon="save" size="sm" label={strings.response.save} onclick={onsave} />
      </span>
    {/if}
  </div>
  {#if hopsShown && response.redirects.length > 0}
    <ol class="hops" id="{id}-hops" aria-label={strings.response.redirectList}>
      {#each response.redirects as hop, index (index)}
        <li>
          <span class={["status", statusClass(hop.status)]}>{hop.status}</span>
          <span class={["method", methodClass(hop.method)]}>{hop.method}</span>
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

  /* The method a hop was sent with, in its colour. */
  .method {
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
  .actions {
    display: flex;
    gap: var(--space);
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

  /* The rich motion level: the status of a new response springs into its place with a glow of its own colour. */
  :global(:root[data-motion="rich"]) .status.pop-in {
    animation: status-arrive var(--dur-5) var(--ease-spring);
  }

  @keyframes status-arrive {
    from {
      opacity: 0;
      transform: translateY(-6px) scale(0.9);
      text-shadow:
        0 0 14px currentColor,
        0 0 4px currentColor;
    }

    60% {
      text-shadow: 0 0 14px currentColor;
    }
  }

  /* The rich motion level: after a copy the icon of the button is a check mark that draws itself. */
  :global(:root[data-motion="rich"]) .actions :global(.copied) {
    color: var(--status-ok);
  }

  :global(:root[data-motion="rich"]) .actions :global(.copied path) {
    stroke-dasharray: 16;
    animation: check-draw var(--dur-4) var(--ease);
  }

  @keyframes check-draw {
    from {
      stroke-dashoffset: 16;
    }
  }

  .hops {
    margin: calc(var(--space) * 2) 0 0;
    padding-left: calc(var(--space) * 6);
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }
</style>
