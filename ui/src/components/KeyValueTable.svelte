<script lang="ts" generics="Row extends { key: string; value: string; enabled: boolean }">
  import { tick } from "svelte";
  import IconButton from "./ui/IconButton.svelte";
  import VarInput from "./VarInput.svelte";
  import { strings } from "../lib/strings";

  interface Props {
    /** The rows, edited in place. Only `key`, `value` and `enabled` of a row are written. */
    rows: Row[];
    /** Accessible name of the table. */
    label: string;
    /** Heading of the key column. */
    keyLabel?: string;
    /** Makes the row that is added when the user types into the empty row at the end. */
    create: () => Row;
    /** Whether the value of the row is masked until the user asks to show it. */
    secret?: (row: Row) => boolean;
    /**
     * The `{{variables}}` in the cells are marked, explained and offered: `true` — in the keys and the values,
     * `"value"` — in the values only. A masked value is never marked.
     */
    variables?: boolean | "value";
  }

  let { rows = $bindable(), label, keyLabel = strings.table.key, create, secret, variables = false }: Props = $props();

  let body: HTMLTableSectionElement;
  /** Rows whose masked value the user asked to show. */
  let revealed = $state.raw<Row[]>([]);

  /**
   * The first character typed into the empty row makes a real row of it; the focus follows the text there,
   * so the user goes on typing in the same place.
   */
  async function add(field: "key" | "value", event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const row = create();
    (row as { key: string; value: string })[field] = input.value;
    input.value = "";
    rows.push(row);
    await tick();
    const added = body.rows[rows.length - 1]?.querySelector<HTMLInputElement>(`input[data-field="${field}"]`);
    added?.focus();
    added?.setSelectionRange(added.value.length, added.value.length);
  }

  function remove(index: number) {
    const [removed] = rows.splice(index, 1);
    revealed = revealed.filter((row) => row !== removed);
  }

  function toggleRevealed(row: Row) {
    revealed = revealed.includes(row) ? revealed.filter((other) => other !== row) : [...revealed, row];
  }

  // A row that stopped being secret is not "shown" any more: made secret again, it is masked.
  $effect(() => {
    if (revealed.some((row) => !secret?.(row))) revealed = revealed.filter((row) => secret?.(row));
  });
</script>

<table aria-label={label}>
  <thead>
    <tr>
      <th class="check"><span class="visually-hidden">{strings.table.enabled}</span></th>
      <th>{keyLabel}</th>
      <th>{strings.table.value}</th>
      <th class="actions"></th>
    </tr>
  </thead>
  <tbody bind:this={body}>
    {#each rows as row, index (index)}
      {@const masked = secret?.(row) ?? false}
      {@const shown = revealed.includes(row)}
      <tr class:off={!row.enabled}>
        <td class="check"><input type="checkbox" aria-label={strings.table.enabled} bind:checked={row.enabled} /></td>
        <td>
          {#if variables === true}
            <VarInput data-field="key" aria-label={keyLabel} spellcheck="false" bind:value={row.key} />
          {:else}
            <input type="text" data-field="key" aria-label={keyLabel} spellcheck="false" bind:value={row.key} />
          {/if}
        </td>
        <td>
          {#if variables && !(masked && !shown)}
            <VarInput data-field="value" aria-label={strings.table.value} spellcheck="false" autocomplete="off" bind:value={row.value} />
          {:else}
            <input
              type={masked && !shown ? "password" : "text"}
              data-field="value"
              aria-label={strings.table.value}
              spellcheck="false"
              autocomplete="off"
              bind:value={row.value}
            />
          {/if}
        </td>
        <td class="actions">
          {#if masked}
            <IconButton
              icon={shown ? "eye-off" : "eye"}
              size="sm"
              label={shown ? strings.table.hide : strings.table.show}
              aria-pressed={shown}
              onclick={() => toggleRevealed(row)}
            />
          {/if}
          <IconButton class="remove" icon="close" size="sm" label={strings.table.remove} onclick={() => remove(index)} />
        </td>
      </tr>
    {/each}
    <!-- The empty row: typing into it adds a row. -->
    <tr>
      <td class="check"></td>
      <td><input type="text" aria-label={keyLabel} placeholder={keyLabel} spellcheck="false" oninput={(event) => add("key", event)} /></td>
      <td>
        <input
          type="text"
          aria-label={strings.table.value}
          placeholder={strings.table.value}
          spellcheck="false"
          oninput={(event) => add("value", event)}
        />
      </td>
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
    vertical-align: middle;
  }

  /* A row that is switched off is kept but not sent. */
  .off input[type="text"],
  .off input[type="password"],
  .off :global(.var-input) {
    color: var(--text-muted);
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
