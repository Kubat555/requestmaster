<script lang="ts">
  import Button from "./ui/Button.svelte";
  import { strings } from "../lib/strings";
  import type { Warning } from "../lib/types";

  interface Props {
    /** What the core noticed while sending; none of it stopped the send. */
    warnings: Warning[];
    /** When given, the warning `scripts_not_allowed` gets the button «Разрешить». */
    onallow?: () => void;
  }

  let { warnings, onallow }: Props = $props();

  /** Scripts that did not run, and variables that were not saved, change what the send means: they stand out. */
  const STRONG: Warning["kind"][] = ["scripts_disabled", "scripts_not_allowed", "variables_not_saved"];
</script>

{#if warnings.length > 0}
  <ul class="send-warnings">
    {#each warnings as warning, index (index)}
      <li class:strong={STRONG.includes(warning.kind)}>
        <span class="text">{strings.response.warnings[warning.kind](warning.items)}</span>
        {#if onallow && warning.kind === "scripts_not_allowed"}
          <Button size="sm" onclick={onallow}>{strings.response.allowScripts}</Button>
        {/if}
      </li>
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
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 2);
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    background: var(--surface-2);
    border-left: 3px solid var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  li.strong {
    background: color-mix(in srgb, var(--status-client) 16%, var(--surface));
    border-left-color: var(--status-client);
    color: var(--text);
    font-weight: 600;
  }
</style>
