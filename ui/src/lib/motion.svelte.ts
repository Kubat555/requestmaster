// How much the window animates. The level is one of the application settings: the dialog of the settings saves
// it, the window reads it once at the start and applies a saved one at once. The stylesheets read the level in
// effect from `<html data-motion>`: `off` zeroes the durations (tokens.css), `rich` switches the extra effects on.

import type { Api } from "./api";
import { toasts } from "./toasts.svelte";
import { MOTION_LEVELS, type MotionLevel } from "./types";

/** The question to the operating system: does the user ask for less motion. */
const REDUCED_MOTION = "(prefers-reduced-motion: reduce)";

/** The level a stored value stands for; anything that is not a level is the normal one. */
export function motionLevelOf(value: unknown): MotionLevel {
  return MOTION_LEVELS.find((level) => level === value) ?? "normal";
}

export class Motion {
  /** The level of the settings; `normal` until they are read. */
  #chosen = $state<MotionLevel>("normal");
  /** The operating system asks for less motion: then nothing moves, whatever is chosen. */
  #reduced = $state(false);
  /** Changes with every {@link choose}: a read of the settings that began before it must not undo it. */
  #choices = 0;

  constructor() {
    // The test environment has no `matchMedia`.
    const query = typeof window.matchMedia === "function" ? window.matchMedia(REDUCED_MOTION) : null;
    this.#reduced = query?.matches ?? false;
    query?.addEventListener("change", (event) => {
      this.#reduced = event.matches;
      this.#show();
    });
    this.#show();
  }

  /** The level in effect: the chosen one, or `off` while the operating system asks for less motion. */
  get level(): MotionLevel {
    return this.#reduced ? "off" : this.#chosen;
  }

  /** Whether the extra effects are on; what makes them by script asks here. */
  get rich(): boolean {
    return this.level === "rich";
  }

  /** Takes the level and applies it at once. Saving it is the job of the dialog of the settings. */
  choose(level: MotionLevel): void {
    this.#choices++;
    this.#chosen = level;
    this.#show();
  }

  /**
   * Reads the saved level: one command, not waited for by the first draw. Never rejects: a failure to read is
   * shown as a notification and leaves the level as it is.
   */
  async start(api: Api): Promise<void> {
    const choices = this.#choices;
    let level: MotionLevel;
    try {
      level = motionLevelOf((await api.getSettings()).motion);
    } catch (error) {
      toasts.error(String(error));
      return;
    }
    if (choices === this.#choices) this.choose(level);
  }

  /** Tells the stylesheets the level in effect. */
  #show(): void {
    document.documentElement.dataset.motion = this.level;
  }
}

/** The motion level of the window. */
export const motion = new Motion();
