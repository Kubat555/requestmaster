import { describe, expect, it } from "vitest";
import { Layout } from "./layout.svelte";

describe("Layout", () => {
  it("starts with the automatic placement and equal shares", () => {
    const layout = new Layout();
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(50);
  });

  it("applies the remembered placement and share", () => {
    const layout = new Layout();
    layout.apply({ placement: "left", split: 36 });
    expect(layout.placement).toBe("left");
    expect(layout.split).toBe(36);
  });

  it("falls back to the defaults when nothing was remembered", () => {
    const layout = new Layout();
    layout.choose("top");
    layout.resize(30);

    layout.apply({});
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(50);
  });

  it("takes an unknown placement for the automatic one and keeps the share", () => {
    const layout = new Layout();
    layout.apply({ placement: "diagonal", split: 30 });
    expect(layout.placement).toBe("auto");
    expect(layout.split).toBe(30);
  });

  it.each([
    [5, 20],
    [95, 80],
  ])("brings a remembered share of %d to the limit of %d", (remembered, expected) => {
    const layout = new Layout();
    layout.apply({ placement: "top", split: remembered });
    expect(layout.placement).toBe("top");
    expect(layout.split).toBe(expected);
  });

  it.each(["wide", null, Number.NaN])("resets a share that is not a number: %s", (split) => {
    const layout = new Layout();
    layout.apply({ placement: "bottom", split });
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

  it("writes nothing to localStorage when the placement is chosen and the share is changed", () => {
    const layout = new Layout();
    layout.choose("right");
    layout.resize(64);

    expect(layout.placement).toBe("right");
    expect(layout.split).toBe(64);
    expect(localStorage.length).toBe(0);
  });

  it("turns the automatic placement into a side by the width of the window", () => {
    const layout = new Layout();
    expect(layout.side(1279)).toBe("bottom");
    expect(layout.side(1280)).toBe("right");
    layout.choose("top");
    expect(layout.side(1920)).toBe("top");
  });
});
