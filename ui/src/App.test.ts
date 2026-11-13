import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import App from "./App.svelte";
import { createFakeApi } from "./lib/testing/fakeApi";

function renderApp() {
  return render(App, { context: new Map([["api", createFakeApi()]]) });
}

/** Width of the sidebar in pixels as the layout applies it. */
function sidebarWidth(): number {
  return parseInt(screen.getByRole("navigation").style.width, 10);
}

describe("App", () => {
  it("renders the sidebar and the main area", () => {
    renderApp();
    expect(screen.getByRole("navigation")).toBeTruthy();
    expect(screen.getByRole("main")).toBeTruthy();
  });

  it("resizes the sidebar by 16 px with the arrow keys on the separator", async () => {
    renderApp();
    const separator = screen.getByRole("separator");
    expect(sidebarWidth()).toBe(280);

    await fireEvent.keyDown(separator, { key: "ArrowRight" });
    expect(sidebarWidth()).toBe(296);
    expect(separator.getAttribute("aria-valuenow")).toBe("296");

    await fireEvent.keyDown(separator, { key: "ArrowLeft" });
    await fireEvent.keyDown(separator, { key: "ArrowLeft" });
    expect(sidebarWidth()).toBe(264);
  });

  it("keeps the sidebar width between 200 and 480 px", async () => {
    renderApp();
    const separator = screen.getByRole("separator");

    for (let i = 0; i < 20; i++) await fireEvent.keyDown(separator, { key: "ArrowRight" });
    expect(sidebarWidth()).toBe(480);

    for (let i = 0; i < 30; i++) await fireEvent.keyDown(separator, { key: "ArrowLeft" });
    expect(sidebarWidth()).toBe(200);
  });
});
