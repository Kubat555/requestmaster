<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import VarSuggest, { optionId } from "./VarSuggest.svelte";
  import VarTooltip from "./VarTooltip.svelte";
  import { editBetween, glowPieces, glowsAfter, type FieldGlow, type GlowPiece } from "../lib/fieldGlow";
  import { motion } from "../lib/motion.svelte";
  import { whenAnimationOver } from "../lib/playOnce";
  import type { ScopeVariable } from "../lib/types";
  import { useVariableScope } from "../lib/variableScope.svelte";
  import { applyCompletion, completionAt, findTokens, suggest, type VarToken } from "../lib/variables";

  type Props = Omit<HTMLInputAttributes, "value"> & {
    /** Text of the field. */
    value: string;
  };

  let { value = $bindable(), oninput, onkeydown, onblur, onmousemove, onmouseleave, ...rest }: Props = $props();

  /** The variables of the editor around the field; without one the field is a plain one. */
  const scope = useVariableScope();
  const id = $props.id();

  let input = $state<HTMLInputElement>();
  let layer = $state<HTMLDivElement>();
  /** How far the text of the field is scrolled to the left: the layer follows. */
  let scrollLeft = $state(0);

  /**
   * A piece of the text: a token with the classes of its scope, or the text between two tokens. `pieces` — the
   * same text cut where its just typed letters glow; `null` when none of it does.
   */
  interface Part {
    text: string;
    token: VarToken | null;
    className: string;
    pieces: GlowPiece[] | null;
  }

  function classOf(token: VarToken): string {
    // Until the variables are read nothing is known about the name: neither a scope nor "not found".
    if (!scope?.loaded) return "var";
    const variable = scope.lookup(token.name);
    return variable ? `var var-${variable.scope}` : "var var-unresolved";
  }

  /**
   * The letters just typed or pasted, which glow at the rich motion level, and the text they are the letters
   * of: a text that came from outside since then has none.
   */
  let glow = $state.raw<{ text: string; glows: FieldGlow[] }>({ text: "", glows: [] });
  /** The id of the glowing piece made last. */
  let lastGlowId = 0;
  /** The text of the field before the edit being handled. */
  let textBefore = "";

  $effect(() => {
    textBefore = value;
  });

  const parts = $derived.by(() => {
    const glows = glow.text === value ? glow.glows : [];
    const found: Part[] = [];
    const add = (from: number, to: number, token: VarToken | null) => {
      const text = value.slice(from, to);
      found.push({ text, token, className: token ? classOf(token) : "", pieces: glowPieces(text, from, glows) });
    };
    let end = 0;
    for (const token of findTokens(value)) {
      if (token.from > end) add(end, token.from, null);
      add(token.from, token.to, token);
      end = token.to;
    }
    if (end < value.length) add(end, value.length, null);
    return found;
  });

  /** The edit the field has just taken: at the rich motion level its new letters glow, the ones before move with the text. */
  function glowTyped(field: HTMLInputElement) {
    const before = textBefore;
    textBefore = field.value;
    if (!motion.rich) return;
    const glows = glow.text === before ? glow.glows : [];
    const edit = editBetween(before, field.value, field.selectionStart ?? field.value.length);
    glow = { text: field.value, glows: glowsAfter(glows, edit, ++lastGlowId) };
  }

  /** The glow of a piece is over, or was cut short: the piece is plain text again. */
  function endGlow(id: number) {
    glow = { text: glow.text, glows: glow.glows.filter((piece) => piece.id !== id) };
  }

  /** The token under the pointer and the place of the hint about it; `null` — no hint. */
  let hovered = $state.raw<{ name: string; left: number; top: number } | null>(null);
  /**
   * The list of the autocompletion: what is offered, the chosen item and the edges of the field the list stands
   * at; `null` — closed.
   */
  let offer = $state.raw<{ items: ScopeVariable[]; active: number; left: number; top: number; bottom: number } | null>(null);

  /** The keys that move the caret of a field without changing its text. */
  const CARET_KEYS = new Set(["ArrowLeft", "ArrowRight", "Home", "End"]);

  /** Space between a token and the hint under it, px. */
  const HINT_GAP = 4;

  function syncScroll() {
    scrollLeft = input?.scrollLeft ?? 0;
  }

  // A text set from outside may scroll the field as well.
  $effect(() => {
    void value;
    syncScroll();
  });

  // Whatever may scroll the text of the field: typing, the arrows, a selection dragged past its edge, the focus.
  $effect(() => {
    const field = input;
    if (!field) return;
    const events = ["scroll", "keyup", "mouseup", "select", "focus"];
    for (const name of events) field.addEventListener(name, syncScroll);
    return () => {
      for (const name of events) field.removeEventListener(name, syncScroll);
    };
  });

  // The caret may move without the text changing — the arrows, Home and End, a click, a selection. An open list
  // is then about another place: it is worked out again for the name at the caret, or closed.
  $effect(() => {
    const field = input;
    if (!field) return;
    const follow = (event: Event) => {
      if (!offer) return;
      // The keys of the list itself do not move the caret; Escape has just closed the list.
      if (event instanceof KeyboardEvent && !CARET_KEYS.has(event.key)) return;
      openOffer();
    };
    const events = ["keyup", "mouseup", "select"];
    for (const name of events) field.addEventListener(name, follow);
    return () => {
      for (const name of events) field.removeEventListener(name, follow);
    };
  });

  /** Offers the variables for the name at the caret; closes the list when there is nothing to offer. */
  function openOffer() {
    if (!scope || !input) return;
    const typed = completionAt(input.value, input.selectionStart ?? input.value.length);
    const items = typed ? suggest(scope.variables, typed.prefix) : [];
    if (items.length === 0) {
      offer = null;
      return;
    }
    const box = input.getBoundingClientRect();
    offer = { items, active: 0, left: box.left, top: box.top, bottom: box.bottom };
    hovered = null;
  }

  /**
   * Puts the name in place of the one being typed. The field gets the text the way typing gives it — with an
   * `input` event — so that everything that listens to the field sees the change.
   */
  function pick(variable: ScopeVariable) {
    if (!input) return;
    const caret = input.selectionStart ?? input.value.length;
    const typed = completionAt(input.value, caret);
    offer = null;
    if (!typed) return;
    const result = applyCompletion(input.value, caret, typed.from, variable.name);
    input.value = result.text;
    input.setSelectionRange(result.caret, result.caret);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }

  const onInput: NonNullable<Props["oninput"]> = (event) => {
    oninput?.(event);
    glowTyped(event.currentTarget);
    hovered = null;
    syncScroll();
    openOffer();
  };

  const onKeyDown: NonNullable<Props["onkeydown"]> = (event) => {
    if (offer) {
      const { items, active } = offer;
      switch (event.key) {
        case "ArrowDown":
          offer = { ...offer, active: (active + 1) % items.length };
          break;
        case "ArrowUp":
          offer = { ...offer, active: (active - 1 + items.length) % items.length };
          break;
        case "Enter":
        case "Tab":
          pick(items[active]);
          break;
        case "Escape":
          offer = null;
          break;
        default:
          onkeydown?.(event);
          return;
      }
      // The key was for the list: neither the field nor a dialog around it acts on it.
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (event.ctrlKey && !event.altKey && !event.metaKey && event.code === "Space") {
      event.preventDefault();
      openOffer();
      return;
    }
    onkeydown?.(event);
  };

  const onBlur: NonNullable<Props["onblur"]> = (event) => {
    offer = null;
    syncScroll();
    onblur?.(event);
  };

  /** The field lies over the layer and gets the pointer; the token under it is found by the places of the tokens. */
  const onMouseMove: NonNullable<Props["onmousemove"]> = (event) => {
    onmousemove?.(event);
    // Until the variables are read nothing can be said about a name: no hint, as there is no list.
    if (!layer || offer || !scope?.loaded) {
      hovered = null;
      return;
    }
    for (const span of layer.querySelectorAll<HTMLElement>("span.var")) {
      const box = span.getBoundingClientRect();
      if (event.clientX >= box.left && event.clientX < box.right && event.clientY >= box.top && event.clientY < box.bottom) {
        const name = span.dataset.name ?? "";
        if (hovered?.name !== name || hovered.left !== box.left) hovered = { name, left: box.left, top: box.bottom + HINT_GAP };
        return;
      }
    }
    hovered = null;
  };

  const onMouseLeave: NonNullable<Props["onmouseleave"]> = (event) => {
    hovered = null;
    onmouseleave?.(event);
  };
</script>

<!-- The text of a part: as it is, or — at the rich motion level, while some of its letters glow — in pieces, the
     glowing ones each in a span that tells when its glow is over. No space between the tags: the layer keeps
     every space of its text. -->
{#snippet textOf(part: Part)}{#if part.pieces}{#each part.pieces as piece (piece.key)}{#if piece.glow !== null}{@const id = piece.glow}<span
          class="typed"
          {@attach whenAnimationOver(() => endGlow(id))}>{piece.text}</span
        >{:else}{piece.text}{/if}{/each}{:else}{part.text}{/if}{/snippet}

<!-- A one-line text field that marks the `{{variables}}` in its text, tells about the one under the pointer and
     offers names while one is typed. An `<input>` cannot colour a part of its text, so the same text is drawn
     under it with the tokens coloured, and the text of the field itself is transparent. Any attribute goes to
     the input. Without the variables of an editor around it this is a plain field. -->
<span class="var-input">
  {#if scope}
    <div bind:this={layer} class="var-layer" aria-hidden="true">
      <span class="var-line" style:transform="translateX({-scrollLeft}px)"
        >{#each parts as part, index (index)}{#if part.token}<span class={part.className} data-name={part.token.name}
              >{@render textOf(part)}</span
            >{:else}{@render textOf(part)}{/if}{/each}</span
      >
    </div>
    <input
      {...rest}
      bind:this={input}
      type="text"
      class="layered"
      aria-describedby={hovered && scope.loaded ? `${id}-hint` : rest["aria-describedby"]}
      aria-controls={offer ? `${id}-list` : rest["aria-controls"]}
      aria-activedescendant={offer ? optionId(`${id}-list`, offer.active) : undefined}
      bind:value
      oninput={onInput}
      onkeydown={onKeyDown}
      onblur={onBlur}
      onmousemove={onMouseMove}
      onmouseleave={onMouseLeave}
    />
    {#if hovered && scope.loaded}
      <VarTooltip id="{id}-hint" name={hovered.name} variable={scope.lookup(hovered.name)} left={hovered.left} top={hovered.top} />
    {/if}
    {#if offer}
      <VarSuggest
        id="{id}-list"
        items={offer.items}
        active={offer.active}
        left={offer.left}
        fieldTop={offer.top}
        fieldBottom={offer.bottom}
        onpick={pick}
      />
    {/if}
  {:else}
    <input {...rest} type="text" bind:value {oninput} {onkeydown} {onblur} {onmousemove} {onmouseleave} />
  {/if}
</span>

<style>
  /* The place decides the height, the font and the padding at the sides (`--var-input-padding`). */
  .var-input {
    position: relative;
    display: block;
    flex: 1;
    width: 100%;
    min-width: 0;
    height: 100%;
  }

  input {
    display: block;
    width: 100%;
    height: 100%;
    margin: 0;
    padding: 0 var(--var-input-padding, 0);
    background: transparent;
    border: none;
    font-family: inherit;
  }

  /* The focus is shown around the whole place of the field, the layer included. */
  input:focus-visible {
    outline: none;
  }

  .var-input:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  /* The text is drawn by the layer; the caret and the selection are the field's own. */
  input.layered {
    position: relative;
    color: transparent;
    caret-color: var(--text);
  }

  /* See-through, so that the selected text of the layer stays readable. */
  input.layered::selection {
    background: color-mix(in srgb, var(--accent) 35%, transparent);
    color: transparent;
  }

  /* As wide as the text area of the field, so that both cut a long text at the same places. */
  .var-layer {
    position: absolute;
    top: 0;
    bottom: 0;
    left: var(--var-input-padding, 0);
    right: var(--var-input-padding, 0);
    display: flex;
    align-items: center;
    overflow: hidden;
    white-space: pre;
    pointer-events: none;
  }

  .var-line {
    flex: none;
  }

  .var-global {
    color: var(--var-global);
  }

  .var-collection {
    color: var(--var-collection);
  }

  .var-environment {
    color: var(--var-environment);
  }

  .var-dynamic {
    color: var(--var-dynamic);
  }

  .var-unresolved {
    color: var(--danger);
    text-decoration: underline;
    text-decoration-color: var(--danger);
  }

  /* The rich motion level: a letter just typed or pasted starts in the accent colour with a glow and comes to
     its own colour. Only the colour and the shadow change, so the text stays under the caret of the field. */
  :global(:root[data-motion="rich"]) .typed {
    animation: typed-glow var(--dur-glow) ease-out;
  }

  @keyframes typed-glow {
    from {
      color: var(--accent);
      text-shadow:
        var(--glow),
        var(--glow);
    }
  }
</style>
