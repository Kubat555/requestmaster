<script lang="ts">
  import { getContext, onMount } from "svelte";
  import CollectionEditor from "./components/CollectionEditor.svelte";
  import Dialog from "./components/Dialog.svelte";
  import EnvironmentEditor from "./components/EnvironmentEditor.svelte";
  import EnvironmentSwitcher from "./components/EnvironmentSwitcher.svelte";
  import FolderEditor from "./components/FolderEditor.svelte";
  import HistoryEntryView from "./components/HistoryEntryView.svelte";
  import LayoutSwitcher from "./components/LayoutSwitcher.svelte";
  import RequestEditor from "./components/RequestEditor.svelte";
  import ResponsePanel from "./components/ResponsePanel.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Splitter from "./components/Splitter.svelte";
  import TabBar from "./components/TabBar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import Button from "./components/ui/Button.svelte";
  import EmptyState from "./components/ui/EmptyState.svelte";
  import type { Api } from "./lib/api";
  import { installContextMenuGuard, installHotkeys } from "./lib/hotkeys";
  import { layout, SPLIT_DEFAULT, SPLIT_MAX, SPLIT_MIN } from "./lib/layout.svelte";
  import { canSend } from "./lib/sending.svelte";
  import { session, SIDEBAR_MAX, SIDEBAR_MIN } from "./lib/session.svelte";
  import { strings } from "./lib/strings";
  import { tabs, type AnyTab, type EnvironmentTab, type Tab } from "./lib/tabs.svelte";
  import { toasts } from "./lib/toasts.svelte";
  import type { CollectionDraft, FolderDraft, RequestDraft } from "./lib/types";
  import { workspace } from "./lib/workspace.svelte";

  const api = getContext<Api>("api");

  let windowWidth = $state(window.innerWidth);
  /** The area of the editor and the response: the divider counts its way in shares of it. */
  let work = $state<HTMLDivElement>();

  /** The tab with unsaved edits the user asked to close;  — no question on the screen. */
  let closing = $state<Tab | null>(null);

  /**
   * Closes the tab; one with unsaved edits is closed only after the user says what to do with them. An
   * environment has nothing to ask about: it saves by itself; a history entry has nothing to save.
   */
  function requestClose(tab: AnyTab) {
    if (tab.kind === "environment") void closeEnvironment(tab);
    else if (tab.kind !== "history" && tab.dirty) closing = tab;
    else tabs.close(tab.id);
  }

  /** The save that was waiting for its time is done first; when it fails, the tab stays with its edits. */
  async function closeEnvironment(tab: EnvironmentTab) {
    if (await tab.flush(api)) tabs.close(tab.id);
  }

  /** The answer to the question about unsaved edits. A failed save leaves the tab open. */
  async function answerClose(save: boolean) {
    const tab = closing;
    closing = null;
    if (!tab) return;
    if (!save || (await tab.save(api))) tabs.close(tab.id);
  }

  /**
   * The question asked when the window is closed and something could not be written down: `answer` tells the
   * window whether it may close. `null` — no question on the screen.
   */
  let closeFailed = $state.raw<{ answer: (close: boolean) => void } | null>(null);
  /** A history entry shows its own response, so its tab takes the area of the response too. */
  const whole = $derived(tabs.active?.kind === "history");
  /** The editor and the response share the area; otherwise there is one cell and nothing to divide. */
  const divided = $derived(tabs.list.length > 0 && !whole);
  /** The side of the editor the response stands on. */
  const side = $derived(layout.side(windowWidth));
  /** The response beside the editor, not above or below it: the divider is a vertical line. */
  const beside = $derived(side === "right" || side === "left");

  /** Percents of the area in one pixel along the way of the divider. */
  function percentsPerPixel(): number {
    const size = (beside ? work?.clientWidth : work?.clientHeight) ?? 0;
    return size > 0 ? 100 / size : 0;
  }

  /**
   * The user asked to close the window. First the environments save what was waiting for its time (a failed
   * save is shown as a notification by the tab), then the drafts of the changed tabs and the state of the
   * window are written down. When all of that is in the storage, the window closes at once, changed tabs or
   * not: their drafts come back at the next launch. Otherwise it closes only after the answer.
   */
  async function onCloseRequested(): Promise<boolean> {
    // The question is already on the screen: this request does not close the window, the answer will.
    if (closeFailed) return false;
    let saved = true;
    // One after another, and all of them even when one fails.
    for (const tab of [...tabs.list]) {
      if (tab.kind === "environment") saved = (await tab.flush(api)) && saved;
    }
    saved = (await session.flush(api)) && saved;
    if (saved) return true;
    // Another request came while this one was writing, and has asked already.
    if (closeFailed) return false;
    closing = null;
    return new Promise((resolve) => (closeFailed = { answer: resolve }));
  }

  /** The answer to the question about closing the window with something not written down. */
  function answerCloseFailed(close: boolean) {
    const question = closeFailed;
    closeFailed = null;
    question?.answer(close);
  }

  function focusUrl() {
    const input = document.querySelector<HTMLInputElement>("[data-tab-panel]:not([hidden]) [data-url-input]");
    input?.focus();
    input?.select();
  }

  /** Sends the request of the shown tab; nothing happens while its send is running. */
  function sendActive() {
    if (tabs.active?.kind !== "request") return;
    const tab = tabs.active as Tab<RequestDraft>;
    if (canSend(tab)) void tab.sending.send(api, tab);
  }

  onMount(() => {
    workspace.onOpen = (id) => void tabs.open(api, id);
    // The tabs are restored over the tree that is already read; from then on the changes are written down.
    void workspace.load(api).then(() => session.open(api));
    const removeHotkeys = installHotkeys({
      save: () => void tabs.active?.save(api),
      closeTab: () => {
        if (tabs.active) requestClose(tabs.active);
      },
      focusUrl,
      send: sendActive,
    });
    const removeContextMenuGuard = installContextMenuGuard();
    let unmounted = false;
    let stopCloseListening: (() => void) | undefined;
    api.onCloseRequested(onCloseRequested).then(
      (stop) => {
        if (unmounted) stop();
        else stopCloseListening = stop;
      },
      // Without the listener the window closes without the question; better said than silent.
      (error) => toasts.error(String(error)),
    );
    return () => {
      unmounted = true;
      session.stop();
      removeHotkeys();
      removeContextMenuGuard();
      stopCloseListening?.();
    };
  });
