<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";
  import type { IconName } from "../../lib/icons";

  interface Props extends HTMLButtonAttributes {
    /** `primary` — the main action of its place; `ghost` — a quiet one, without a border. */
    variant?: "default" | "primary" | "ghost";
    /** `sm` — 28 px high instead of 32. */
    size?: "md" | "sm";
    /** Decorative icon before the text. */
    icon?: IconName;
    children?: Snippet;
  }

  let { variant = "default", size = "md", icon, children, class: className, ...rest }: Props = $props();
</script>

<!-- A button with text. Any other attribute goes to the `<button>` itself. -->
<button type="button" {...rest} class={["btn", variant, size === "sm" && "sm", className]}>
  {#if icon}
    <Icon name={icon} />
  {/if}
  {@render children?.()}
</button>

<style>
  .btn {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: calc(var(--space) * 1.5);
    height: var(--control-height);
    padding: 0 calc(var(--space) * 3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    white-space: nowrap;
    cursor: pointer;
  }

  .btn.sm {
    height: var(--control-height-sm);
    padding: 0 calc(var(--space) * 2);
  }

  .btn:hover {
    border-color: var(--text-muted);
  }

  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-contrast);
    font-weight: 600;
  }

  /* The focus ring has the accent colour and would not be seen on the accent background. */
  .btn.primary:focus-visible {
    outline-color: var(--accent-contrast);
  }

  .btn.ghost {
    background: transparent;
    border-color: transparent;
    color: var(--text-muted);
  }

  .btn.ghost:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .btn:disabled {
    background: var(--surface-2);
    border-color: var(--border);
    color: var(--text-muted);
    font-weight: 400;
    cursor: default;
  }

  .btn.ghost:disabled {
    background: transparent;
    border-color: transparent;
  }
</style>
