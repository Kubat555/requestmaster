import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { createFakeApi, type FakeApi } from "../lib/testing/fakeApi";
import { requestTab, resultWithChanges } from "../lib/testing/scriptSend";
import type { CollectionContent, CollectionDraft } from "../lib/types";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const BASE: CollectionContent = {
  variables: [
    { key: "host", value: "api.test", postman_extra: { type: "string", description: "server" } },
    { key: "password", value: "hunter2", secret: true },
  ],
};

/** Renders the window over a fake api and opens the collection `col` in a tab. */
async function openCollection(content: CollectionContent = BASE) {
  const api = createFakeApi({ nodes: [{ id: "col", parent_id: null, kind: "collection", name: "Demo API", content }] });
  const saveNodeContent = vi.spyOn(api, "saveNodeContent");
  render(App, { context: new Map([["api", api]]) });
  await tabs.open(api, "col");
  await screen.findByRole("tab", { name: /Demo API/ });
  return { api, saveNodeContent, draft: () => (tabs.active as Tab<CollectionDraft>).draft };
}

async function pressCtrlS(): Promise<void> {
  await fireEvent(document.body, createEvent.keyDown(document.body, { key: "s", code: "KeyS", ctrlKey: true }));
}

/** Rows of the variables table without the header: the variables and the empty row at the end. */
function variableRows(): HTMLElement[] {
  return within(screen.getByRole("table", { name: strings.collection.variables })).getAllByRole("row").slice(1);
}

// The tabs of the window live in a module; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
});

