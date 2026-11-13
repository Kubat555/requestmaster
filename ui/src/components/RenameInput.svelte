<script lang="ts">
  import { onMount } from "svelte";
  import { strings } from "../lib/strings";

  interface Props {
    /** The name being changed. */
    value: string;
    /**
     * The editing is over. `name` — the new name, trimmed and not empty; `null` — nothing to save: Escape, the
     * name was not changed, or the field was left empty. `byKey` — it ended with Enter or Escape, so the
     * keyboard focus has nowhere to go; `false` — the focus has already left for another place.
     */
    ondone: (name: string | null, byKey: boolean) => void;
  }

  let { value, ondone }: Props = $props();

  let input: HTMLInputElement;
  // The field starts with the name as it is now and does not follow it afterwards.
  // svelte-ignore state_referenced_locally
  let text = $state(value);
  /** Enter was pressed on an empty name: the field is marked until the text changes. */
  let invalid = $state(false);
  /** The answer was given; the loss of the focus that follows the removal of the field is not another one. */
  let done = false;

  onMount(() => {
    input.focus();
    input.select();
  });

  function finish(name: string | null, byKey: boolean) {
    if (done) return;
    done = true;
    ondone(name !== null && name !== value ? name : null, byKey);
  }

  function onKeydown(event: KeyboardEvent) {
    // The keys of the field are its own: the row around it must not open, move the focus or ask about a deletion.
    event.stopPropagation();
    if (event.key === "Enter") {
      event.preventDefault();
      const name = text.trim();
      if (name === "") invalid = true;
      else finish(name, true);
    } else if (event.key === "Escape") {
      event.preventDefault();
      finish(null, true);
    }
  }

  /** Leaving the field saves the name; an empty one is not a name, so the old one stays. */
  function onBlur() {
    const name = text.trim();
    finish(name === "" ? null : name, false);
  }

  function stop(event: Event) {
    event.stopPropagation();
  }
</script>

<!-- The field a row of the tree shows instead of its name. The owner shows it with `{#if}`. -->
<input
  bind:this={input}
  class="rename"
  class:invalid
  type="text"
  autocomplete="off"
  spellcheck="false"
  aria-label={strings.tree.renameLabel}
  aria-invalid={invalid ? "true" : undefined}
  title={invalid ? strings.tree.emptyName : undefined}
  bind:value={text}
  oninput={() => (invalid = false)}
  onkeydown={onKeydown}
  onblur={onBlur}
  onclick={stop}
  ondblclick={stop}
  oncontextmenu={stop}
/>

<style>
  .rename {
    flex: 1;
    min-width: 0;
    height: calc(var(--row-height) - 6px);
    padding: 0 var(--space);
    background: var(--bg);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    color: var(--text);
    font: inherit;
    cursor: text;
    user-select: text;
  }

  .rename.invalid {
    border-color: var(--danger);
    outline-color: var(--danger);
  }
</style>
