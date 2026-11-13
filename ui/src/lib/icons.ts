// The icons of the interface: outlines on a 16 x 16 grid, drawn by `Icon.svelte` in the colour of the text.

const TRAY = "M2.5 11v1.5a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V11";
const SHEET = "M7 5.5h5.5A1.5 1.5 0 0 1 14 7v5.5a1.5 1.5 0 0 1-1.5 1.5H7a1.5 1.5 0 0 1-1.5-1.5V7A1.5 1.5 0 0 1 7 5.5z";
const SHEET_BEHIND = "M10.5 3.5A1.5 1.5 0 0 0 9 2H3.5A1.5 1.5 0 0 0 2 3.5V9a1.5 1.5 0 0 0 1.5 1.5";

/** Paths (`d`) of every icon, by its name. */
export const icons = {
  "chevron-down": ["M4 6l4 4 4-4"],
  "chevron-right": ["M6 4l4 4-4 4"],
  search: ["M11.5 7a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0z", "M10.5 10.5L14 14"],
  plus: ["M8 3v10", "M3 8h10"],
  send: ["M14 2L7.5 8.5", "M14 2L9.5 14l-2-5.5L2 6.5z"],
  copy: [SHEET, SHEET_BEHIND],
  close: ["M4 4l8 8", "M12 4l-8 8"],
  import: ["M8 2v8", "M5 7l3 3 3-3", TRAY],
  export: ["M8 10V2", "M5 5l3-3 3 3", TRAY],
  trash: ["M2.5 4.5h11", "M6 4.5V3a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1v1.5", "M4 4.5l.6 8.1a1 1 0 0 0 1 .9h4.8a1 1 0 0 0 1-.9l.6-8.1"],
  edit: ["M10.5 3L13 5.5 6 12.5l-3 .5.5-3z", "M9 4.5L11.5 7"],
  duplicate: [SHEET, SHEET_BEHIND, "M9.75 8v3.5", "M8 9.75h3.5"],
  folder: ["M2 4.5a1 1 0 0 1 1-1h3L7.5 5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"],
  collection: ["M3 2.5h10a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z", "M5 6h6", "M5 8.5h6", "M5 11h3"],
  environment: ["M8 2l6 3-6 3-6-3z", "M2 8l6 3 6-3", "M2 11l6 3 6-3"],
  history: ["M14 8A6 6 0 1 1 2 8a6 6 0 0 1 12 0z", "M8 4.5V8l2.5 1.5"],
  more: ["M3.5 8h.01", "M8 8h.01", "M12.5 8h.01"],
  check: ["M3 8.5L6.5 12 13 5"],
} as const satisfies Record<string, readonly string[]>;

/** Name of an icon of the set. */
export type IconName = keyof typeof icons;
