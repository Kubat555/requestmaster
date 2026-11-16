import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { createFakeApi } from "../lib/testing/fakeApi";
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
});
