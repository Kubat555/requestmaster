import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { motion } from "./motion.svelte";
import { slideIndicator } from "./slideIndicator";

/** Gives the element a place in the row: jsdom lays nothing out. */
function at(element: HTMLElement, left: number, width: number) {
  Object.defineProperty(element, "offsetLeft", { configurable: true, value: left });
  Object.defineProperty(element, "offsetWidth", { configurable: true, value: width });
}

/** A row of tabs, each 80 px wide but the last, the one at `chosen` selected. */
function rowOf(names: string[], chosen: number): HTMLElement {
  const row = document.body.appendChild(document.createElement("div"));
  row.setAttribute("role", "tablist");
  names.forEach((name, index) => {
    const tab = row.appendChild(document.createElement("button"));
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-selected", String(index === chosen));
    tab.textContent = name;
    at(tab, index * 80, index === names.length - 1 ? 120 : 80);
  });
  return row;
}

function markOf(row: HTMLElement): HTMLElement | null {
  return row.querySelector<HTMLElement>(".slide-indicator");
}

/** Where the mark was told to stand: its shift and its width. */
function placeOf(row: HTMLElement): [string, string] {
  const mark = markOf(row)!;
  return [mark.style.getPropertyValue("--slide-x"), mark.style.getPropertyValue("--slide-width")];
}

function choose(row: HTMLElement, index: number) {
  [...row.querySelectorAll('[role="tab"]')].forEach((tab, at) => tab.setAttribute("aria-selected", String(at === index)));
}

/** The observers answer a little later. */
async function settled() {
  await new Promise((resolve) => setTimeout(resolve));
}

/** The next frame has come. */
async function frame() {
  await new Promise((resolve) => requestAnimationFrame(resolve));
}

let remove: (() => void) | undefined;

beforeEach(() => motion.choose("rich"));

afterEach(() => {
  remove?.();
  remove = undefined;
  motion.choose("normal");
  document.body.replaceChildren();
});

