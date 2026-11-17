<script lang="ts">
  import { getContext, onMount } from "svelte";
  import Dialog from "./Dialog.svelte";
  import EnvironmentList from "./EnvironmentList.svelte";
  import HistoryList from "./HistoryList.svelte";
  import ImportReport from "./ImportReport.svelte";
  import Tree from "./Tree.svelte";
  import Button from "./ui/Button.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import Field from "./ui/Field.svelte";
  import IconButton from "./ui/IconButton.svelte";
  import Menu, { type MenuItem } from "./ui/Menu.svelte";
  import Segmented from "./ui/Segmented.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import { tabs } from "../lib/tabs.svelte";
  import { toasts } from "../lib/toasts.svelte";
  import { NODE_DRAG_TYPE } from "../lib/treeDrop";
  import { GLOBALS_ID, type GlobalsPolicy, type ImportResult } from "../lib/types";
  import { bumpVariables } from "../lib/variableScope.svelte";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  const api = getContext<Api>("api");

  /** What the sidebar shows under its header. Kept while the window is open; a new window starts with the collections. */
  type Section = "collections" | "environments" | "history";
  const SECTIONS: { value: Section; label: string }[] = [
    { value: "collections", label: strings.sidebar.collections },
    { value: "environments", label: strings.sidebar.environments },
    { value: "history", label: strings.sidebar.history },
  ];
  let section = $state<Section>("collections");

  let root: HTMLDivElement;
  let fileInput: HTMLInputElement;

  /** The frame of the "+" button: its menu appears under it. */
  let addAnchor = $state<HTMLSpanElement>();
  let adding = $state(false);

  /**
   * What the "+" button offers. A folder and a request go into the target of the tree — the focused row,
   * else the selected node, else the first collection; next to the target when it is a request. Without a
   * collection there is nowhere to put them.
   */
  const addItems = $derived.by<MenuItem[]>(() => {
    const target = workspace.createTarget;
    const parentId = target === null ? null : target.kind === "request" ? target.parent_id : target.id;
    const inside = (kind: "folder" | "request", icon: MenuItem["icon"]): MenuItem => ({
      label: strings.tree.newNode[kind],
      icon,
      disabled: parentId === null,
      title: parentId === null ? strings.tree.needCollection : undefined,
      onselect: () => void workspace.createNode(api, parentId, kind),
    });
    return [
      {
        label: strings.tree.newNode.collection,
        icon: "collection",
        onselect: () => void workspace.createNode(api, null, "collection"),
      },
      inside("folder", "folder"),
      inside("request", "plus"),
    ];
  });

  /** An import is running, its questions included; another one does not start until it ends. */
  let importing = false;
  /** Results shown in the report dialog; `null` — the dialog is closed. */
  let report = $state<ImportResult[] | null>(null);
  /** The question about conflicting globals on the screen; `answer(null)` — the user cancelled. */
  let question = $state<{ fileName: string; names: string[]; answer: (policy: GlobalsPolicy | null) => void } | null>(null);

  /** Shows the question about the conflicting globals of one file and waits for the choice. */
  function askPolicy(result: ImportResult): Promise<GlobalsPolicy | null> {
    return new Promise((resolve) => {
      question = {
        fileName: result.file_name,
        names: result.conflicts,
        answer: (policy) => {
          question = null;
          resolve(policy);
        },
      };
    });
  }

  /**
   * Globals with conflicts were not applied by the import: asks the user and applies them with the chosen
   * policy. Returns the result as the report shows it.
   */
  async function resolveGlobals(result: ImportResult, text: string): Promise<ImportResult> {
    const policy = await askPolicy(result);
    if (policy === null) return { ...result, kind: "error", conflicts: [], error: strings.import.globalsCancelled };
    try {
      await api.importGlobals(text, policy);
      // The global variables have changed after the tree was read again: the open requests read theirs anew.
      bumpVariables();
      return { ...result, conflicts: [] };
    } catch (error) {
      return { ...result, kind: "error", conflicts: [], error: String(error) };
    }
  }

  /**
   * Imports the files in one call, re-reads the tree and the environments and shows the report. The open tab of
   * the global variables holds its own copy of them and saves the whole set: its pending edit is saved before
   * the import, and after an import that changed the globals the tab reads them again — otherwise its next save
   * would erase what was imported.
   */
  async function importFiles(files: File[]) {
    if (files.length === 0 || importing) return;
    importing = true;
    try {
      // The failure is already shown as a notification; importing over an unsaved edit would lose one of the two.
      if ((await tabs.environment(GLOBALS_ID)?.flush(api)) === false) return;
      const payload = await Promise.all(files.map(async (file) => ({ name: file.name, text: await file.text() })));
      const results = await api.importFiles(payload);
      await workspace.load(api);
      workspace.expand(results.flatMap((result) => (result.kind === "collection" && result.root_id ? [result.root_id] : [])));

      // The results come in the order of the files.
      for (const [index, result] of results.entries()) {
        if (result.kind === "globals" && result.conflicts.length > 0 && payload[index]) {
          results[index] = await resolveGlobals(result, payload[index].text);
        }
      }
      if (results.some((result) => result.kind === "globals")) await tabs.environment(GLOBALS_ID)?.reload(api);
      report = results;
    } catch (error) {
      toasts.error(String(error));
    } finally {
      importing = false;
    }
  }

  function onFilesChosen() {
    const files = Array.from(fileInput.files ?? []);
    // Emptied so that choosing the same file again fires `change`.
    fileInput.value = "";
    void importFiles(files);
  }

  // A file, a link or a text dropped anywhere in the window must not be opened by the webview: the window would
  // navigate there and the interface would be gone. So a drop is always cancelled, and only files dropped
  // inside the sidebar are imported.
  onMount(() => {
    const inside = (event: DragEvent) => event.target instanceof Node && root.contains(event.target);
    const onDragOver = (event: DragEvent) => {
      const types = event.dataTransfer?.types ?? [];
      // A node of the tree: its rows alone decide where it may be dropped. They allow it with the effect
      // "move"; anything written here on the way up would overwrite that, and with any other effect the
      // webview forbids the drop of a drag that allows only "move". Over any other place the event is left
      // alone, and the webview forbids the drop itself.
      if (!types.includes("Files") && types.includes(NODE_DRAG_TYPE)) return;
      event.preventDefault();
      // Only files are taken, and only inside the sidebar; a link or a text is taken nowhere.
      if (event.dataTransfer) event.dataTransfer.dropEffect = types.includes("Files") && inside(event) ? "copy" : "none";
    };
    const onDrop = (event: DragEvent) => {
      event.preventDefault();
      if (!event.dataTransfer?.types.includes("Files") || !inside(event)) return;
      void importFiles(Array.from(event.dataTransfer.files));
    };
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("drop", onDrop);
    return () => {
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("drop", onDrop);
    };
  });
