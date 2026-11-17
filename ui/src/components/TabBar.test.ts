import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { motion } from "../lib/motion.svelte";
import { strings } from "../lib/strings";
import { tabs } from "../lib/tabs.svelte";
import { createFakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import { workspace } from "../lib/workspace.svelte";

/** Renders the window over a fake api and opens the requests of the default tree in tabs, the last one shown. */
async function openTabs(...nodeIds: string[]) {
  const api = createFakeApi();
  render(App, { context: new Map([["api", api]]) });
  await screen.findByRole("treeitem", { name: /Demo API/ });
  for (const nodeId of nodeIds) await tabs.open(api, nodeId);
  await waitFor(() => expect(titles()).toHaveLength(nodeIds.length));
  return api;
}

/** The bar of the tabs. */
function bar(): HTMLElement {
  return screen.getByRole("tablist", { name: strings.tabs.label });
}

/** The titles of the tabs, left to right; none while no tab is open. */
function titles(): HTMLElement[] {
  const list = screen.queryByRole("tablist", { name: strings.tabs.label });
  return list ? within(list).queryAllByRole("tab") : [];
}

/** The title of the shown tab. */
function activeTitle(): HTMLElement {
  return within(bar()).getByRole("tab", { selected: true });
}

/** Gives the bar the sizes of one whose tabs do not fit: jsdom lays nothing out. */
function overflow(element: HTMLElement, scrollWidth: number, clientWidth: number) {
  Object.defineProperty(element, "scrollWidth", { configurable: true, value: scrollWidth });
  Object.defineProperty(element, "clientWidth", { configurable: true, value: clientWidth });
}

// The tabs, the notifications and the workspace of the window live in modules; every test starts without them.
afterEach(() => {
  for (const open of [...tabs.list]) tabs.close(open.id);
  vi.restoreAllMocks();
  toasts.items = [];
  workspace.rows = [];
});

describe("wheel over the tab bar", () => {
  it("moves an overflowing bar sideways and cancels the turn", async () => {
    await openTabs("req-list-users", "req-create-user");
    overflow(bar(), 900, 300);

    const turn = createEvent.wheel(bar(), { deltaY: 100 });
    await fireEvent(bar(), turn);

    expect(bar().scrollLeft).toBe(100);
    expect(turn.defaultPrevented).toBe(true);
  });

  it("leaves the turn alone when the bar has nowhere to move", async () => {
    await openTabs("req-list-users", "req-create-user");
    overflow(bar(), 300, 300);

    const turn = createEvent.wheel(bar(), { deltaY: 100 });
    await fireEvent(bar(), turn);

    expect(bar().scrollLeft).toBe(0);
    expect(turn.defaultPrevented).toBe(false);
  });

  it("leaves a horizontal gesture alone", async () => {
    await openTabs("req-list-users", "req-create-user");
    overflow(bar(), 900, 300);

    const turn = createEvent.wheel(bar(), { deltaX: 40 });
    await fireEvent(bar(), turn);

    expect(bar().scrollLeft).toBe(0);
    expect(turn.defaultPrevented).toBe(false);
  });
});

describe("shown tab", () => {
  it("is brought into view when a tab is opened and when another one is shown, without animation", async () => {
    const scrollIntoView = vi.spyOn(Element.prototype, "scrollIntoView");
    const api = await openTabs("req-list-users");
    await waitFor(() => expect(scrollIntoView.mock.contexts).toContain(activeTitle()));
    scrollIntoView.mockClear();

    await tabs.open(api, "req-create-user");

    await waitFor(() => expect(titles()).toHaveLength(2));
    await waitFor(() => expect(scrollIntoView).toHaveBeenCalled());
    expect(activeTitle().textContent).toContain("Create user");
    expect(scrollIntoView.mock.contexts.at(-1)).toBe(activeTitle());
    for (const [options] of scrollIntoView.mock.calls) {
      expect(options).toEqual({ block: "nearest", inline: "nearest" });
    }
    scrollIntoView.mockClear();

    await fireEvent.click(titles()[0]);

    await waitFor(() => expect(scrollIntoView).toHaveBeenCalled());
    expect(scrollIntoView.mock.contexts.at(-1)).toBe(titles()[0]);
    expect(scrollIntoView.mock.calls.at(-1)).toEqual([{ block: "nearest", inline: "nearest" }]);
  });

  it("is brought into view when a tab before it is closed", async () => {
    const scrollIntoView = vi.spyOn(Element.prototype, "scrollIntoView");
    await openTabs("req-list-users", "req-create-user");
    await waitFor(() => expect(scrollIntoView).toHaveBeenCalled());
    scrollIntoView.mockClear();

    tabs.close("req-list-users");

    await waitFor(() => expect(scrollIntoView).toHaveBeenCalled());
    expect(titles()).toHaveLength(1);
    expect(scrollIntoView.mock.contexts.at(-1)).toBe(activeTitle());
  });
});

describe("focus ring of a tab", () => {
  it("is hidden after a press of the pointer and comes back with the arrows", async () => {
    await openTabs("req-list-users", "req-create-user");
    await fireEvent.click(titles()[0]);
    const [first] = titles();
    expect(first.classList.contains("by-pointer")).toBe(false);

    await fireEvent.pointerDown(first);
    first.focus();
    expect(first.classList.contains("by-pointer")).toBe(true);

    await fireEvent.keyDown(first, { key: "ArrowRight" });

    await waitFor(() => expect(document.activeElement).toBe(titles()[1]));
    expect(titles()[1].classList.contains("by-pointer")).toBe(false);
    expect(titles()[0].classList.contains("by-pointer")).toBe(false);
  });

  it("comes back when the title loses the focus and when Tab is pressed on it", async () => {
    await openTabs("req-list-users");
    const [title] = titles();

    await fireEvent.pointerDown(title);
    title.focus();
    expect(title.classList.contains("by-pointer")).toBe(true);
    await fireEvent.blur(title);
    expect(title.classList.contains("by-pointer")).toBe(false);

    await fireEvent.pointerDown(title);
    expect(title.classList.contains("by-pointer")).toBe(true);
    await fireEvent.keyDown(title, { key: "Tab" });
    expect(title.classList.contains("by-pointer")).toBe(false);
  });
});

describe("tab and its panel", () => {
  it("name each other for screen readers", async () => {
    await openTabs("req-list-users", "req-create-user");

    const ids = new Set<string>();
    for (const title of titles()) {
      const panel = document.getElementById(title.getAttribute("aria-controls")!)!;
      expect(title.id).not.toBe("");
      expect(panel.getAttribute("role")).toBe("tabpanel");
      expect(panel.getAttribute("aria-labelledby")).toBe(title.id);
      expect(panel.hasAttribute("data-tab-panel")).toBe(true);
      // The panel of the shown tab is the one on the screen.
      expect(panel.hidden).toBe(title.getAttribute("aria-selected") !== "true");
      ids.add(title.id).add(panel.id);
    }
    expect(ids.size).toBe(4);
    expect(screen.getByRole("tabpanel", { name: /Create user/ })).toBeTruthy();
  });
});

describe("tabs at the rich motion level", () => {
  afterEach(() => motion.choose("normal"));

  /** The tabs of the bar as they are drawn: the title with its cross. */
  function drawn(): HTMLElement[] {
    return [...bar().querySelectorAll<HTMLElement>("[data-tab-id]")];
  }

  /** Lays the tabs out in a row, 100 px each: jsdom lays nothing out. */
  function layOut() {
    vi.spyOn(HTMLElement.prototype, "offsetLeft", "get").mockImplementation(function (this: HTMLElement) {
      return this.hasAttribute("data-tab-id") ? [...this.parentElement!.children].indexOf(this) * 100 : 0;
    });
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
      return this.hasAttribute("data-tab-id") ? 100 : 0;
    });
  }

  function mark(): HTMLElement | null {
    return bar().querySelector<HTMLElement>(".slide-indicator");
  }

  it("has one sliding mark for the shown tab, which is not a tab and cannot be focused", async () => {
    layOut();
    motion.choose("rich");
    await openTabs("req-list-users", "req-create-user");

    await waitFor(() => expect(mark()?.style.getPropertyValue("--slide-x")).toBe("100px"));
    expect(bar().querySelectorAll(".slide-indicator")).toHaveLength(1);
    expect(mark()!.style.getPropertyValue("--slide-width")).toBe("100");
    expect(mark()!.getAttribute("aria-hidden")).toBe("true");
    expect(mark()!.matches("button, [role], [tabindex]")).toBe(false);
    expect(titles()).toHaveLength(2);

    await fireEvent.click(titles()[0]);
    await waitFor(() => expect(mark()!.style.getPropertyValue("--slide-x")).toBe("0px"));
  });

  it("has no mark at the normal level, and gets one when the level is switched to rich", async () => {
    layOut();
    await openTabs("req-list-users", "req-create-user");
    expect(mark()).toBeNull();

    motion.choose("rich");
    await waitFor(() => expect(mark()).not.toBeNull());
    expect(mark()!.style.getPropertyValue("--slide-x")).toBe("100px");

    motion.choose("off");
    await waitFor(() => expect(mark()).toBeNull());
  });

  it("slides the tabs after a closed one into its place; the closed tab is gone at once", async () => {
    layOut();
    motion.choose("rich");
    await openTabs("req-list-users", "req-create-user", "req-get-order");
    const [, second, third] = drawn();

    tabs.close(tabs.list[0].id);
    await waitFor(() => expect(drawn()).toHaveLength(2));

    // Nothing of the closed tab is left: every title and every cross of the bar belongs to an open tab.
    expect(drawn()).toEqual([second, third]);
    expect(titles()).toHaveLength(2);
    expect(within(bar()).getAllByRole("button")).toHaveLength(2);
    expect(bar().querySelectorAll("[inert], [aria-hidden='true']:not(.slide-indicator):not(svg)")).toHaveLength(0);
    // The tabs that stay start where they stood.
    for (const tab of [second, third]) {
      expect(tab.classList.contains("sliding")).toBe(true);
      expect(tab.style.getPropertyValue("--slide-from")).toBe("100px");
    }

    await fireEvent.animationEnd(second);
    await fireEvent.animationEnd(third);
    expect(second.classList.contains("sliding")).toBe(false);
    expect(third.classList.contains("sliding")).toBe(false);
  });

  it("does not move the tabs before a closed one, nor any tab when one is opened", async () => {
    layOut();
    motion.choose("rich");
    const api = await openTabs("req-list-users", "req-create-user");

    await tabs.open(api, "req-get-order");
    await waitFor(() => expect(drawn()).toHaveLength(3));
    expect(bar().querySelector(".sliding")).toBeNull();

    tabs.close(tabs.list[2].id);
    await waitFor(() => expect(drawn()).toHaveLength(2));
    expect(bar().querySelector(".sliding")).toBeNull();
  });

  it("closes a tab at the normal level as before: nothing slides", async () => {
    layOut();
    await openTabs("req-list-users", "req-create-user");

    tabs.close(tabs.list[0].id);
    await waitFor(() => expect(drawn()).toHaveLength(1));

    expect(bar().querySelector(".sliding")).toBeNull();
    expect(drawn()[0].style.getPropertyValue("--slide-from")).toBe("");
  });
});
