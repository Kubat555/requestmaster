import { afterEach, describe, expect, it, vi } from "vitest";
import { Motion, motion, motionLevelOf } from "./motion.svelte";
import { createFakeApi } from "./testing/fakeApi";
import { toasts } from "./toasts.svelte";
import type { MotionLevel, SettingsDto } from "./types";

function settings(level: unknown): SettingsDto {
  return { timeout_ms: 0, verify_tls: true, proxy: { mode: "system" }, motion: level as MotionLevel };
}

/** What `<html data-motion>` says: the stylesheets read the level from there. */
function shown(): string | undefined {
  return document.documentElement.dataset.motion;
}

/** A `matchMedia` of the window whose answer about reduced motion a test sets and changes. */
function stubReducedMotion(matches: boolean) {
  const listeners = new Set<(event: { matches: boolean }) => void>();
  const query = {
    matches,
    addEventListener: (_: "change", listener: (event: { matches: boolean }) => void) => void listeners.add(listener),
  };
  const matchMedia = vi.fn(() => query);
  vi.stubGlobal("matchMedia", matchMedia);
  return {
    matchMedia,
    /** The user switched the reduced motion of the operating system. */
    change(next: boolean) {
      query.matches = next;
      for (const listener of listeners) listener({ matches: next });
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  motion.choose("normal");
  toasts.items = [];
});

describe("motionLevelOf", () => {
  it("takes the three levels and turns anything else into the normal one", () => {
    expect(motionLevelOf("off")).toBe("off");
    expect(motionLevelOf("normal")).toBe("normal");
    expect(motionLevelOf("rich")).toBe("rich");
    for (const unknown of ["extreme", "RICH", "", undefined, null, 2, {}]) expect(motionLevelOf(unknown)).toBe("normal");
  });
});

describe("Motion", () => {
  it("is normal until the settings are read, and says so on the root element", () => {
    const fresh = new Motion();

    expect(fresh.level).toBe("normal");
    expect(fresh.rich).toBe(false);
    expect(shown()).toBe("normal");
  });

  it("the window has its level on the root element as soon as the module is loaded", () => {
    expect(motion.level).toBe("normal");
    expect(shown()).toBe("normal");
  });

  it.each(["off", "normal", "rich"] as const)("takes the saved level %j at the start with one command", async (level) => {
    const api = createFakeApi({ settings: settings(level) });
    const getSettings = vi.spyOn(api, "getSettings");
    const fresh = new Motion();

    await fresh.start(api);

    expect(getSettings).toHaveBeenCalledTimes(1);
    expect(fresh.level).toBe(level);
    expect(fresh.rich).toBe(level === "rich");
    expect(shown()).toBe(level);
  });

  it("takes settings without a level, or with one it does not know, as normal", async () => {
    for (const stored of [undefined, "extreme", 5]) {
      const fresh = new Motion();
      fresh.choose("rich");

      await fresh.start(createFakeApi({ settings: settings(stored) }));

      expect(fresh.level).toBe("normal");
      expect(shown()).toBe("normal");
    }
  });

  it("stays normal and tells when the settings cannot be read", async () => {
    const api = createFakeApi();
    vi.spyOn(api, "getSettings").mockRejectedValueOnce("storage is locked");
    const fresh = new Motion();

    await expect(fresh.start(api)).resolves.toBeUndefined();

    expect(fresh.level).toBe("normal");
    expect(shown()).toBe("normal");
    expect(toasts.items.map((toast) => toast.text)).toEqual(["storage is locked"]);
  });

  it("applies a chosen level at once", () => {
    const fresh = new Motion();

    fresh.choose("rich");
    expect(fresh.level).toBe("rich");
    expect(fresh.rich).toBe(true);
    expect(shown()).toBe("rich");

    fresh.choose("off");
    expect(fresh.level).toBe("off");
    expect(shown()).toBe("off");
  });

  it("keeps a level chosen while the saved one was still being read", async () => {
    const api = createFakeApi({ settings: settings("off") });
    const fresh = new Motion();

    const reading = fresh.start(api);
    fresh.choose("rich");
    await reading;

    expect(fresh.level).toBe("rich");
    expect(shown()).toBe("rich");
  });

  it("is off under the reduced motion of the operating system, whatever is chosen", async () => {
    const system = stubReducedMotion(true);
    const fresh = new Motion();

    expect(system.matchMedia).toHaveBeenCalledWith("(prefers-reduced-motion: reduce)");
    expect(fresh.level).toBe("off");
    expect(shown()).toBe("off");

    fresh.choose("rich");
    expect(fresh.level).toBe("off");
    expect(fresh.rich).toBe(false);
    expect(shown()).toBe("off");

    await fresh.start(createFakeApi({ settings: settings("normal") }));
    expect(fresh.level).toBe("off");
    expect(shown()).toBe("off");
  });

  it("follows the operating system when its reduced motion is switched", () => {
    const system = stubReducedMotion(false);
    const fresh = new Motion();
    fresh.choose("rich");
    expect(shown()).toBe("rich");

    system.change(true);
    expect(fresh.level).toBe("off");
    expect(shown()).toBe("off");

    system.change(false);
    expect(fresh.level).toBe("rich");
    expect(shown()).toBe("rich");
  });
});
