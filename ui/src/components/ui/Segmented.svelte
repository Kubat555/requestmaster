<script lang="ts" generics="Value extends string">
  import { tick } from "svelte";
  import { slideIndicator } from "../../lib/slideIndicator";

  interface Option {
    value: Value;
    label: string;
    /** Cannot be chosen; the arrows pass over it. */
    disabled?: boolean;
  }

  interface Props {
    options: Option[];
    /** Value of the chosen option. */
    value: Value;
    /** Accessible name of the group. */
    label: string;
    /** Called with the new value when the user chooses another option. */
    onchange?: (value: Value) => void;
  }

  let { options, value = $bindable(), label, onchange }: Props = $props();

  let group: HTMLDivElement;

  function choose(next: Value) {
    if (next === value) return;
    value = next;
    onchange?.(next);
  }

  /** The arrows choose the nearest option on that side that is not disabled and move the focus to it. */
  async function onKeydown(event: KeyboardEvent, index: number) {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    let next = index + step;
    while (options[next]?.disabled) next += step;
    if (!options[next]) return;
    choose(options[next].value);
    await tick();
    group.querySelectorAll<HTMLElement>('[role="tab"]')[next]?.focus();
  }
</script>

<!-- Buttons under one border, one of them chosen: switches the content of a panel. At the rich motion level
     the background of the chosen one is a mark that slides from button to button. -->
<div bind:this={group} class="segmented" role="tablist" aria-label={label} {@attach slideIndicator}>
  {#each options as option, index (option.value)}
    {@const selected = option.value === value}
    <!-- Tab stops at the chosen option only; the arrows go to the others. -->
    <button
      type="button"
      role="tab"
      aria-selected={selected}
      tabindex={selected ? 0 : -1}
      disabled={option.disabled}
      onclick={() => choose(option.value)}
      onkeydown={(event) => onKeydown(event, index)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: flex;
    height: var(--control-height);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  button {
    flex: 1;
    min-width: 0;
    padding: 0 calc(var(--space) * 2);
    background: transparent;
    border: none;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--dur-2) var(--ease),
      color var(--dur-2) var(--ease);
  }

  button + button {
    border-left: 1px solid var(--border);
  }

  button:hover {
    color: var(--text);
  }

  button[aria-selected="true"] {
    background: var(--surface-2);
    color: var(--text);
  }

  button:disabled {
    color: var(--text-muted);
    cursor: default;
    opacity: 0.5;
  }

  /* The rich motion level: the sliding mark (slideIndicator.ts) is the background of the chosen button. It lies
     under the texts — the group is a stacking context of its own — and the button itself gives its background up. */
  :global(:root[data-motion="rich"]) .segmented {
    position: relative;
    isolation: isolate;
  }

  :global(:root[data-motion="rich"]) .segmented > :global(.slide-indicator) {
    top: 0;
    bottom: 0;
    z-index: -1;
    background: var(--surface-2);
  }

  :global(:root[data-motion="rich"]) .segmented:has(> :global(.slide-indicator)) > button[aria-selected="true"] {
    background: transparent;
  }
</style>
