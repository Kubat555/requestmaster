import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import EnvironmentSwitcher from "./EnvironmentSwitcher.svelte";
import { strings } from "../lib/strings";
import { createFakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import { Workspace } from "../lib/workspace.svelte";

const ENVIRONMENTS = [
  { id: "env-dev", name: "Dev" },
  { id: "env-prod", name: "Production" },
];

/** Loads the workspace from a fake api, as the window does at start, and renders the switcher over it. */
async function renderSwitcher(activeEnvironment: string | null = null) {
  const api = createFakeApi({ environments: ENVIRONMENTS, activeEnvironment });
  const workspace = new Workspace();
  await workspace.load(api);
  render(EnvironmentSwitcher, { props: { workspace }, context: new Map([["api", api]]) });
  const setActiveEnvironment = vi.spyOn(api, "setActiveEnvironment");
  return { api, workspace, setActiveEnvironment };
}

/** The button of the switcher: its name starts with the word "environment". */
function button(): HTMLElement {
  return screen.getByRole("button", { name: new RegExp(`^${strings.environment.label}`) });
}

async function openMenu(): Promise<HTMLElement> {
  await fireEvent.click(button());
  return screen.getByRole("menu", { name: strings.environment.label });
}

// The notifications of the window live in a module; every test starts without them.
afterEach(() => {
  toasts.items = [];
});

describe("EnvironmentSwitcher", () => {
  it("shows that no environment is chosen", async () => {
    await renderSwitcher();

    expect(button().textContent).toContain(strings.environment.none);
    expect(button().getAttribute("aria-expanded")).toBe("false");
    expect(button().querySelector(".dot.on")).toBeNull();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("shows the name of the active environment", async () => {
    await renderSwitcher("env-prod");

    expect(button().textContent).toContain("Production");
    expect(button().textContent).not.toContain(strings.environment.none);
    expect(button().querySelector(".dot.on")).not.toBeNull();
  });

  it("opens a menu with no environment and every environment on a click", async () => {
    await renderSwitcher();

    await openMenu();

    expect(screen.getAllByRole("menuitem").map((item) => item.textContent!.trim())).toEqual([
      strings.environment.none,
      "Dev",
      "Production",
    ]);
    expect(button().getAttribute("aria-expanded")).toBe("true");
  });

  it("makes the chosen environment active", async () => {
    const { workspace, setActiveEnvironment } = await renderSwitcher();
    await openMenu();

    await fireEvent.click(screen.getByRole("menuitem", { name: "Dev" }));

    expect(setActiveEnvironment).toHaveBeenCalledTimes(1);
    expect(setActiveEnvironment).toHaveBeenCalledWith("env-dev");
    expect(workspace.activeEnvironmentId).toBe("env-dev");
    await waitFor(() => expect(button().textContent).toContain("Dev"));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("clears the choice with the item of no environment", async () => {
    const { workspace, setActiveEnvironment } = await renderSwitcher("env-dev");
    await openMenu();

    await fireEvent.click(screen.getByRole("menuitem", { name: strings.environment.none }));

    expect(setActiveEnvironment).toHaveBeenCalledTimes(1);
    expect(setActiveEnvironment).toHaveBeenCalledWith(null);
    expect(workspace.activeEnvironmentId).toBeNull();
    await waitFor(() => expect(button().textContent).toContain(strings.environment.none));
  });

  it("brings the previous name back and notifies when the choice is not saved", async () => {
    const { workspace, setActiveEnvironment } = await renderSwitcher("env-dev");
    setActiveEnvironment.mockRejectedValue("storage is locked");
    await openMenu();

    await fireEvent.click(screen.getByRole("menuitem", { name: "Production" }));

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ kind: "error", text: "storage is locked" });
    expect(setActiveEnvironment).toHaveBeenCalledWith("env-prod");
    expect(workspace.activeEnvironmentId).toBe("env-dev");
    await waitFor(() => expect(button().textContent).toContain("Dev"));
    expect(button().textContent).not.toContain("Production");
  });

  it("closes the menu on Escape without a call", async () => {
    const { workspace, setActiveEnvironment } = await renderSwitcher("env-dev");
    const menu = await openMenu();
    expect(menu.contains(document.activeElement)).toBe(true);

    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });

    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(setActiveEnvironment).not.toHaveBeenCalled();
    expect(workspace.activeEnvironmentId).toBe("env-dev");
    expect(button().getAttribute("aria-expanded")).toBe("false");
  });
});
