<script lang="ts">
  import { getContext, tick } from "svelte";
  import Button from "./ui/Button.svelte";
  import IconButton from "./ui/IconButton.svelte";
  import VarInput from "./VarInput.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
  import type { FormPartRow } from "../lib/types";

  interface Props {
    /** The parts of the body, edited in place. */
    parts: FormPartRow[];
  }

  let { parts = $bindable() }: Props = $props();

  const api = getContext<Api>("api");

  let body: HTMLTableSectionElement;

  /** The name of the file without its folders; the whole path is its tooltip. */
  function fileName(path: string): string {
    return path.split(/[\\/]/).pop() || path;
  }

  /**
   * The first character typed into the empty row makes a text part of it; the focus follows the text there,
   * so the user goes on typing in the same place.
   */
  async function add(field: "key" | "value", event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const typed = input.value;
    input.value = "";
    parts.push({
      key: field === "key" ? typed : "",
      value: { text: field === "value" ? typed : "" },
      enabled: true,
      content_type: null,
      postman_extra: {},
    });
    await tick();
    const added = body.rows[parts.length - 1]?.querySelector<HTMLInputElement>(`input[data-field="${field}"]`);
    added?.focus();
    added?.setSelectionRange(added.value.length, added.value.length);
  }

  /** The other kind starts empty; everything else of the part stays. */
  function setKind(part: FormPartRow, kind: string) {
    if (kind === "file" && "text" in part.value) part.value = { files: [] };
    else if (kind === "text" && "files" in part.value) part.value = { text: "" };
  }

  /**
   * Asks for files with the system window; the chosen ones take the place of the files of the part. A window
   * that was closed changes nothing. Only a path that came from that window is confirmed.
   */
  async function choose(part: FormPartRow) {
    let paths: string[];
    try {
      paths = await api.pickFiles(true);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    // The part may have become a text one while the window was open.
    if (paths.length > 0 && "files" in part.value) part.value.files = paths.map((path) => ({ path, confirmed: true }));
  }
</script>

<!-- The parts of a form-data body, a row each: a text or a list of files. A file gets into a part only through
     the system window; there is no field to type a path into. -->
<table aria-label={strings.body.parts}>
  <thead>
    <tr>
      <th class="check"><span class="visually-hidden">{strings.table.enabled}</span></th>
      <th>{strings.table.key}</th>
      <th class="kind">{strings.body.partKind}</th>
      <th class="wide">{strings.table.value}</th>
      <th>{strings.body.contentType}</th>
      <th class="actions"></th>
    </tr>
  </thead>
  <tbody bind:this={body}>
    {#each parts as part, index (index)}
      <tr class:off={!part.enabled}>
        <td class="check"><input type="checkbox" aria-label={strings.table.enabled} bind:checked={part.enabled} /></td>
        <td><VarInput data-field="key" aria-label={strings.table.key} spellcheck="false" bind:value={part.key} /></td>
        <td class="kind">
          <select
            aria-label={strings.body.partKind}
            value={"text" in part.value ? "text" : "file"}
            onchange={(event) => setKind(part, event.currentTarget.value)}
          >
            <option value="text">{strings.body.partText}</option>
            <option value="file">{strings.body.file}</option>
          </select>
        </td>
        <td>
          {#if "text" in part.value}
            <VarInput data-field="value" aria-label={strings.table.value} spellcheck="false" autocomplete="off" bind:value={part.value.text} />
          {:else}
            {@const files = part.value.files}
            <div class="files">
              {#each files as file, fileIndex (fileIndex)}
                {@const name = fileName(file.path)}
                <span class="file">
                  <span class="name" title={file.path}>{name}</span>
                  {#if !file.confirmed}
                    <span class="unconfirmed">{strings.body.fileUnconfirmed}</span>
                  {/if}
                  <IconButton icon="close" size="sm" label={strings.body.removeFile(name)} onclick={() => files.splice(fileIndex, 1)} />
                </span>
              {/each}
              <Button size="sm" onclick={() => choose(part)}>{strings.body.chooseFiles}</Button>
            </div>
          {/if}
        </td>
        <td>
          <!-- An emptied field is "no Content-Type": the core chooses one itself. -->
          <input
            type="text"
            aria-label={strings.body.contentType}
            spellcheck="false"
            autocomplete="off"
            value={part.content_type ?? ""}
            oninput={(event) => (part.content_type = event.currentTarget.value === "" ? null : event.currentTarget.value)}
          />
        </td>
        <td class="actions">
          <IconButton class="remove" icon="close" size="sm" label={strings.table.remove} onclick={() => parts.splice(index, 1)} />
        </td>
      </tr>
    {/each}
    <!-- The empty row: typing into it adds a text part. -->
    <tr>
      <td class="check"></td>
      <td><input type="text" aria-label={strings.table.key} placeholder={strings.table.key} spellcheck="false" oninput={(event) => add("key", event)} /></td>
      <td class="kind"></td>
      <td>
        <input
          type="text"
          aria-label={strings.table.value}
          placeholder={strings.table.value}
          spellcheck="false"
          oninput={(event) => add("value", event)}
        />
      </td>
      <td></td>
      <td class="actions"></td>
    </tr>
  </tbody>
</table>

<style>
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  th {
    height: var(--row-height);
    padding: 0 calc(var(--space) * 2);
    color: var(--text-muted);
    font-weight: 400;
    text-align: left;
  }

  th,
  td {
    border: 1px solid var(--border);
  }

  td {
    padding: 0;
  }

  .check {
    width: calc(var(--row-height) + var(--space));
    text-align: center;
  }

  .kind {
    width: 96px;
  }

  /* The value gets the most room: a text or several file names. */
  .wide {
    width: 40%;
  }

  .actions {
    width: 40px;
    padding: 0 var(--space);
    text-align: right;
  }

  /* Deleting a part cannot be undone here, so its button warns with the colour. */
  td.actions > :global(.remove:hover),
  td.actions > :global(.remove:focus-visible) {
    color: var(--danger);
  }

  input[type="text"],
  select {
    width: 100%;
    height: var(--row-height);
    background: transparent;
    border: none;
  }

  input[type="text"] {
    padding: 0 calc(var(--space) * 2);
    font-family: var(--font-mono);
  }

  select {
    padding: 0 var(--space);
  }

  /* A cell that marks variables draws its field itself; the size, the font and the padding are the same. */
  td > :global(.var-input) {
    height: var(--row-height);
    font-family: var(--font-mono);
    --var-input-padding: calc(var(--space) * 2);
  }

  input::placeholder {
    color: var(--text-muted);
  }

  input[type="checkbox"] {
    accent-color: var(--accent);
  }

  /* A part that is switched off is kept but not sent. */
  .off input[type="text"],
  .off :global(.var-input),
  .off .name {
    color: var(--text-muted);
  }

  .files {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space) calc(var(--space) * 2);
    min-height: var(--row-height);
    padding: var(--space) calc(var(--space) * 2);
  }

  .file {
    display: inline-flex;
    align-items: center;
    gap: var(--space);
    min-width: 0;
  }

  .name {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .unconfirmed {
    color: var(--danger);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