describe("slideIndicator", () => {
  it("puts one hidden-from-readers mark into the row, at the chosen tab", () => {
    const row = rowOf(["Params", "Headers", "Body"], 1);

    remove = slideIndicator(row);

    expect(row.querySelectorAll(".slide-indicator")).toHaveLength(1);
    expect(markOf(row)!.getAttribute("aria-hidden")).toBe("true");
    expect(markOf(row)!.hidden).toBe(false);
    expect(placeOf(row)).toEqual(["80px", "80"]);
    // The tabs of the row are the same as before.
    expect(row.querySelectorAll('[role="tab"]')).toHaveLength(3);
    expect(markOf(row)!.matches('[role="tab"], button, [tabindex]')).toBe(false);
  });

  it("takes its first place at once and slides from the next frame on", async () => {
    const row = rowOf(["Params", "Headers"], 0);

    remove = slideIndicator(row);
    expect(markOf(row)!.classList.contains("sliding")).toBe(false);

    await frame();
    expect(markOf(row)!.classList.contains("sliding")).toBe(true);
  });

  it("follows the choice", async () => {
    const row = rowOf(["Params", "Headers", "Body"], 0);
    remove = slideIndicator(row);
    expect(placeOf(row)).toEqual(["0px", "80"]);

    choose(row, 2);
    await settled();

    expect(placeOf(row)).toEqual(["160px", "120"]);
    expect(row.querySelectorAll(".slide-indicator")).toHaveLength(1);
  });

  it("follows the chosen tab when the tabs before it change: one is removed, a text grows", async () => {
    const row = rowOf(["One", "Two", "Three"], 2);
    remove = slideIndicator(row);
    expect(placeOf(row)).toEqual(["160px", "120"]);

    const [first, , third] = row.querySelectorAll<HTMLElement>('[role="tab"]');
    first.remove();
    at(third, 80, 120);
    await settled();
    expect(placeOf(row)).toEqual(["80px", "120"]);

    // A text node changes, as when a tab is renamed or a counter appears.
    row.querySelector('[role="tab"]')!.firstChild!.textContent = "A much longer name";
    at(third, 200, 120);
    await settled();
    expect(placeOf(row)).toEqual(["200px", "120"]);
  });

  it("takes the place of what stands around the chosen tab when that is marked", () => {
    const row = document.body.appendChild(document.createElement("div"));
    const item = row.appendChild(document.createElement("div"));
    item.setAttribute("data-slide-item", "");
    at(item, 30, 200);
    const title = item.appendChild(document.createElement("button"));
    title.setAttribute("aria-selected", "true");
    at(title, 0, 150);

    remove = slideIndicator(row);

    expect(placeOf(row)).toEqual(["30px", "200"]);
  });

  it("hides while no tab is chosen and comes back with a choice", async () => {
    const row = rowOf(["One", "Two"], -1);
    remove = slideIndicator(row);
    expect(markOf(row)!.hidden).toBe(true);

    choose(row, 1);
    await settled();

    expect(markOf(row)!.hidden).toBe(false);
    expect(placeOf(row)).toEqual(["80px", "120"]);
  });

  it("keeps its place while the row is not laid out, and takes its first place at once when it is", async () => {
    // The row stands in a hidden tab: nothing in it has a size.
    const row = rowOf(["One", "Two"], 1);
    const tabsOfRow = [...row.querySelectorAll<HTMLElement>('[role="tab"]')];
    tabsOfRow.forEach((tab) => at(tab, 0, 0));

    remove = slideIndicator(row);
    await frame();
    expect(placeOf(row)).toEqual(["", ""]);
    expect(markOf(row)!.classList.contains("sliding")).toBe(false);

    // The tab is shown: the row is laid out and something in it changes.
    tabsOfRow.forEach((tab, index) => at(tab, index * 80, 80));
    choose(row, 1);
    row.append(document.createComment("shown"));
    await settled();
    expect(placeOf(row)).toEqual(["80px", "80"]);
    expect(markOf(row)!.classList.contains("sliding")).toBe(false);
    await frame();
    expect(markOf(row)!.classList.contains("sliding")).toBe(true);

    // Hidden again: the mark stays where it was instead of going to the corner.
    tabsOfRow.forEach((tab) => at(tab, 0, 0));
    row.append(document.createComment("hidden"));
    await settled();
    expect(placeOf(row)).toEqual(["80px", "80"]);
  });

  it("comes back into a row that was emptied as a whole", async () => {
    const row = rowOf(["One", "Two"], 0);
    remove = slideIndicator(row);

    row.textContent = "";
    const tab = row.appendChild(document.createElement("button"));
    tab.setAttribute("aria-selected", "true");
    at(tab, 0, 50);
    await settled();

    expect(row.querySelectorAll(".slide-indicator")).toHaveLength(1);
    expect(placeOf(row)).toEqual(["0px", "50"]);
  });

  it("leaves nothing behind when it is taken away", async () => {
    const row = rowOf(["One", "Two"], 0);
    slideIndicator(row)!();

    expect(markOf(row)).toBeNull();
    choose(row, 1);
    await settled();
    expect(markOf(row)).toBeNull();
  });

  it.each(["off", "normal"] as const)("does nothing at the motion level %j: the row keeps its static mark", (level) => {
    motion.choose(level);
    const row = rowOf(["One", "Two"], 0);
    const before = row.innerHTML;

    expect(slideIndicator(row)).toBeUndefined();

    expect(row.innerHTML).toBe(before);
  });

  it("never writes a style attribute by text: only properties are set", () => {
    const row = rowOf(["One"], 0);
    remove = slideIndicator(row);

    // The policy of the window drops inline styles; properties set by script are allowed.
    expect(markOf(row)!.style.length).toBe(2);
    expect(markOf(row)!.style.getPropertyValue("--slide-x")).toBe("0px");
  });
});
