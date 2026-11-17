<script lang="ts">
  import { getContext, tick } from "svelte";
  import Dialog from "./Dialog.svelte";
  import RenameInput from "./RenameInput.svelte";
  import Button from "./ui/Button.svelte";
  import Icon from "./ui/Icon.svelte";
  import IconButton from "./ui/IconButton.svelte";
  import Menu, { type MenuItem } from "./ui/Menu.svelte";
  import type { Api } from "../lib/api";
  import { exportToFile } from "../lib/exporting";
  import { strings } from "../lib/strings";
  import { environmentTabId, tabs } from "../lib/tabs.svelte";
  import { toasts } from "../lib/toasts.svelte";
  import { GLOBALS_ID, type EnvSummary } from "../lib/types";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  const api = getContext<Api>("api");

  /** Where a menu appears: under an element, or at a point of the window. */
  type Anchor = HTMLElement | { x: number; y: number };

  let list = $state<HTMLUListElement>();
  /** The menu of a row on the screen; `null` — none. */
  let menu = $state.raw<{ environment: EnvSummary; anchor: Anchor } | null>(null);
  /** The environment the user asked to delete, until the question is answered; `null` — no question. */
  let deleting = $state.raw<EnvSummary | null>(null);
  /**
   * The environment (or the global variables) with secret variables the user asked to export, until the warning
   * is answered; `null` — no warning on the screen.
   */
  let exporting = $state.raw<EnvSummary | null>(null);

  /**
   * The global variables first, then the environments by name. They are not an environment: no dot, and their
   * menu only exports.
   */
  const rows = $derived<EnvSummary[]>([{ id: GLOBALS_ID, name: strings.environment.globals }, ...workspace.environments]);

  const menuItems = $derived.by<MenuItem[]>(() => {
    if (!menu) return [];
    const { environment } = menu;
    const exportItem: MenuItem = {
      label: strings.export.item,
      icon: "export",
      onselect: () => void tick().then(() => startExport(environment)),
    };
    if (environment.id === GLOBALS_ID) return [exportItem];
    const active = environment.id === workspace.activeEnvironmentId;
    return [
      {
        label: active ? strings.environment.deactivate : strings.environment.activate,
        icon: "check",
        onselect: () => void workspace.setEnvironment(api, active ? null : environment.id),
      },
      // A field or a dialog appears after the menu is gone and the focus is back on the row: it takes the
      // focus from there, not from the menu being removed.
      {
        label: strings.environment.rename,
        icon: "edit",
        onselect: () => void tick().then(() => (workspace.renamingEnvironmentId = environment.id)),
      },
      exportItem,
      {
        label: strings.environment.delete,
        icon: "trash",
        danger: true,
        onselect: () => void tick().then(() => (deleting = environment)),
      },
    ];
  });

  /** Moves the keyboard to the row of the environment, if it is shown. */
  async function focusRow(id: string) {
    await tick();
    const buttons = list?.querySelectorAll<HTMLElement>("button[data-environment]") ?? [];
    [...buttons].find((button) => button.dataset.environment === id)?.focus();
  }

  /**
   * Shows the menu of the row. A second request for the menu that is already shown closes it when it comes
   * from the same button, and is ignored otherwise: the key that opens the menu may be followed by a
   * `contextmenu` event of its own.
   */
  function openMenu(environment: EnvSummary, anchor: Anchor) {
    if (menu?.environment.id === environment.id) {
      if (menu.anchor === anchor) menu = null;
      return;
    }
    menu = { environment, anchor };
  }

  function onContextMenu(event: MouseEvent, environment: EnvSummary) {
    event.preventDefault();
    openMenu(environment, { x: event.clientX, y: event.clientY });
  }

  /**
   * The keys of a row: what its menu does, without the menu. The global variables are neither renamed nor
   * deleted.
   */
  function onKeydown(event: KeyboardEvent, environment: EnvSummary) {
    const isGlobals = environment.id === GLOBALS_ID;
    switch (event.key) {
      case "F2":
        if (isGlobals) return;
        workspace.renamingEnvironmentId = environment.id;
        break;
      case "Delete":
        if (isGlobals) return;
        deleting = environment;
        break;
      case "F10":
        // Shift+F10 is the right click of the keyboard; F10 alone is not a key of the list.
        if (!event.shiftKey) return;
        openMenu(environment, event.currentTarget as HTMLElement);
        break;
      case "ContextMenu":
        openMenu(environment, event.currentTarget as HTMLElement);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  /** Creates an environment with the default name; its row starts in the renaming mode. */
  function create() {
    void workspace.createEnvironment(api);
  }

  /**
   * The field for the name is closed. A changed name is saved, and the open tab of the environment shows it at
   * once. After Enter and Escape the keyboard goes back to the row.
   */
  async function onRenamed(environment: EnvSummary, name: string | null, byKey: boolean) {
    workspace.renamingEnvironmentId = null;
    if (byKey) void focusRow(environment.id);
    if (name === null) return;
    const tab = tabs.environment(environment.id);
    // The renaming keeps the variables the storage has, so what the tab has not saved yet is written first.
    await tab?.flush(api);
    const saved = await workspace.renameEnvironment(api, environment.id, name);
    if (saved === null) return;
    // From now on the tab saves under the new name.
    if (tab) tab.name = saved;
    if (byKey) void focusRow(environment.id);
  }

  /**
   * The user confirmed the deletion. The tab of the environment is closed and its pending save is dropped:
   * there is nothing left to save to. While the command runs the save only waits, so that a failed deletion
   * loses no edit.
   */
  async function confirmDelete() {
    const environment = deleting;
    deleting = null;
    if (!environment) return;
    const tab = tabs.environment(environment.id);
    tab?.suspend();
    if (await workspace.deleteEnvironment(api, environment.id)) {
      tabs.close(environmentTabId(environment.id));
      void focusRow(GLOBALS_ID);
    } else {
      void tab?.flush(api);
    }
  }

  /**
   * The user asked to export the environment or the global variables. The file is made of what the storage
   * has, so the pending edit of the open tab is saved first; when that fails — already shown as a
   * notification — nothing is exported. Secret values go to the file as they are, so the user is warned first.
   */
  async function startExport(environment: EnvSummary) {
    if ((await tabs.environment(environment.id)?.flush(api)) === false) return;
    let secrets: boolean;
    try {
      const { variables } = await api.getEnvironment(environment.id);
      secrets = variables.some((variable) => variable.secret);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    if (secrets) exporting = environment;
    else await exportToFile(() => api.exportEnvironment(environment.id));
  }

  /** The user agreed to have the secret values in the file. */
  function confirmExport() {
    const environment = exporting;
    exporting = null;
    if (environment) void exportToFile(() => api.exportEnvironment(environment.id));
  }
</script>

<div class="toolbar">
  <IconButton icon="plus" label={strings.environment.add} onclick={create} />
</div>
<div class="content">
  <ul bind:this={list} aria-label={strings.environment.list}>
    {#each rows as environment (environment.id)}
      {@const active = environment.id === workspace.activeEnvironmentId}
      <li class="row hover-row" class:selected={tabs.active?.id === environmentTabId(environment.id)}>
        {#if workspace.renamingEnvironmentId === environment.id}
          <span class="mark"><Icon name="environment" /></span>
          <RenameInput value={environment.name} ondone={(name, byKey) => onRenamed(environment, name, byKey)} />
        {:else}
          <!-- The active environment is told to the eye by the dot and to a screen reader by `aria-current`. -->
          <button
            type="button"
            class="open"
            data-environment={environment.id}
            aria-current={active ? "true" : undefined}
            onclick={() => void tabs.openEnvironment(api, environment.id)}
            onkeydown={(event) => onKeydown(event, environment)}
            oncontextmenu={(event) => onContextMenu(event, environment)}
          >
            <span class="mark"><Icon name="environment" /></span>
            <span class="name">{environment.name}</span>
            {#if active}
              <span class="dot" aria-hidden="true"></span>
            {/if}
          </button>
          <!-- For the pointer only, as in the tree: the keyboard opens the same menu with Shift+F10 or the
               menu key. A press does not take the focus from where it is. -->
          <!-- svelte-ignore a11y_consider_explicit_label -->
          <button
            type="button"
            class="more"
            aria-hidden="true"
            tabindex="-1"
            onmousedown={(event) => event.preventDefault()}
            onclick={(event) => openMenu(environment, event.currentTarget)}
          >
            <Icon name="more" />
          </button>
        {/if}
      </li>
    {/each}
  </ul>
</div>

{#if menu}
  {@const { anchor } = menu}
  {#key menu}
    <Menu label={strings.environment.menu} items={menuItems} {anchor} onclose={() => (menu = null)} />
  {/key}
{/if}

{#if deleting}
  <Dialog title={strings.environment.deleteTitle(deleting.name)} onclose={() => (deleting = null)}>
    <div class="dialog-actions">
      <Button class="danger" onclick={confirmDelete}>{strings.environment.delete}</Button>
      <Button data-initial-focus onclick={() => (deleting = null)}>{strings.environment.cancel}</Button>
    </div>
  </Dialog>
{/if}

{#if exporting}
  <Dialog title={strings.export.secretsTitle} onclose={() => (exporting = null)}>
    <div class="dialog-actions">
      <Button variant="primary" onclick={confirmExport}>{strings.export.confirm}</Button>
      <Button onclick={() => (exporting = null)}>{strings.export.cancel}</Button>
    </div>
  </Dialog>
{/if}

<style>
  .toolbar {
    flex: none;
    display: flex;
    justify-content: flex-end;
    padding: calc(var(--space) * 2);
    border-bottom: 1px solid var(--border);
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 1.5);
    height: var(--row-height);
    padding: 0 calc(var(--space) * 3) 0 calc(var(--space) * 2);
    white-space: nowrap;
  }

  .row.selected {
    background: var(--surface-2);
  }

  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 1.5);
    height: 100%;
    padding: 0;
    background: transparent;
    border: none;
    text-align: left;
    cursor: default;
  }

  .mark {
    flex: none;
    display: flex;
    color: var(--text-muted);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    background: var(--status-ok);
    border-radius: 50%;
  }

  /* Seen while the pointer is over the row. */
  .more {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius);
    color: var(--text-muted);
    cursor: pointer;
    visibility: hidden;
  }

  .row:hover .more {
    visibility: visible;
  }

  .more:hover {
    background: var(--border);
    color: var(--text);
  }

  /* The button that destroys. The class goes to the `<button>` of `Button`, hence `:global`. */
  .dialog-actions :global(.danger) {
    border-color: var(--danger);
    color: var(--danger);
  }
</style>
