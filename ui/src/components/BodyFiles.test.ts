import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { tick } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { createFakeApi } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import type { Body, FormPart, NodeDto, RequestDraft } from "../lib/types";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const TEXT = strings.body;

function nodes(body: Body | undefined): NodeDto[] {
  return [
    { id: "col", parent_id: null, kind: "collection", name: "API", content: {} },
    { id: "req", parent_id: "col", kind: "request", name: "Upload", content: { method: "POST", url: "https://api.test/upload", body } },
  ];
}

function formData(...parts: FormPart[]): Body {
  return { mode: "form_data", data: parts };
}

/**
 * Renders the window over a fake api whose active environment has the variable `host`, opens the request `req`
 * with this body in a tab and shows its Body section. `picked` — what the window that chooses files answers.
 */
async function openBody(body?: Body, picked: string[] = []) {
  const api = createFakeApi({
    nodes: nodes(body),
    environments: [{ id: "env-dev", name: "Dev" }],
    activeEnvironment: "env-dev",
    variables: { "env-dev": [{ key: "host", value: "dev.example.test" }] },
  });
  api.filesToPick = picked;
  const pickFiles = vi.spyOn(api, "pickFiles");
  render(App, { context: new Map([["api", api]]) });
  await screen.findByRole("treeitem", { name: "API" });
  await tabs.open(api, "req");
  const tab = await screen.findByRole("tab", { name: /Upload/ });
  await fireEvent.click(screen.getByRole("tab", { name: /^Body/ }));
  const current = () => tabs.active as Tab<RequestDraft>;
  return { api, pickFiles, tab, current, body: () => current().draft.body };
}

async function chooseKind(mode: Body["mode"]) {
  await fireEvent.change(screen.getByRole("combobox", { name: TEXT.kind }), { target: { value: mode } });
}

/** Rows of the table of the parts without the header: the parts and the empty row at the end. */
function partRows(): HTMLElement[] {
  return within(screen.getByRole("table", { name: TEXT.parts })).getAllByRole("row").slice(1);
}

function keyOf(row: HTMLElement): HTMLInputElement {
  return within(row).getByRole("textbox", { name: strings.table.key });
}

function textOf(row: HTMLElement): HTMLInputElement {
  return within(row).getByRole("textbox", { name: strings.table.value });
}

function contentTypeOf(row: HTMLElement): HTMLInputElement {
  return within(row).getByRole("textbox", { name: TEXT.contentType });
}

function kindOf(row: HTMLElement): HTMLSelectElement {
  return within(row).getByRole("combobox", { name: TEXT.partKind });
}

/** The layer that draws the variables of the field; `null` — the field is a plain one. */
function layerOf(input: HTMLElement): HTMLElement | null {
  return input.parentElement!.querySelector<HTMLElement>(".var-layer");
}

const TWO_UNCONFIRMED: FormPart = {
  key: "docs",
  value: { files: [{ path: "C:/import/a.txt" }, { path: "C:/import/b.txt" }] },
  postman_extra: { type: "file" },
};

/** A plain copy of a piece of the draft: the draft itself is reactive state, which cannot be cloned. */
function copy<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

// The tabs and the notifications of the window live in modules; every test starts without them.
afterEach(() => {
  for (const tab of [...tabs.list]) tabs.close(tab.id);
  toasts.items = [];
});

