// Where the response stands next to the editor and how the area is shared between them. Remembering it between
// launches is the job of `session.svelte.ts`: it applies what was remembered and writes the changes down.

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

  /**
   * Takes what was remembered. Anything missing or broken falls back to its default, a share out of the limits
   * is brought to the nearest one.
   */
  apply(state: { placement?: unknown; split?: unknown }): void {
    const { placement, split } = state;
    this.placement = isPlacement(placement) ? placement : "auto";
    this.split = typeof split === "number" && Number.isFinite(split) ? clampSplit(split) : SPLIT_DEFAULT;
  }

  /** Puts the response on the side; applied at once. */
  choose(placement: Placement) {
    this.placement = placement;
  }

  /** Changes the share of the editor, within the limits. */
  resize(split: number) {
    this.split = clampSplit(split);
  }

  /** The side `auto` stands for in a window of this width; any other placement is its own side. */
  side(windowWidth: number): Side {
    if (this.placement !== "auto") return this.placement;
    return windowWidth >= WIDE_WINDOW ? "right" : "bottom";
  }
}

/** The layout of the window. */
export const layout = new Layout();
