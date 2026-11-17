<script lang="ts">
  import { untrack } from "svelte";
  import { motion } from "../lib/motion.svelte";
  import { whenAnimationOver } from "../lib/playOnce";
  import { strings } from "../lib/strings";
  import { toasts, type Toast } from "../lib/toasts.svelte";

  /**
   * The rich motion level: the notifications whose time is over, shown for a moment more while they fade away.
   * They are gone from `toasts` at once; what stays here is a picture — it says nothing to a screen reader and
   * takes neither the pointer nor the focus.
   */
  let leaving = $state.raw<Toast[]>([]);
  /** The notifications that were on the screen before the last change. */
  let before: Toast[] = [];

  $effect.pre(() => {
    const now = [...toasts.items];
    const gone = before.filter((toast) => !now.includes(toast));
    before = now;
    if (motion.rich && gone.length > 0) leaving = [...untrack(() => leaving), ...gone];
  });

  /** What is drawn, oldest first: the notifications, and among them the ones that are leaving. */
  const drawn = $derived(
    leaving.length === 0
      ? toasts.items.map((toast) => ({ toast, gone: false }))
      : [...toasts.items.map((toast) => ({ toast, gone: false })), ...leaving.map((toast) => ({ toast, gone: true }))].sort(
          (a, b) => a.toast.id - b.toast.id,
        ),
  );
</script>

<!-- Shows the notifications of `toasts`; each one disappears by itself after 6 s. -->
<div class="toasts" role="region" aria-label={strings.toasts.label}>
  {#each drawn as { toast, gone } (toast.id)}
    {#if gone}
      <div
        class="toast leaving"
        class:error={toast.kind === "error"}
        aria-hidden="true"
        inert
        {@attach whenAnimationOver(() => (leaving = leaving.filter((other) => other !== toast)))}
      >
        {toast.text}
      </div>
    {:else}
      <div class="toast rise-in" class:error={toast.kind === "error"} role={toast.kind === "error" ? "alert" : "status"}>
        {toast.text}
      </div>
    {/if}
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: calc(var(--space) * 4);
    bottom: calc(var(--space) * 4);
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: calc(var(--space) * 2);
    max-width: 420px;
    pointer-events: none;
  }

  .toast {
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-popup);
    box-shadow: var(--shadow-popup);
    overflow-wrap: anywhere;
    pointer-events: auto;
  }

  .toast.error {
    border-left-color: var(--danger);
  }

  /* The rich motion level: a notification fades away instead of vanishing. Nothing waits for it — it is out of
     the list already — and it stays in its place until it is gone, so the others do not jump twice. */
  :global(:root[data-motion="rich"]) .toast.leaving {
    pointer-events: none;
    animation: toast-leave var(--dur-2) var(--ease) forwards;
  }

  @keyframes toast-leave {
    to {
      opacity: 0;
      transform: translateY(4px) scale(0.98);
    }
  }
</style>
