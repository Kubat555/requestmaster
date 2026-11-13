<script lang="ts">
  import { getContext, onMount } from "svelte";
  import EnvironmentSwitcher from "./components/EnvironmentSwitcher.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Splitter from "./components/Splitter.svelte";
  import Toasts from "./components/Toasts.svelte";
  import type { Api } from "./lib/api";
  import { strings } from "./lib/strings";
  import { workspace } from "./lib/workspace.svelte";

  const api = getContext<Api>("api");

  const SIDEBAR_DEFAULT = 280;
  const SIDEBAR_MIN = 200;
  const SIDEBAR_MAX = 480;

  let sidebarWidth = $state(SIDEBAR_DEFAULT);

  onMount(() => {
    void workspace.load(api);
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
        <p class="placeholder">{strings.tabs.placeholder}</p>
      </section>
      <EnvironmentSwitcher {workspace} />
    </div>
    <div class="work">
      <section class="editor" aria-label={strings.editor.label}>
        <p class="placeholder">{strings.editor.placeholder}</p>
      </section>
      <section class="response" aria-label={strings.response.label}>
        <p class="placeholder">{strings.response.placeholder}</p>
      </section>
    </div>
  </main>
</div>
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
    display: flex;
    align-items: center;
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
