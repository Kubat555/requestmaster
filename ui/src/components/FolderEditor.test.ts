import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { createFakeApi } from "../lib/testing/fakeApi";
import type { FolderContent, FolderDraft } from "../lib/types";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

/** Renders the window over a fake api and opens the folder `fold` of the collection `col` in a tab. */
async function openFolder(content: FolderContent = {}) {
  const api = createFakeApi({
    nodes: [
      { id: "col", parent_id: null, kind: "collection", name: "Demo API", content: {} },
      { id: "fold", parent_id: "col", kind: "folder", name: "Users", content },
    ],
  });
  const saveNodeContent = vi.spyOn(api, "saveNodeContent");
  render(App, { context: new Map([["api", api]]) });
  await tabs.open(api, "fold");
  await screen.findByRole("tab", { name: /Users/ });
  const tab = tabs.active as Tab<FolderDraft>;
  return { api, saveNodeContent, tab, draft: () => tab.draft };
}

async function pressCtrlS(): Promise<void> {
  await fireEvent(document.body, createEvent.keyDown(document.body, { key: "s", code: "KeyS", ctrlKey: true }));
}

function authType(): HTMLSelectElement {
  return screen.getByRole("combobox", { name: strings.auth.type });
}

function scriptEditor(): HTMLTextAreaElement {
  return screen.getByRole("textbox", { name: strings.scripts.text });
}

// The tabs of the window live in a module; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
});

describe("FolderEditor", () => {
  it("has the sections Auth, Scripts and the description and shows Auth first", async () => {
    const { tab } = await openFolder();

    expect(tab.kind).toBe("folder");
    const sections = within(screen.getByRole("tablist", { name: strings.folder.sections })).getAllByRole("tab");
    expect(sections.map((section) => section.textContent!.trim())).toEqual(["Auth", "Scripts", "Описание"]);
    expect(sections[0].getAttribute("aria-selected")).toBe("true");
    expect(strings.folder.auth).toBe("Auth");
    expect(strings.folder.scripts).toBe("Scripts");
    // A folder has no variables.
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("inherits the authorization by default, and is not changed after Basic is chosen and inheriting is chosen back", async () => {
    const { tab, draft } = await openFolder();
    const type = authType();

    // A folder has a parent, so it can inherit.
    expect(type.value).toBe("inherit");
    expect(within(type).getByRole<HTMLOptionElement>("option", { name: strings.auth.inherit }).value).toBe("inherit");
    expect(strings.auth.inherit).toBe("Наследовать от родителя");

    await fireEvent.change(type, { target: { value: "basic" } });
    expect(draft().auth).toEqual({ type: "basic", username: "", password: "" });
    expect(tab.dirty).toBe(true);
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    await fireEvent.change(type, { target: { value: "inherit" } });
    expect(draft().auth).toBeNull();
    expect(tab.dirty).toBe(false);
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("saves no authorization of a folder as no_auth", async () => {
    const { saveNodeContent, tab } = await openFolder();
    const type = authType();
    expect(within(type).getByRole<HTMLOptionElement>("option", { name: strings.auth.none }).value).toBe("no_auth");
    expect(strings.auth.none).toBe("Без авторизации");

    await fireEvent.change(type, { target: { value: "no_auth" } });
    expect(tab.dirty).toBe(true);
    await pressCtrlS();
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("fold", {
      auth: { type: "no_auth" },
      scripts: { pre_request: "", test: "" },
      description: "",
    });
    expect(tab.dirty).toBe(false);
  });

  it("marks the tab when a script is edited and saves it with Ctrl+S", async () => {
    const { saveNodeContent, draft } = await openFolder({ scripts: { test: "after();" } });
    // The editor of the scripts is created when the section is first shown.
    expect(screen.queryByRole("textbox", { name: strings.scripts.text })).toBeNull();
    expect(screen.getByRole("tab", { name: /^Scripts/ }).textContent).toMatch(/Scripts\s*1$/);

    await fireEvent.click(screen.getByRole("tab", { name: /^Scripts/ }));
    expect(scriptEditor().value).toBe("");
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();

    await fireEvent.input(scriptEditor(), { target: { value: "pm.variables.set('a', 1);" } });

    expect(draft().scripts).toEqual({ pre_request: "pm.variables.set('a', 1);", test: "after();" });
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
    expect(screen.getByRole("tab", { name: /^Scripts/ }).textContent).toMatch(/Scripts\s*2$/);

    await pressCtrlS();
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("fold", {
      auth: null,
      scripts: { pre_request: "pm.variables.set('a', 1);", test: "after();" },
      description: "",
    });
  });

  it("keeps a stored authorization and scripts as they came, without marking the tab", async () => {
    const { tab, draft } = await openFolder({ auth: { type: "bearer", token: "t" }, scripts: { pre_request: "a();\r\n" } });

    expect(authType().value).toBe("bearer");
    expect(draft()).toEqual({
      auth: { type: "bearer", token: "t" },
      scripts: { pre_request: "a();\r\n", test: "" },
      description: "",
    });
    expect(tab.dirty).toBe(false);
  });

  it("edits the description as plain text: marks the tab, takes the text back on revert, saves it with Ctrl+S", async () => {
    const { saveNodeContent, tab, draft } = await openFolder({ description: "About **the folder**" });
    expect(screen.queryByRole("textbox", { name: strings.collection.description })).toBeNull();

    await fireEvent.click(screen.getByRole("tab", { name: strings.collection.description }));
    const field = screen.getByRole<HTMLTextAreaElement>("textbox", { name: strings.collection.description });
    // The source text: Markdown is not rendered.
    expect(field.tagName).toBe("TEXTAREA");
    expect(field.value).toBe("About **the folder**");
    expect(tab.dirty).toBe(false);

    await fireEvent.input(field, { target: { value: "Thrown away" } });
    expect(draft().description).toBe("Thrown away");
    expect(tab.dirty).toBe(true);
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    tab.revert();
    await waitFor(() => expect(field.value).toBe("About **the folder**"));
    expect(tab.dirty).toBe(false);
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();

    await fireEvent.input(field, { target: { value: "" } });
    expect(tab.dirty).toBe(true);
    await pressCtrlS();
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    // An emptied description is sent as an empty text: that is what clears it.
    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("fold", {
      auth: null,
      scripts: { pre_request: "", test: "" },
      description: "",
    });
    expect(tab.dirty).toBe(false);
  });
});
