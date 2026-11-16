<script lang="ts">
  import IconButton from "./ui/IconButton.svelte";
  import Menu, { type MenuItem } from "./ui/Menu.svelte";
  import type { IconName } from "../lib/icons";
  import { PLACEMENTS, type Layout, type Placement } from "../lib/layout.svelte";
  import { strings } from "../lib/strings";

  interface Props {
    layout: Layout;
  }

  let { layout }: Props = $props();

  const ICONS: Record<Placement, IconName> = {
    right: "panel-right",
    bottom: "panel-bottom",
    left: "panel-left",
    top: "panel-top",
    auto: "layout-auto",
  };

  /** The frame of the button: the menu appears under it. */
  let anchor = $state<HTMLSpanElement>();
  let open = $state(false);

  const items = $derived<MenuItem[]>(
    PLACEMENTS.map((placement) => ({
      label: strings.layout.placements[placement],
      icon: ICONS[placement],
      checked: layout.placement === placement,
      onselect: () => layout.choose(placement),
    })),
  );
</script>

<!-- The button draws the placement that is chosen now; its menu offers all of them. -->
<span bind:this={anchor} class="switcher">
  <IconButton icon={ICONS[layout.placement]} label={strings.layout.button} aria-haspopup="menu" aria-expanded={open} onclick={() => (open = !open)} />
</span>
{#if open && anchor}
  <Menu label={strings.layout.button} {items} {anchor} onclose={() => (open = false)} />
{/if}

<style>
  .switcher {
    flex: none;
    display: flex;
    margin-right: calc(var(--space) * 2);
  }
</style>