</script>

{#if adding && addAnchor}
  <Menu label={strings.tree.add} items={addItems} anchor={addAnchor} onclose={() => (adding = false)} />
{/if}

<div bind:this={root} class="sidebar">
  <div class="sections">
    <Segmented options={SECTIONS} bind:value={section} label={strings.sidebar.sections} />
  </div>
  <!-- Outside the sections: the field belongs to the import, which a dropped file starts in any of them. -->
  <input bind:this={fileInput} type="file" multiple accept=".json" hidden onchange={onFilesChosen} />
  {#if section === "collections"}
    <div class="toolbar">
      <IconButton icon="import" label={strings.import.button} onclick={() => fileInput.click()} />
      <span bind:this={addAnchor} class="add">
        <IconButton icon="plus" label={strings.tree.add} aria-haspopup="menu" aria-expanded={adding} onclick={() => (adding = !adding)} />
      </span>
      <Field type="search" icon="search" data-measure="tree-filter" label={strings.sidebar.filter} placeholder={strings.sidebar.filter} bind:value={workspace.filter} />
    </div>
    <div class="content" data-measure="tree">
      {#if workspace.rows.length === 0}
        <EmptyState icon="collection" title={strings.sidebar.emptyTitle} text={strings.sidebar.empty} />
      {:else}
        <Tree {workspace} />
      {/if}
    </div>
  {:else if section === "environments"}
    <EnvironmentList {workspace} />
  {:else}
    <HistoryList />
  {/if}
</div>

{#if question}
  {@const { fileName, names, answer } = question}
  <Dialog title={strings.import.conflictsTitle} onclose={() => answer(null)}>
    <p class="conflicts-text"><strong>{fileName}</strong> — {strings.import.conflictsText}</p>
    <ul class="conflicts">
      {#each names as name, index (index)}
        <li>{name}</li>
      {/each}
    </ul>
    <div class="dialog-actions">
      <Button variant="primary" onclick={() => answer("take_incoming")}>{strings.import.takeIncoming}</Button>
      <Button onclick={() => answer("keep_current")}>{strings.import.keepCurrent}</Button>
      <Button onclick={() => answer(null)}>{strings.import.cancel}</Button>
    </div>
  </Dialog>
{:else if report}
  <ImportReport results={report} onclose={() => (report = null)} />
{/if}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .sections {
    flex: none;
    padding: calc(var(--space) * 2) calc(var(--space) * 2) 0;
  }

  /* The rich motion level: what the chosen section shows fades in. */
  :global(:root[data-motion="rich"]) .sections ~ :global(*) {
    animation: fade-in var(--dur-3) var(--ease);
  }

  .toolbar {
    flex: none;
    display: flex;
    gap: calc(var(--space) * 2);
    padding: calc(var(--space) * 2);
    border-bottom: 1px solid var(--border);
  }

  .add {
    flex: none;
    display: flex;
  }

  /* Does not scroll: the tree scrolls inside itself and draws only the rows that are seen. */
  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .conflicts-text {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .conflicts {
    margin: 0;
    padding-left: calc(var(--space) * 5);
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }
</style>
