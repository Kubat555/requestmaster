<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";
  import type { IconName } from "../../lib/icons";

  interface Props extends Omit<HTMLButtonAttributes, "aria-label" | "children"> {
    icon: IconName;
    /** What the button does: its accessible name and, unless `title` is given, its tooltip. */
    label: string;
    /** `sm` — 28 px instead of 32. */
    size?: "md" | "sm";
  }

  let { icon, label, size = "md", class: className, ...rest }: Props = $props();
</script>

<!-- A square button with an icon alone. Any other attribute goes to the `<button>` itself. -->
<button type="button" title={label} {...rest} class={["icon-btn", size === "sm" && "sm", className]} aria-label={label}>
  <Icon name={icon} />
</button>

<style>
  .icon-btn {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--control-height);
    height: var(--control-height);
    padding: 0;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    color: var(--text-muted);
    cursor: pointer;
  }

  .icon-btn.sm {
    width: var(--control-height-sm);
    height: var(--control-height-sm);
  }

  .icon-btn:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .icon-btn:disabled {
    background: transparent;
    color: var(--text-muted);
    cursor: default;
    opacity: 0.5;
  }
</style>
