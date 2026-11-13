import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { createFakeApi, type FakeApi, type FakeSeed } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import { GLOBALS_ID, type CollectionDraft, type NodeDto, type RequestDraft } from "../lib/types";
import { Workspace } from "../lib/workspace.svelte";

const TEXT = strings.export;
const PATH = "C:\\Exports\\Demo API.postman_collection.json";

/** The default tree and one more collection with a request of its own. */
function nodes(): NodeDto[] {
  return [
    { id: "col-1", parent_id: null, kind: "collection", name: "Demo API", content: {} },
    { id: "fold-users", parent_id: "col-1", kind: "folder", name: "Users", content: {} },
    { id: "req-list-users", parent_id: "fold-users", kind: "request", name: "List users", content: { method: "GET", url: "https://api.example.test/users" } },
    { id: "req-create-user", parent_id: "fold-users", kind: "request", name: "Create user", content: { method: "POST", url: "https://api.example.test/users" } },
    { id: "req-ping", parent_id: "col-1", kind: "request", name: "Ping", content: { method: "GET", url: "https://api.example.test/ping" } },
    { id: "col-2", parent_id: null, kind: "collection", name: "Other API", content: {} },
    { id: "req-other", parent_id: "col-2", kind: "request", name: "Other", content: { method: "GET", url: "https://other.example.test" } },
  ];
}

/** Two environments — "Dev" has a secret variable — and one global variable. */
function seed(more: FakeSeed = {}): FakeSeed {
  return {
    nodes: nodes(),
    environments: [
      { id: "env-dev", name: "Dev" },
      { id: "env-prod", name: "Production" },
    ],
    variables: {
      "env-dev": [
        { key: "host", value: "dev.example.test" },
        { key: "token", value: "s3cret", secret: true },
      ],
      "env-prod": [{ key: "host", value: "example.test" }],
      globals: [{ key: "tenant", value: "acme" }],
    },
    ...more,
  };
}

/** Loads the workspace from a fake api and renders the sidebar over it; a node opened in the tree gets a tab. */
async function renderSidebar(more: FakeSeed = {}) {
  const api = createFakeApi(seed(more));
  const workspace = new Workspace();
  workspace.onOpen = (id) => void tabs.open(api, id);
  await workspace.load(api);
  workspace.expand(["col-1", "fold-users", "col-2"]);
  render(Sidebar, { props: { workspace }, context: new Map([["api", api]]) });
  return { api, workspace };
}

/** Opens the menu of a row with the right mouse button. */
async function openMenu(row: HTMLElement): Promise<HTMLElement> {
  await fireEvent.contextMenu(row);
  return screen.findByRole("menu");
}

/** Chooses "Export…" in the menu of a row. */
async function chooseExport(row: HTMLElement) {
  const menu = await openMenu(row);
  await fireEvent.click(within(menu).getByRole("menuitem", { name: TEXT.item }));
}

function collectionRow(name = "Demo API"): HTMLElement {
  return screen.getByRole("treeitem", { name });
}

/** Shows the section of the environments and returns the row with this name. */
async function environmentRow(name: string): Promise<HTMLElement> {
  await fireEvent.click(screen.getByRole("tab", { name: strings.sidebar.environments }));
  return within(await screen.findByRole("list", { name: strings.environment.list })).getByRole("button", { name });
}

/** Opens the requests in tabs and changes their drafts, so that the tabs have unsaved edits. */
async function editRequests(api: FakeApi, ...ids: string[]) {
  for (const id of ids) {
    await tabs.open(api, id);
    (tabs.list.find((tab) => tab.nodeId === id) as Tab<RequestDraft>).draft.url += "?edited";
  }
}

function button(name: string): HTMLElement {
  return within(screen.getByRole("dialog")).getByRole("button", { name });
}

function toastList(): { kind: string; text: string }[] {
  return toasts.items.map(({ kind, text }) => ({ kind, text }));
}

/** Lets the commands that were started end and their results be handled. */
async function settle() {
  for (let turn = 0; turn < 10; turn++) await Promise.resolve();
}

