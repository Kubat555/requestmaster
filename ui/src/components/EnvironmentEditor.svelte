<script lang="ts">
  import { getContext, tick } from "svelte";
  import IconButton from "./ui/IconButton.svelte";
  import type { Api } from "../lib/api";
  import { strings } from "../lib/strings";
  import type { EnvironmentTab } from "../lib/tabs.svelte";
  import type { VariableRow } from "../lib/types";

  interface Props {
    /** The tab of the environment; the table changes its variables and the tab saves them. */
    tab: EnvironmentTab;
  }

  let { tab }: Props = $props();

  const api = getContext<Api>("api");

  let body: HTMLTableSectionElement;
  /** Secret variables whose value the user asked to show. */
  let revealed = $state.raw<VariableRow[]>([]);

  /** Something of a variable was changed: the tab saves when the user stops for a moment. */
  function edited() {
    tab.edited(api);
  }

  /** The focus left the row for another place: what was changed in it is saved now. */
  function onRowFocusOut(event: FocusEvent) {
    const row = event.currentTarget as HTMLElement;
    if (event.relatedTarget instanceof Node && row.contains(event.relatedTarget)) return;
    void tab.flush(api);
  }

  /**
   * The first character typed into the empty row makes a real row of it; the focus follows the text there,
   * so the user goes on typing in the same place.
   */
  async function add(field: "key" | "value", event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const variable: VariableRow = { key: "", value: "", secret: false, enabled: true, postman_extra: {} };
    variable[field] = input.value;
    input.value = "";
    tab.variables.push(variable);
    edited();
    await tick();
    const added = body.rows[tab.variables.length - 1]?.querySelector<HTMLInputElement>(`input[data-field="${field}"]`);
    added?.focus();
    added?.setSelectionRange(added.value.length, added.value.length);
  }

  function remove(index: number) {
    const [removed] = tab.variables.splice(index, 1);
    revealed = revealed.filter((variable) => variable !== removed);
    edited();
  }

  /**
   * The mark "secret" of a variable was changed. One that is not secret is not "shown" any more: made secret
   * again, it is masked.
   */
  function onSecretChanged(variable: VariableRow) {
    if (!variable.secret) revealed = revealed.filter((other) => other !== variable);
    edited();
  }

  function toggleRevealed(variable: VariableRow) {
    revealed = revealed.includes(variable) ? revealed.filter((other) => other !== variable) : [...revealed, variable];
  }
</script>

<div class="environment-editor">
  <header>
    <h2>{tab.name}</h2>
    {#if tab.failed}
      <!-- The tab has no mark of unsaved edits: they are saved by themselves. This one tells that they were not. -->
      <span class="unsaved" role="status">{strings.environment.unsaved}</span>
    {/if}
  </header>
  <div class="panel">
    <table aria-label={strings.environment.variables}>
      <thead>
        <tr>
          <th class="check"><span class="visually-hidden">{strings.table.enabled}</span></th>
          <th>{strings.environment.name}</th>
          <th>{strings.table.value}</th>
          <th class="secret">{strings.environment.secret}</th>
          <th class="actions"></th>
        </tr>
      </thead>
      <tbody bind:this={body}>
        {#each tab.variables as variable, index (index)}
          {@const shown = revealed.includes(variable)}
          <tr class="hover-row" class:off={!variable.enabled} onfocusout={onRowFocusOut}>
            <td class="check">
              <input type="checkbox" aria-label={strings.table.enabled} bind:checked={variable.enabled} onchange={edited} />
            </td>
            <td>
              <input
                type="text"
                data-field="key"
                aria-label={strings.environment.name}
                spellcheck="false"
                bind:value={variable.key}
                oninput={edited}
              />
            </td>
            <td>
              <input
                type={variable.secret && !shown ? "password" : "text"}
                data-field="value"
                aria-label={strings.table.value}
                spellcheck="false"
                autocomplete="off"
                bind:value={variable.value}
                oninput={edited}
              />
            </td>
            <td class="secret">
              <input
                type="checkbox"
                aria-label={strings.environment.secret}
                bind:checked={variable.secret}
                onchange={() => onSecretChanged(variable)}
              />
            </td>
            <td class="actions">
              {#if variable.secret}
                <IconButton
                  icon={shown ? "eye-off" : "eye"}
                  size="sm"
                  label={shown ? strings.table.hide : strings.table.show}
                  aria-pressed={shown}
                  onclick={() => toggleRevealed(variable)}
                />
              {/if}
              <IconButton class="remove" icon="close" size="sm" label={strings.table.remove} onclick={() => remove(index)} />
            </td>
          </tr>
        {/each}
        <!-- The empty row: typing into it adds a variable. -->
        <tr>
          <td class="check"></td>
          <td>
            <input
              type="text"
              aria-label={strings.environment.name}
              placeholder={strings.environment.name}
              spellcheck="false"
              oninput={(event) => add("key", event)}
            />
          </td>
          <td>
            <input
              type="text"
              aria-label={strings.table.value}
              placeholder={strings.table.value}
              spellcheck="false"
              oninput={(event) => add("value", event)}
            />
          </td>
          <td class="secret"></td>
          <td class="actions"></td>
        </tr>
      </tbody>
    </table>
  </div>
</div>

<style>
  .environment-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: calc(var(--space) * 3);
    padding: calc(var(--space) * 3) calc(var(--space) * 2) var(--space);
  }

  h2 {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    font-size: inherit;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unsaved {
    flex: none;
    color: var(--danger);
  }

  .panel {
    flex: 1;
    min-height: 0;
    padding: calc(var(--space) * 2);
    overflow: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  /* The heading row stands apart from the rows: a raised background and bold text, so that it is not
     taken for the empty row with its placeholders. */
  th {
    height: var(--row-height);
    padding: 0 calc(var(--space) * 2);
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
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

  .secret {
    width: 72px;
    text-align: center;
  }

  /* Room for the two icon buttons of a row: show or hide, and remove. */
  .actions {
    width: 72px;
    padding: 0 var(--space);
    text-align: right;
    white-space: nowrap;
  }

  /* The buttons of a row stand side by side in the middle of its height. */
  td.actions > :global(*) {
    vertical-align: middle;
  }

  /* Deleting a row cannot be undone here, so its button warns with the colour. */
  td.actions > :global(.remove:hover),
  td.actions > :global(.remove:focus-visible) {
    color: var(--danger);
  }

  input[type="text"],
  input[type="password"] {
    width: 100%;
    height: var(--row-height);
    padding: 0 calc(var(--space) * 2);
    background: transparent;
    border: none;
    font-family: var(--font-mono);
  }

  input::placeholder {
    color: var(--text-muted);
  }

  /* A variable that is switched off is kept but not substituted. */
  .off input[type="text"],
  .off input[type="password"] {
    color: var(--text-muted);
  }
</style>
