// Where the response stands next to the editor and how the area is shared between them. Remembered between
// launches in `localStorage` of the window (to be moved into the database with the rest of the window state).

/** Side of the editor the response stands on; `auto` — on the right in a wide window, below in a narrow one. */
export const PLACEMENTS = ["right", "bottom", "left", "top", "auto"] as const;
export type Placement = (typeof PLACEMENTS)[number];
/** A side the response really stands on: what `auto` turns into. */
export type Side = Exclude<Placement, "auto">;

/** Share of the editor in percent: the limits and the value of a new window. */
export const SPLIT_MIN = 20;
export const SPLIT_MAX = 80;
export const SPLIT_DEFAULT = 50;
/** From this width of the window on, `auto` puts the response on the right. */
export const WIDE_WINDOW = 1280;

const STORAGE_KEY = "rqm.layout";

function isPlacement(value: unknown): value is Placement {
  return typeof value === "string" && (PLACEMENTS as readonly string[]).includes(value);
}

function clampSplit(value: number): number {
  return Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, value));
}

export class Layout {
  placement = $state<Placement>("auto");
  /** Share of the editor in percent, `SPLIT_MIN`..`SPLIT_MAX`; the response takes the rest. */
  split = $state(SPLIT_DEFAULT);

  constructor() {
    this.load();
  }

  /**
   * Reads what was remembered. Anything missing or broken falls back to its default, a share out of the
   * limits is brought to the nearest one; a storage that cannot be read leaves the defaults.
   */
  load() {
    let stored: unknown = null;
    try {
      stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    } catch {
      // Broken JSON or no access to the storage: the defaults.
    }
    const { placement, split } = (typeof stored === "object" && stored !== null ? stored : {}) as Record<string, unknown>;
    this.placement = isPlacement(placement) ? placement : "auto";
    this.split = typeof split === "number" && Number.isFinite(split) ? clampSplit(split) : SPLIT_DEFAULT;
  }

  /** Puts the response on the side; applied at once and remembered. */
  choose(placement: Placement) {
    this.placement = placement;
    this.save();
  }

  /** Changes the share of the editor, within the limits; remembered. */
  resize(split: number) {
    this.split = clampSplit(split);
    this.save();
  }

  /** The side `auto` stands for in a window of this width; any other placement is its own side. */
  side(windowWidth: number): Side {
    if (this.placement !== "auto") return this.placement;
    return windowWidth >= WIDE_WINDOW ? "right" : "bottom";
  }

  private save() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ placement: this.placement, split: this.split }));
    } catch {
      // Not remembered: the layout still works until the window is closed.
    }
  }
}

/** The layout of the window. */
export const layout = new Layout();
