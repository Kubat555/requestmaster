import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs } from "../lib/tabs.svelte";
import { createFakeApi, type FakeApi, type FakeSeed } from "../lib/testing/fakeApi";
import { pressEnter } from "../lib/testing/keys";
import { toasts } from "../lib/toasts.svelte";
import { GLOBALS_ID, type ImportResult, type VariableRow } from "../lib/types";
import { workspace } from "../lib/workspace.svelte";

const TEXT = strings.environment;

/** The variables of "Dev" as the core returns them: every key written out. */
const DEV_VARIABLES = [
  { key: "host", value: "dev.example.test", secret: false, enabled: true, postman_extra: {} },
  { key: "token", value: "s3cret", secret: true, enabled: true, postman_extra: { type: "secret", note: "kept" } },
];

/** Two environments, "Dev" active, and one global variable. */
function seed(more: FakeSeed = {}): FakeSeed {
  return {
    environments: [
      { id: "env-dev", name: "Dev" },
      { id: "env-prod", name: "Production" },
    ],
    activeEnvironment: "env-dev",
    variables: {
      "env-dev": [
        { key: "host", value: "dev.example.test" },
        { key: "token", value: "s3cret", secret: true, postman_extra: { type: "secret", note: "kept" } },
      ],
      globals: [{ key: "tenant", value: "acme" }],
    },
    ...more,
  };
}

/** Renders the window over a fake api and shows the section of the environments in the sidebar. */
async function openEnvironments(more: FakeSeed = {}) {
  const api = createFakeApi(seed(more));
  render(App, { context: new Map([["api", api]]) });
  await fireEvent.click(screen.getByRole("tab", { name: strings.sidebar.environments }));
  await screen.findByRole("button", { name: "Dev" });
  return api;
}

/** The list of the section. */
function list(): HTMLElement {
  return screen.getByRole("list", { name: TEXT.list });
}

/** What the rows of the list show, top to bottom. */
function rowNames(): string[] {
  return within(list())
    .getAllByRole("button")
    .map((button) => button.textContent!.trim());
}

/** The row of the list with this name. */
function row(name: string): HTMLElement {
  return within(list()).getByRole("button", { name });
}

/** Opens the environment in a tab and returns the table of its variables. */
async function openEditor(name: string): Promise<HTMLElement> {
  await fireEvent.click(row(name));
  return screen.findByRole("table", { name: TEXT.variables });
}

/** The table of the shown editor. */
function table(): HTMLElement {
  return screen.getByRole("table", { name: TEXT.variables });
}

/** The value fields of the shown editor; the last one belongs to the empty row. */
function values(): HTMLInputElement[] {
  return within(table()).getAllByLabelText<HTMLInputElement>(strings.table.value);
}

/** The name fields of the shown editor; the last one belongs to the empty row. */
function names(): HTMLInputElement[] {
  return within(table()).getAllByLabelText<HTMLInputElement>(TEXT.name);
}

async function type(input: HTMLInputElement, value: string) {
  await fireEvent.input(input, { target: { value } });
}

/** Opens the menu of a row with the right mouse button and chooses an item. */
async function choose(name: string, label: string) {
  await fireEvent.contextMenu(row(name));
  const menu = await screen.findByRole("menu");
  await fireEvent.click(within(menu).getByRole("menuitem", { name: label }));
}

/** The button of the switcher in the corner of the window. */
function switcher(): HTMLElement {
  return screen.getByRole("button", { name: new RegExp(`^${TEXT.label}:`) });
}

/** The tab of the tab bar with this name, if it is open. */
function tab(name: string): HTMLElement | null {
  return within(screen.getByRole("region", { name: strings.tabs.label })).queryByRole("tab", { name });
}

/** Only the timers: the promises of the fake api and the rendering go on as usual. */
function useFakeTimers() {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
}

// The tabs, the notifications and the workspace of the window live in modules; every test starts without them.
afterEach(() => {
  for (const open of [...tabs.list]) tabs.close(open.id);
  vi.useRealTimers();
  toasts.items = [];
  workspace.rows = [];
  workspace.environments = [];
  workspace.activeEnvironmentId = null;
  workspace.renamingEnvironmentId = null;
});

