import { afterEach, describe, expect, it, vi } from "vitest";
import { Layout } from "./layout.svelte";

const KEY = "rqm.layout";

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("Layout", () => {
  it("starts with the automatic placement and equal shares", () => {
    const layout = new Layout();
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(50);
  });

  it("remembers the placement and the share for the next launch under one key", () => {
    const layout = new Layout();
    layout.choose("left");
    layout.resize(36);

    expect(JSON.parse(localStorage.getItem(KEY)!)).toEqual({ placement: "left", split: 36 });
    const next = new Layout();
    expect(next.placement).toBe("left");
    expect(next.split).toBe(36);
  });

  it("falls back to the defaults when the stored text is not JSON", () => {
    localStorage.setItem(KEY, "{not json");
    const layout = new Layout();
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(50);
  });

  it.each(["null", "42", '"right"', "[]"])("falls back to the defaults when the stored value is %s", (stored) => {
    localStorage.setItem(KEY, stored);
    const layout = new Layout();
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(50);
  });

  it("takes an unknown placement for the automatic one and keeps the share", () => {
    localStorage.setItem(KEY, JSON.stringify({ placement: "diagonal", split: 30 }));
    const layout = new Layout();
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(30);
  });

  it.each([
    [5, 20],
    [95, 80],
  ])("brings a stored share of %d to the limit of %d", (stored, expected) => {
    localStorage.setItem(KEY, JSON.stringify({ placement: "top", split: stored }));
    const layout = new Layout();
    expect(layout.placement).toBe("top");
    expect(layout.split).toBe(expected);
  });

  it("resets a share that is not a number", () => {
    localStorage.setItem(KEY, JSON.stringify({ placement: "bottom", split: "wide" }));
    const layout = new Layout();
    expect(layout.placement).toBe("bottom");
    expect(layout.split).toBe(50);
  });

  it("keeps a new share within 20..80", () => {
    const layout = new Layout();
    layout.resize(3);
    expect(layout.split).toBe(20);
    layout.resize(120);
    expect(layout.split).toBe(80);
  });

  it("works with the defaults when the storage cannot be read", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    const layout = new Layout();
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(50);
  });

  it("still applies a choice when the storage cannot be written", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("quota");
    });
    const layout = new Layout();
    layout.choose("right");
    layout.resize(64);
    expect(layout.placement).toBe("right");
    expect(layout.split).toBe(64);
  });

  it("turns the automatic placement into a side by the width of the window", () => {
    const layout = new Layout();
    expect(layout.side(1279)).toBe("bottom");
    expect(layout.side(1280)).toBe("right");
    layout.choose("top");
    expect(layout.side(1920)).toBe("top");
  });
});
