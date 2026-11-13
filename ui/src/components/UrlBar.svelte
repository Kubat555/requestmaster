<script lang="ts">
  import Button from "./ui/Button.svelte";
  import Field from "./ui/Field.svelte";
  import { strings } from "../lib/strings";

  interface Props {
    /** HTTP method of the request. */
    method: string;
    /** Address of the request without the query part. */
    url: string;
    /** A send is running: the button stops it instead of starting one. */
    sending: boolean;
    /** Whether a send may be started. */
    canSend: boolean;
    onsend: () => void;
    oncancel: () => void;
  }

  let { method = $bindable(), url = $bindable(), sending, canSend, onsend, oncancel }: Props = $props();

  /** Methods of the list; any other one is typed as text. */
  const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
  /** Value of the option "a method of my own"; never a method itself. */
  const CUSTOM = "";

  /** The user chose to type the method, though it may still be one of the list. */
  let typing = $state(false);
  const custom = $derived(typing || !METHODS.includes(method));

  function onMethodChosen(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    typing = value === CUSTOM;
    if (!typing) method = value;
  }
</script>

<div class="url-bar">
  <select class="select method" aria-label={strings.editor.method} value={custom ? CUSTOM : method} onchange={onMethodChosen}>
    {#each METHODS as name (name)}
      <option value={name}>{name}</option>
    {/each}
    <option value={CUSTOM}>{strings.editor.customMethodOption}</option>
  </select>
  {#if custom}
    <div class="custom">
      <Field mono label={strings.editor.customMethod} spellcheck="false" bind:value={method} />
    </div>
  {/if}
  <!-- `data-url-input`: Ctrl+L looks for the field of the shown tab by it. -->
  <div class="url">
    <Field mono data-url-input label={strings.editor.url} placeholder={strings.editor.urlPlaceholder} spellcheck="false" bind:value={url} />
  </div>
  <!-- One button for both actions, so that the focus stays on it; the two texts lie on top of each other
       and only the one of the current action is visible. -->
  <Button
    variant={sending ? "default" : "primary"}
    icon={sending ? "close" : "send"}
    disabled={!sending && !canSend}
    aria-label={sending ? strings.editor.cancel : strings.editor.send}
    onclick={sending ? oncancel : onsend}
  >
    <span class="texts">
      <span class="text" class:shown={!sending} aria-hidden="true">{strings.editor.send}</span>
      <span class="text" class:shown={sending} aria-hidden="true">{strings.editor.cancel}</span>
    </span>
  </Button>
</div>

<style>
  .url-bar {
    display: flex;
    gap: calc(var(--space) * 2);
    padding: calc(var(--space) * 2);
  }

  .method {
    flex: none;
    font-family: var(--font-mono);
    font-weight: 600;
  }

  .custom {
    flex: none;
    width: 120px;
  }

  .url {
    flex: 1;
    min-width: 0;
  }

  .texts {
    display: inline-grid;
    place-items: center;
  }

  .text {
    grid-area: 1 / 1;
    opacity: 0;
    transform: translateY(4px);
    transition:
      opacity var(--dur-3) var(--ease),
      transform var(--dur-3) var(--ease);
  }

  .text.shown {
    opacity: 1;
    transform: none;
  }
</style>
