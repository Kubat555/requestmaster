import { fireEvent, render, screen } from "@testing-library/svelte";
import { createRawSnippet, type ComponentProps } from "svelte";
import { describe, expect, it, vi } from "vitest";
import Button from "./Button.svelte";
import EmptyState from "./EmptyState.svelte";
import Icon from "./Icon.svelte";
import IconButton from "./IconButton.svelte";
import Menu from "./Menu.svelte";
import Segmented from "./Segmented.svelte";

/** Content of a button: plain text. */
function text(value: string) {
  return createRawSnippet(() => ({ render: () => `<span>${value}</span>` }));
}

describe("Button", () => {
  it("gives every variant a class of its own", () => {
    render(Button, { props: { children: text("Default") } });
    render(Button, { props: { variant: "primary", children: text("Primary") } });
    render(Button, { props: { variant: "ghost", children: text("Ghost") } });

    const classes = ["Default", "Primary", "Ghost"].map((name) => screen.getByRole("button", { name }).className);

    expect(new Set(classes).size).toBe(3);
    expect(screen.getByRole("button", { name: "Default" }).classList.contains("default")).toBe(true);
    expect(screen.getByRole("button", { name: "Primary" }).classList.contains("primary")).toBe(true);
    expect(screen.getByRole("button", { name: "Ghost" }).classList.contains("ghost")).toBe(true);
  });

  it("is a plain button that calls its handler", async () => {
    const onclick = vi.fn();
    render(Button, { props: { onclick, children: text("Save") } });
    const button = screen.getByRole<HTMLButtonElement>("button", { name: "Save" });

    await fireEvent.click(button);

    expect(button.type).toBe("button");
    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it("does not call the handler while disabled", async () => {
    const onclick = vi.fn();
    render(Button, { props: { disabled: true, onclick, children: text("Save") } });
    const button = screen.getByRole<HTMLButtonElement>("button", { name: "Save" });

    // What a user's click does: a disabled button gets no click at all. `fireEvent` would hand the event
    // to the element itself, past the rule of the browser.
    button.click();

    expect(button.disabled).toBe(true);
    expect(onclick).not.toHaveBeenCalled();
  });

  it("draws a decorative icon before the text", () => {
    render(Button, { props: { icon: "send", children: text("Send") } });

    const icon = screen.getByRole("button", { name: "Send" }).querySelector("svg");

    expect(icon?.getAttribute("aria-hidden")).toBe("true");
  });
});

describe("IconButton", () => {
  it("takes its accessible name from the label", async () => {
    const onclick = vi.fn();
    render(IconButton, { props: { icon: "close", label: "Close the tab", onclick } });
    const button = screen.getByRole("button", { name: "Close the tab" });

    await fireEvent.click(button);

    expect(button.getAttribute("aria-label")).toBe("Close the tab");
    expect(button.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it("cannot be written without a label", () => {
    // Checked by `npm run check`: the line below must be a type error.
    // @ts-expect-error `label` is required
    const props: ComponentProps<typeof IconButton> = { icon: "close" };
    expect(props.icon).toBe("close");
  });
});

describe("Icon", () => {
  it("is hidden from assistive technology", () => {
    const decorative = render(Icon, { props: { name: "search" } });
    const svg = decorative.container.querySelector("svg")!;
    expect(svg.getAttribute("aria-hidden")).toBe("true");
    expect(svg.getAttribute("width")).toBe("16");
    expect(svg.querySelectorAll("path").length).toBeGreaterThan(0);
  });

  it("knows only the names of the set", () => {
    // Checked by `npm run check`: the line below must be a type error.
    // @ts-expect-error not a name of an icon
    const props: ComponentProps<typeof Icon> = { name: "rocket" };
    expect(props).toBeTruthy();
  });
});

describe("Segmented", () => {
  const options = [
    { value: "collections", label: "Collections" },
    { value: "environments", label: "Environments", disabled: true },
    { value: "history", label: "History" },
  ];

  it("shows the options as tabs with the chosen one selected", () => {
    render(Segmented, { props: { options, value: "collections", label: "Sections" } });

    expect(screen.getByRole("tablist", { name: "Sections" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Collections" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "History" }).getAttribute("aria-selected")).toBe("false");
    expect(screen.getByRole<HTMLButtonElement>("tab", { name: "Environments" }).disabled).toBe(true);
    // Tab stops at the chosen option only.
    expect(screen.getByRole("tab", { name: "Collections" }).tabIndex).toBe(0);
    expect(screen.getByRole("tab", { name: "History" }).tabIndex).toBe(-1);
  });

  it("moves the choice with the right arrow, skipping a disabled option, and reports it", async () => {
    const onchange = vi.fn();
    render(Segmented, { props: { options, value: "collections", label: "Sections", onchange } });

    await fireEvent.keyDown(screen.getByRole("tab", { name: "Collections" }), { key: "ArrowRight" });

    const history = screen.getByRole("tab", { name: "History" });
    expect(onchange).toHaveBeenCalledTimes(1);
    expect(onchange).toHaveBeenCalledWith("history");
    expect(history.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "Collections" }).getAttribute("aria-selected")).toBe("false");
    expect(document.activeElement).toBe(history);
  });

  it("moves the choice back with the left arrow and stops at the ends", async () => {
    const onchange = vi.fn();
    render(Segmented, { props: { options, value: "history", label: "Sections", onchange } });

    await fireEvent.keyDown(screen.getByRole("tab", { name: "History" }), { key: "ArrowRight" });
    expect(onchange).not.toHaveBeenCalled();

    await fireEvent.keyDown(screen.getByRole("tab", { name: "History" }), { key: "ArrowLeft" });
    expect(onchange).toHaveBeenCalledWith("collections");
    expect(screen.getByRole("tab", { name: "Collections" }).getAttribute("aria-selected")).toBe("true");
  });

  it("chooses an option with a click", async () => {
    const onchange = vi.fn();
    render(Segmented, { props: { options, value: "collections", label: "Sections", onchange } });

    await fireEvent.click(screen.getByRole("tab", { name: "History" }));

    expect(onchange).toHaveBeenCalledWith("history");
    expect(screen.getByRole("tab", { name: "History" }).getAttribute("aria-selected")).toBe("true");
  });
});

describe("Menu", () => {
  /** Opens a menu of three actions at a point; closing it removes it, as its owner does. */
  function openMenu() {
    const rename = vi.fn();
    const duplicate = vi.fn();
    const remove = vi.fn();
    const onclose = vi.fn(() => view.unmount());
    const view = render(Menu, {
      props: {
        label: "Actions",
        anchor: { x: 10, y: 20 },
        items: [
          { label: "Rename", onselect: rename },
          { label: "Duplicate", onselect: duplicate },
          { label: "Delete", danger: true, onselect: remove },
        ],
        onclose,
      },
    });
    return { rename, duplicate, remove, onclose };
  }

  function item(name: string): HTMLElement {
    return screen.getByRole("menuitem", { name });
  }

  it("opens with its items and the focus on the first one", () => {
    openMenu();

    expect(screen.getByRole("menu", { name: "Actions" })).toBeTruthy();
    expect(screen.getAllByRole("menuitem").map((element) => element.textContent!.trim())).toEqual(["Rename", "Duplicate", "Delete"]);
    expect(document.activeElement).toBe(item("Rename"));
  });

  it("announces items with `checked` as radio items, marks the current one and walks them like the others", async () => {
    const left = vi.fn();
    const onclose = vi.fn();
    render(Menu, {
      props: {
        label: "Side",
        anchor: { x: 10, y: 20 },
        items: [
          { label: "Left", checked: false, onselect: left },
          { label: "Right", checked: true, onselect: vi.fn() },
        ],
        onclose,
      },
    });
    const radios = screen.getAllByRole("menuitemradio");

    expect(screen.queryAllByRole("menuitem")).toHaveLength(0);
    expect(radios.map((radio) => radio.getAttribute("aria-checked"))).toEqual(["false", "true"]);
    expect(radios.map((radio) => radio.querySelectorAll("svg").length)).toEqual([0, 1]);
    expect(document.activeElement).toBe(radios[0]);

    await fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(document.activeElement).toBe(radios[1]);
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
    await fireEvent.keyDown(document.activeElement!, { key: "Enter" });
    expect(left).toHaveBeenCalledTimes(1);
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  it("leaves plain items without `aria-checked`", () => {
    openMenu();
    expect(screen.getAllByRole("menuitem").every((element) => !element.hasAttribute("aria-checked"))).toBe(true);
  });

  it("moves the focus with the arrows, round the ends", async () => {
    openMenu();

    await fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(document.activeElement).toBe(item("Duplicate"));
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(document.activeElement).toBe(item("Rename"));
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
    expect(document.activeElement).toBe(item("Delete"));
  });

  it("runs the action of the focused item on Enter and closes", async () => {
    const { rename, duplicate, onclose } = openMenu();
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });

    await fireEvent.keyDown(document.activeElement!, { key: "Enter" });

    expect(duplicate).toHaveBeenCalledTimes(1);
    expect(rename).not.toHaveBeenCalled();
    expect(onclose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("runs the action of a clicked item and closes", async () => {
    const { remove, onclose } = openMenu();

    await fireEvent.click(item("Delete"));

    expect(remove).toHaveBeenCalledTimes(1);
    expect(onclose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("closes on Escape without an action", async () => {
    const { rename, duplicate, remove, onclose } = openMenu();

    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });

    expect(onclose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
    for (const action of [rename, duplicate, remove]) expect(action).not.toHaveBeenCalled();
  });

  it("closes on a press outside but not on one inside", async () => {
    const { onclose } = openMenu();

    await fireEvent.mouseDown(screen.getByRole("menu"));
    expect(onclose).not.toHaveBeenCalled();

    await fireEvent.mouseDown(document.body);
    expect(onclose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("marks a dangerous item with a class of its own", () => {
    openMenu();

    expect(item("Delete").classList.contains("danger")).toBe(true);
    expect(item("Rename").classList.contains("danger")).toBe(false);
  });

  it("returns the focus to where it was when it closes", async () => {
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();
    try {
      openMenu();
      expect(document.activeElement).not.toBe(opener);

      await fireEvent.keyDown(document.activeElement!, { key: "Escape" });

      expect(document.activeElement).toBe(opener);
    } finally {
      opener.remove();
    }
  });
});

describe("EmptyState", () => {
  it("shows the title and the explanation", () => {
    render(EmptyState, { props: { icon: "collection", title: "Nothing here", text: "Import a collection" } });

    expect(screen.getByText("Nothing here")).toBeTruthy();
    expect(screen.getByText("Import a collection")).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
