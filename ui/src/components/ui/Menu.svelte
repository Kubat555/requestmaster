<script lang="ts" module>
  import type { IconName } from "../../lib/icons";

  /** One action of a menu. */
  export interface MenuItem {
    label: string;
    /** Decorative icon before the text. */
    icon?: IconName;
    /** The action destroys something: drawn in the colour of danger. */
    danger?: boolean;
    /** The action is not available now: the item stays in its place and can be walked to, but does nothing. */
    disabled?: boolean;
    /** Tooltip, e.g. why the item is disabled. */
    title?: string;
    onselect: () => void;
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    /** Accessible name of the menu. */
    label: string;
    items: MenuItem[];
    /** Where the menu appears: under this element, or at this point of the window (a right click). */
    anchor: HTMLElement | { x: number; y: number };
    /** Called when the menu must go away: an item was chosen, Escape, Tab, or a press outside. The owner removes it. */
    onclose: () => void;
  }

  let { label, items, anchor, onclose }: Props = $props();

  /** Distance between the menu and its anchor, and between the menu and the edges of the window. */
  const GAP = 4;

  let menu: HTMLDivElement;
  let position = $state({ left: 0, top: 0 });

  function buttons(): HTMLElement[] {
    return [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')];
  }

  /** Under the anchor, or at the point; moved back inside the window when it would stick out. */
  function place() {
    const start = anchor instanceof HTMLElement ? anchor.getBoundingClientRect() : null;
    const left = start ? start.left : (anchor as { x: number }).x;
    const top = start ? start.bottom + GAP : (anchor as { y: number }).y;
    const { width, height } = menu.getBoundingClientRect();
    position = {
      left: Math.max(GAP, Math.min(left, window.innerWidth - width - GAP)),
      top: Math.max(GAP, Math.min(top, window.innerHeight - height - GAP)),
    };
  }

  // The focus goes into the menu when it appears and back to where it was when the menu goes away. A press
  // outside is heard on the window in the capture phase, before anything under the pointer answers it.
  onMount(() => {
    const previous = document.activeElement;
    place();
    (buttons()[0] ?? menu).focus();
    // A press on the anchor is left to the anchor: the button that opened the menu closes it itself.
    const inside = (target: EventTarget | null) =>
      target instanceof Node && (menu.contains(target) || (anchor instanceof HTMLElement && anchor.contains(target)));
    const onMousedown = (event: MouseEvent) => {
      if (!inside(event.target)) onclose();
    };
    window.addEventListener("mousedown", onMousedown, true);
    return () => {
      window.removeEventListener("mousedown", onMousedown, true);
      if (previous instanceof HTMLElement) previous.focus();
    };
  });

  /** The menu goes away first, so that the focus is back in its place before the action starts. */
  function select(item: MenuItem) {
    if (item.disabled) return;
    onclose();
    item.onselect();
  }

  function onKeydown(event: KeyboardEvent) {
    const all = buttons();
    const current = all.indexOf(document.activeElement as HTMLElement);
    switch (event.key) {
      case "ArrowDown":
        all[(current + 1) % all.length]?.focus();
        break;
      case "ArrowUp":
        all[(current - 1 + all.length) % all.length]?.focus();
        break;
      case "Home":
        all[0]?.focus();
        break;
      case "End":
        all[all.length - 1]?.focus();
        break;
      case "Enter":
      case " ":
        if (items[current]) select(items[current]);
        break;
      case "Escape":
      case "Tab":
        onclose();
        break;
      default:
        return;
    }
    // Nothing behind the menu answers its keys: Escape must not also close a dialog, for one.
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<!-- A list of actions that pops up at a button or at the point of a click. The owner shows it with `{#if}`. -->
<div
  bind:this={menu}
  class="menu"
  role="menu"
  aria-label={label}
  tabindex="-1"
  style:left="{position.left}px"
  style:top="{position.top}px"
  onkeydown={onKeydown}
>
  {#each items as item, index (index)}
    <!-- The arrows walk the items; the handler of the menu answers Enter, so a click is the pointer alone. -->
    <button
      type="button"
      role="menuitem"
      tabindex="-1"
      class:danger={item.danger}
      aria-disabled={item.disabled ? "true" : undefined}
      title={item.title}
      onclick={() => select(item)}
    >
      {#if item.icon}
        <Icon name={item.icon} />
      {/if}
      <span class="label">{item.label}</span>
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 60;
    display: flex;
    flex-direction: column;
    min-width: 180px;
    max-width: 320px;
    max-height: calc(100vh - 8px);
    padding: var(--space);
    overflow-y: auto;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-popup);
    box-shadow: var(--shadow-popup);
    animation: menu-in var(--dur-2) var(--ease);
  }

  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .menu {
      animation: none;
    }
  }

  button {
    flex: none;
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 2);
    height: var(--control-height-sm);
    padding: 0 calc(var(--space) * 2);
    background: transparent;
    border: none;
    border-radius: var(--radius);
    text-align: left;
    cursor: pointer;
  }

  /* The item under the pointer and the one with the focus look the same: one list, two ways to walk it. */
  button:hover,
  button:focus {
    background: var(--border);
  }

  button.danger {
    color: var(--danger);
  }

  button[aria-disabled="true"] {
    color: var(--text-muted);
    cursor: default;
    opacity: 0.6;
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
