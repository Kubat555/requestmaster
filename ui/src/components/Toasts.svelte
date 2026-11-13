<script lang="ts">
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
</script>

<!-- Shows the notifications of `toasts`; each one disappears by itself after 6 s. -->
<div class="toasts" role="region" aria-label={strings.toasts.label}>
  {#each toasts.items as toast (toast.id)}
    <div class="toast" class:error={toast.kind === "error"} role={toast.kind === "error" ? "alert" : "status"}>
      {toast.text}
    </div>
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
</style>
