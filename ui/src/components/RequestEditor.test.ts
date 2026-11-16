import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { toasts } from "../lib/toasts.svelte";
import { createFakeApi } from "../lib/testing/fakeApi";
import type { NodeDto, RequestContent, RequestDraft } from "../lib/types";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const BASE: RequestContent = {
  method: "GET",
  url: "https://api.test/users",
  query: [
    { key: "page", value: "1" },
    { key: "limit", value: "20", postman_extra: { description: "page size" } },
  ],
  headers: [{ key: "Accept", value: "application/json" }],
};

function nodes(content: RequestContent = BASE): NodeDto[] {
  return [
    { id: "col", parent_id: null, kind: "collection", name: "API", content: {} },
    { id: "req", parent_id: "col", kind: "request", name: "List users", content },
    { id: "other", parent_id: "col", kind: "request", name: "Other", content: { method: "POST", url: "https://api.test/other" } },
  ];
}

/** Renders the window over a fake api and opens the request `req` in a tab. */
async function openRequest(content: RequestContent = BASE) {
  const api = createFakeApi({ nodes: nodes(content) });
  render(App, { context: new Map([["api", api]]) });
  await tabs.open(api, "req");
  const tab = await screen.findByRole("tab", { name: /List users/ });
  return { api, tab, draft: () => (tabs.active as Tab<RequestDraft>).draft };
}

/** Presses a key combination on the window and returns the event. */
async function press(init: KeyboardEventInit): Promise<KeyboardEvent> {
  const event = createEvent.keyDown(document.body, init);
  await fireEvent(document.body, event);
  return event as KeyboardEvent;
}

const CTRL_S = { key: "s", code: "KeyS", ctrlKey: true };
const CTRL_W = { key: "w", code: "KeyW", ctrlKey: true };

function urlInput(): HTMLInputElement {
  return screen.getByRole("textbox", { name: strings.editor.url });
}

async function showSection(name: RegExp): Promise<void> {
  await fireEvent.click(screen.getByRole("tab", { name }));
}

/** Rows of a key-value table without the header: the data rows and the empty one at the end. */
function tableRows(tableName: string): HTMLElement[] {
  return within(screen.getByRole("table", { name: tableName })).getAllByRole("row").slice(1);
}