describe("sections of the sidebar", () => {
  it("shows the collections first and switches the sections with the arrows", async () => {
    render(App, { context: new Map([["api", createFakeApi(seed())]]) });
    const sections = screen.getByRole("tablist", { name: strings.sidebar.sections });
    const section = (name: string) => within(sections).getByRole("tab", { name });

    expect(within(sections).getAllByRole("tab").map((item) => item.textContent!.trim())).toEqual([
      strings.sidebar.collections,
      strings.sidebar.environments,
      strings.sidebar.history,
    ]);
    expect(section(strings.sidebar.collections).getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("tree")).toBeTruthy();

    section(strings.sidebar.collections).focus();
    await fireEvent.keyDown(section(strings.sidebar.collections), { key: "ArrowRight" });

    expect(section(strings.sidebar.environments).getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(section(strings.sidebar.environments));
    expect(screen.queryByRole("tree")).toBeNull();
    expect(await screen.findByRole("button", { name: TEXT.globals })).toBeTruthy();

    await fireEvent.keyDown(section(strings.sidebar.environments), { key: "ArrowRight" });

    expect(section(strings.sidebar.history).getAttribute("aria-selected")).toBe("true");
    expect(screen.queryByRole("list", { name: TEXT.list })).toBeNull();
    expect(screen.getByText(strings.sidebar.historyEmpty)).toBeTruthy();

    await fireEvent.keyDown(section(strings.sidebar.history), { key: "ArrowLeft" });
    await fireEvent.keyDown(section(strings.sidebar.environments), { key: "ArrowLeft" });

    expect(section(strings.sidebar.collections).getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("tree")).toBeTruthy();
  });
});

describe("list of environments", () => {
  it("shows the globals first, then the environments, and marks the active one", async () => {
    await openEnvironments();

    expect(rowNames()).toEqual([TEXT.globals, "Dev", "Production"]);
    expect(row("Dev").getAttribute("aria-current")).toBe("true");
    expect(row("Production").getAttribute("aria-current")).toBeNull();
    expect(row(TEXT.globals).getAttribute("aria-current")).toBeNull();
  });

  it("creates an environment and starts renaming its row", async () => {
    const api = await openEnvironments();
    const create = vi.spyOn(api, "createEnvironment");
    const save = vi.spyOn(api, "saveEnvironment");

    await fireEvent.click(screen.getByRole("button", { name: TEXT.add }));
    const input = await screen.findByRole<HTMLInputElement>("textbox", { name: strings.tree.renameLabel });

    expect(create).toHaveBeenCalledTimes(1);
    expect(create).toHaveBeenCalledWith(TEXT.newName);
    expect(input.value).toBe(TEXT.newName);
    const id = await create.mock.results[0].value;

    await type(input, "Stage");
    await fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(save).toHaveBeenCalledWith(id, "Stage", []));
    await waitFor(() => expect(rowNames()).toEqual([TEXT.globals, "Dev", "Production", "Stage"]));
    // The switcher offers the new environment too.
    await fireEvent.click(switcher());
    expect(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Stage" })).toBeTruthy();
  });

  it("keeps the default name when the renaming of a new environment is cancelled", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");

    await fireEvent.click(screen.getByRole("button", { name: TEXT.add }));
    const input = await screen.findByRole<HTMLInputElement>("textbox", { name: strings.tree.renameLabel });
    await fireEvent.keyDown(input, { key: "Escape" });

    // The core orders by the lower-cased name, code point by code point: Cyrillic goes after Latin.
    await waitFor(() => expect(rowNames()).toEqual([TEXT.globals, "Dev", "Production", TEXT.newName]));
    expect(save).not.toHaveBeenCalled();
  });

  it("shows a notification when an environment is not created", async () => {
    const api = await openEnvironments();
    vi.spyOn(api, "createEnvironment").mockRejectedValue("storage is locked");

    await fireEvent.click(screen.getByRole("button", { name: TEXT.add }));

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ kind: "error", text: "storage is locked" });
    expect(rowNames()).toEqual([TEXT.globals, "Dev", "Production"]);
  });

  it("renames an environment with its variables kept; the list, the switcher and the open tab follow", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");

    await choose("Dev", TEXT.rename);
    const input = await screen.findByRole<HTMLInputElement>("textbox", { name: strings.tree.renameLabel });
    expect(input.value).toBe("Dev");
    await type(input, "  Zeta  ");
    await fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(save).toHaveBeenCalledWith("env-dev", "Zeta", DEV_VARIABLES));
    await waitFor(() => expect(rowNames()).toEqual([TEXT.globals, "Production", "Zeta"]));
    expect(switcher().textContent).toContain("Zeta");
    expect(tab("Zeta")).not.toBeNull();
    expect(tab("Dev")).toBeNull();
    // A later edit is saved under the new name.
    await type(values()[0], "zeta.example.test");
    await fireEvent.focusOut(values()[0]);
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1].slice(0, 2)).toEqual(["env-dev", "Zeta"]);
  });

  it("starts renaming with F2 and does not rename the globals", async () => {
    await openEnvironments();

    await fireEvent.keyDown(row(TEXT.globals), { key: "F2" });
    expect(screen.queryByRole("textbox", { name: strings.tree.renameLabel })).toBeNull();

    await fireEvent.keyDown(row("Production"), { key: "F2" });
    const input = await screen.findByRole<HTMLInputElement>("textbox", { name: strings.tree.renameLabel });
    expect(input.value).toBe("Production");
  });

  it("makes an environment active and clears the choice from the menu of a row", async () => {
    const api = await openEnvironments();
    const setActive = vi.spyOn(api, "setActiveEnvironment");

    await fireEvent.contextMenu(row("Production"));
    let menu = await screen.findByRole("menu");
    expect(within(menu).queryByRole("menuitem", { name: TEXT.deactivate })).toBeNull();
    await fireEvent.click(within(menu).getByRole("menuitem", { name: TEXT.activate }));

    await waitFor(() => expect(setActive).toHaveBeenCalledWith("env-prod"));
    await waitFor(() => expect(row("Production").getAttribute("aria-current")).toBe("true"));
    expect(row("Dev").getAttribute("aria-current")).toBeNull();
    expect(switcher().textContent).toContain("Production");

    await fireEvent.contextMenu(row("Production"));
    menu = await screen.findByRole("menu");
    expect(within(menu).queryByRole("menuitem", { name: TEXT.activate })).toBeNull();
    await fireEvent.click(within(menu).getByRole("menuitem", { name: TEXT.deactivate }));

    await waitFor(() => expect(setActive).toHaveBeenLastCalledWith(null));
    await waitFor(() => expect(switcher().textContent).toContain(TEXT.none));
    expect(row("Production").getAttribute("aria-current")).toBeNull();
  });

  it("opens the menu of a row with Shift+F10", async () => {
    await openEnvironments();

    await fireEvent.keyDown(row("Production"), { key: "F10", shiftKey: true });

    const menu = await screen.findByRole("menu");
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent!.trim())).toEqual([
      TEXT.activate,
      TEXT.rename,
      strings.export.item,
      TEXT.delete,
    ]);
  });

  it("offers no deletion for the globals", async () => {
    const api = await openEnvironments();
    const remove = vi.spyOn(api, "deleteEnvironment");

    await fireEvent.contextMenu(row(TEXT.globals));
    await fireEvent.keyDown(row(TEXT.globals), { key: "F10", shiftKey: true });
    await fireEvent.keyDown(row(TEXT.globals), { key: "Delete" });

    expect(screen.queryByRole("menuitem", { name: TEXT.delete })).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(remove).not.toHaveBeenCalled();
  });

  it("asks before deleting; Cancel deletes nothing", async () => {
    const api = await openEnvironments();
    const remove = vi.spyOn(api, "deleteEnvironment");

    await fireEvent.keyDown(row("Production"), { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: TEXT.deleteTitle("Production") });
    await fireEvent.click(within(dialog).getByRole("button", { name: TEXT.cancel }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(remove).not.toHaveBeenCalled();
    expect(rowNames()).toEqual([TEXT.globals, "Dev", "Production"]);
  });

  it("opens the question with the focus on Cancel, so Enter right after Delete deletes nothing", async () => {
    const api = await openEnvironments();
    const remove = vi.spyOn(api, "deleteEnvironment");

    await fireEvent.keyDown(row("Production"), { key: "Delete" });
    const dialog = await screen.findByRole("dialog", { name: TEXT.deleteTitle("Production") });
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: TEXT.cancel }));

    await pressEnter();

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(remove).not.toHaveBeenCalled();
    expect(rowNames()).toEqual([TEXT.globals, "Dev", "Production"]);
  });

  it("deletes the active environment: the switcher shows no environment, the tab is closed, its pending save is dropped", async () => {
    const api = await openEnvironments();
    const remove = vi.spyOn(api, "deleteEnvironment");
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    expect(switcher().textContent).toContain("Dev");
    useFakeTimers();
    await type(values()[0], "edited.example.test");

    await choose("Dev", TEXT.delete);
    const dialog = await screen.findByRole("dialog", { name: TEXT.deleteTitle("Dev") });
    expect(remove).not.toHaveBeenCalled();
    await fireEvent.click(within(dialog).getByRole("button", { name: TEXT.delete }));

    await waitFor(() => expect(rowNames()).toEqual([TEXT.globals, "Production"]));
    expect(remove).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledWith("env-dev");
    expect(switcher().textContent).toContain(TEXT.none);
    expect(workspace.activeEnvironmentId).toBeNull();
    expect(await api.activeEnvironment()).toBeNull();
    expect(tabs.list).toHaveLength(0);
    expect(screen.queryByRole("table", { name: TEXT.variables })).toBeNull();

    await vi.advanceTimersByTimeAsync(1000);
    expect(save).not.toHaveBeenCalled();
    expect(toasts.items).toHaveLength(0);
  });

  it("keeps the environment and its tab when the deletion fails, and still saves the edit", async () => {
    const api = await openEnvironments();
    vi.spyOn(api, "deleteEnvironment").mockRejectedValue("storage is locked");
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Production");
    await type(names()[0], "region");

    await choose("Production", TEXT.delete);
    const dialog = await screen.findByRole("dialog");
    await fireEvent.click(within(dialog).getByRole("button", { name: TEXT.delete }));

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ kind: "error", text: "storage is locked" });
    expect(rowNames()).toEqual([TEXT.globals, "Dev", "Production"]);
    expect(tab("Production")).not.toBeNull();
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0][2]).toEqual([{ key: "region", value: "", secret: false, enabled: true, postman_extra: {} }]);
  });
});

