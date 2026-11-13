<script lang="ts">
  import { strings } from "../lib/strings";
  import type { Warning } from "../lib/types";

  interface Props {
    /** What the core noticed while sending; none of it stopped the send. */
    warnings: Warning[];
  }

  let { warnings }: Props = $props();
</script>

{#if warnings.length > 0}
  <ul class="send-warnings">
    {#each warnings as warning, index (index)}
      <!-- Scripts that did not run change what the request means, so their warning stands out. -->
      <li class:strong={warning.kind === "scripts_not_executed"}>{strings.response.warnings[warning.kind](warning.items)}</li>
    {/each}
  </ul>
{/if}

<style>
  .send-warnings {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: var(--space);
    margin: 0;
    padding: calc(var(--space) * 2) calc(var(--space) * 2) 0;
    list-style: none;
  }

  li {
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    background: var(--surface-2);
    border-left: 3px solid var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  li.strong {
    background: color-mix(in srgb, var(--status-client) 16%, var(--surface));
    border-left-color: var(--status-client);
    color: var(--text);
    font-weight: 600;
  }
</style>
