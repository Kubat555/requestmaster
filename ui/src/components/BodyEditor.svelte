<script lang="ts">
  import { getContext, untrack } from "svelte";
  import BinaryBodyEditor from "./BinaryBodyEditor.svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import Dialog from "./Dialog.svelte";
  import FormDataEditor from "./FormDataEditor.svelte";
  import KeyValueTable from "./KeyValueTable.svelte";
  import Button from "./ui/Button.svelte";
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
  /** A kind whose data the user enters. */
  type EnteredMode = "raw" | "url_encoded" | "form_data" | "binary";

  /** Kinds that can be chosen and edited. */
  const EDITABLE: Mode[] = ["none", "raw", "url_encoded", "form_data", "binary"];
  const LANGUAGES: RawLanguage[] = ["text", "json", "xml", "html", "javascript"];

  /** A kind that is only shown: it stays in the list while the body has it. */
  const readOnly = $derived(!EDITABLE.includes(body.mode));
  /** The kind the user asked for while the body is a read-only one; `null` — no question on the screen. */
  let pending = $state<Mode | null>(null);
  /** What was entered in the kinds left during this visit, so that switching back does not lose it. */
  let kept: { [M in EnteredMode]?: Extract<DraftBody, { mode: M }> } = {};
  /**
   * The body this editor was given or has set itself. Another one means the body was replaced from outside —
   * the edits of the tab were thrown away, a draft was restored — and what was kept belongs to the old one.
   */
  let known = untrack(() => body);

  function emptyBody(mode: Mode): DraftBody {
    switch (mode) {
      case "raw":
        return { mode, data: { text: "", language: "text" } };
      case "url_encoded":
      case "form_data":
        return { mode, data: [] };
      case "binary":
        return { mode, data: { file: null } };
      default:
        return { mode: "none" };
    }
  }

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
    if (body !== known) kept = {};
    if (body.mode === "raw") kept.raw = body;
    else if (body.mode === "url_encoded") kept.url_encoded = body;
    else if (body.mode === "form_data") kept.form_data = body;
    else if (body.mode === "binary") kept.binary = body;

    body = (mode === "raw" || mode === "url_encoded" || mode === "form_data" || mode === "binary" ? kept[mode] : undefined) ?? emptyBody(mode);
    // Read back: what the tab holds now, not the object that was handed to it.
    known = body;
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
    <select class="select" aria-label={strings.body.kind} value={body.mode} onchange={onKindChosen}>
      {#each EDITABLE as mode (mode)}
        <option value={mode}>{strings.body.kinds[mode]}</option>
      {/each}
      {#if readOnly}
        <option value={body.mode}>{strings.body.kinds[body.mode]}</option>
      {/if}
    </select>
    {#if body.mode === "raw"}
      <select class="select" aria-label={strings.body.language} bind:value={body.data.language}>
        {#each LANGUAGES as language (language)}
          <option value={language}>{strings.body.languages[language]}</option>
        {/each}
      </select>
      {#if body.data.language === "json"}
        <Button icon="format" onclick={beautify}>{strings.body.beautify}</Button>
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
      <!-- JSON and JavaScript have highlighting; the other languages decide the Content-Type alone. -->
      <CodeEditor
        value={raw.data.text}
        language={raw.data.language === "json" || raw.data.language === "javascript" ? raw.data.language : "text"}
        label={strings.body.text}
        variables
        onchange={(text) => (raw.data.text = text)}
      />
    </div>
  {:else if body.mode === "url_encoded"}
    <div class="scroll">
      <KeyValueTable bind:rows={body.data} label={strings.body.fields} create={newField} variables />
    </div>
  {:else if body.mode === "form_data"}
    <div class="scroll">
      <FormDataEditor bind:parts={body.data} />
    </div>
  {:else if body.mode === "binary"}
    <BinaryBodyEditor bind:file={body.data.file} />
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
      <Button variant="primary" onclick={confirmReplace}>{strings.body.replace}</Button>
      <Button data-initial-focus onclick={() => (pending = null)}>{strings.body.cancel}</Button>
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
    padding: calc(var(--space) * 2) calc(var(--space) * 3);
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

  .empty {
    margin: 0;
    padding: calc(var(--space) * 2);
  }

  pre {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
    margin: calc(var(--space) * 1) 0 calc(var(--space) * 3);
    white-space: pre-wrap;
  }

  .question {
    margin: 0;
  }
</style>
