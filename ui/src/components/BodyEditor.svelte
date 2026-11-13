<script lang="ts">
  import { getContext } from "svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import Dialog from "./Dialog.svelte";
  import KeyValueTable from "./KeyValueTable.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import { toasts } from "../lib/toasts.svelte";
  import type { DraftBody, ParamRow, RawLanguage } from "../lib/types";

  interface Props {
    /** Body of the request, edited in place or replaced when its kind changes. */
    body: DraftBody;
  }

  let { body = $bindable() }: Props = $props();

  const api = getContext<Api>("api");

  type Mode = DraftBody["mode"];
  type RawBody = Extract<DraftBody, { mode: "raw" }>;
  type FieldsBody = Extract<DraftBody, { mode: "url_encoded" }>;

  /** Kinds that can be chosen and edited. */
  const EDITABLE: Mode[] = ["none", "raw", "url_encoded"];
  const LANGUAGES: RawLanguage[] = ["text", "json", "xml", "html", "javascript"];

  /** A kind that is only shown: it stays in the list while the body has it. */
  const readOnly = $derived(!EDITABLE.includes(body.mode));
  /** The kind the user asked for while the body is a read-only one; `null` — no question on the screen. */
  let pending = $state<Mode | null>(null);
  /** What was typed in the kinds left during this visit, so that switching back does not lose it. */
  let kept: { raw?: RawBody; url_encoded?: FieldsBody } = {};

  function onKindChosen(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const mode = select.value as Mode;
    if (readOnly) {
      // The choice stays as it was until the user agrees to replace the data.
      select.value = body.mode;
      pending = mode;
    } else {
      setMode(mode);
    }
  }

  function setMode(mode: Mode) {
    if (body.mode === "raw") kept.raw = body;
    else if (body.mode === "url_encoded") kept.url_encoded = body;

    if (mode === "raw") body = kept.raw ?? { mode: "raw", data: { text: "", language: "text" } };
    else if (mode === "url_encoded") body = kept.url_encoded ?? { mode: "url_encoded", data: [] };
    else body = { mode: "none" };
  }

  function confirmReplace() {
    if (pending) setMode(pending);
    pending = null;
  }

  /** Re-indents the JSON text; the core does it, so that every number and key stays as written. */
  async function beautify() {
    if (body.mode !== "raw") return;
    const raw = body;
    try {
      raw.data.text = await api.formatJson(raw.data.text);
    } catch {
      // Text that is not JSON stays as it is.
      toasts.error(strings.body.notJson);
    }
  }

  function newField(): ParamRow {
    return { key: "", value: "", enabled: true, postman_extra: {} };
  }
</script>

<div class="body-editor">
  <div class="bar">
    <select aria-label={strings.body.kind} value={body.mode} onchange={onKindChosen}>
      {#each EDITABLE as mode (mode)}
        <option value={mode}>{strings.body.kinds[mode]}</option>
      {/each}
      {#if readOnly}
        <option value={body.mode}>{strings.body.kinds[body.mode]}</option>
      {/if}
    </select>
    {#if body.mode === "raw"}
      <select aria-label={strings.body.language} bind:value={body.data.language}>
        {#each LANGUAGES as language (language)}
          <option value={language}>{strings.body.languages[language]}</option>
        {/each}
      </select>
      {#if body.data.language === "json"}
        <button type="button" onclick={beautify}>{strings.body.beautify}</button>
      {/if}
    {/if}
    {#if readOnly}
      <span class="note">{strings.body.readOnly}</span>
    {/if}
  </div>

  {#if body.mode === "none"}
    <p class="note empty">{strings.body.empty}</p>
  {:else if body.mode === "raw"}
    {@const raw = body}
    <div class="code">
      <!-- Only JSON has highlighting; the other languages decide the Content-Type alone. -->
      <CodeEditor
        value={raw.data.text}
        language={raw.data.language === "json" ? "json" : "text"}
        label={strings.body.text}
        onchange={(text) => (raw.data.text = text)}
      />
    </div>
  {:else if body.mode === "url_encoded"}
    <div class="scroll">
      <KeyValueTable bind:rows={body.data} label={strings.body.fields} create={newField} />
    </div>
  {:else if body.mode === "form_data"}
    <div class="scroll">
      <table class="parts">
        <tbody>
          {#each body.data as part, index (index)}
            <tr class:off={part.enabled === false}>
              <th scope="row">{part.key}</th>
              <td>
                {#if "text" in part.value}
                  {part.value.text}
                {:else}
                  {strings.body.file}: {part.value.files.map((file) => file.path).join(", ") || strings.body.fileNotChosen}
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else if body.mode === "binary"}
    <p class="value">{strings.body.file}: {body.data.file?.path || strings.body.fileNotChosen}</p>
  {:else}
    <div class="scroll">
      <p class="note">{strings.body.graphqlQuery}</p>
      <pre>{body.data.query}</pre>
      <p class="note">{strings.body.graphqlVariables}</p>
      <pre>{body.data.variables}</pre>
    </div>
  {/if}
</div>

{#if pending}
  <Dialog title={strings.body.replaceTitle} onclose={() => (pending = null)}>
    <p class="question">{strings.body.replaceText}</p>
    <div class="dialog-actions">
      <button type="button" class="primary" onclick={confirmReplace}>{strings.body.replace}</button>
      <button type="button" onclick={() => (pending = null)}>{strings.body.cancel}</button>
    </div>
  </Dialog>
{/if}

<style>
  .body-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    flex: none;
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 2);
    padding: calc(var(--space) * 2);
  }

  select {
    height: var(--row-height);
    padding: 0 calc(var(--space) * 2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .bar button {
    height: var(--row-height);
    padding: 0 calc(var(--space) * 3);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
  }

  .bar button:hover {
    border-color: var(--text-muted);
  }

  .code {
    flex: 1;
    min-height: 120px;
    border-top: 1px solid var(--border);
  }

  .scroll {
    flex: 1;
    min-height: 0;
    padding: 0 calc(var(--space) * 2) calc(var(--space) * 2);
    overflow: auto;
  }

  .note {
    margin: 0;
    color: var(--text-muted);
  }

  .empty,
  .value {
    margin: 0;
    padding: calc(var(--space) * 2);
  }

  .value,
  .parts,
  pre {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .parts {
    width: 100%;
    border-collapse: collapse;
  }

  .parts th,
  .parts td {
    padding: calc(var(--space) * 1.5) calc(var(--space) * 2);
    border: 1px solid var(--border);
    font-weight: 400;
    text-align: left;
    vertical-align: top;
  }

  .parts th {
    width: 30%;
  }

  .parts .off {
    color: var(--text-muted);
  }

  pre {
    margin: calc(var(--space) * 1) 0 calc(var(--space) * 3);
    white-space: pre-wrap;
  }

  .question {
    margin: 0;
  }
</style>
