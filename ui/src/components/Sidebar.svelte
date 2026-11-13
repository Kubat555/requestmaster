<script lang="ts">
  import { getContext, onMount } from "svelte";
  import Dialog from "./Dialog.svelte";
  import ImportReport from "./ImportReport.svelte";
  import Tree from "./Tree.svelte";
  import Button from "./ui/Button.svelte";
  import EmptyState from "./ui/EmptyState.svelte";
  import Field from "./ui/Field.svelte";
  import IconButton from "./ui/IconButton.svelte";
  import Menu, { type MenuItem } from "./ui/Menu.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
  import type { GlobalsPolicy, ImportResult } from "../lib/types";
  import type { Workspace } from "../lib/workspace.svelte";

  interface Props {
    workspace: Workspace;
  }

  let { workspace }: Props = $props();

  const api = getContext<Api>("api");

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
      return { ...result, conflicts: [] };
    } catch (error) {
      return { ...result, kind: "error", conflicts: [], error: String(error) };
    }
  }

  /** Imports the files in one call, re-reads the tree and the environments and shows the report. */
  async function importFiles(files: File[]) {
    if (files.length === 0 || importing) return;
    importing = true;
    try {
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

  // A file dropped anywhere in the window must not be opened by the webview: the window would navigate to
  // the file and the interface would be gone. So both events are always cancelled, and only a drop inside
  // the sidebar imports.
  onMount(() => {
    const inside = (event: DragEvent) => event.target instanceof Node && root.contains(event.target);
    const onDragOver = (event: DragEvent) => {
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = inside(event) ? "copy" : "none";
    };
    const onDrop = (event: DragEvent) => {
      event.preventDefault();
      if (inside(event)) void importFiles(Array.from(event.dataTransfer?.files ?? []));
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
  <div class="toolbar">
    <Button icon="import" onclick={() => fileInput.click()}>{strings.import.button}</Button>
    <input bind:this={fileInput} type="file" multiple accept=".json" hidden onchange={onFilesChosen} />
    <span bind:this={addAnchor} class="add">
      <IconButton icon="plus" label={strings.tree.add} aria-haspopup="menu" aria-expanded={adding} onclick={() => (adding = !adding)} />
    </span>
    <Field type="search" icon="search" label={strings.sidebar.filter} placeholder={strings.sidebar.filter} bind:value={workspace.filter} />
  </div>
  <div class="content">
    {#if workspace.rows.length === 0}
      <EmptyState icon="collection" title={strings.sidebar.emptyTitle} text={strings.sidebar.empty} />
    {:else}
      <Tree {workspace} />
    {/if}
  </div>
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

  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
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
