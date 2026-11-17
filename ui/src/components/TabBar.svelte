<script lang="ts">
  import { tick } from "svelte";
  import IconButton from "./ui/IconButton.svelte";
  import Menu, { type MenuItem } from "./ui/Menu.svelte";
  import { methodClass, methodLabel, wheelScrollLeft } from "../lib/format";
  import { strings } from "../lib/strings";
  import { TAB_MENU_ACTIONS, tabPanelId, tabsToClose, tabTitleId, type AnyTab, type TabMenuAction, type Tabs } from "../lib/tabs.svelte";
  import type { RequestDraft } from "../lib/types";

  interface Props {
    tabs: Tabs;
    /** The user asked to close the tab: the cross or the middle mouse button. The owner decides what happens. */
    onclose: (tab: AnyTab) => void;
    /**
     * The user chose an item of the menu of the tab `from` that closes several tabs: `targets`, never empty.
     * The owner decides what happens.
     */
    onclosemany: (targets: AnyTab[], from: AnyTab) => void;
  }

  let { tabs, onclose, onclosemany }: Props = $props();

  /** Where a menu appears: under an element, or at a point of the window. */
  type Anchor = HTMLElement | { x: number; y: number };

  let bar: HTMLDivElement;
  /**
   * Id of the tab whose title got the focus from a press of the pointer: its focus ring is not drawn. The
   * webview shows the ring after a click on a tab (its own guess about the keyboard), so the bar remembers.
   */
  let pointerFocusedId = $state<string | null>(null);
  /** The menu of a tab on the screen; `null` — none. */
  let menu = $state.raw<{ tab: AnyTab; anchor: Anchor } | null>(null);

  /** The items of the menu on the screen; one that would close nothing is not available. */
  const menuItems = $derived.by<MenuItem[]>(() => {
    const tab = menu?.tab;
    if (!tab) return [];
    return TAB_MENU_ACTIONS.map((action) => ({
      label: strings.tabs.menuItems[action],
      disabled: tabsToClose(tabs.list, tab.id, action).length === 0,
      onselect: () => void tick().then(() => run(tab, action)),
    }));
  });

  /**
   * Shows the menu of the tab. A second request for the menu that is already shown is ignored: the key that
   * opens the menu may be followed by a `contextmenu` event of its own.
   */
  function openMenu(tab: AnyTab, anchor: Anchor) {
    if (menu?.tab !== tab) menu = { tab, anchor };
  }

  /**
   * Does what the item of the menu says. It starts after the menu is gone and the focus is back where it was:
   * a dialog that appears takes the focus from there, not from the menu being removed. The tabs are the ones
   * open by then.
   */
  function run(tab: AnyTab, action: TabMenuAction) {
    const targets = tabsToClose(tabs.list, tab.id, action);
    if (targets.length === 0) return;
    if (action === "close") onclose(tab);
    else onclosemany(targets, tab);
  }

  /** Our menu instead of the one of the webview. */
  function onContextMenu(event: MouseEvent, tab: AnyTab) {
    event.preventDefault();
    openMenu(tab, { x: event.clientX, y: event.clientY });
  }

  // The shown tab is brought into view when another tab is shown and when tabs are added or removed.
  $effect(() => {
    const activeId = tabs.activeId;
    if (activeId === null || tabs.list.length === 0) return;
    // Without `behavior`: the scrolling is not animated.
    document.getElementById(tabTitleId(activeId))?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });

  /** The wheel moves the bar sideways: there is nothing to scroll down to. */
  function onWheel(event: WheelEvent) {
    const left = wheelScrollLeft(event.deltaX, event.deltaY, bar);
    if (left === null) return;
    // Only when the bar has moved: at its edge the turn goes on to whatever is under the bar.
    event.preventDefault();
    bar.scrollLeft = left;
  }

  /** Method of a request tab as its draft has it; `null` for any other tab. */
  function methodOf(tab: AnyTab): string | null {
    return tab.kind === "request" ? (tab.draft as RequestDraft).method : null;
  }

  /** The arrows show the neighbouring tab and move the focus to it; the menu key shows the menu of the tab. */
  async function onKeydown(event: KeyboardEvent, index: number) {
    // The right click of the keyboard: the menu key, or Shift+F10.
    if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
      event.preventDefault();
      openMenu(tabs.list[index], event.currentTarget as HTMLElement);
      return;
    }
    // From here on the focus is moved by the keyboard: its ring is drawn again.
    if (event.key.startsWith("Arrow") || event.key === "Tab") pointerFocusedId = null;
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    const next = tabs.list[index + step];
    if (step === 0 || !next) return;
    event.preventDefault();
    tabs.activate(next.id);
    await tick();
    bar.querySelectorAll<HTMLElement>('[role="tab"]')[index + step]?.focus();
  }

  function onAuxClick(event: MouseEvent, tab: AnyTab) {
    if (event.button !== 1) return;
    event.preventDefault();
    onclose(tab);
  }

  /** Without this the middle button starts the autoscroll of the webview. */
  function onMouseDown(event: MouseEvent) {
    if (event.button === 1) event.preventDefault();
  }
