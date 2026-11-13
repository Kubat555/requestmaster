<script lang="ts">
  import { getContext, onMount } from "svelte";
  import CollectionEditor from "./components/CollectionEditor.svelte";
  import Dialog from "./components/Dialog.svelte";
  import EnvironmentSwitcher from "./components/EnvironmentSwitcher.svelte";
  import RequestEditor from "./components/RequestEditor.svelte";
  import ResponsePanel from "./components/ResponsePanel.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Splitter from "./components/Splitter.svelte";
  import TabBar from "./components/TabBar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import type { Api } from "./lib/api";
  import { installContextMenuGuard, installHotkeys } from "./lib/hotkeys";
  import { canSend } from "./lib/sending.svelte";
  import { strings } from "./lib/strings";
  import { tabs, type Tab } from "./lib/tabs.svelte";
  import { toasts } from "./lib/toasts.svelte";
  import type { CollectionDraft, RequestDraft } from "./lib/types";
  import { workspace } from "./lib/workspace.svelte";

  const api = getContext<Api>("api");

  const SIDEBAR_DEFAULT = 280;
  const SIDEBAR_MIN = 200;
  const SIDEBAR_MAX = 480;

  let sidebarWidth = $state(SIDEBAR_DEFAULT);

  /** The tab with unsaved edits the user asked to close;  — no question on the screen. */
  let closing = $state<Tab | null>(null);

  /** Closes the tab; one with unsaved edits is closed only after the user says what to do with them. */
  function requestClose(tab: Tab) {
    if (tab.dirty) closing = tab;
    else tabs.close(tab.id);
  }

  /** The answer to the question about unsaved edits. A failed save leaves the tab open. */
  async function answerClose(save: boolean) {
    const tab = closing;
    closing = null;
    if (!tab) return;
    if (!save || (await tab.save(api))) tabs.close(tab.id);
  }

  /**
   * The question asked when the window is closed with unsaved edits: `answer` tells the window whether it may
   * close. `null` — no question on the screen.
   */
  let quitting = $state.raw<{ answer: (close: boolean) => void } | null>(null);
  const dirtyTabs = $derived(tabs.list.filter((tab) => tab.dirty));

  /** The user asked to close the window. Without unsaved edits it closes at once, otherwise after the answer. */
  function onCloseRequested(): Promise<boolean> | boolean {
    if (dirtyTabs.length === 0) return true;
    // The question is already on the screen: this request does not close the window, the answer will.
    if (quitting) return false;
    closing = null;
    return new Promise((resolve) => (quitting = { answer: resolve }));
  }

  /**
   * The answer to the question about closing the window. "save" writes every tab with unsaved edits; a failed
   * save is shown as a notification (by the tab) and leaves the window open.
   */
  async function answerQuit(choice: "save" | "discard" | "cancel") {
    const question = quitting;
    quitting = null;
    if (!question) return;
    let close = choice !== "cancel";
    if (choice === "save") {
      // One after another: every save re-reads the tree. The rest is saved even when one fails.
      for (const tab of dirtyTabs) close = (await tab.save(api)) && close;
    }
    question.answer(close);
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
    void workspace.load(api);
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
      removeHotkeys();
      removeContextMenuGuard();
      stopCloseListening?.();
    };
  });
</script>

<div class="app">
  <nav class="sidebar" aria-label={strings.sidebar.label} style:width="{sidebarWidth}px">
    <Sidebar {workspace} />
  </nav>
  <Splitter bind:value={sidebarWidth} min={SIDEBAR_MIN} max={SIDEBAR_MAX} label={strings.sidebar.resize} />
  <main class="main">
    <div class="top">
      <section class="tabs" aria-label={strings.tabs.label}>
        {#if tabs.list.length === 0}
          <p class="placeholder">{strings.tabs.placeholder}</p>
        {:else}
          <TabBar {tabs} onclose={requestClose} />
        {/if}
      </section>
      <EnvironmentSwitcher {workspace} />
    </div>
    <div class="work">
      <section class="editor" aria-label={strings.editor.label}>
        {#if tabs.list.length === 0}
          <p class="placeholder">{strings.editor.placeholder}</p>
        {/if}
        <!-- Every open tab keeps its editor; the ones that are not shown are only hidden, so switching is
             instant and nothing of their state is lost. -->
        {#each tabs.list as tab (tab.id)}
          <div class="tab-panel" data-tab-panel hidden={tabs.active !== tab}>
            {#if tab.kind === "request"}
              <RequestEditor tab={tab as Tab<RequestDraft>} />
            {:else}
              <CollectionEditor tab={tab as Tab<CollectionDraft>} />
            {/if}
          </div>
        {/each}
      </section>
      <section class="response" aria-label={strings.response.label}>
        {#if tabs.list.length === 0}
          <p class="placeholder">{strings.response.placeholder}</p>
        {/if}
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
      <button type="button" class="primary" onclick={() => answerClose(true)}>{strings.tabs.save}</button>
      <button type="button" onclick={() => answerClose(false)}>{strings.tabs.discard}</button>
      <button type="button" onclick={() => (closing = null)}>{strings.tabs.cancel}</button>
    </div>
  </Dialog>
{/if}
{#if quitting}
  <Dialog title={strings.app.closeTitle} onclose={() => answerQuit("cancel")}>
    <p class="question">{strings.app.closeText(dirtyTabs.length)}</p>
    <div class="dialog-actions">
      <button type="button" class="primary" onclick={() => answerQuit("save")}>{strings.app.saveAllAndClose}</button>
      <button type="button" onclick={() => answerQuit("discard")}>{strings.app.closeWithoutSaving}</button>
      <button type="button" onclick={() => answerQuit("cancel")}>{strings.app.cancel}</button>
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
    min-height: calc(var(--row-height) + var(--space) * 2);
    display: flex;
    align-items: center;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .tabs {
    flex: 1;
    min-width: 0;
    align-self: stretch;
    display: flex;
    align-items: center;
  }

  .tab-panel {
    height: 100%;
  }

  .question {
    margin: 0;
    overflow-wrap: anywhere;
  }

  /* The editor above the response; side by side when the window is at least 1280 px wide. */
  .work {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template: minmax(0, 1fr) minmax(0, 1fr) / minmax(0, 1fr);
  }

  .editor,
  .response {
    overflow: auto;
  }

  .response {
    border-top: 1px solid var(--border);
  }

  @media (min-width: 1280px) {
    .work {
      grid-template: minmax(0, 1fr) / minmax(0, 1fr) minmax(0, 1fr);
    }

    .response {
      border-top: none;
      border-left: 1px solid var(--border);
    }
  }

  .placeholder {
    margin: 0;
    padding: calc(var(--space) * 3);
    color: var(--text-muted);
  }
</style>