</script>

<svelte:window bind:innerWidth={windowWidth} />

<div class="app">
  <nav class="sidebar" aria-label={strings.sidebar.label} style:width="{session.sidebarWidth}px">
    <Sidebar {workspace} />
  </nav>
  <Splitter bind:value={session.sidebarWidth} min={SIDEBAR_MIN} max={SIDEBAR_MAX} label={strings.sidebar.resize} />
  <main class="main">
    <div class="top">
      <section class="tabs" aria-label={strings.tabs.label}>
        {#if tabs.list.length > 0}
          <TabBar {tabs} onclose={requestClose} />
        {/if}
      </section>
      <EnvironmentSwitcher {workspace} />
      <LayoutSwitcher {layout} />
    </div>
    <!-- Without tabs the editor takes the whole area and says what to do; there is no response to show.
         The editor always comes first in the markup and the response second, for the keyboard and for screen
         readers; the side the response is drawn on is a matter of the grid alone. -->
    <div
      bind:this={work}
      class="work"
      class:idle={tabs.list.length === 0}
      data-placement={layout.placement}
      data-side={divided ? side : undefined}
      style:--editor-share="{layout.split}fr"
      style:--response-share="{100 - layout.split}fr"
    >
      <section class="editor" aria-label={strings.editor.label}>
        {#if tabs.list.length === 0}
          <EmptyState icon="collection" title={strings.editor.emptyTitle} text={strings.editor.emptyText} />
        {/if}
        <!-- Every open tab keeps its editor; the ones that are not shown are only hidden, so switching is
             instant and nothing of their state is lost. -->
        {#each tabs.list as tab (tab.id)}
          <div class="tab-panel" data-tab-panel hidden={tabs.active !== tab}>
            {#if tab.kind === "request"}
              <RequestEditor tab={tab as Tab<RequestDraft>} />
            {:else if tab.kind === "collection"}
              <CollectionEditor tab={tab as Tab<CollectionDraft>} />
            {:else if tab.kind === "folder"}
              <FolderEditor tab={tab as Tab<FolderDraft>} />
            {:else if tab.kind === "environment"}
              <EnvironmentEditor {tab} />
            {:else if tab.kind === "history"}
              <HistoryEntryView {tab} />
            {/if}
          </div>
        {/each}
      </section>
      {#if divided}
        <!-- With the response on the left or above, the editor stands after the divider. -->
        <div class="divider">
          <Splitter
            bind:value={() => layout.split, (split) => layout.resize(split)}
            min={SPLIT_MIN}
            max={SPLIT_MAX}
            step={2}
            digits={1}
            reset={SPLIT_DEFAULT}
            orientation={beside ? "vertical" : "horizontal"}
            reverse={side === "left" || side === "top"}
            scale={percentsPerPixel}
            label={strings.layout.resize}
          />
        </div>
      {/if}
      <section class="response" aria-label={strings.response.label} hidden={!divided}>
        <!-- Kept for every request tab, like the editors: the response of a tab that is not shown stays. -->
        {#each tabs.list as tab (tab.id)}
          {#if tab.kind === "request"}
            <div class="tab-panel" hidden={tabs.active !== tab}>
              <ResponsePanel sending={tab.sending} />
            </div>
          {/if}
        {/each}
      </section>
    </div>
  </main>
</div>
{#if closing}
  <Dialog title={strings.tabs.closeTitle} onclose={() => (closing = null)}>
    <p class="question"><strong>{closing.name}</strong> — {strings.tabs.closeText}</p>
    <div class="dialog-actions">
      <Button variant="primary" onclick={() => answerClose(true)}>{strings.tabs.save}</Button>
      <Button onclick={() => answerClose(false)}>{strings.tabs.discard}</Button>
      <Button onclick={() => (closing = null)}>{strings.tabs.cancel}</Button>
    </div>
  </Dialog>
{/if}
{#if closeFailed}
  <Dialog title={strings.app.closeFailedTitle} onclose={() => answerCloseFailed(false)}>
    <div class="dialog-actions">
      <Button onclick={() => answerCloseFailed(true)}>{strings.app.closeWithoutSaving}</Button>
      <Button data-initial-focus onclick={() => answerCloseFailed(false)}>{strings.app.cancel}</Button>
    </div>
  </Dialog>
{/if}
<Toasts />

<style>
  .app {
    display: flex;
    height: 100%;
  }

  .sidebar {
    flex: none;
    overflow: hidden;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  /* The tabs with the environment switcher in the top right corner. */
  .top {
    flex: none;
    height: var(--tab-height);
    display: flex;
    align-items: center;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .tabs {
    flex: 1;
    min-width: 0;
    align-self: stretch;
  }

  .tab-panel {
    height: 100%;
  }

  .question {
    margin: 0;
    overflow-wrap: anywhere;
  }

  /* One cell until the editor and the response share the area (`data-side`): no tab is open, or the tab is a
     history entry. Then three tracks — editor, divider, response — in the order of the chosen side; the track
     of the divider has no size, the divider itself lies over the edges of its neighbours. */
  .work {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template: "editor" minmax(0, 1fr) / minmax(0, 1fr);
  }

  .work[data-side="right"] {
    grid-template: "editor divider response" minmax(0, 1fr) / minmax(0, var(--editor-share)) 0 minmax(0, var(--response-share));
  }

  .work[data-side="left"] {
    grid-template: "response divider editor" minmax(0, 1fr) / minmax(0, var(--response-share)) 0 minmax(0, var(--editor-share));
  }

  .work[data-side="bottom"] {
    grid-template:
      "editor" minmax(0, var(--editor-share))
      "divider" 0
      "response" minmax(0, var(--response-share))
      / minmax(0, 1fr);
  }

  .work[data-side="top"] {
    grid-template:
      "response" minmax(0, var(--response-share))
      "divider" 0
      "editor" minmax(0, var(--editor-share))
      / minmax(0, 1fr);
  }

  .editor {
    grid-area: editor;
    overflow: auto;
  }

  .response {
    grid-area: response;
    overflow: auto;
  }

  .divider {
    grid-area: divider;
    display: flex;
    min-width: 0;
    min-height: 0;
  }

  .work[data-side="bottom"] .divider,
  .work[data-side="top"] .divider {
    flex-direction: column;
  }

  /* The line between the two is drawn on the edge of the response that faces the editor. */
  .work[data-side="right"] .response {
    border-left: 1px solid var(--border);
  }

  .work[data-side="left"] .response {
    border-right: 1px solid var(--border);
  }

  .work[data-side="bottom"] .response {
    border-top: 1px solid var(--border);
  }

  .work[data-side="top"] .response {
    border-bottom: 1px solid var(--border);
  }

  .work.idle .editor {
    display: grid;
    place-items: center;
  }
</style>