</script>

<div bind:this={bar} class="tab-bar" role="tablist" aria-label={strings.tabs.label} onwheel={onWheel}>
  {#each tabs.list as tab, index (tab.id)}
    {@const active = tabs.active === tab}
    {@const method = methodOf(tab)}
    <!-- The menu is for the pointer here; the keyboard asks for it on the title. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="tab" class:active oncontextmenu={(event) => onContextMenu(event, tab)}>
      <!-- Tab stops at the shown tab only; the arrows go to the others. -->
      <button
        type="button"
        class="title"
        class:by-pointer={pointerFocusedId === tab.id}
        role="tab"
        data-measure="tab"
        id={tabTitleId(tab.id)}
        aria-controls={tabPanelId(tab.id)}
        aria-selected={active}
        tabindex={active ? 0 : -1}
        title={tab.name}
        onclick={() => tabs.activate(tab.id)}
        onauxclick={(event) => onAuxClick(event, tab)}
        onmousedown={onMouseDown}
        onpointerdown={() => (pointerFocusedId = tab.id)}
        onblur={() => {
          if (pointerFocusedId === tab.id) pointerFocusedId = null;
        }}
        onkeydown={(event) => onKeydown(event, index)}
      >
        {#if method !== null}
          <span class={["method", methodClass(method)]}>{methodLabel(method)}</span>
        {/if}
        <span class="name">{tab.name}</span>
        {#if tab.dirty}
          <span class="dirty" role="img" aria-label={strings.tabs.unsaved}></span>
        {/if}
      </button>
      <!-- For the pointer; the keyboard closes the shown tab with Ctrl+W. -->
      <IconButton icon="close" size="sm" tabindex={-1} label="{strings.tabs.close}: {tab.name}" onclick={() => onclose(tab)} />
    </div>
  {/each}
</div>

<!-- Outside the bar: the items of a menu are not tabs of the tab list. -->
{#if menu}
  {#key menu}
    <Menu label={strings.tabs.menu} items={menuItems} anchor={menu.anchor} onclose={() => (menu = null)} />
  {/key}
{/if}

<style>
  .tab-bar {
    display: flex;
    min-width: 0;
    height: 100%;
    overflow-x: auto;
  }

  /* Switching is instant: no transition. */
  .tab {
    flex: none;
    display: flex;
    align-items: center;
    max-width: 240px;
    padding-right: var(--space);
    border-right: 1px solid var(--border);
    border-top: 2px solid transparent;
    color: var(--text-muted);
  }

  .tab:hover {
    background: var(--surface-2);
  }

  .tab.active {
    background: var(--bg);
    border-top-color: var(--accent);
    color: var(--text);
  }

  .title {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 2);
    min-width: 0;
    height: 100%;
    padding: 0 calc(var(--space) * 1) 0 calc(var(--space) * 3);
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
  }

  /* The focus came from the pointer: the ring is for the keyboard. */
  .title.by-pointer:focus-visible {
    outline: none;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dirty {
    flex: none;
    width: 8px;
    height: 8px;
    background: var(--accent);
    border-radius: 50%;
  }

  .method {
    flex: none;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
  }
</style>
