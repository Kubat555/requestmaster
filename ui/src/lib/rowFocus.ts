// Where the focus goes after the empty row at the end of a table of rows has become a real one.

/**
 * Puts the focus into the `field` cell (`input[data-field]`) of the row `index` of the table body, with the
 * caret at the end of its text. Nothing happens when there is no such row or cell.
 */
export function focusRowField(body: HTMLTableSectionElement, index: number, field: string): void {
  const added = body.rows[index]?.querySelector<HTMLInputElement>(`input[data-field="${field}"]`);
  added?.focus();
  added?.setSelectionRange(added.value.length, added.value.length);
}
