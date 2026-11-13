<script lang="ts">
  import { tick } from "svelte";
  import IconButton from "./ui/IconButton.svelte";
  import { methodClass, methodLabel } from "../lib/format";
  import { strings } from "../lib/strings";
  import type { AnyTab, Tabs } from "../lib/tabs.svelte";
  import type { RequestDraft } from "../lib/types";

  interface Props {
    tabs: Tabs;
    /** The user asked to close the tab: the cross or the middle mouse button. The owner decides what happens. */
    onclose: (tab: AnyTab) => void;
  }

  let { tabs, onclose }: Props = $props();

  let bar: HTMLDivElement;

  /** Method of a request tab as its draft has it; `null` for any other tab. */
  function methodOf(tab: AnyTab): string | null {
    return tab.kind === "request" ? (tab.draft as RequestDraft).method : null;
  }

  /** The arrows show the neighbouring tab and move the focus to it. */
  async function onKeydown(event: KeyboardEvent, index: number) {
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

<div bind:this={bar} class="tab-bar" role="tablist" aria-label={strings.tabs.label}>
  {#each tabs.list as tab, index (tab.id)}
    {@const active = tabs.active === tab}
    {@const method = methodOf(tab)}
    <div class="tab" class:active>
      <!-- Tab stops at the shown tab only; the arrows go to the others. -->
      <button
        type="button"
        class="title"
        role="tab"
        aria-selected={active}
        tabindex={active ? 0 : -1}
        title={tab.name}
        onclick={() => tabs.activate(tab.id)}
        onauxclick={(event) => onAuxClick(event, tab)}
        onmousedown={onMouseDown}
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
      <IconButton icon="close" size="sm" tabindex={-1} label="{strings.tabs.close}: {tab.name}" title={strings.tabs.close} onclick={() => onclose(tab)} />
    </div>
  {/each}
</div>

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
