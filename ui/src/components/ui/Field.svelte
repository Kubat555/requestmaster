<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";
  import VarInput from "../VarInput.svelte";
  import type { IconName } from "../../lib/icons";

  interface Props extends Omit<HTMLInputAttributes, "aria-label" | "value"> {
    /** Text of the field. */
    value: string;
    /** Accessible name of the field. */
    label: string;
    /** Decorative icon at the left edge. */
    icon?: IconName;
    /** The text is data (an address, a token): drawn with the monospace font. */
    mono?: boolean;
    /** The `{{variables}}` in the text are marked, explained and offered; for a plain text field only. */
    variables?: boolean;
  }

  let { value = $bindable(), label, icon, mono = false, variables = false, type = "text", ...rest }: Props = $props();
</script>

<!-- A one-line text field as wide as its place. Any other attribute goes to the input itself. The frame is
     what pulses when something is copied from the field or pasted into it (`fieldEffects.ts`). -->
<span class="field" class:mono data-pulse-frame>
  {#if icon}
    <Icon name={icon} />
  {/if}
  {#if variables && type === "text"}
    <VarInput {...rest} aria-label={label} bind:value />
  {:else}
    <input {...rest} {type} class:mono aria-label={label} bind:value />
  {/if}
</span>

<style>
  .field {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 1.5);
    width: 100%;
    min-width: 0;
    height: var(--control-height);
    padding: 0 calc(var(--space) * 2);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
  }

  /* The frame shows the focus of the field inside it. */
  .field:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  input {
    flex: 1;
    width: 100%;
    min-width: 0;
    height: 100%;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--text);
  }

  input:focus-visible {
    outline: none;
  }

  input.mono,
  .field.mono > :global(.var-input) {
    font-family: var(--font-mono);
  }

  /* The field that marks variables draws itself; here it only gets the colour of the text. */
  .field > :global(.var-input) {
    color: var(--text);
  }

  /* The frame already shows the focus. */
  .field > :global(.var-input:focus-within) {
    outline: none;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  /* The cross of a search field is drawn by the webview in its own colours. */
  input::-webkit-search-cancel-button {
    display: none;
  }
</style>
