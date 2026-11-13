<script lang="ts">
  import { tick } from "svelte";
  import { strings } from "../lib/strings";
  import type { Tab, Tabs } from "../lib/tabs.svelte";
  import type { RequestDraft } from "../lib/types";

  interface Props {
    tabs: Tabs;
    /** The user asked to close the tab: the cross or the middle mouse button. The owner decides what happens. */
    onclose: (tab: Tab) => void;
  }

  let { tabs, onclose }: Props = $props();

  let bar: HTMLDivElement;

  /** Methods that have a colour of their own, as in the tree. */
  const COLOURED = ["get", "post", "put", "patch", "delete"];
  /** A longer method is cut, as in the tree. */
  const METHOD_MAX = 6;

  /** Method of a request tab as its draft has it; `null` for a collection. */
  function methodOf(tab: Tab): string | null {
    return tab.kind === "request" ? (tab.draft as RequestDraft).method : null;
  }

  function methodClass(method: string): string {
    return COLOURED.includes(method.toLowerCase()) ? `method-${method.toLowerCase()}` : "method-other";
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

  function onAuxClick(event: MouseEvent, tab: Tab) {
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
          <span class={["method", methodClass(method)]}>{method.slice(0, METHOD_MAX)}</span>
        {/if}
        <span class="name">{tab.name}</span>
        {#if tab.dirty}
          <span class="dirty" role="img" aria-label={strings.tabs.unsaved}></span>
        {/if}
      </button>
      <!-- For the pointer; the keyboard closes the shown tab with Ctrl+W. -->
      <button
        type="button"
        class="close"
        tabindex="-1"
        aria-label="{strings.tabs.close}: {tab.name}"
        title={strings.tabs.close}
        onclick={() => onclose(tab)}
      >
        <span aria-hidden="true">×</span>
      </button>
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
    border-right: 1px solid var(--border);
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
  }

  .tab:hover {
    background: var(--surface-2);
  }

  .tab.active {
    background: var(--bg);
    border-bottom-color: var(--accent);
    color: var(--text);
  }

  button {
    height: 100%;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
  }

  .title {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 1.5);
    min-width: 0;
    padding: 0 calc(var(--space) * 1) 0 calc(var(--space) * 3);
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

  .close {
    flex: none;
    width: var(--row-height);
    font-size: 15px;
  }

  .close:hover {
    color: var(--danger);
  }

  .method {
    flex: none;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
  }

  .method-get {
    color: var(--method-get);
  }

  .method-post {
    color: var(--method-post);
  }

  .method-put {
    color: var(--method-put);
  }

  .method-patch {
    color: var(--method-patch);
  }

  .method-delete {
    color: var(--method-delete);
  }

  .method-other {
    color: var(--method-other);
  }
</style>