describe("editor of an environment", () => {
  it("opens the environment in a tab with the table of its variables", async () => {
    const api = await openEnvironments();
    const get = vi.spyOn(api, "getEnvironment");

    await openEditor("Dev");

    expect(get).toHaveBeenCalledWith("env-dev");
    expect(tab("Dev")?.getAttribute("aria-selected")).toBe("true");
    expect(tabs.active).toMatchObject({ id: "env:env-dev", kind: "environment", nodeId: null, dirty: false });
    expect(names().map((input) => input.value)).toEqual(["host", "token", ""]);
    expect(values().map((input) => input.value)).toEqual(["dev.example.test", "s3cret", ""]);
    expect(within(table()).getAllByRole("checkbox", { name: strings.table.enabled })).toHaveLength(2);
    expect(
      within(table())
        .getAllByRole<HTMLInputElement>("checkbox", { name: TEXT.secret })
        .map((box) => box.checked),
    ).toEqual([false, true]);

    // A second click shows the same tab.
    await fireEvent.click(row("Dev"));
    expect(tabs.list).toHaveLength(1);
    expect(get).toHaveBeenCalledTimes(1);
  });

  it("opens the globals in a tab of their own", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");

    await openEditor(TEXT.globals);

    expect(tabs.active).toMatchObject({ id: "env:globals", kind: "environment" });
    expect(tab(TEXT.globals)).not.toBeNull();
    expect(names().map((input) => input.value)).toEqual(["tenant", ""]);

    await type(values()[0], "umbrella");
    await fireEvent.focusOut(values()[0]);

    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0][0]).toBe("globals");
    expect(save.mock.calls[0][2]).toEqual([{ key: "tenant", value: "umbrella", secret: false, enabled: true, postman_extra: {} }]);
  });

  it("shows a notification and opens no tab when the environment cannot be read", async () => {
    const api = await openEnvironments();
    vi.spyOn(api, "getEnvironment").mockRejectedValue("environment not found");

    await fireEvent.click(row("Dev"));

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ kind: "error", text: "environment not found" });
    expect(tabs.list).toHaveLength(0);
  });

  it("saves an edited value 400 ms after the last key with the whole set of variables", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();

    await type(values()[0], "changed.example.test");
    await vi.advanceTimersByTimeAsync(399);
    expect(save).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith("env-dev", "Dev", [{ ...DEV_VARIABLES[0], value: "changed.example.test" }, DEV_VARIABLES[1]]);
    expect((await api.getEnvironment("env-dev")).variables[0].value).toBe("changed.example.test");

    // Saved by itself: the tab never shows the mark of unsaved edits.
    expect(within(tab("Dev")!).queryByRole("img")).toBeNull();
    expect(screen.queryByText(TEXT.unsaved)).toBeNull();
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("saves a quick series of edits with one call", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();

    await type(values()[0], "a");
    await vi.advanceTimersByTimeAsync(300);
    await type(values()[0], "ab");
    await vi.advanceTimersByTimeAsync(300);
    await type(names()[0], "address");
    await vi.advanceTimersByTimeAsync(399);
    expect(save).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][2][0]).toMatchObject({ key: "address", value: "ab" });
  });

  it("saves at once when the row loses the focus", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();

    await type(values()[0], "blurred.example.test");
    // Inside the row: the focus only moves to another field of it.
    await fireEvent.focusOut(values()[0], { relatedTarget: names()[0] });
    await vi.advanceTimersByTimeAsync(0);
    expect(save).not.toHaveBeenCalled();

    await fireEvent.focusOut(names()[0], { relatedTarget: names()[1] });
    await vi.advanceTimersByTimeAsync(0);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][2][0]).toMatchObject({ key: "host", value: "blurred.example.test" });

    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("does not save when the focus leaves a row that was not edited", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");

    await fireEvent.focusOut(values()[0]);
    await fireEvent.click(tab("Dev")!);

    expect(save).not.toHaveBeenCalled();
  });

  it("masks the value of a secret variable until the user asks to show it", async () => {
    await openEnvironments();
    await openEditor("Dev");

    expect(values().map((input) => input.type)).toEqual(["text", "password", "text"]);

    await fireEvent.click(within(table()).getByRole("button", { name: strings.table.show }));
    expect(values()[1].type).toBe("text");
    expect(values()[1].value).toBe("s3cret");

    await fireEvent.click(within(table()).getByRole("button", { name: strings.table.hide }));
    expect(values()[1].type).toBe("password");
  });

  it("makes a variable secret with its checkbox and saves it", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();

    await fireEvent.click(within(table()).getAllByRole("checkbox", { name: TEXT.secret })[0]);
    expect(values()[0].type).toBe("password");
    await fireEvent.click(within(table()).getAllByRole("checkbox", { name: strings.table.enabled })[1]);
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][2]).toEqual([
      { ...DEV_VARIABLES[0], secret: true },
      { ...DEV_VARIABLES[1], enabled: false },
    ]);
  });

  it("returns the postman_extra of a variable unchanged", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();

    await type(values()[1], "rotated");
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][2][1]).toEqual({
      key: "token",
      value: "rotated",
      secret: true,
      enabled: true,
      postman_extra: { type: "secret", note: "kept" },
    });
  });

  it("adds a variable from the empty row at the end and removes a row", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();

    await type(names()[2], "r");
    expect(names().map((input) => input.value)).toEqual(["host", "token", "r", ""]);
    expect(document.activeElement).toBe(names()[2]);
    await type(names()[2], "region");
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][2]).toEqual([
      ...DEV_VARIABLES,
      { key: "region", value: "", secret: false, enabled: true, postman_extra: {} },
    ]);

    await fireEvent.click(within(table()).getAllByRole("button", { name: strings.table.remove })[0]);
    expect(names().map((input) => input.value)).toEqual(["token", "region", ""]);
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(2);
    expect(save.mock.calls[1][2].map((variable) => variable.key)).toEqual(["token", "region"]);
  });

  it("marks the editor and notifies when a save fails, and tries again on the next edit", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    save.mockRejectedValueOnce("storage is locked");
    await openEditor("Dev");
    useFakeTimers();

    await type(values()[0], "first.example.test");
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(1);
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({ kind: "error", text: "storage is locked" });
    expect(screen.getByText(TEXT.unsaved)).toBeTruthy();
    // The mark is in the editor, not on the tab.
    expect(within(tab("Dev")!).queryByRole("img")).toBeNull();
    expect((await api.getEnvironment("env-dev")).variables[0].value).toBe("dev.example.test");

    await type(names()[0], "address");
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(2);
    expect(save.mock.calls[1][2][0]).toMatchObject({ key: "address", value: "first.example.test" });
    expect(screen.queryByText(TEXT.unsaved)).toBeNull();
    expect((await api.getEnvironment("env-dev")).variables[0]).toMatchObject({ key: "address", value: "first.example.test" });
  });
});