describe("CollectionEditor", () => {
  it("saves an edited variable with Ctrl+S and keeps its postman_extra", async () => {
    const { saveNodeContent } = await openCollection();
    const rows = variableRows();
    expect(rows).toHaveLength(3);
    expect(within(rows[0]).getByRole<HTMLInputElement>("textbox", { name: strings.collection.name }).value).toBe("host");

    await fireEvent.input(within(rows[0]).getByRole("textbox", { name: strings.table.value }), { target: { value: "staging.test" } });
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
    await pressCtrlS();
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("col", {
      auth: null,
      variables: [
        { key: "host", value: "staging.test", secret: false, enabled: true, postman_extra: { type: "string", description: "server" } },
        { key: "password", value: "hunter2", secret: true, enabled: true, postman_extra: {} },
      ],
      scripts: { pre_request: "", test: "" },
      description: "",
    });
  });

  it("hides the value of a secret variable until it is asked to show", async () => {
    await openCollection();
    const rows = variableRows();
    const plain = within(rows[0]).getByLabelText<HTMLInputElement>(strings.table.value);
    const secret = within(rows[1]).getByLabelText<HTMLInputElement>(strings.table.value);

    expect(plain.type).toBe("text");
    expect(within(rows[0]).queryByRole("button", { name: strings.table.show })).toBeNull();
    expect(secret.type).toBe("password");
    expect(screen.queryByDisplayValue("hunter2")).toBe(secret);

    // An icon alone (an eye): the name is said to a screen reader and shown as the tooltip.
    const show = within(rows[1]).getByRole("button", { name: strings.table.show });
    expect(show.title).toBe(strings.table.show);
    expect(show.textContent!.trim()).toBe("");
    expect(show.getAttribute("aria-pressed")).toBe("false");

    await fireEvent.click(show);
    expect(secret.type).toBe("text");
    expect(secret.value).toBe("hunter2");

    const hide = within(rows[1]).getByRole("button", { name: strings.table.hide });
    expect(hide.title).toBe(strings.table.hide);
    expect(hide.getAttribute("aria-pressed")).toBe("true");
    await fireEvent.click(hide);
    expect(secret.type).toBe("password");
  });

  it("adds a variable that is neither secret nor disabled", async () => {
    const { draft } = await openCollection();
    const rows = variableRows();

    await fireEvent.input(within(rows[2]).getByRole("textbox", { name: strings.collection.name }), { target: { value: "t" } });

    expect(draft().variables[2]).toEqual({ key: "t", value: "", secret: false, enabled: true, postman_extra: {} });
    expect(screen.getByRole("tab", { name: /^Variables/ }).textContent).toMatch(/Variables\s*3/);
  });

  it("shows a collection without auth as no authorization and has no inherit option", async () => {
    const { draft } = await openCollection();
    await fireEvent.click(screen.getByRole("tab", { name: /^Auth/ }));
    const type = screen.getByRole<HTMLSelectElement>("combobox", { name: strings.auth.type });

    expect(type.value).toBe("no_auth");
    expect(within(type).getAllByRole<HTMLOptionElement>("option").map((option) => option.value)).not.toContain("inherit");

    await fireEvent.change(type, { target: { value: "basic" } });
    await fireEvent.input(screen.getByLabelText(strings.auth.username), { target: { value: "ann" } });
    expect(draft().auth).toEqual({ type: "basic", username: "ann", password: "" });

    // "No authorization" of a collection is the absence of auth, not `no_auth`.
    await fireEvent.change(type, { target: { value: "no_auth" } });
    expect(draft().auth).toBeNull();
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("shows a stored no_auth the same way and does not rewrite it", async () => {
    const { draft, saveNodeContent } = await openCollection({ auth: { type: "no_auth" } });
    await fireEvent.click(screen.getByRole("tab", { name: /^Auth/ }));

    expect(screen.getByRole<HTMLSelectElement>("combobox", { name: strings.auth.type }).value).toBe("no_auth");
    expect(draft().auth).toEqual({ type: "no_auth" });
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();

    // Saved together with another edit, it is still `no_auth`.
    await fireEvent.click(screen.getByRole("tab", { name: /^Variables/ }));
    await fireEvent.input(within(variableRows()[0]).getByRole("textbox", { name: strings.collection.name }), { target: { value: "v" } });
    await pressCtrlS();
    await waitFor(() => expect(saveNodeContent).toHaveBeenCalledTimes(1));
    expect(saveNodeContent.mock.calls[0][1]).toMatchObject({ auth: { type: "no_auth" } });
  });

  it("has the sections Variables, Auth, Scripts and the description", async () => {
    await openCollection();

    const sections = within(screen.getByRole("tablist", { name: strings.collection.sections })).getAllByRole("tab");

    expect(sections.map((section) => section.textContent!.trim().replace(/\s*\d+$/, ""))).toEqual([
      "Variables",
      "Auth",
      "Scripts",
      "Описание",
    ]);
    expect(strings.collection.scripts).toBe("Scripts");
  });

  it("counts the scripts that are not empty next to Scripts", async () => {
    await openCollection({ scripts: { pre_request: "before();", test: " \n" } });
    expect(screen.getByRole("tab", { name: /^Scripts/ }).textContent).toMatch(/Scripts\s*1$/);
  });

  it("shows no number next to Scripts while there are no scripts", async () => {
    await openCollection();
    expect(screen.getByRole("tab", { name: /^Scripts/ }).textContent!.trim()).toBe("Scripts");
  });

  it("marks the tab when a script is edited and saves it with Ctrl+S", async () => {
    const { saveNodeContent, draft } = await openCollection({ scripts: { test: "after();" } });
    // The editor of the scripts is created when the section is first shown.
    expect(screen.queryByRole("textbox", { name: strings.scripts.text })).toBeNull();
    await fireEvent.click(screen.getByRole("tab", { name: /^Scripts/ }));
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();

    await fireEvent.input(screen.getByRole("textbox", { name: strings.scripts.text }), { target: { value: "before();" } });

    expect(draft().scripts).toEqual({ pre_request: "before();", test: "after();" });
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
    expect(screen.getByRole("tab", { name: /^Scripts/ }).textContent).toMatch(/Scripts\s*2$/);

    await pressCtrlS();
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("col", {
      auth: null,
      variables: [],
      scripts: { pre_request: "before();", test: "after();" },
      description: "",
    });
  });

  it("edits the description as plain text: marks the tab, takes the text back on revert, saves it with Ctrl+S", async () => {
    const { saveNodeContent, draft } = await openCollection({ description: "About **the collection**\nsecond line" });
    expect(strings.collection.description).toBe("Описание");
    expect(screen.queryByRole("textbox", { name: strings.collection.description })).toBeNull();

    await fireEvent.click(screen.getByRole("tab", { name: strings.collection.description }));
    const field = screen.getByRole<HTMLTextAreaElement>("textbox", { name: strings.collection.description });
    // The source text: Markdown is not rendered.
    expect(field.tagName).toBe("TEXTAREA");
    expect(field.value).toBe("About **the collection**\nsecond line");
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();

    await fireEvent.input(field, { target: { value: "Thrown away" } });
    expect(draft().description).toBe("Thrown away");
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    (tabs.active as Tab<CollectionDraft>).revert();
    await waitFor(() => expect(field.value).toBe("About **the collection**\nsecond line"));
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();

    await fireEvent.input(field, { target: { value: "New text" } });
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
    await pressCtrlS();
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("col", {
      auth: null,
      variables: [],
      scripts: { pre_request: "", test: "" },
      description: "New text",
    });
  });

  it("opens a collection without a description with an empty one, not marked", async () => {
    const { draft } = await openCollection({});

    expect(draft().description).toBe("");
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
    await fireEvent.click(screen.getByRole("tab", { name: strings.collection.description }));
    expect(screen.getByRole<HTMLTextAreaElement>("textbox", { name: strings.collection.description }).value).toBe("");
  });
});

describe("variables changed by a script with the tab of the collection open", () => {
  const TOKEN = { key: "token", value: "T", secret: false, enabled: true, postman_extra: {} };

  /** A send whose script sets the variable `token` of the collection to `T`. */
  async function sendSettingToken(api: FakeApi) {
    api.onSend = async () => resultWithChanges([{ scope: "collection", target_id: "col", op: "set", key: "token", value: "T" }]);
    const sender = requestTab();
    await sender.sending.send(api, sender);
  }

  it("script_change_reaches_a_clean_collection_tab", async () => {
    const { api, saveNodeContent } = await openCollection();

    await sendSettingToken(api);

    await waitFor(() => expect(variableRows()).toHaveLength(4));
    const row = variableRows()[2];
    expect(within(row).getByRole<HTMLInputElement>("textbox", { name: strings.collection.name }).value).toBe("token");
    expect(within(row).getByRole<HTMLInputElement>("textbox", { name: strings.table.value }).value).toBe("T");
    // The core has written it already: the tab has nothing to save.
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
    expect((tabs.active as Tab<CollectionDraft>).saved.variables[2]).toEqual(TOKEN);
    expect(saveNodeContent).not.toHaveBeenCalled();
  });

  it("script_change_keeps_unsaved_edits_of_a_dirty_tab", async () => {
    const { api, saveNodeContent, draft } = await openCollection();
    draft().description = "About";
    await fireEvent.input(within(variableRows()[0]).getByRole("textbox", { name: strings.table.value }), {
      target: { value: "staging.test" },
    });
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    await sendSettingToken(api);

    await waitFor(() => expect(variableRows()).toHaveLength(4));
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
    await pressCtrlS();
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("col", {
      auth: null,
      variables: [
        { key: "host", value: "staging.test", secret: false, enabled: true, postman_extra: { type: "string", description: "server" } },
        { key: "password", value: "hunter2", secret: true, enabled: true, postman_extra: {} },
        TOKEN,
      ],
      scripts: { pre_request: "", test: "" },
      description: "About",
    });
  });
});
