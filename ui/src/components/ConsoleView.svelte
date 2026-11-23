<script lang="ts">
  import EmptyState from "./ui/EmptyState.svelte";
  import { strings } from "../lib/strings";
  import type { ConsoleLine } from "../lib/types";

  interface Props {
    /** The lines of the console of a send, in the order they were written. */
    lines: ConsoleLine[];
    /** Lines were dropped: the console is limited. */
    truncated: boolean;
  }

  let { lines, truncated }: Props = $props();
</script>

<!-- What a script wrote is text and is put into the page as text only. -->
{#if lines.length === 0}
  <EmptyState icon="format" title={strings.console.empty} text={strings.console.emptyText} />
{:else}
  <div class="console-view">
    <ul>
      {#each lines as line, index (index)}
        <li class={line.level}>
          <span class="at">{strings.console.at(line.at_ms)}</span>
          <span class="level">{strings.console.levels[line.level]}</span>
          <span class="text">{line.text}</span>
          {#if line.source}
            <span class="where">{strings.tests.where(strings.scripts.kinds[line.source.phase], line.source.node_name)}</span>
          {/if}
        </li>
      {/each}
    </ul>
    {#if truncated}
      <p class="strip">{strings.console.truncated}</p>
    {/if}
  </div>
{/if}

<style>
  ul {
    display: flex;
    flex-direction: column;
    gap: var(--space);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: baseline;
    gap: calc(var(--space) * 2);
  }

  .at,
  .level,
  .where {
    flex: none;
    color: var(--text-muted);
    font-size: 11px;
  }

  .at {
    font-variant-numeric: tabular-nums;
  }

  .level {
    font-family: var(--font-mono);
  }

  .text {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .warn :is(.level, .text) {
    color: var(--status-client);
  }

  .error :is(.level, .text) {
    color: var(--danger);
  }

  :is(.debug, .network) .text {
    color: var(--text-muted);
  }

  .strip {
    margin: calc(var(--space) * 2) 0 0;
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    background: var(--surface-2);
    border-left: 3px solid var(--status-client);
    border-radius: var(--radius);
  }
</style>