describe("closing with a pending save", () => {
  it("saves at once when the tab is closed, then closes it", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();
    await type(values()[0], "closing.example.test");

    await fireEvent.click(screen.getByRole("button", { name: `${strings.tabs.close}: Dev` }));

    await waitFor(() => expect(tabs.list).toHaveLength(0));
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][2][0]).toMatchObject({ value: "closing.example.test" });
    // No question about unsaved edits was asked.
    expect(screen.queryByRole("dialog")).toBeNull();
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("keeps the tab when the save before closing fails", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment").mockRejectedValue("storage is locked");
    await openEditor("Dev");
    useFakeTimers();
    await type(values()[0], "closing.example.test");

    await fireEvent.click(screen.getByRole("button", { name: `${strings.tabs.close}: Dev` }));

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(save).toHaveBeenCalledTimes(1);
    expect(tab("Dev")).not.toBeNull();
    expect(values()[0].value).toBe("closing.example.test");
    expect(screen.getByText(TEXT.unsaved)).toBeTruthy();
  });

  it("closes a tab without edits without a call", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");

    await fireEvent.keyDown(window, { key: "w", code: "KeyW", ctrlKey: true });

    await waitFor(() => expect(tabs.list).toHaveLength(0));
    expect(save).not.toHaveBeenCalled();
  });

  it("saves once and lets the window close when it is closed with a pending save", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    useFakeTimers();
    await type(values()[0], "quitting.example.test");

    expect(await api.requestClose()).toBe(true);

    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][2][0]).toMatchObject({ value: "quitting.example.test" });
    expect(screen.queryByRole("dialog")).toBeNull();
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("keeps the window open and notifies when the save before closing the window fails", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment").mockRejectedValue("storage is locked");
    await openEditor("Dev");
    useFakeTimers();
    await type(values()[0], "quitting.example.test");

    expect(await api.requestClose()).toBe(false);

    expect(save).toHaveBeenCalledTimes(1);
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({ kind: "error", text: "storage is locked" });
    expect(tab("Dev")).not.toBeNull();
  });

  it("asks about the drafts of requests only after the environments are saved", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    await openEditor("Dev");
    await type(values()[0], "quitting.example.test");
    await tabs.open(api, "req-list-users");
    const url = await waitFor(() => {
      const input = document.querySelector<HTMLInputElement>("[data-tab-panel]:not([hidden]) [data-url-input]");
      expect(input).not.toBeNull();
      return input!;
    });
    await type(url, "https://api.example.test/changed");

    const closed = api.requestClose();
    const dialog = await screen.findByRole("dialog", { name: strings.app.closeTitle });

    expect(save).toHaveBeenCalledTimes(1);
    // Only the request counts as a tab with unsaved edits.
    expect(within(dialog).getByText(strings.app.closeText(1))).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.app.cancel }));
    expect(await closed).toBe(false);
  });

  it("Ctrl+S on the tab of an environment saves its edit at once and never calls saveNodeContent", async () => {
    const api = await openEnvironments();
    const save = vi.spyOn(api, "saveEnvironment");
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await openEditor("Dev");
    useFakeTimers();

    await fireEvent.keyDown(window, { key: "s", code: "KeyS", ctrlKey: true });
    await vi.advanceTimersByTimeAsync(0);
    expect(save).not.toHaveBeenCalled();

    await type(values()[0], "hotkey.example.test");
    await fireEvent.keyDown(window, { key: "s", code: "KeyS", ctrlKey: true });
    await vi.advanceTimersByTimeAsync(0);

    expect(save).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });
});