// The tabs and the notifications of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("export of a collection", () => {
  // One row per test: three menus in one test, the first render of the file among them, came close to the time limit.
  it.each([
    ["Demo API", true],
    ["Users", false],
    ["GET Ping", false],
  ])("is offered for a collection only: the menu of \"%s\" has it — %s", async (name, offered) => {
    await renderSidebar();

    const menu = await openMenu(screen.getByRole("treeitem", { name }));

    expect(within(menu).queryByRole("menuitem", { name: TEXT.item }) !== null).toBe(offered);
  });

  it("exports a collection without unsaved edits with one call and tells the path", async () => {
    const { api } = await renderSidebar();
    const exportCollection = vi.spyOn(api, "exportCollection").mockResolvedValue(PATH);
    const exportEnvironment = vi.spyOn(api, "exportEnvironment");
    // A clean tab of the collection and unsaved edits of another collection do not matter.
    await tabs.open(api, "req-ping");
    await editRequests(api, "req-other");

    await chooseExport(collectionRow());

    await waitFor(() => expect(toastList()).toEqual([{ kind: "info", text: TEXT.saved(PATH) }]));
    expect(TEXT.saved(PATH)).toBe(`Сохранено: ${PATH}`);
    expect(exportCollection).toHaveBeenCalledTimes(1);
    expect(exportCollection).toHaveBeenCalledWith("col-1");
    expect(exportEnvironment).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows nothing when the user closes the window that asks where to save", async () => {
    const { api } = await renderSidebar();
    const exportCollection = vi.spyOn(api, "exportCollection").mockResolvedValue(null);

    await chooseExport(collectionRow());

    await waitFor(() => expect(exportCollection).toHaveBeenCalledTimes(1));
    await settle();
    expect(toastList()).toEqual([]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows a notification with the text of the error when the export fails", async () => {
    const { api } = await renderSidebar();
    const exportCollection = vi.spyOn(api, "exportCollection").mockRejectedValue("the disk is full");

    await chooseExport(collectionRow());

    await waitFor(() => expect(toastList()).toEqual([{ kind: "error", text: "the disk is full" }]));
    expect(exportCollection).toHaveBeenCalledTimes(1);
  });

  describe("with unsaved edits in open tabs", () => {
    /** Two requests of the collection are edited; a third tab of it is clean, and so is not counted. */
    async function renderWithEdits() {
      const view = await renderSidebar();
      const { api } = view;
      await tabs.open(api, "req-ping");
      await editRequests(api, "req-list-users", "req-create-user", "req-other");
      /** The commands in the order they were called. */
      const calls: string[] = [];
      const saveNodeContent = api.saveNodeContent.bind(api);
      const save = vi.spyOn(api, "saveNodeContent").mockImplementation(async (id, content) => {
        calls.push(`save:${id}`);
        await saveNodeContent(id, content);
      });
      const exportCollection = vi.spyOn(api, "exportCollection").mockImplementation(async (id) => {
        calls.push(`export:${id}`);
        return PATH;
      });
      await chooseExport(collectionRow());
      await screen.findByRole("dialog");
      return { ...view, calls, save, exportCollection };
    }

    const dirtyIds = () => tabs.list.filter((tab) => tab.dirty).map((tab) => tab.nodeId);

    it("asks first, telling how many tabs are not saved", async () => {
      const { calls } = await renderWithEdits();

      const dialog = screen.getByRole("dialog");
      expect(dialog.textContent).toContain(TEXT.unsavedTitle(2));
      expect(dialog.textContent).toContain(TEXT.unsavedText);
      expect(`${TEXT.unsavedTitle(2)}. ${TEXT.unsavedText}`).toBe(
        "В коллекции есть несохранённые изменения (вкладок: 2). Экспортируется сохранённая версия",
      );
      expect(within(dialog).getAllByRole("button").map((item) => item.textContent!.trim())).toEqual([
        "Сохранить и экспортировать",
        "Экспортировать как есть",
        "Отмена",
      ]);
      await settle();
      expect(calls).toEqual([]);
    });

    it("saves every edited tab and then exports", async () => {
      const { calls } = await renderWithEdits();

      await fireEvent.click(button(TEXT.saveAndExport));

      await waitFor(() => expect(toastList()).toEqual([{ kind: "info", text: TEXT.saved(PATH) }]));
      expect(calls).toEqual(["save:req-list-users", "save:req-create-user", "export:col-1"]);
      // The other collection keeps its edit.
      expect(dirtyIds()).toEqual(["req-other"]);
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("does not export when a tab cannot be saved", async () => {
      const { calls, save, exportCollection } = await renderWithEdits();
      save.mockImplementation(async (id) => {
        calls.push(`save:${id}`);
        if (id === "req-create-user") throw "the storage is locked";
      });

      await fireEvent.click(button(TEXT.saveAndExport));

      await waitFor(() => expect(toastList()).toEqual([{ kind: "error", text: "the storage is locked" }]));
      await settle();
      expect(calls).toEqual(["save:req-list-users", "save:req-create-user"]);
      expect(exportCollection).not.toHaveBeenCalled();
      expect(dirtyIds()).toEqual(["req-create-user", "req-other"]);
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("exports the saved version and keeps the edits when asked to export as is", async () => {
      const { calls } = await renderWithEdits();

      await fireEvent.click(button(TEXT.exportAsIs));

      await waitFor(() => expect(toastList()).toEqual([{ kind: "info", text: TEXT.saved(PATH) }]));
      expect(calls).toEqual(["export:col-1"]);
      expect(dirtyIds()).toEqual(["req-list-users", "req-create-user", "req-other"]);
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("does nothing on Cancel and on Escape", async () => {
      const { calls } = await renderWithEdits();

      await fireEvent.click(button(TEXT.cancel));
      expect(screen.queryByRole("dialog")).toBeNull();

      await chooseExport(collectionRow());
      await screen.findByRole("dialog");
      await fireEvent.keyDown(window, { key: "Escape" });
      expect(screen.queryByRole("dialog")).toBeNull();

      await settle();
      expect(calls).toEqual([]);
      expect(toastList()).toEqual([]);
      expect(dirtyIds()).toEqual(["req-list-users", "req-create-user", "req-other"]);
    });
  });

  it("counts the unsaved tab of the collection itself", async () => {
    const { api } = await renderSidebar();
    const exportCollection = vi.spyOn(api, "exportCollection").mockResolvedValue(PATH);
    const save = vi.spyOn(api, "saveNodeContent");
    await tabs.open(api, "col-1");
    (tabs.list[0] as Tab<CollectionDraft>).draft.variables.push({ key: "host", value: "x", secret: false, enabled: true, postman_extra: {} });

    await chooseExport(collectionRow());
    const dialog = await screen.findByRole("dialog");

    expect(dialog.textContent).toContain(TEXT.unsavedTitle(1));
    await fireEvent.click(button(TEXT.saveAndExport));
    await waitFor(() => expect(exportCollection).toHaveBeenCalledWith("col-1"));
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][0]).toBe("col-1");
  });
});

describe("export of an environment", () => {
  it("adds the item to the menu of an environment", async () => {
    await renderSidebar();

    const menu = await openMenu(await environmentRow("Production"));

    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent!.trim())).toEqual([
      strings.environment.activate,
      strings.environment.rename,
      TEXT.item,
      strings.environment.delete,
    ]);
  });

  it("exports an environment without secrets at once and tells the path", async () => {
    const { api } = await renderSidebar();
    const path = "C:\\Exports\\Production.postman_environment.json";
    const exportEnvironment = vi.spyOn(api, "exportEnvironment").mockResolvedValue(path);
    const exportCollection = vi.spyOn(api, "exportCollection");

    await chooseExport(await environmentRow("Production"));

    await waitFor(() => expect(toastList()).toEqual([{ kind: "info", text: TEXT.saved(path) }]));
    expect(exportEnvironment).toHaveBeenCalledTimes(1);
    expect(exportEnvironment).toHaveBeenCalledWith("env-prod");
    expect(exportCollection).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows nothing when the window that asks where to save is closed, and the error when the export fails", async () => {
    const { api } = await renderSidebar();
    const exportEnvironment = vi.spyOn(api, "exportEnvironment").mockResolvedValueOnce(null);
    const row = await environmentRow("Production");

    await chooseExport(row);
    await waitFor(() => expect(exportEnvironment).toHaveBeenCalledTimes(1));
    await settle();
    expect(toastList()).toEqual([]);

    exportEnvironment.mockRejectedValueOnce("access is denied");
    await chooseExport(row);
    await waitFor(() => expect(toastList()).toEqual([{ kind: "error", text: "access is denied" }]));
    expect(exportEnvironment).toHaveBeenCalledTimes(2);
  });

  it("warns that the secret values go to the file as plain text; Cancel exports nothing", async () => {
    const { api } = await renderSidebar();
    const path = "C:\\Exports\\Dev.postman_environment.json";
    const exportEnvironment = vi.spyOn(api, "exportEnvironment").mockResolvedValue(path);
    const row = await environmentRow("Dev");

    await chooseExport(row);
    const dialog = await screen.findByRole("dialog");

    expect(dialog.textContent).toContain("Секретные значения попадут в файл открытым текстом");
    expect(within(dialog).getAllByRole("button").map((item) => item.textContent!.trim())).toEqual(["Экспортировать", "Отмена"]);
    await fireEvent.click(button(TEXT.cancel));
    expect(screen.queryByRole("dialog")).toBeNull();
    await settle();
    expect(exportEnvironment).not.toHaveBeenCalled();
    expect(toastList()).toEqual([]);

    await chooseExport(row);
    await screen.findByRole("dialog");
    await fireEvent.click(button(TEXT.confirm));

    await waitFor(() => expect(toastList()).toEqual([{ kind: "info", text: TEXT.saved(path) }]));
    expect(exportEnvironment).toHaveBeenCalledTimes(1);
    expect(exportEnvironment).toHaveBeenCalledWith("env-dev");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("saves the pending edit of the open tab first, so a variable just made secret is warned about", async () => {
    const { api } = await renderSidebar();
    const calls: string[] = [];
    const saveEnvironment = api.saveEnvironment.bind(api);
    vi.spyOn(api, "saveEnvironment").mockImplementation(async (id, name, variables) => {
      calls.push(`save:${id}`);
      await saveEnvironment(id, name, variables);
    });
    vi.spyOn(api, "exportEnvironment").mockImplementation(async (id) => {
      calls.push(`export:${id}`);
      return "C:\\Exports\\Production.postman_environment.json";
    });
    const row = await environmentRow("Production");
    await tabs.openEnvironment(api, "env-prod");
    const tab = tabs.environment("env-prod")!;
    tab.variables[0].secret = true;
    tab.edited(api);

    await chooseExport(row);
    await screen.findByRole("dialog");

    expect(calls).toEqual(["save:env-prod"]);
    await fireEvent.click(button(TEXT.confirm));
    await waitFor(() => expect(calls).toEqual(["save:env-prod", "export:env-prod"]));
  });

  it("does not export when the pending edit cannot be saved", async () => {
    const { api } = await renderSidebar();
    vi.spyOn(api, "saveEnvironment").mockRejectedValue("the storage is locked");
    const exportEnvironment = vi.spyOn(api, "exportEnvironment");
    const row = await environmentRow("Production");
    await tabs.openEnvironment(api, "env-prod");
    const tab = tabs.environment("env-prod")!;
    tab.variables[0].value = "edited";
    tab.edited(api);

    await chooseExport(row);

    await waitFor(() => expect(toastList()).toEqual([{ kind: "error", text: "the storage is locked" }]));
    await settle();
    expect(exportEnvironment).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("export of the global variables", () => {
  it("has a menu with the export alone, opened by the pointer and by the keyboard", async () => {
    await renderSidebar();
    const row = await environmentRow(strings.environment.globals);
    const labels = () => within(screen.getByRole("menu")).getAllByRole("menuitem").map((item) => item.textContent!.trim());

    await openMenu(row);
    expect(labels()).toEqual([TEXT.item]);
    await fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();

    await fireEvent.keyDown(row, { key: "F10", shiftKey: true });
    await screen.findByRole("menu");
    expect(labels()).toEqual([TEXT.item]);
    await fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });

    await fireEvent.click(row.closest("li")!.querySelector("button.more")!);
    await screen.findByRole("menu");
    expect(labels()).toEqual([TEXT.item]);
  });

  it("exports them under the id of the globals and tells the path", async () => {
    const { api } = await renderSidebar();
    const path = "C:\\Exports\\globals.postman_globals.json";
    const exportEnvironment = vi.spyOn(api, "exportEnvironment").mockResolvedValue(path);

    await chooseExport(await environmentRow(strings.environment.globals));

    await waitFor(() => expect(toastList()).toEqual([{ kind: "info", text: TEXT.saved(path) }]));
    expect(exportEnvironment).toHaveBeenCalledTimes(1);
    expect(exportEnvironment).toHaveBeenCalledWith(GLOBALS_ID);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("warns about the secret global variables as well", async () => {
    const { api } = await renderSidebar({
      variables: { globals: [{ key: "api_key", value: "s3cret", secret: true, enabled: false }] },
    });
    const exportEnvironment = vi.spyOn(api, "exportEnvironment").mockResolvedValue(null);

    await chooseExport(await environmentRow(strings.environment.globals));
    await screen.findByRole("dialog");
    expect(exportEnvironment).not.toHaveBeenCalled();

    await fireEvent.click(button(TEXT.confirm));
    await waitFor(() => expect(exportEnvironment).toHaveBeenCalledWith(GLOBALS_ID));
    await settle();
    expect(toastList()).toEqual([]);
  });
});

describe("the fake api", () => {
  it("names the exported file as the core does and rejects what cannot be exported", async () => {
    const api = createFakeApi(seed());

    expect(await api.exportCollection("col-1")).toMatch(/Demo API\.postman_collection\.json$/);
    expect(await api.exportEnvironment("env-dev")).toMatch(/Dev\.postman_environment\.json$/);
    expect(await api.exportEnvironment(GLOBALS_ID)).toMatch(/globals\.postman_globals\.json$/);
    await expect(api.exportCollection("fold-users")).rejects.toBe("not a collection");
    await expect(api.exportEnvironment("env-none")).rejects.toBe("environment not found");
  });
});
