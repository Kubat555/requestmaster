<script lang="ts">
  import Button from "./ui/Button.svelte";
  import Field from "./ui/Field.svelte";
  import { methodClass } from "../lib/format";
  import { motion } from "../lib/motion.svelte";
  import { whenAnimationOver } from "../lib/playOnce";
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
    /** Names of the variables the request uses that are not found; the send is not held back by them. */
    unresolved: string[];
    onsend: () => void;
    oncancel: () => void;
  }

  let { method = $bindable(), url = $bindable(), sending, canSend, unresolved, onsend, oncancel }: Props = $props();

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

  /**
   * The rings that spread over the Send button from where it was pressed, at the rich motion level: each is on
   * the screen until its animation is over. `x` and `y` are counted from the corner of the button, px; `null`
   * — the middle of the button, for a press of the keyboard.
   */
  let ripples = $state.raw<{ id: number; x: number | null; y: number | null }[]>([]);
  let lastRippleId = 0;

  function ripple(x: number | null, y: number | null) {
    if (motion.rich) ripples = [...ripples, { id: ++lastRippleId, x, y }];
  }

  /** A press of the pointer: the ring starts under it. Only shown — the click does the work, as always. */
  function onSendPointerDown(event: PointerEvent) {
    const button = event.currentTarget as HTMLButtonElement;
    if (button.disabled) return;
    const box = button.getBoundingClientRect();
    ripple(event.clientX - box.left, event.clientY - box.top);
  }

  /** The action of the button; a click made by the keyboard (no pointer was pressed) gets its ring here. */
  function onSendClick(event: MouseEvent) {
    if (event.detail === 0) ripple(null, null);
    if (sending) oncancel();
    else onsend();
  }
</script>

<div class="url-bar">
  <!-- The colour of the method, as in the tree and on a tab: the closed list has the colour of the chosen
       method, every item of the open one — its own. -->
  <select
    class={["select", "method", methodClass(custom ? CUSTOM : method)]}
    aria-label={strings.editor.method}
    value={custom ? CUSTOM : method}
    onchange={onMethodChosen}
  >
    {#each METHODS as name (name)}
      <option class={methodClass(name)} value={name}>{name}</option>
    {/each}
    <option class={methodClass(CUSTOM)} value={CUSTOM}>{strings.editor.customMethodOption}</option>
  </select>
  {#if custom}
    <div class="custom">
      <Field mono label={strings.editor.customMethod} spellcheck="false" bind:value={method} />
    </div>
  {/if}
  <!-- `data-url-input`: Ctrl+L looks for the field of the shown tab by it. -->
  <div class="url" data-measure="url">
    <Field
      mono
      variables
      data-url-input
      label={strings.editor.url}
      placeholder={strings.editor.urlPlaceholder}
      spellcheck="false"
      bind:value={url}
    />
  </div>
  <!-- Next to the button, not in it: the name of the button stays the same. The number is for the eyes, the
       sentence for a screen reader, the names are in the tooltip. -->
  {#if unresolved.length > 0}
    <span class="unresolved" role="status" title={unresolved.join(", ")}>
      <span aria-hidden="true">{unresolved.length}</span>
      <span class="visually-hidden">{strings.variables.unresolved(unresolved.length)}</span>
    </span>
  {/if}
  <!-- One button for both actions, so that the focus stays on it; the two texts lie on top of each other
       and only the one of the current action is visible. The arrows stay while the request is on its way and
       move towards each other's side; that the button now stops the send is said by its text and its look. -->
  <Button
    class={["send", sending && "exchanging"]}
    variant={sending ? "default" : "primary"}
    icon="send"
    disabled={!sending && !canSend}
    aria-label={sending ? strings.editor.cancel : strings.editor.send}
    onpointerdown={onSendPointerDown}
    onclick={onSendClick}
  >
    <span class="texts">
      <span class="text" class:shown={!sending} aria-hidden="true">{strings.editor.send}</span>
      <span class="text" class:shown={sending} aria-hidden="true">{strings.editor.cancel}</span>
    </span>
    <!-- Decoration of the rich motion level: it takes no room and no pointer. -->
    {#each ripples as { id, x, y } (id)}
      <span
        class="ripple"
        class:centred={x === null}
        aria-hidden="true"
        style:--ripple-x={x === null ? undefined : `${x}px`}
        style:--ripple-y={y === null ? undefined : `${y}px`}
        {@attach whenAnimationOver(() => (ripples = ripples.filter((ring) => ring.id !== id)))}
      ></span>
    {/each}
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

  /* How many variables are not found: the colour of such a variable in the fields. */
  .unresolved {
    flex: none;
    align-self: center;
    min-width: 20px;
    padding: 0 var(--space);
    border: 1px solid var(--danger);
    border-radius: var(--radius);
    color: var(--danger);
    font-family: var(--font-mono);
    font-weight: 600;
    text-align: center;
    cursor: default;
  }

  /* While the request is on its way each arrow of the button goes a little its own way and back; without
     motion they stand still (`--loop-state`). */
  .url-bar :global(.exchanging .icon path) {
    animation: exchange var(--dur-loop) ease-in-out infinite var(--loop-state);
  }

  .url-bar :global(.exchanging .icon path:first-child) {
    --exchange-shift: 1.5px;
  }

  .url-bar :global(.exchanging .icon path:last-child) {
    --exchange-shift: -1.5px;
  }

  @keyframes exchange {
    50% {
      transform: translateX(var(--exchange-shift));
    }
  }

  /* The rich motion level. The button glows under the pointer, a ring spreads from where it is pressed, and
     while the request is on its way a sheen runs across it. All of it lies inside the button and is cut at
     its edge; none of it takes room or the pointer. */
  :global(:root[data-motion="rich"]) .url-bar :global(.send) {
    position: relative;
    overflow: hidden;
    transition:
      background-color var(--dur-2) var(--ease),
      border-color var(--dur-2) var(--ease),
      color var(--dur-2) var(--ease),
      transform var(--dur-1) var(--ease),
      box-shadow var(--dur-3) var(--ease);
  }

  :global(:root[data-motion="rich"]) .url-bar :global(.send:hover:enabled) {
    box-shadow: var(--glow);
  }

  :global(:root[data-motion="rich"]) .ripple {
    position: absolute;
    left: var(--ripple-x);
    top: var(--ripple-y);
    width: 8px;
    height: 8px;
    margin: -4px 0 0 -4px;
    background: currentColor;
    border-radius: 50%;
    opacity: 0;
    pointer-events: none;
    animation: ripple var(--dur-6) var(--ease);
  }

  /* A press of the keyboard has no place: the ring starts in the middle. */
  :global(:root[data-motion="rich"]) .ripple.centred {
    left: 50%;
    top: 50%;
  }

  @keyframes ripple {
    from {
      opacity: 0.35;
      transform: scale(1);
    }

    to {
      opacity: 0;
      transform: scale(40);
    }
  }

  :global(:root[data-motion="rich"]) .url-bar :global(.send.exchanging)::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(100deg, transparent 30%, color-mix(in srgb, var(--accent) 35%, transparent) 50%, transparent 70%);
    pointer-events: none;
    animation: sheen var(--dur-loop) ease-in-out infinite var(--loop-state);
  }

  @keyframes sheen {
    from {
      transform: translateX(-100%);
    }

    to {
      transform: translateX(100%);
    }
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
