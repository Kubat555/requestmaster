<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  interface Props {
    /** Heading of the dialog and its accessible name. */
    title: string;
    /** Called when the user dismisses the dialog with Escape; the owner removes it. */
    onclose: () => void;
    /** Content under the heading, buttons included. */
    children: Snippet;
  }

  let { title, onclose, children }: Props = $props();

  const titleId = $props.id();
  let dialog: HTMLDivElement;

  const FOCUSABLE = 'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

  /** Controls inside the dialog that Tab stops at, in order. */
  function focusable(): HTMLElement[] {
    return [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (element) => !(element as HTMLButtonElement).disabled && !element.hidden,
    );
  }

  // Focus goes into the dialog when it appears and back to where it was when the dialog goes away.
  // The keys are heard on the window, not on the dialog: Escape and the focus trap must work even when the
  // focus has left the dialog (it falls to `body` when the focused control disappears, for one). The capture
  // phase lets the dialog answer before anything in the window behind it.
  onMount(() => {
    const previous = document.activeElement;
    (focusable()[0] ?? dialog).focus();
    window.addEventListener("keydown", onKeydown, true);
    return () => {
      window.removeEventListener("keydown", onKeydown, true);
      if (previous instanceof HTMLElement) previous.focus();
    };
  });

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onclose();
      return;
    }
    if (event.key !== "Tab") return;

    // The focus trap: Tab on the last control goes to the first one, Shift+Tab on the first — to the last.
    const controls = focusable();
    if (controls.length === 0) {
      event.preventDefault();
      dialog.focus();
      return;
    }
    const first = controls[0];
    const last = controls[controls.length - 1];
    const active = document.activeElement;
    if (!dialog.contains(active)) {
      // The focus is outside the dialog: Tab brings it back instead of walking the window behind.
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    } else if (event.shiftKey && (active === first || active === dialog)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  }

  /** A press on the backdrop itself must not take the focus out of the dialog. */
  function onBackdropMousedown(event: MouseEvent) {
    if (event.target === event.currentTarget) event.preventDefault();
  }
</script>

<!-- Not `<dialog>.showModal()`: the test environment may lack it. The backdrop keeps the pointer out of the
     window behind, the focus trap — the keyboard. -->
<div class="backdrop" role="presentation" onmousedown={onBackdropMousedown}>
  <div
    bind:this={dialog}
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    tabindex="-1"
  >
    <h2 id={titleId}>{title}</h2>
    {@render children()}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: calc(var(--space) * 6);
    background: color-mix(in srgb, var(--bg) 70%, transparent);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space) * 3);
    width: 560px;
    max-width: 100%;
    max-height: 100%;
    padding: calc(var(--space) * 5);
    overflow: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-popup);
    box-shadow: var(--shadow-popup);
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  /* The row of buttons of every dialog; the content comes from the owner, hence `:global`. */
  .dialog :global(.dialog-actions) {
    display: flex;
    justify-content: flex-end;
    gap: calc(var(--space) * 2);
  }
</style>