describe("import of global variables with their tab open", () => {
  const FILE_NAME = "workspace.postman_globals.json";
  const IMPORTED_VARIABLE: VariableRow = { key: "region", value: "eu", secret: false, enabled: true, postman_extra: {} };

  function globalsResult(conflicts: string[] = []): ImportResult {
    return { file_name: FILE_NAME, kind: "globals", name: null, root_id: null, report: [], conflicts, error: null };
  }

  /** Adds {@link IMPORTED_VARIABLE} to the stored globals, as the core does when it applies a file. */
  function storeImported(api: FakeApi): () => Promise<void> {
    const getEnvironment = api.getEnvironment.bind(api);
    const saveEnvironment = api.saveEnvironment.bind(api);
    return async () => {
      const { name, variables } = await getEnvironment(GLOBALS_ID);
      await saveEnvironment(GLOBALS_ID, name, [...variables, IMPORTED_VARIABLE]);
    };
  }

  /** Drops a file on the sidebar and waits for the dialog the import ends with. */
  async function dropFile() {
    const drop = createEvent.drop(list(), {
      dataTransfer: { types: ["Files"], files: [new File(["{}"], FILE_NAME, { type: "application/json" })] },
    });
    await fireEvent(list(), drop);
    return screen.findByRole("dialog");
  }

  /** The variables of the last call of the spy on `saveEnvironment`. */
  function lastSaved(save: { mock: { calls: unknown[][] } }): VariableRow[] {
    return save.mock.calls.at(-1)![2] as VariableRow[];
  }

  it("shows the imported variables and keeps them in the next save", async () => {
    const api = await openEnvironments();
    await openEditor(TEXT.globals);
    const apply = storeImported(api);
    vi.spyOn(api, "importFiles").mockImplementation(async () => {
      await apply();
      return [globalsResult()];
    });
    const save = vi.spyOn(api, "saveEnvironment");

    await dropFile();
    await waitFor(() => expect(names().map((input) => input.value)).toEqual(["tenant", "region", ""]));
    expect(save).not.toHaveBeenCalled();

    useFakeTimers();
    await type(values()[0], "globex");
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(1);
    expect(lastSaved(save).map(({ key, value }) => [key, value])).toEqual([
      ["tenant", "globex"],
      ["region", "eu"],
    ]);
  });

  it("saves the pending edit before the import, so both the edit and the imported variables stay", async () => {
    const api = await openEnvironments();
    await openEditor(TEXT.globals);
    const apply = storeImported(api);
    const save = vi.spyOn(api, "saveEnvironment");
    vi.spyOn(api, "importFiles").mockImplementation(async () => {
      // The edit is in the storage by the time the core merges the file.
      expect(save).toHaveBeenCalledTimes(1);
      await apply();
      return [globalsResult()];
    });

    await type(values()[0], "globex");
    await dropFile();

    await waitFor(() => expect(names().map((input) => input.value)).toEqual(["tenant", "region", ""]));
    expect(values()[0].value).toBe("globex");
    expect((await api.getEnvironment(GLOBALS_ID)).variables.map(({ key, value }) => [key, value])).toEqual([
      ["tenant", "globex"],
      ["region", "eu"],
    ]);
  });

  it("reads the globals again after the conflicts are resolved with a policy", async () => {
    const api = await openEnvironments();
    await openEditor(TEXT.globals);
    const apply = storeImported(api);
    vi.spyOn(api, "importFiles").mockResolvedValue([globalsResult(["tenant"])]);
    const importGlobals = vi.spyOn(api, "importGlobals").mockImplementation(apply);
    const save = vi.spyOn(api, "saveEnvironment");

    const question = await dropFile();
    await fireEvent.click(within(question).getByRole("button", { name: strings.import.keepCurrent }));
    await waitFor(() => expect(names().map((input) => input.value)).toEqual(["tenant", "region", ""]));
    expect(importGlobals).toHaveBeenCalledWith("{}", "keep_current");

    useFakeTimers();
    await type(values()[0], "globex");
    await vi.advanceTimersByTimeAsync(400);

    expect(save).toHaveBeenCalledTimes(1);
    expect(lastSaved(save).map(({ key }) => key)).toEqual(["tenant", "region"]);
  });

  it("does not import when the pending edit of the globals cannot be saved", async () => {
    const api = await openEnvironments();
    await openEditor(TEXT.globals);
    const importFiles = vi.spyOn(api, "importFiles");
    vi.spyOn(api, "saveEnvironment").mockRejectedValue("disk is full");

    await type(values()[0], "globex");
    const drop = createEvent.drop(list(), { dataTransfer: { types: ["Files"], files: [new File(["{}"], FILE_NAME)] } });
    await fireEvent(list(), drop);

    await waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(importFiles).not.toHaveBeenCalled();
    expect(values()[0].value).toBe("globex");
  });
});