describe("FormDataEditor", () => {
  it("adds a text part when a key is typed into the empty row", async () => {
    const { body, tab } = await openBody();
    await chooseKind("form_data");
    expect(body()).toEqual({ mode: "form_data", data: [] });
    expect(partRows()).toHaveLength(1);

    await fireEvent.input(keyOf(partRows()[0]), { target: { value: "note" } });

    expect(body()).toEqual({
      mode: "form_data",
      data: [{ key: "note", value: { text: "" }, enabled: true, content_type: null, postman_extra: {} }],
    });
    expect(partRows()).toHaveLength(2);
    expect(keyOf(partRows()[0]).value).toBe("note");
    expect(kindOf(partRows()[0]).value).toBe("text");
    expect(within(tab).getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    await fireEvent.input(textOf(partRows()[0]), { target: { value: "hello" } });
    expect(body()).toMatchObject({ data: [{ key: "note", value: { text: "hello" } }] });
  });

  it("keeps the key, the switch, the Content-Type and postman_extra when the kind of a part changes", async () => {
    const part: FormPart = {
      key: "doc",
      value: { text: "typed" },
      enabled: false,
      content_type: "text/plain",
      postman_extra: { description: "d", type: "text" },
    };
    const { body } = await openBody(formData(part));
    const rest = { key: "doc", enabled: false, content_type: "text/plain", postman_extra: { description: "d", type: "text" } };

    await fireEvent.change(kindOf(partRows()[0]), { target: { value: "file" } });
    expect(body()).toEqual({ mode: "form_data", data: [{ ...rest, value: { files: [] } }] });
    expect(keyOf(partRows()[0]).value).toBe("doc");
    expect(within(partRows()[0]).queryByRole("textbox", { name: strings.table.value })).toBeNull();

    await fireEvent.change(kindOf(partRows()[0]), { target: { value: "text" } });
    expect(body()).toEqual({ mode: "form_data", data: [{ ...rest, value: { text: "" } }] });
  });

  it("replaces the files of a part with the chosen ones, confirmed", async () => {
    const { body, pickFiles } = await openBody(formData(TWO_UNCONFIRMED), ["C:/picked/one.png", "C:/picked/two.png"]);

    await fireEvent.click(within(partRows()[0]).getByRole("button", { name: TEXT.chooseFiles }));

    await waitFor(() =>
      expect(body()).toMatchObject({
        data: [
          {
            key: "docs",
            value: {
              files: [
                { path: "C:/picked/one.png", confirmed: true },
                { path: "C:/picked/two.png", confirmed: true },
              ],
            },
            postman_extra: { type: "file" },
          },
        ],
      }),
    );
    expect(pickFiles).toHaveBeenCalledExactlyOnceWith(true);
    expect(within(partRows()[0]).getByText("one.png").title).toBe("C:/picked/one.png");
    expect(within(partRows()[0]).queryByText(TEXT.fileUnconfirmed)).toBeNull();
  });

  it("changes nothing when the window that chooses files is closed", async () => {
    const { body, pickFiles, tab } = await openBody(formData(TWO_UNCONFIRMED), []);
    const before = copy(body());

    await fireEvent.click(within(partRows()[0]).getByRole("button", { name: TEXT.chooseFiles }));
    await waitFor(() => expect(pickFiles).toHaveBeenCalledTimes(1));
    await tick();

    expect(body()).toEqual(before);
    expect(within(tab).queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("shows every file of an imported part with the mark and removes one of them", async () => {
    const { body } = await openBody(formData(TWO_UNCONFIRMED));
    const row = partRows()[0];

    expect(kindOf(row).value).toBe("file");
    expect(within(row).getByText("a.txt").title).toBe("C:/import/a.txt");
    expect(within(row).getByText("b.txt").title).toBe("C:/import/b.txt");
    expect(within(row).getAllByText(TEXT.fileUnconfirmed)).toHaveLength(2);

    await fireEvent.click(within(row).getByRole("button", { name: TEXT.removeFile("a.txt") }));

    expect(body()).toMatchObject({ data: [{ value: { files: [{ path: "C:/import/b.txt", confirmed: false }] } }] });
    expect(within(partRows()[0]).queryByText("a.txt")).toBeNull();
    expect(within(partRows()[0]).getAllByText(TEXT.fileUnconfirmed)).toHaveLength(1);
  });

  it("takes an emptied Content-Type for none: the tab is not changed", async () => {
    const { body, tab } = await openBody(formData({ key: "note", value: { text: "hello" } }));
    const contentType = () => contentTypeOf(partRows()[0]);
    expect(contentType().value).toBe("");

    await fireEvent.input(contentType(), { target: { value: "application/json" } });
    expect(body()).toMatchObject({ data: [{ content_type: "application/json" }] });
    expect(within(tab).getByLabelText(strings.tabs.unsaved)).toBeTruthy();

    await fireEvent.input(contentType(), { target: { value: "" } });
    expect(body()).toMatchObject({ data: [{ content_type: null }] });
    expect(within(tab).queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("removes a part", async () => {
    const { body } = await openBody(formData({ key: "a", value: { text: "1" } }, { key: "b", value: { text: "2" } }));

    await fireEvent.click(within(partRows()[0]).getByRole("button", { name: strings.table.remove }));

    expect(body()).toMatchObject({ data: [{ key: "b" }] });
    expect(partRows()).toHaveLength(2);
  });

  it("marks the variables in the key and the text of a text part and in the key of a file part alone", async () => {
    await openBody(
      formData({ key: "{{host}}-key", value: { text: "{{host}}/v" } }, { key: "{{host}}-file", value: { files: [{ path: "C:/{{host}}.txt" }] } }),
    );
    const [text, file] = partRows();
    const marked = (input: HTMLElement) => layerOf(input)?.querySelector("span.var-environment")?.textContent;

    await waitFor(() => expect(marked(keyOf(text))).toBe("{{host}}"));
    expect(marked(textOf(text))).toBe("{{host}}");
    expect(marked(keyOf(file))).toBe("{{host}}");
    // The files of a part are not a text with variables.
    const valueCell = within(file).getByRole("button", { name: TEXT.chooseFiles }).closest("td")!;
    expect(valueCell.querySelector(".var-layer")).toBeNull();
    expect(valueCell.querySelector("span.var")).toBeNull();
  });
});

describe("BinaryBodyEditor", () => {
  it("chooses the file and takes it away", async () => {
    const { body, pickFiles } = await openBody(undefined, ["C:/picked/data.bin"]);
    await chooseKind("binary");
    expect(body()).toEqual({ mode: "binary", data: { file: null } });
    expect(screen.getByText(TEXT.fileNotChosen)).toBeTruthy();
    expect(screen.queryByRole("button", { name: TEXT.removeChosen })).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: TEXT.chooseFile }));

    await waitFor(() => expect(body()).toEqual({ mode: "binary", data: { file: { path: "C:/picked/data.bin", confirmed: true } } }));
    expect(pickFiles).toHaveBeenCalledExactlyOnceWith(false);
    expect(screen.getByText("C:/picked/data.bin")).toBeTruthy();
    expect(screen.queryByText(TEXT.fileUnconfirmed)).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: TEXT.removeChosen }));
    expect(body()).toEqual({ mode: "binary", data: { file: null } });
    expect(screen.getByText(TEXT.fileNotChosen)).toBeTruthy();
  });

  it("marks an imported file and keeps it when the window that chooses is closed", async () => {
    const { body, pickFiles, tab } = await openBody({ mode: "binary", data: { file: { path: "C:/import/data.bin" } } }, []);

    expect(screen.getByText("C:/import/data.bin")).toBeTruthy();
    expect(screen.getByText(TEXT.fileUnconfirmed)).toBeTruthy();

    await fireEvent.click(screen.getByRole("button", { name: TEXT.chooseFile }));
    await waitFor(() => expect(pickFiles).toHaveBeenCalledTimes(1));
    await tick();

    expect(body()).toEqual({ mode: "binary", data: { file: { path: "C:/import/data.bin", confirmed: false } } });
    expect(within(tab).queryByLabelText(strings.tabs.unsaved)).toBeNull();
  });

  it("has no field to type a path into", async () => {
    await openBody({ mode: "binary", data: { file: null } });
    expect(within(screen.getByRole("tabpanel", { name: /^Body/ })).queryByRole("textbox")).toBeNull();
  });
});

describe("BodyEditor with files", () => {
  it("gives back what was entered in a kind when the user returns to it", async () => {
    const { body } = await openBody(formData({ key: "note", value: { text: "hello" } }), ["C:/picked/data.bin"]);
    const parts = copy(body());

    await chooseKind("binary");
    await fireEvent.click(screen.getByRole("button", { name: TEXT.chooseFile }));
    await waitFor(() => expect(body()).toMatchObject({ data: { file: { confirmed: true } } }));
    const binary = copy(body());

    await chooseKind("raw");
    expect(body()).toEqual({ mode: "raw", data: { text: "", language: "text" } });

    await chooseKind("form_data");
    expect(body()).toEqual(parts);
    expect(keyOf(partRows()[0]).value).toBe("note");

    await chooseKind("binary");
    expect(body()).toEqual(binary);
  });

  it("gives the saved body, not the entered one, after the edits of the tab were thrown away", async () => {
    const saved: Body = { mode: "raw", data: { text: "saved text", language: "text" } };
    const { body, current } = await openBody(saved);

    // A part is entered and its kind is left: the editor keeps it for the way back.
    await chooseKind("form_data");
    await fireEvent.input(keyOf(partRows()[0]), { target: { value: "typed" } });
    expect(body()).toMatchObject({ data: [{ key: "typed" }] });
    await chooseKind("raw");
    await fireEvent.input(screen.getByRole("textbox", { name: TEXT.text }), { target: { value: "typed text" } });
    expect(body()).toEqual({ mode: "raw", data: { text: "typed text", language: "text" } });

    current().revert();
    await tick();
    expect(body()).toEqual(saved);

    // The part entered before the revert does not come back, and neither does the text.
    await chooseKind("form_data");
    expect(body()).toEqual({ mode: "form_data", data: [] });
    expect(partRows()).toHaveLength(1);
    await chooseKind("raw");
    expect(body()).toEqual(saved);
  });
});
