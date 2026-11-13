import { createEvent, fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import { strings } from "../lib/strings";
import { createFakeApi, type FakeApi, type FakeSeed } from "../lib/testing/fakeApi";
import type { ImportResult, TreeRow } from "../lib/types";
import { Workspace } from "../lib/workspace.svelte";

/** Loads the workspace from a fake api, as the window does at start, and renders the sidebar over it. */
async function renderSidebar(seed: FakeSeed = {}) {
  const api = createFakeApi(seed);
  const workspace = new Workspace();
  await workspace.load(api);
  const view = render(Sidebar, { props: { workspace }, context: new Map([["api", api]]) });
  const fileInput = view.container.querySelector<HTMLInputElement>('input[type="file"]')!;
  return { api, workspace, fileInput, ...view };
}

function jsonFile(name: string, text: string): File {
  return new File([text], name, { type: "application/json" });
}

function result(overrides: Partial<ImportResult>): ImportResult {
  return { file_name: "file.json", kind: "collection", name: null, root_id: null, report: [], conflicts: [], error: null, ...overrides };
}

/** The tree after the import of {@link IMPORTED}. */
const IMPORTED_ROWS: TreeRow[] = [
  { id: "col-new", parent_id: null, kind: "collection", name: "Imported API", sort_order: 0, method: null },
  { id: "req-login", parent_id: "col-new", kind: "request", name: "Login", sort_order: 0, method: "POST" },
];

const IMPORTED: ImportResult = result({
  file_name: "imported.postman_collection.json",
  name: "Imported API",
  root_id: "col-new",
  report: [{ kind: "has_scripts", node_id: "req-login", node_path: "Imported API / Login", detail: "test" }],
});

/** Makes the fake answer an import with `results`; afterwards the tree is {@link IMPORTED_ROWS}. */
function answerImport(api: FakeApi, results: ImportResult[]) {
  const tree = vi.spyOn(api, "tree");
  const importFiles = vi.spyOn(api, "importFiles").mockImplementation(async () => {
    tree.mockResolvedValue(IMPORTED_ROWS);
    return results;
  });
  return { tree, importFiles };
}

function treeItemNames(): string[] {
  return screen.queryAllByRole("treeitem").map((item) => item.textContent!.replace(/\s+/g, " ").trim());
}

describe("Sidebar", () => {
  it("shows the placeholder when the tree is empty", async () => {
    await renderSidebar({ nodes: [] });
    expect(screen.getByText(strings.sidebar.empty)).toBeTruthy();
    expect(screen.queryAllByRole("treeitem")).toHaveLength(0);
  });

  it("has a hidden input for several .json files behind the import button", async () => {
    const { fileInput } = await renderSidebar();
    expect(screen.getByRole("button", { name: strings.import.button })).toBeTruthy();
    expect(fileInput.multiple).toBe(true);
    expect(fileInput.accept).toBe(".json");
    expect(fileInput.hidden).toBe(true);
  });

  it("imports the chosen file, re-reads the tree and shows the report", async () => {
    const { api, fileInput } = await renderSidebar({ nodes: [] });
    const { tree, importFiles } = answerImport(api, [IMPORTED]);
    const text = '{"info":{"name":"Imported API"}}';

    await fireEvent.change(fileInput, { target: { files: [jsonFile(IMPORTED.file_name, text)] } });
    const dialog = await screen.findByRole("dialog");

    expect(importFiles).toHaveBeenCalledTimes(1);
    expect(importFiles).toHaveBeenCalledWith([{ name: IMPORTED.file_name, text }]);
    // The tree is re-read and the root of the imported collection is expanded.
    expect(tree).toHaveBeenCalledTimes(1);
    expect(treeItemNames()).toEqual(["Imported API", "POST Login"]);
    expect(screen.queryByText(strings.sidebar.empty)).toBeNull();
    // The report names the file, the collection and the record with the path of its node.
    expect(within(dialog).getByText(IMPORTED.file_name)).toBeTruthy();
    expect(within(dialog).getByText(/Imported API(?! \/)/)).toBeTruthy();
    expect(within(dialog).getByText(strings.import.reportKinds.has_scripts)).toBeTruthy();
    expect(within(dialog).getByText("Imported API / Login")).toBeTruthy();
  });

  it("shows the message of a failed file next to the imported one", async () => {
    const { api, fileInput } = await renderSidebar({ nodes: [] });
    const failed = result({ file_name: "photo.json", kind: "error", error: "photo.json is not a Postman file" });
    const { importFiles } = answerImport(api, [failed, IMPORTED]);

    await fireEvent.change(fileInput, {
      target: { files: [jsonFile("photo.json", "not json"), jsonFile(IMPORTED.file_name, "{}")] },
    });
    const dialog = await screen.findByRole("dialog");

    expect(importFiles).toHaveBeenCalledWith([
      { name: "photo.json", text: "not json" },
      { name: IMPORTED.file_name, text: "{}" },
    ]);
    expect(within(dialog).getByText("photo.json is not a Postman file")).toBeTruthy();
    expect(within(dialog).getByText(IMPORTED.file_name)).toBeTruthy();
    expect(treeItemNames()).toEqual(["Imported API", "POST Login"]);
  });

  it("asks what to do with conflicting globals and applies the file's values on request", async () => {
    const { api, fileInput } = await renderSidebar();
    const conflicting = result({ file_name: "globals.json", kind: "globals", conflicts: ["token", "host"] });
    answerImport(api, [conflicting]);
    const importGlobals = vi.spyOn(api, "importGlobals");
    const text = '{"values":[]}';

    await fireEvent.change(fileInput, { target: { files: [jsonFile("globals.json", text)] } });
    const dialog = await screen.findByRole("dialog", { name: strings.import.conflictsTitle });

    expect(within(dialog).getByText("token")).toBeTruthy();
    expect(within(dialog).getByText("host")).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: strings.import.keepCurrent })).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: strings.import.cancel })).toBeTruthy();
    expect(importGlobals).not.toHaveBeenCalled();

    await fireEvent.click(within(dialog).getByRole("button", { name: strings.import.takeIncoming }));
    // After the choice the report takes the place of the question.
    await screen.findByRole("dialog", { name: strings.import.reportTitle });

    expect(importGlobals).toHaveBeenCalledTimes(1);
    expect(importGlobals).toHaveBeenCalledWith(text, "take_incoming");
    expect(screen.queryByRole("dialog", { name: strings.import.conflictsTitle })).toBeNull();
  });

  it("does not apply conflicting globals when the question is cancelled", async () => {
    const { api, fileInput } = await renderSidebar();
    answerImport(api, [result({ file_name: "globals.json", kind: "globals", conflicts: ["token"] })]);
    const importGlobals = vi.spyOn(api, "importGlobals");

    await fireEvent.change(fileInput, { target: { files: [jsonFile("globals.json", "{}")] } });
    const question = await screen.findByRole("dialog", { name: strings.import.conflictsTitle });
    await fireEvent.click(within(question).getByRole("button", { name: strings.import.cancel }));
    const report = await screen.findByRole("dialog", { name: strings.import.reportTitle });

    expect(importGlobals).not.toHaveBeenCalled();
    expect(within(report).getByText(strings.import.globalsCancelled)).toBeTruthy();
  });

  it("leaves only the matching rows and their ancestors when the filter is typed", async () => {
    await renderSidebar();
    expect(treeItemNames()).toEqual(["Demo API"]);

    await fireEvent.input(screen.getByRole("searchbox", { name: strings.sidebar.filter }), { target: { value: "get" } });
    expect(treeItemNames()).toEqual(["Demo API", "Orders", "GET Get order"]);

    await fireEvent.input(screen.getByRole("searchbox", { name: strings.sidebar.filter }), { target: { value: "zzz" } });
    expect(treeItemNames()).toEqual([]);
    expect(screen.getByText(strings.sidebar.nothingFound)).toBeTruthy();
  });

  it("opens a request with Enter", async () => {
    const { workspace } = await renderSidebar();
    workspace.onOpen = vi.fn();
    workspace.toggle("col-1");
    workspace.toggle("fold-users");
    const request = await screen.findByRole("treeitem", { name: /List users/ });

    await fireEvent.keyDown(request, { key: "Enter" });

    expect(workspace.onOpen).toHaveBeenCalledTimes(1);
    expect(workspace.onOpen).toHaveBeenCalledWith("req-list-users");
    expect(workspace.selectedId).toBe("req-list-users");
    expect(request.getAttribute("aria-selected")).toBe("true");
  });

  it("opens a request and a collection with a click and only toggles a folder", async () => {
    const { workspace } = await renderSidebar();
    workspace.onOpen = vi.fn();

    await fireEvent.click(screen.getByRole("treeitem", { name: "Demo API" }));
    expect(workspace.onOpen).toHaveBeenLastCalledWith("col-1");
    expect(workspace.selectedId).toBe("col-1");

    workspace.toggle("col-1");
    const folder = await screen.findByRole("treeitem", { name: "Users" });
    await fireEvent.click(folder);
    expect(folder.getAttribute("aria-expanded")).toBe("true");
    expect(workspace.onOpen).toHaveBeenCalledTimes(1);
    expect(workspace.selectedId).toBe("col-1");

    await fireEvent.click(screen.getByRole("treeitem", { name: /Create user/ }));
    expect(workspace.onOpen).toHaveBeenLastCalledWith("req-create-user");
  });

  it("expands a folder with the right arrow and collapses it with the left one", async () => {
    const { workspace } = await renderSidebar();
    workspace.toggle("col-1");
    const folder = await screen.findByRole("treeitem", { name: "Users" });
    expect(folder.getAttribute("aria-expanded")).toBe("false");
    expect(folder.getAttribute("aria-level")).toBe("2");
    expect(screen.queryByRole("treeitem", { name: /List users/ })).toBeNull();

    await fireEvent.keyDown(folder, { key: "ArrowRight" });
    expect(folder.getAttribute("aria-expanded")).toBe("true");
    const request = screen.getByRole("treeitem", { name: /List users/ });
    expect(request.getAttribute("aria-level")).toBe("3");
    expect(request.hasAttribute("aria-expanded")).toBe(false);

    await fireEvent.keyDown(folder, { key: "ArrowLeft" });
    expect(folder.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("treeitem", { name: /List users/ })).toBeNull();
  });

  it("moves the focus with the up and down arrows", async () => {
    const { workspace } = await renderSidebar();
    workspace.toggle("col-1");
    const users = await screen.findByRole("treeitem", { name: "Users" });
    const collection = screen.getByRole("treeitem", { name: "Demo API" });
    expect(screen.getByRole("tree")).toBeTruthy();

    collection.focus();
    await fireEvent.keyDown(collection, { key: "ArrowDown" });
    expect(document.activeElement).toBe(users);
    await fireEvent.keyDown(users, { key: "ArrowDown" });
    expect(document.activeElement).toBe(screen.getByRole("treeitem", { name: "Orders" }));
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
    expect(document.activeElement).toBe(collection);
  });

  it("colours the method of a request and shortens a long unknown one", async () => {
    const { workspace } = await renderSidebar({
      nodes: [
        { id: "c", parent_id: null, kind: "collection", name: "C", content: {} },
        { id: "r1", parent_id: "c", kind: "request", name: "One", content: { method: "GET", url: "" } },
        { id: "r2", parent_id: "c", kind: "request", name: "Two", content: { method: "PROPFIND", url: "" } },
      ],
    });
    workspace.toggle("c");

    const get = (await screen.findByRole("treeitem", { name: /One/ })).querySelector(".method")!;
    expect(get.textContent).toBe("GET");
    expect(get.classList.contains("method-get")).toBe(true);

    const other = screen.getByRole("treeitem", { name: /Two/ }).querySelector(".method")!;
    expect(other.textContent).toBe("PROPFI");
    expect(other.classList.contains("method-other")).toBe(true);
  });

  it("cancels a drop and a dragover outside the sidebar without importing", async () => {
    const { api } = await renderSidebar();
    const importFiles = vi.spyOn(api, "importFiles");
    const dataTransfer = { files: [jsonFile("stray.json", "{}")] };

    const dragover = createEvent.dragOver(document.body, { dataTransfer });
    await fireEvent(document.body, dragover);
    const drop = createEvent.drop(document.body, { dataTransfer });
    await fireEvent(document.body, drop);
    // Lets a started import reach the api, if there were one.
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(dragover.defaultPrevented).toBe(true);
    expect(drop.defaultPrevented).toBe(true);
    expect(importFiles).not.toHaveBeenCalled();
  });

  it("imports the files dropped on the sidebar", async () => {
    const { api } = await renderSidebar({ nodes: [] });
    const { importFiles } = answerImport(api, [IMPORTED]);
    const target = screen.getByText(strings.sidebar.empty);

    const drop = createEvent.drop(target, { dataTransfer: { files: [jsonFile(IMPORTED.file_name, "{}")] } });
    await fireEvent(target, drop);
    await screen.findByRole("dialog");

    expect(drop.defaultPrevented).toBe(true);
    expect(importFiles).toHaveBeenCalledWith([{ name: IMPORTED.file_name, text: "{}" }]);
  });

  it("closes the report with Escape", async () => {
    const { api, fileInput } = await renderSidebar({ nodes: [] });
    answerImport(api, [IMPORTED]);

    await fireEvent.change(fileInput, { target: { files: [jsonFile(IMPORTED.file_name, "{}")] } });
    const dialog = await screen.findByRole("dialog");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.contains(document.activeElement)).toBe(true);

    await fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