// The tabs and the notifications of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("RequestEditor", () => {
  it("shows the method and the URL of the opened request", async () => {
    const { tab } = await openRequest();

    expect(screen.getByRole<HTMLSelectElement>("combobox", { name: strings.editor.method }).value).toBe("GET");
    expect(urlInput().value).toBe("https://api.test/users");
    expect(tab.getAttribute("aria-selected")).toBe("true");
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
    expect(screen.getByRole<HTMLButtonElement>("button", { name: strings.editor.send }).disabled).toBe(false);
  });

  it("marks the tab when the URL is edited", async () => {
    const { tab, draft } = await openRequest();

    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    expect(draft().url).toBe("https://api.test/people");
    expect(within(tab).getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    await fireEvent.input(urlInput(), { target: { value: "https://api.test/users" } });
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("saves the draft with Ctrl+S and removes the mark", async () => {
    const { api } = await openRequest();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    await press(CTRL_S);
    await waitFor(() => expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull());

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("req", expect.objectContaining({ url: "https://api.test/people", method: "GET" }));
  });

  it("saves with Ctrl+S on a keyboard layout where the key is not a Latin letter", async () => {
    const { api } = await openRequest();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    const event = await press({ key: "ы", code: "KeyS", ctrlKey: true });

    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(saveNodeContent).toHaveBeenCalledTimes(1));
  });

  it("keeps the draft and shows a notification when the save fails", async () => {
    const { api, draft } = await openRequest();
    vi.spyOn(api, "saveNodeContent").mockRejectedValue("disk is full");
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    await press(CTRL_S);

    expect((await screen.findByRole("alert")).textContent).toContain("disk is full");
    expect(draft().url).toBe("https://api.test/people");
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
  });

  it("changes the method in the draft while the tree keeps the stored one", async () => {
    const { api, draft } = await openRequest();
    const tree = vi.spyOn(api, "tree");
    const method = screen.getByRole<HTMLSelectElement>("combobox", { name: strings.editor.method });
    const options = within(method).getAllByRole<HTMLOptionElement>("option").map((option) => option.value);
    expect(options.slice(0, 7)).toEqual(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]);

    await fireEvent.change(method, { target: { value: "DELETE" } });

    expect(draft().method).toBe("DELETE");
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
    expect(tree).not.toHaveBeenCalled();
  });

  it("takes a method of the user's own as text", async () => {
    const { draft } = await openRequest();
    const method = screen.getByRole<HTMLSelectElement>("combobox", { name: strings.editor.method });
    expect(screen.queryByRole("textbox", { name: strings.editor.customMethod })).toBeNull();

    await fireEvent.change(method, { target: { value: within(method).getAllByRole<HTMLOptionElement>("option")[7].value } });
    await fireEvent.input(screen.getByRole("textbox", { name: strings.editor.customMethod }), { target: { value: "PROPFIND" } });

    expect(draft().method).toBe("PROPFIND");
  });

  it("shows a stored method that is not in the list as the user's own", async () => {
    await openRequest({ method: "PURGE", url: "https://api.test/cache" });
    expect(screen.getByRole<HTMLInputElement>("textbox", { name: strings.editor.customMethod }).value).toBe("PURGE");
  });

  it("switches a parameter off in Params and counts the enabled rows", async () => {
    const { draft } = await openRequest();
    expect(screen.getByRole("tab", { name: /^Params/ }).textContent).toMatch(/Params\s*2/);
    expect(screen.getByRole("tab", { name: /^Headers/ }).textContent).toMatch(/Headers\s*1/);
    const rows = tableRows(strings.editor.query);
    // Two parameters and the empty row.
    expect(rows).toHaveLength(3);
    expect(within(rows[0]).getByRole<HTMLInputElement>("textbox", { name: strings.table.key }).value).toBe("page");

    await fireEvent.click(within(rows[0]).getByRole("checkbox", { name: strings.table.enabled }));

    expect(draft().query[0].enabled).toBe(false);
    expect(screen.getByRole("tab", { name: /^Params/ }).textContent).toMatch(/Params\s*1/);
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    // Switched back on: the tab is not changed any more, though the stored row had no `enabled`.
    await fireEvent.click(within(rows[0]).getByRole("checkbox", { name: strings.table.enabled }));
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("edits a parameter in place and keeps its postman_extra", async () => {
    const { draft } = await openRequest();
    const rows = tableRows(strings.editor.query);

    await fireEvent.input(within(rows[1]).getByRole("textbox", { name: strings.table.value }), { target: { value: "50" } });

    expect(draft().query[1]).toEqual({ key: "limit", value: "50", enabled: true, postman_extra: { description: "page size" } });
  });

  it("deletes a row", async () => {
    const { draft } = await openRequest();
    const rows = tableRows(strings.editor.query);

    await fireEvent.click(within(rows[0]).getByRole("button", { name: strings.table.remove }));

    expect(draft().query.map((row) => row.key)).toEqual(["limit"]);
    expect(tableRows(strings.editor.query)).toHaveLength(2);
  });

  it("deletes a row with a real button that the keyboard reaches", async () => {
    await openRequest();
    const remove = within(tableRows(strings.editor.query)[0]).getByRole<HTMLButtonElement>("button", { name: strings.table.remove });

    expect(remove.tagName).toBe("BUTTON");
    expect(remove.type).toBe("button");
    expect(remove.classList.contains("remove")).toBe(true);
    expect(remove.tabIndex).toBe(0);
    expect(remove.getAttribute("aria-label")).toBe(strings.table.remove);
    // The empty row at the end has nothing to delete.
    expect(within(tableRows(strings.editor.query)[2]).queryByRole("button")).toBeNull();
  });

  it("has no path variables block in Params when the request has none", async () => {
    await openRequest();

    expect(screen.getByRole("table", { name: strings.editor.query })).toBeTruthy();
    expect(screen.queryByRole("table", { name: strings.editor.pathVariables })).toBeNull();
    expect(screen.queryByRole("heading", { name: strings.editor.pathVariables })).toBeNull();
  });

  it("shows the path variables as a group of their own in Params", async () => {
    const { draft } = await openRequest({ method: "GET", url: "https://api.test/users/:id", path_variables: [{ key: "id", value: "7" }] });
    expect(screen.getByRole("heading", { name: strings.editor.pathVariables })).toBeTruthy();
    const rows = tableRows(strings.editor.pathVariables);

    expect(within(rows[0]).getByRole<HTMLInputElement>("textbox", { name: strings.table.key }).value).toBe("id");
    await fireEvent.input(within(rows[0]).getByRole("textbox", { name: strings.table.value }), { target: { value: "8" } });
    expect(draft().path_variables[0].value).toBe("8");
  });

  it("adds a header typed into the empty row and moves the focus to the new row", async () => {
    const { draft } = await openRequest();
    await showSection(/^Headers/);
    let rows = tableRows(strings.editor.headers);
    expect(rows).toHaveLength(2);

    await fireEvent.input(within(rows[1]).getByRole("textbox", { name: strings.table.key }), { target: { value: "X" } });

    expect(draft().headers).toEqual([
      { key: "Accept", value: "application/json", enabled: true, postman_extra: {} },
      { key: "X", value: "", enabled: true, postman_extra: {} },
    ]);
    rows = tableRows(strings.editor.headers);
    expect(rows).toHaveLength(3);
    const added = within(rows[1]).getByRole<HTMLInputElement>("textbox", { name: strings.table.key });
    await waitFor(() => expect(document.activeElement).toBe(added));
    // The empty row is empty again.
    expect(within(rows[2]).getByRole<HTMLInputElement>("textbox", { name: strings.table.key }).value).toBe("");
    expect(screen.getByRole("tab", { name: /^Headers/ }).textContent).toMatch(/Headers\s*2/);
  });

  it("puts a raw JSON body typed in the editor into the draft", async () => {
    const { draft } = await openRequest();
    await showSection(/^Body/);
    expect(draft().body).toEqual({ mode: "none" });

    await fireEvent.change(screen.getByRole("combobox", { name: strings.body.kind }), { target: { value: "raw" } });
    await fireEvent.change(screen.getByRole("combobox", { name: strings.body.language }), { target: { value: "json" } });
    const editor = screen.getByRole<HTMLTextAreaElement>("textbox", { name: strings.body.text });
    await fireEvent.input(editor, { target: { value: '{"name":"Ann"}' } });

    expect(draft().body).toEqual({ mode: "raw", data: { text: '{"name":"Ann"}', language: "json" } });
    expect(editor.dataset.language).toBe("json");
  });

  it("offers Beautify only for a raw JSON body", async () => {
    await openRequest({ method: "POST", url: "u", body: { mode: "raw", data: { text: "{}", language: "text" } } });
    await showSection(/^Body/);
    expect(screen.queryByRole("button", { name: strings.body.beautify })).toBeNull();

    await fireEvent.change(screen.getByRole("combobox", { name: strings.body.language }), { target: { value: "json" } });
    expect(screen.getByRole("button", { name: strings.body.beautify })).toBeTruthy();

    await fireEvent.change(screen.getByRole("combobox", { name: strings.body.kind }), { target: { value: "url_encoded" } });
    expect(screen.queryByRole("button", { name: strings.body.beautify })).toBeNull();
  });

  it("replaces the text of the body with the one the core formatted", async () => {
    const { api, draft } = await openRequest({ method: "POST", url: "u", body: { mode: "raw", data: { text: '{"a":1}', language: "json" } } });
    const formatJson = vi.spyOn(api, "formatJson").mockResolvedValue('{\n  "a": 1\n}');
    await showSection(/^Body/);
    await fireEvent.input(screen.getByRole("textbox", { name: strings.body.text }), { target: { value: '{"a":1 }' } });

    await fireEvent.click(screen.getByRole("button", { name: strings.body.beautify }));

    await waitFor(() => expect(draft().body).toEqual({ mode: "raw", data: { text: '{\n  "a": 1\n}', language: "json" } }));
    expect(formatJson).toHaveBeenCalledTimes(1);
    expect(formatJson).toHaveBeenCalledWith('{"a":1 }');
    expect(screen.getByRole<HTMLTextAreaElement>("textbox", { name: strings.body.text }).value).toBe('{\n  "a": 1\n}');
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("leaves the text and explains when the body is not valid JSON", async () => {
    const text = '{"id": {{id}}}';
    const { api, draft } = await openRequest({ method: "POST", url: "u", body: { mode: "raw", data: { text, language: "json" } } });
    const formatJson = vi.spyOn(api, "formatJson");
    await showSection(/^Body/);

    await fireEvent.click(screen.getByRole("button", { name: strings.body.beautify }));

    expect((await screen.findByRole("alert")).textContent).toContain(strings.body.notJson);
    expect(formatJson).toHaveBeenCalledWith(text);
    expect(draft().body).toEqual({ mode: "raw", data: { text, language: "json" } });
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("edits the fields of an x-www-form-urlencoded body", async () => {
    const { draft } = await openRequest({ method: "POST", url: "u", body: { mode: "url_encoded", data: [{ key: "a", value: "1" }] } });
    await showSection(/^Body/);
    expect(screen.getByRole<HTMLSelectElement>("combobox", { name: strings.body.kind }).value).toBe("url_encoded");

    const rows = tableRows(strings.body.fields);
    await fireEvent.input(within(rows[0]).getByRole("textbox", { name: strings.table.value }), { target: { value: "2" } });

    expect(draft().body).toEqual({ mode: "url_encoded", data: [{ key: "a", value: "2", enabled: true, postman_extra: {} }] });
  });

  it("brings the text back when the kind of the body is switched away and back", async () => {
    const { draft } = await openRequest({ method: "POST", url: "u", body: { mode: "raw", data: { text: "hello", language: "text" } } });
    await showSection(/^Body/);
    const kind = screen.getByRole("combobox", { name: strings.body.kind });

    await fireEvent.change(kind, { target: { value: "none" } });
    expect(draft().body).toEqual({ mode: "none" });
    await fireEvent.change(kind, { target: { value: "raw" } });

    expect(draft().body).toEqual({ mode: "raw", data: { text: "hello", language: "text" } });
  });

  it("shows a form-data body read-only with a note and asks before replacing it", async () => {
    const formData: RequestContent = {
      method: "POST",
      url: "u",
      body: { mode: "form_data", data: [{ key: "avatar", value: { files: [{ path: "C:/pic.png" }] } }, { key: "name", value: { text: "Ann" } }] },
    };
    const { draft } = await openRequest(formData);
    await showSection(/^Body/);
    const kind = screen.getByRole<HTMLSelectElement>("combobox", { name: strings.body.kind });

    expect(kind.value).toBe("form_data");
    expect(screen.getByText(strings.body.readOnly)).toBeTruthy();
    expect(screen.getByText("avatar")).toBeTruthy();
    expect(screen.getByText("Ann")).toBeTruthy();
    // Nothing of the body can be typed into.
    expect(screen.queryByRole("textbox", { name: strings.table.value })).toBeNull();

    // Cancel leaves the body and the choice as they were.
    await fireEvent.change(kind, { target: { value: "raw" } });
    let dialog = await screen.findByRole("dialog", { name: strings.body.replaceTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.body.cancel }));
    expect(draft().body.mode).toBe("form_data");
    expect(kind.value).toBe("form_data");
    expect(screen.queryByLabelText(strings.tabs.unsaved)).toBeNull();

    await fireEvent.change(kind, { target: { value: "raw" } });
    dialog = await screen.findByRole("dialog", { name: strings.body.replaceTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.body.replace }));
    expect(draft().body).toEqual({ mode: "raw", data: { text: "", language: "text" } });
    expect(screen.queryByText(strings.body.readOnly)).toBeNull();
  });

  it("sets a Bearer token as the core serializes it", async () => {
    const { draft } = await openRequest();
    await showSection(/^Auth/);
    const type = screen.getByRole<HTMLSelectElement>("combobox", { name: strings.auth.type });
    expect(type.value).toBe("inherit");

    await fireEvent.change(type, { target: { value: "bearer" } });
    const token = screen.getByLabelText<HTMLInputElement>(strings.auth.token);
    await fireEvent.input(token, { target: { value: "abc" } });

    expect(draft().auth).toEqual({ type: "bearer", token: "abc" });
    // The token is masked until the button is pressed.
    expect(token.type).toBe("password");
    const show = screen.getByRole("button", { name: `${strings.table.show}: ${strings.auth.token}` });
    expect(show.title).toBe(`${strings.table.show}: ${strings.auth.token}`);
    expect(show.textContent!.trim()).toBe("");
    await fireEvent.click(show);
    expect(token.type).toBe("text");
    expect(screen.getByRole("button", { name: `${strings.table.hide}: ${strings.auth.token}` }).title).toBe(
      `${strings.table.hide}: ${strings.auth.token}`,
    );
  });

  it("sets auth to null when the request inherits it", async () => {
    const { draft } = await openRequest({ ...BASE, auth: { type: "basic", username: "u", password: "p" } });
    await showSection(/^Auth/);
    const type = screen.getByRole<HTMLSelectElement>("combobox", { name: strings.auth.type });
    expect(type.value).toBe("basic");
    expect(screen.getByLabelText<HTMLInputElement>(strings.auth.username).value).toBe("u");
    expect(screen.getByLabelText<HTMLInputElement>(strings.auth.password).type).toBe("password");

    await fireEvent.change(type, { target: { value: "inherit" } });
    expect(draft().auth).toBeNull();

    await fireEvent.change(type, { target: { value: "no_auth" } });
    expect(draft().auth).toEqual({ type: "no_auth" });
  });

  it("fills an API key and an OAuth 2.0 token", async () => {
    const { draft } = await openRequest();
    await showSection(/^Auth/);
    const type = screen.getByRole("combobox", { name: strings.auth.type });

    await fireEvent.change(type, { target: { value: "api_key" } });
    // The tables of Params have fields with the same names.
    const panel = within(screen.getByRole("tabpanel", { name: /^Auth/ }));
    await fireEvent.input(panel.getByLabelText(strings.auth.key), { target: { value: "X-Key" } });
    await fireEvent.input(panel.getByLabelText(strings.auth.value), { target: { value: "v" } });
    await fireEvent.change(screen.getByRole("combobox", { name: strings.auth.placement }), { target: { value: "query" } });
    expect(draft().auth).toEqual({ type: "api_key", key: "X-Key", value: "v", placement: "query" });

    await fireEvent.change(type, { target: { value: "oauth2_token" } });
    await fireEvent.input(screen.getByLabelText(strings.auth.token), { target: { value: "tok" } });
    expect(draft().auth).toEqual({ type: "oauth2_token", token: "tok", header_prefix: "Bearer", placement: "header" });
  });

  it("names an unsupported authorization and lets only another type be chosen", async () => {
    const { draft } = await openRequest({ ...BASE, auth: { type: "unsupported", auth_type: "digest" } });
    await showSection(/^Auth/);
    const type = screen.getByRole<HTMLSelectElement>("combobox", { name: strings.auth.type });

    expect(type.value).toBe("unsupported");
    expect(type.selectedOptions[0].textContent).toContain("digest");
    expect(type.selectedOptions[0].textContent).toContain(strings.auth.unsupported);
    expect(draft().auth).toEqual({ type: "unsupported", auth_type: "digest" });

    await fireEvent.change(type, { target: { value: "no_auth" } });
    expect(draft().auth).toEqual({ type: "no_auth" });
    // Once left, the unsupported type cannot be chosen again.
    expect(within(type).getAllByRole<HTMLOptionElement>("option").map((option) => option.value)).not.toContain("unsupported");
  });

  it("cancels the default action of Ctrl+S, F5 and Ctrl+R", async () => {
    await openRequest();

    expect((await press(CTRL_S)).defaultPrevented).toBe(true);
    expect((await press({ key: "F5", code: "F5" })).defaultPrevented).toBe(true);
    expect((await press({ key: "r", code: "KeyR", ctrlKey: true })).defaultPrevented).toBe(true);
    // A key of no combination is left alone.
    expect((await press({ key: "s", code: "KeyS" })).defaultPrevented).toBe(false);
  });

  it("does not let a combination reach the listeners below", async () => {
    await openRequest();
    const below = vi.fn();
    document.body.addEventListener("keydown", below);
    try {
      await press(CTRL_S);
      expect(below).not.toHaveBeenCalled();
      await press({ key: "a", code: "KeyA" });
      expect(below).toHaveBeenCalledTimes(1);
    } finally {
      document.body.removeEventListener("keydown", below);
    }
  });

  it("moves the focus to the URL field with Ctrl+L", async () => {
    await openRequest();
    screen.getByRole<HTMLElement>("tab", { name: /^Headers/ }).focus();

    const event = await press({ key: "l", code: "KeyL", ctrlKey: true });

    expect(event.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(urlInput());
  });

  it("asks before closing a tab with unsaved edits; Cancel leaves the tab", async () => {
    const { api, draft } = await openRequest();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    await fireEvent.click(screen.getByRole("button", { name: `${strings.tabs.close}: List users` }));
    const dialog = await screen.findByRole("dialog", { name: strings.tabs.closeTitle });

    expect(within(dialog).getAllByRole("button").map((button) => button.textContent!.trim())).toEqual([
      strings.tabs.save,
      strings.tabs.discard,
      strings.tabs.cancel,
    ]);

    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tabs.cancel }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("tab", { name: /List users/ })).toBeTruthy();
    expect(draft().url).toBe("https://api.test/people");
    expect(saveNodeContent).not.toHaveBeenCalled();
  });

  it("closes without saving on request", async () => {
    const { api } = await openRequest();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    await press(CTRL_W);
    const dialog = await screen.findByRole("dialog", { name: strings.tabs.closeTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tabs.discard }));

    expect(screen.queryByRole("tab", { name: /List users/ })).toBeNull();
    expect(saveNodeContent).not.toHaveBeenCalled();
  });

  it("saves and closes on request", async () => {
    const { api, tab } = await openRequest();
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    // The middle mouse button.
    await fireEvent(tab, new MouseEvent("auxclick", { bubbles: true, button: 1 }));
    const dialog = await screen.findByRole("dialog", { name: strings.tabs.closeTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tabs.save }));

    await waitFor(() => expect(screen.queryByRole("tab", { name: /List users/ })).toBeNull());
    expect(saveNodeContent).toHaveBeenCalledWith("req", expect.objectContaining({ url: "https://api.test/people" }));
  });

  it("leaves the tab open when saving before the close fails", async () => {
    const { api } = await openRequest();
    vi.spyOn(api, "saveNodeContent").mockRejectedValue("disk is full");
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    await press(CTRL_W);
    const dialog = await screen.findByRole("dialog", { name: strings.tabs.closeTitle });
    await fireEvent.click(within(dialog).getByRole("button", { name: strings.tabs.save }));

    await screen.findByRole("alert");
    expect(screen.getByRole("tab", { name: /List users/ })).toBeTruthy();
    expect(screen.getByLabelText(strings.tabs.unsaved)).toBeTruthy();
  });

  it("closes a tab without edits at once", async () => {
    await openRequest();

    await press(CTRL_W);

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("tab", { name: /List users/ })).toBeNull();
  });

  it("keeps the draft of a tab that is not active", async () => {
    const { api } = await openRequest();
    await fireEvent.input(urlInput(), { target: { value: "https://api.test/people" } });

    await tabs.open(api, "other");
    await screen.findByRole("tab", { name: /Other/ });
    expect(urlInput().value).toBe("https://api.test/other");
    // The first tab keeps its mark while the other one is shown.
    expect(within(screen.getByRole("tab", { name: /List users/ })).getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    await fireEvent.click(screen.getByRole("tab", { name: /List users/ }));
    expect(urlInput().value).toBe("https://api.test/people");
  });

  it("opens a request chosen in the tree", async () => {
    const api = createFakeApi({ nodes: nodes() });
    render(App, { context: new Map([["api", api]]) });

    await fireEvent.click(await screen.findByRole("treeitem", { name: "API" }));

    expect(await screen.findByRole("tab", { name: "API" })).toBeTruthy();
  });
});
