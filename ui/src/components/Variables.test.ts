import { createEvent, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { tick } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import KeyValueTable from "./KeyValueTable.svelte";
import type { Api } from "../lib/api";
import { motion } from "../lib/motion.svelte";
import { strings } from "../lib/strings";
import { tabs, type Tab } from "../lib/tabs.svelte";
import { createFakeApi, type FakeSeed } from "../lib/testing/fakeApi";
import { toasts } from "../lib/toasts.svelte";
import type { CollectionDraft, NodeDto, RequestContent, RequestDraft, ScopeVariable } from "../lib/types";
import { bumpVariables, VariableScope } from "../lib/variableScope.svelte";
import { workspace } from "../lib/workspace.svelte";

vi.mock("./CodeEditor.svelte", async () => ({ default: (await import("../lib/testing/CodeEditorStub.svelte")).default }));

const TEXT = strings.variables;
const SECRET_VALUE = "s3cret-value";

const REQUEST: RequestContent = { method: "GET", url: "{{host}}/x/{{nope}}" };

function nodes(content: RequestContent): NodeDto[] {
  return [
    {
      id: "col",
      parent_id: null,
      kind: "collection",
      name: "API",
      content: {
        variables: [
          { key: "base", value: "collection.test" },
          { key: "password", value: "hunter2", secret: true },
        ],
      },
    },
    { id: "req", parent_id: "col", kind: "request", name: "List users", content },
    { id: "other", parent_id: "col", kind: "request", name: "Other", content: { method: "POST", url: "{{host}}/other" } },
  ];
}

/** "Dev" is active and has `host`; without it `host` is the global one. */
function seed(content: RequestContent): FakeSeed {
  return {
    nodes: nodes(content),
    environments: [
      { id: "env-dev", name: "Dev" },
      { id: "env-prod", name: "Production" },
    ],
    activeEnvironment: "env-dev",
    variables: {
      "env-dev": [
        { key: "host", value: "dev.example.test" },
        { key: "token", value: SECRET_VALUE, secret: true },
      ],
      globals: [
        { key: "host", value: "global.example.test" },
        { key: "tenant", value: "acme" },
      ],
    },
  };
}

/**
 * Renders the window over a fake api, waits until it has read the tree and the environments, and opens the
 * request `req` in a tab. `prepare` changes the api before anything is read from it.
 */
async function openRequest(content: RequestContent = REQUEST, prepare: (api: Api) => void = () => {}) {
  const api = createFakeApi(seed(content));
  prepare(api);
  const variablesInScope = vi.spyOn(api, "variablesInScope");
  render(App, { context: new Map([["api", api]]) });
  await screen.findByRole("treeitem", { name: "API" });
  await tabs.open(api, "req");
  await screen.findByRole("tab", { name: /List users/ });
  return { api, variablesInScope, draft: () => (tabs.active as Tab<RequestDraft>).draft };
}

/** The URL field of the shown tab. */
function urlInput(): HTMLInputElement {
  const panel = document.querySelector<HTMLElement>("[data-tab-panel]:not([hidden])")!;
  return within(panel).getByRole("textbox", { name: strings.editor.url });
}

/** The layer under the field; `null` — the field is a plain one. */
function layerOf(input: HTMLElement): HTMLElement | null {
  return input.parentElement!.querySelector<HTMLElement>(".var-layer");
}

/** The token of the layer of the field with this text. */
function tokenOf(input: HTMLElement, text: string): HTMLElement {
  const token = [...layerOf(input)!.querySelectorAll<HTMLElement>("span.var")].find((span) => span.textContent === text);
  if (!token) throw new Error(`no token ${text} in the layer`);
  return token;
}

/** Waits until the token of the field has the class. */
async function tokenHasClass(input: () => HTMLElement, text: string, className: string) {
  await waitFor(() => expect(tokenOf(input(), text).classList.contains(className)).toBe(true));
}

/** Types the text into the field as the user does: the caret stands at its end. */
async function type(input: HTMLInputElement, value: string) {
  await fireEvent.input(input, { target: { value } });
}

/** Puts the pointer over the token: the place of the token on the screen is made up, jsdom has no layout. */
async function hover(input: HTMLInputElement, text: string) {
  const token = tokenOf(input, text);
  token.getBoundingClientRect = () => new DOMRect(40, 10, 60, 20);
  await fireEvent.mouseMove(input, { clientX: 50, clientY: 15 });
}

async function showSection(name: RegExp): Promise<void> {
  await fireEvent.click(screen.getByRole("tab", { name }));
}

/** Rows of a key-value table without the header. */
function tableRows(tableName: string): HTMLElement[] {
  return within(screen.getByRole("table", { name: tableName })).getAllByRole("row").slice(1);
}

function cell(row: HTMLElement, name: string): HTMLInputElement {
  return within(row).getByRole("textbox", { name });
}

/** A promise settled from outside. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function scopeVariable(name: string, scope: ScopeVariable["scope"], value: string | null = "v", secret = false): ScopeVariable {
  return { name, scope, value, secret };
}

// The tabs, the notifications and the workspace of the window live in modules; every test starts without them.
afterEach(() => {
  for (const open of [...tabs.list]) tabs.close(open.id);
  toasts.items = [];
  workspace.rows = [];
  workspace.environments = [];
  workspace.activeEnvironmentId = null;
});

describe("variables in the URL", () => {
  it("colours a token by its scope, underlines one that is not found and counts it next to Send", async () => {
    await openRequest();
    const layer = layerOf(urlInput())!;

    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    expect(tokenOf(urlInput(), "{{nope}}").classList.contains("var-unresolved")).toBe(true);
    expect(layer.querySelectorAll("span.var")).toHaveLength(2);
    // The layer repeats the text of the field and is not read aloud a second time.
    expect(layer.tagName).toBe("DIV");
    expect(layer.getAttribute("aria-hidden")).toBe("true");
    expect(layer.textContent).toBe("{{host}}/x/{{nope}}");

    const counter = screen.getByText("Не найдено переменных: 1").closest<HTMLElement>('[role="status"]')!;
    expect(counter).toBeTruthy();
    expect(counter.title).toBe("nope");
    expect(counter.textContent).toContain("1");
    // The button is found by the name it always had and is not disabled.
    const send = screen.getByRole<HTMLButtonElement>("button", { name: strings.editor.send });
    expect(send.disabled).toBe(false);
    expect(send.contains(counter)).toBe(false);
  });

  it("has no counter while every variable is found, and follows the text of the field", async () => {
    const { draft } = await openRequest({ method: "GET", url: "{{host}}/x" });
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    expect(screen.queryByText(/Не найдено переменных/)).toBeNull();

    await type(urlInput(), "{{host}}/{{a}}/{{b}}/{{a}}");

    expect(draft().url).toBe("{{host}}/{{a}}/{{b}}/{{a}}");
    expect(layerOf(urlInput())!.textContent).toBe("{{host}}/{{a}}/{{b}}/{{a}}");
    const counter = screen.getByText("Не найдено переменных: 2").closest<HTMLElement>('[role="status"]')!;
    expect(counter.title).toBe("a, b");
  });

  it("does not count a variable that only a row switched off uses", async () => {
    await openRequest({
      method: "GET",
      url: "{{host}}",
      headers: [{ key: "X", value: "{{missing}}", enabled: false }],
    });
    await tokenHasClass(urlInput, "{{host}}", "var-environment");

    expect(screen.queryByText(/Не найдено переменных/)).toBeNull();
  });

  it("colours the scopes differently: global, collection, dynamic", async () => {
    await openRequest({ method: "GET", url: "{{tenant}}{{base}}{{$guid}}{{$nope}}" });

    await tokenHasClass(urlInput, "{{tenant}}", "var-global");
    expect(tokenOf(urlInput(), "{{base}}").classList.contains("var-collection")).toBe(true);
    expect(tokenOf(urlInput(), "{{$guid}}").classList.contains("var-dynamic")).toBe(true);
    expect(tokenOf(urlInput(), "{{$nope}}").classList.contains("var-unresolved")).toBe(true);
    expect(screen.getByText("Не найдено переменных: 1")).toBeTruthy();
  });

  it("keeps Ctrl+L working: the field of the address is still an input", async () => {
    await openRequest();
    screen.getByRole<HTMLElement>("tab", { name: /^Headers/ }).focus();

    await fireEvent(document.body, createEvent.keyDown(document.body, { key: "l", code: "KeyL", ctrlKey: true }));

    expect(document.activeElement).toBe(urlInput());
    expect(urlInput().tagName).toBe("INPUT");
    expect(urlInput().type).toBe("text");
  });
});

describe("the hint of a variable", () => {
  it("shows the scope in words and the value when the pointer is over a token", async () => {
    await openRequest();
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    expect(screen.queryByRole("tooltip")).toBeNull();

    await hover(urlInput(), "{{host}}");

    const tooltip = screen.getByRole("tooltip");
    expect(tooltip.textContent).toContain("host");
    expect(tooltip.textContent).toContain("Окружение");
    expect(tooltip.textContent).toContain("dev.example.test");

    // Away from the token, and then away from the field.
    await fireEvent.mouseMove(urlInput(), { clientX: 500, clientY: 15 });
    expect(screen.queryByRole("tooltip")).toBeNull();
    await hover(urlInput(), "{{host}}");
    expect(screen.getByRole("tooltip")).toBeTruthy();
    await fireEvent.mouseLeave(urlInput());
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("says that a variable is not found", async () => {
    await openRequest();
    await tokenHasClass(urlInput, "{{nope}}", "var-unresolved");

    await hover(urlInput(), "{{nope}}");

    expect(screen.getByRole("tooltip").textContent).toContain("Переменная не найдена");
    expect(screen.getByRole("tooltip").textContent).toContain("nope");
  });

  it("shows a mask instead of the value of a secret variable", async () => {
    await openRequest({ method: "GET", url: "{{token}}" }, (api) => {
      api.variablesInScope = async () => [scopeVariable("token", "environment", null, true)];
    });
    await tokenHasClass(urlInput, "{{token}}", "var-environment");

    await hover(urlInput(), "{{token}}");

    expect(screen.getByRole("tooltip").textContent).toContain("••••••");
    expect(screen.getByRole("tooltip").textContent).toContain("Окружение");
    expect(TEXT.secretMask).toBe("••••••");
  });

  it("never gets the value of a secret variable from the fake core either", async () => {
    const { api } = await openRequest({ method: "GET", url: "{{token}}" });
    await tokenHasClass(urlInput, "{{token}}", "var-environment");

    await hover(urlInput(), "{{token}}");
    expect(screen.getByRole("tooltip").textContent).toContain("••••••");
    await type(urlInput(), "{{to");
    expect(screen.getByRole("listbox").textContent).toContain("••••••");

    expect(document.body.textContent).not.toContain(SECRET_VALUE);
    expect(JSON.stringify(await api.variablesInScope("req"))).not.toContain(SECRET_VALUE);
  });

  it("explains the value of a dynamic variable", async () => {
    await openRequest({ method: "GET", url: "{{$guid}}" });
    await tokenHasClass(urlInput, "{{$guid}}", "var-dynamic");

    await hover(urlInput(), "{{$guid}}");

    expect(screen.getByRole("tooltip").textContent).toContain("Динамическая");
    expect(screen.getByRole("tooltip").textContent).toContain("значение создаётся при отправке");
  });

  it("has no hint until the variables are read: nothing is called not found meanwhile", async () => {
    const answer = deferred<ScopeVariable[]>();
    await openRequest(REQUEST, (api) => {
      api.variablesInScope = () => answer.promise;
    });

    await hover(urlInput(), "{{host}}");
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(urlInput().hasAttribute("aria-describedby")).toBe(false);

    answer.resolve([scopeVariable("host", "environment", "dev.example.test")]);
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    await hover(urlInput(), "{{host}}");
    expect(screen.getByRole("tooltip").textContent).toContain("dev.example.test");
  });

  it("has no hint after the variables could not be read", async () => {
    await openRequest(REQUEST, (api) => {
      api.variablesInScope = () => Promise.reject("scope failed");
    });
    await screen.findByRole("alert");

    await hover(urlInput(), "{{nope}}");

    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("takes the hint away when the variables shown in it are gone", async () => {
    let fail = false;
    await openRequest(REQUEST, (api) => {
      const read = api.variablesInScope.bind(api);
      api.variablesInScope = (nodeId) => (fail ? Promise.reject("scope failed") : read(nodeId));
    });
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    await hover(urlInput(), "{{host}}");
    expect(screen.getByRole("tooltip")).toBeTruthy();

    fail = true;
    bumpVariables();

    await screen.findByRole("alert");
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});

describe("autocompletion of a variable", () => {
  it("opens the list after {{ and the beginning of a name, and Enter puts the name in", async () => {
    const { draft } = await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    input.focus();
    expect(screen.queryByRole("listbox")).toBeNull();

    await type(input, "{{h");

    const list = screen.getByRole("listbox");
    const options = within(list).getAllByRole("option");
    expect(options[0].textContent).toContain("host");
    expect(options[0].textContent).toContain("Окружение");
    expect(options[0].textContent).toContain("dev.example.test");
    // The focus stays in the field; the chosen item is named to a screen reader through the field.
    expect(document.activeElement).toBe(input);
    expect(options[0].id).not.toBe("");
    expect(input.getAttribute("aria-activedescendant")).toBe(options[0].id);
    expect(options[0].getAttribute("aria-selected")).toBe("true");

    const enter = createEvent.keyDown(input, { key: "Enter" });
    await fireEvent(input, enter);

    expect(enter.defaultPrevented).toBe(true);
    expect(input.value).toBe("{{host}}");
    expect(draft().url).toBe("{{host}}");
    expect(input.selectionStart).toBe(8);
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(input.hasAttribute("aria-activedescendant")).toBe(false);
    expect(document.activeElement).toBe(input);
  });

  it("closes the list with Escape and leaves the text as it is", async () => {
    const { draft } = await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    await type(input, "{{h");
    expect(screen.getByRole("listbox")).toBeTruthy();

    await fireEvent.keyDown(input, { key: "Escape" });

    expect(screen.queryByRole("listbox")).toBeNull();
    expect(input.value).toBe("{{h");
    expect(draft().url).toBe("{{h");
    // Enter and Tab are the keys of the field again.
    const enter = createEvent.keyDown(input, { key: "Enter" });
    await fireEvent(input, enter);
    expect(enter.defaultPrevented).toBe(false);
    expect(input.value).toBe("{{h");
  });

  it("chooses with the arrows and puts the name in with Tab", async () => {
    await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    await type(input, "x={{t");
    // `tenant` and `token` start with the text; nothing else has it.
    const options = within(screen.getByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      expect.stringContaining("tenant"),
      expect.stringContaining("token"),
      expect.stringContaining("host"),
      expect.stringContaining("$timestamp"),
      expect.stringContaining("$isoTimestamp"),
      expect.stringContaining("$randomInt"),
    ]);

    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input.getAttribute("aria-activedescendant")).toBe(options[1].id);
    await fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(input.getAttribute("aria-activedescendant")).toBe(options[0].id);
    // Up from the first item is the last one.
    await fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(input.getAttribute("aria-activedescendant")).toBe(options[5].id);
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input.getAttribute("aria-activedescendant")).toBe(options[1].id);

    const tab = createEvent.keyDown(input, { key: "Tab" });
    await fireEvent(input, tab);

    expect(tab.defaultPrevented).toBe(true);
    expect(input.value).toBe("x={{token}}");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("does not double the closing braces", async () => {
    await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    await type(input, "{{h}}/x");
    expect(screen.queryByRole("listbox")).toBeNull();
    input.setSelectionRange(3, 3);

    // Ctrl+Space opens the list for the token under the caret.
    await fireEvent.keyDown(input, { key: " ", code: "Space", ctrlKey: true });
    expect(screen.getByRole("listbox")).toBeTruthy();
    await fireEvent.keyDown(input, { key: "Enter" });

    expect(input.value).toBe("{{host}}/x");
    expect(input.selectionStart).toBe(8);
  });

  it("puts in the item that is clicked and keeps the focus in the field", async () => {
    await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    input.focus();
    await type(input, "{{ten");
    const option = within(screen.getByRole("listbox")).getByRole("option");

    const mouseDown = createEvent.mouseDown(option);
    await fireEvent(option, mouseDown);

    // The press does not take the focus away from the field.
    expect(mouseDown.defaultPrevented).toBe(true);
    expect(input.value).toBe("{{tenant}}");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("offers for the name at the caret after the caret was moved with a key, and Enter puts it in there", async () => {
    const { draft } = await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    input.focus();
    await type(input, "{{ho}} {{te");
    const names = () => within(screen.getByRole("listbox")).getAllByRole("option").map((option) => option.textContent);
    expect(names()).toEqual([expect.stringContaining("tenant")]);

    // The caret goes into the first token: as the arrows move it.
    input.setSelectionRange(4, 4);
    await fireEvent.keyUp(input, { key: "ArrowRight" });

    expect(names()).toEqual([expect.stringContaining("host")]);
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(input.value).toBe("{{host}} {{te");
    expect(draft().url).toBe("{{host}} {{te");
  });

  it("follows the caret moved with the mouse or by a selection, and closes where there is no name", async () => {
    await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    input.focus();
    await type(input, "{{ho}} {{te");
    const names = () => within(screen.getByRole("listbox")).getAllByRole("option").map((option) => option.textContent);

    input.setSelectionRange(4, 4);
    await fireEvent.mouseUp(input);
    expect(names()).toEqual([expect.stringContaining("host")]);

    input.setSelectionRange(11, 11);
    await fireEvent.select(input);
    expect(names()).toEqual([expect.stringContaining("tenant")]);

    // Between the two tokens there is nothing to complete.
    input.setSelectionRange(7, 7);
    await fireEvent.keyUp(input, { key: "Home" });
    expect(screen.queryByRole("listbox")).toBeNull();
    // A closed list is not opened by a move of the caret.
    input.setSelectionRange(11, 11);
    await fireEvent.keyUp(input, { key: "End" });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("keeps the chosen item when a key of the list is released", async () => {
    await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    await type(input, "{{t");
    await fireEvent.keyUp(input, { key: "t" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyUp(input, { key: "ArrowDown" });

    const options = within(screen.getByRole("listbox")).getAllByRole("option");
    expect(options[1].getAttribute("aria-selected")).toBe("true");
  });

  it("has no list when nothing matches, after a closed token and when the field loses the focus", async () => {
    await openRequest({ method: "GET", url: "" });
    const input = urlInput();
    input.focus();

    await type(input, "{{zzz");
    expect(screen.queryByRole("listbox")).toBeNull();
    await type(input, "{{host}}");
    expect(screen.queryByRole("listbox")).toBeNull();
    await type(input, "plain");
    expect(screen.queryByRole("listbox")).toBeNull();
    // Ctrl+Space without a token under the caret opens nothing.
    await fireEvent.keyDown(input, { key: " ", code: "Space", ctrlKey: true });
    expect(screen.queryByRole("listbox")).toBeNull();

    await type(input, "{{h");
    expect(screen.getByRole("listbox")).toBeTruthy();
    await fireEvent.blur(input);
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});

describe("re-reading the variables", () => {
  it("reads them again when the active environment changes, and the token changes its colour", async () => {
    const { api, variablesInScope } = await openRequest();
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    const before = variablesInScope.mock.calls.length;

    await workspace.setEnvironment(api, "env-prod");

    await tokenHasClass(urlInput, "{{host}}", "var-global");
    expect(variablesInScope.mock.calls.length).toBe(before + 1);
    expect(variablesInScope).toHaveBeenLastCalledWith("req");
  });

  it("does not read them again when the choice of the environment was refused", async () => {
    const { api, variablesInScope } = await openRequest();
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    const before = variablesInScope.mock.calls.length;
    vi.spyOn(api, "setActiveEnvironment").mockRejectedValue("locked");

    await workspace.setEnvironment(api, "env-prod");
    await screen.findByRole("alert");
    await tick();

    expect(variablesInScope.mock.calls.length).toBe(before);
  });

  it("applies the last answer when two come in the wrong order", async () => {
    const first = deferred<ScopeVariable[]>();
    const second = deferred<ScopeVariable[]>();
    const answers = [first, second];
    const { variablesInScope } = await openRequest(REQUEST, (api) => {
      api.variablesInScope = () => (answers.shift() ?? deferred<ScopeVariable[]>()).promise;
    });
    await waitFor(() => expect(variablesInScope).toHaveBeenCalledTimes(1));

    bumpVariables();
    await waitFor(() => expect(variablesInScope).toHaveBeenCalledTimes(2));
    second.resolve([scopeVariable("host", "global")]);
    await tokenHasClass(urlInput, "{{host}}", "var-global");
    first.resolve([scopeVariable("host", "environment"), scopeVariable("nope", "environment")]);
    await first.promise;
    await tick();

    expect(tokenOf(urlInput(), "{{host}}").classList.contains("var-global")).toBe(true);
    expect(tokenOf(urlInput(), "{{nope}}").classList.contains("var-unresolved")).toBe(true);
    expect(variablesInScope).toHaveBeenCalledTimes(2);
  });

  it("does not show a failure of an answer that is not the last one", async () => {
    const first = deferred<ScopeVariable[]>();
    const second = deferred<ScopeVariable[]>();
    const answers = [first, second];
    const { variablesInScope } = await openRequest(REQUEST, (api) => {
      api.variablesInScope = () => (answers.shift() ?? deferred<ScopeVariable[]>()).promise;
    });
    await waitFor(() => expect(variablesInScope).toHaveBeenCalledTimes(1));
    bumpVariables();
    await waitFor(() => expect(variablesInScope).toHaveBeenCalledTimes(2));

    second.resolve([scopeVariable("host", "collection")]);
    await tokenHasClass(urlInput, "{{host}}", "var-collection");
    first.reject("too late");
    await first.promise.catch(() => {});
    await tick();

    expect(screen.queryByRole("alert")).toBeNull();
    expect(tokenOf(urlInput(), "{{host}}").classList.contains("var-collection")).toBe(true);
  });

  it("does not read them for a hidden tab; the tab reads them when it is shown", async () => {
    const { api, variablesInScope } = await openRequest();
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    await tabs.open(api, "other");
    await screen.findByRole("tab", { name: /Other/ });
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    const callsFor = (id: string) => variablesInScope.mock.calls.filter(([nodeId]) => nodeId === id).length;
    const [hidden, shown] = [callsFor("req"), callsFor("other")];

    bumpVariables();
    await waitFor(() => expect(callsFor("other")).toBe(shown + 1));
    await tick();
    expect(callsFor("req")).toBe(hidden);

    await fireEvent.click(screen.getByRole("tab", { name: /List users/ }));
    await waitFor(() => expect(callsFor("req")).toBe(hidden + 1));
    // Shown again without a change in between: nothing to read.
    await fireEvent.click(screen.getByRole("tab", { name: /Other/ }));
    await fireEvent.click(screen.getByRole("tab", { name: /List users/ }));
    await tick();
    expect(callsFor("req")).toBe(hidden + 1);
    expect(callsFor("other")).toBe(shown + 1);
  });

  it("shows one notification when the command fails, and the field goes on working", async () => {
    const { variablesInScope, draft } = await openRequest(REQUEST, (api) => {
      api.variablesInScope = () => Promise.reject("scope failed");
    });

    expect((await screen.findByRole("alert")).textContent).toContain("scope failed");
    await tick();
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(variablesInScope).toHaveBeenCalledTimes(1);
    // The tokens are marked but have no colour of a scope, and nothing is counted as not found.
    const token = tokenOf(urlInput(), "{{host}}");
    expect([...token.classList].filter((name) => name.startsWith("var"))).toEqual(["var"]);
    expect(layerOf(urlInput())!.querySelector(".var-unresolved")).toBeNull();
    expect(screen.queryByText(/Не найдено переменных/)).toBeNull();

    await type(urlInput(), "{{host}}/changed");
    expect(draft().url).toBe("{{host}}/changed");
    expect(layerOf(urlInput())!.textContent).toBe("{{host}}/changed");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole<HTMLButtonElement>("button", { name: strings.editor.send }).disabled).toBe(false);
  });

  it("reads them again after the variables of an environment were saved", async () => {
    const { api, variablesInScope } = await openRequest();
    await tokenHasClass(urlInput, "{{host}}", "var-environment");
    await tabs.openEnvironment(api, "env-dev");
    const table = await screen.findByRole("table", { name: strings.environment.variables });
    const before = variablesInScope.mock.calls.length;

    await fireEvent.input(within(table).getAllByLabelText(strings.environment.name)[0], { target: { value: "server" } });
    await tabs.environment("env-dev")!.flush(api);
    await tick();
    // The request is hidden behind the tab of the environment: it reads when it is shown.
    expect(variablesInScope.mock.calls.length).toBe(before);
    await fireEvent.click(screen.getByRole("tab", { name: /List users/ }));

    await tokenHasClass(urlInput, "{{host}}", "var-global");
    expect(variablesInScope.mock.calls.length).toBe(before + 1);
  });

  it("reads them again after the collection was saved, not while its variables are only a draft", async () => {
    const { api, variablesInScope } = await openRequest({ method: "GET", url: "{{fresh}}" });
    await tokenHasClass(urlInput, "{{fresh}}", "var-unresolved");
    await tabs.open(api, "col");
    await screen.findByRole("tab", { name: /^API/ });
    const collection = tabs.active as Tab<CollectionDraft>;
    collection.draft.variables.push({ key: "fresh", value: "1", secret: false, enabled: true, postman_extra: {} });
    await tick();

    await fireEvent.click(screen.getByRole("tab", { name: /List users/ }));
    await tick();
    expect(tokenOf(urlInput(), "{{fresh}}").classList.contains("var-unresolved")).toBe(true);
    const before = variablesInScope.mock.calls.length;

    expect(await collection.save(api)).toBe(true);

    await tokenHasClass(urlInput, "{{fresh}}", "var-collection");
    expect(variablesInScope.mock.calls.length).toBe(before + 1);
  });
});

describe("variables in the tables of a request", () => {
  const WIRED: RequestContent = {
    method: "POST",
    url: "https://api.test/:id",
    query: [{ key: "q-{{host}}", value: "{{host}}" }],
    path_variables: [{ key: "{{host}}", value: "{{host}}" }],
    headers: [{ key: "h-{{host}}", value: "{{host}}" }],
    body: { mode: "url_encoded", data: [{ key: "f-{{host}}", value: "{{host}}" }] },
  };

  it("colours the keys and the values of the query parameters", async () => {
    await openRequest(WIRED);
    const row = () => tableRows(strings.editor.query)[0];

    await tokenHasClass(() => cell(row(), strings.table.value), "{{host}}", "var-environment");
    expect(tokenOf(cell(row(), strings.table.key), "{{host}}").classList.contains("var-environment")).toBe(true);
    expect(layerOf(cell(row(), strings.table.key))!.textContent).toBe("q-{{host}}");
  });

  it("colours the value of a path variable and leaves its key a plain field", async () => {
    await openRequest(WIRED);
    const row = () => tableRows(strings.editor.pathVariables)[0];

    await tokenHasClass(() => cell(row(), strings.table.value), "{{host}}", "var-environment");
    expect(cell(row(), strings.table.key).value).toBe("{{host}}");
    expect(cell(row(), strings.table.key).closest("td")!.querySelector(".var-layer")).toBeNull();
  });

  it("colours the keys and the values of the headers", async () => {
    await openRequest(WIRED);
    await showSection(/^Headers/);
    const row = () => tableRows(strings.editor.headers)[0];

    await tokenHasClass(() => cell(row(), strings.table.value), "{{host}}", "var-environment");
    expect(tokenOf(cell(row(), strings.table.key), "{{host}}").classList.contains("var-environment")).toBe(true);
  });

  it("colours the keys and the values of an x-www-form-urlencoded body", async () => {
    await openRequest(WIRED);
    await showSection(/^Body/);
    const row = () => tableRows(strings.body.fields)[0];

    await tokenHasClass(() => cell(row(), strings.table.value), "{{host}}", "var-environment");
    expect(tokenOf(cell(row(), strings.table.key), "{{host}}").classList.contains("var-environment")).toBe(true);
  });

  it("switches the variables on in the editor of a raw body", async () => {
    await openRequest({ method: "POST", url: "u", body: { mode: "raw", data: { text: '{"id": {{host}}}', language: "json" } } });
    await showSection(/^Body/);

    expect(screen.getByRole("textbox", { name: strings.body.text }).dataset.variables).toBe("true");
  });

  it("still adds a row typed into the empty one, with the focus in the new field", async () => {
    const { draft } = await openRequest(WIRED);
    const rows = tableRows(strings.editor.query);

    await fireEvent.input(cell(rows[1], strings.table.value), { target: { value: "{" } });

    expect(draft().query[1]).toEqual({ key: "", value: "{", enabled: true, postman_extra: {} });
    const added = cell(tableRows(strings.editor.query)[1], strings.table.value);
    await waitFor(() => expect(document.activeElement).toBe(added));
    expect(layerOf(added)).not.toBeNull();
  });

  it("offers the variables in a cell of a table too", async () => {
    const { draft } = await openRequest(WIRED);
    const value = cell(tableRows(strings.editor.query)[0], strings.table.value);

    await type(value, "{{ten");
    await fireEvent.keyDown(value, { key: "Enter" });

    expect(draft().query[0].value).toBe("{{tenant}}");
  });

  it("does not colour the variables of a collection in its own table", async () => {
    const { api } = await openRequest();
    await tabs.open(api, "col");
    const table = await screen.findByRole("table", { name: strings.collection.variables });

    expect(table.querySelector(".var-layer")).toBeNull();
  });
});

describe("KeyValueTable with variables", () => {
  interface SecretRow {
    key: string;
    value: string;
    enabled: boolean;
    secret: boolean;
  }

  const rows = (): SecretRow[] => [
    { key: "{{host}}", value: "{{host}}", enabled: true, secret: true },
    { key: "plain", value: "{{host}}", enabled: true, secret: false },
  ];

  async function loadedScope(): Promise<VariableScope> {
    const scope = new VariableScope("node");
    await scope.refresh({ variablesInScope: async () => [scopeVariable("host", "environment")] } as unknown as Api);
    return scope;
  }

  function renderTable(scope: VariableScope | null, variables: boolean | "value" | undefined) {
    return render(KeyValueTable<SecretRow>, {
      props: {
        rows: rows(),
        label: "Rows",
        create: () => ({ key: "", value: "", enabled: true, secret: false }),
        secret: (row: SecretRow) => row.secret,
        variables,
      },
      context: new Map(scope ? [["variables", scope]] : []),
    });
  }

  it("leaves a masked cell a plain password field and colours it once it is shown", async () => {
    renderTable(await loadedScope(), true);
    const [masked, open] = tableRows("Rows");
    const value = within(masked).getByLabelText<HTMLInputElement>(strings.table.value);

    expect(value.type).toBe("password");
    expect(value.closest("td")!.querySelector(".var-layer")).toBeNull();
    // The key of the same row and the value of an open row have their layers.
    expect(tokenOf(cell(masked, strings.table.key), "{{host}}").classList.contains("var-environment")).toBe(true);
    expect(tokenOf(cell(open, strings.table.value), "{{host}}").classList.contains("var-environment")).toBe(true);

    await fireEvent.click(within(masked).getByRole("button", { name: strings.table.show }));

    const shown = within(masked).getByLabelText<HTMLInputElement>(strings.table.value);
    expect(shown.type).toBe("text");
    expect(shown.value).toBe("{{host}}");
    expect(tokenOf(shown, "{{host}}").classList.contains("var-environment")).toBe(true);

    await fireEvent.click(within(masked).getByRole("button", { name: strings.table.hide }));
    const hidden = within(masked).getByLabelText<HTMLInputElement>(strings.table.value);
    expect(hidden.type).toBe("password");
    expect(hidden.closest("td")!.querySelector(".var-layer")).toBeNull();
  });

  it("has no layers at all without `variables`", async () => {
    const { container } = renderTable(await loadedScope(), undefined);

    await fireEvent.click(within(tableRows("Rows")[0]).getByRole("button", { name: strings.table.show }));

    expect(container.querySelector(".var-layer")).toBeNull();
    expect(container.querySelectorAll('input[type="text"]').length).toBeGreaterThan(0);
  });

  it("colours only the values with `variables` set to value", async () => {
    renderTable(await loadedScope(), "value");
    const open = tableRows("Rows")[1];

    expect(cell(open, strings.table.key).closest("td")!.querySelector(".var-layer")).toBeNull();
    expect(tokenOf(cell(open, strings.table.value), "{{host}}").classList.contains("var-environment")).toBe(true);
  });

  it("draws plain fields when there are no variables around it", () => {
    const { container } = renderTable(null, true);

    expect(container.querySelector(".var-layer")).toBeNull();
    expect(cell(tableRows("Rows")[1], strings.table.value).value).toBe("{{host}}");
  });
});

describe("a secret that was switched off and on again", () => {
  it("is hidden again in the table of a collection", async () => {
    const { api } = await openRequest();
    await tabs.open(api, "col");
    await screen.findByRole("table", { name: strings.collection.variables });
    const draft = (tabs.active as Tab<CollectionDraft>).draft;
    const row = () => tableRows(strings.collection.variables)[1];
    const value = () => within(row()).getByLabelText<HTMLInputElement>(strings.table.value);
    await fireEvent.click(within(row()).getByRole("button", { name: strings.table.show }));
    expect(value().type).toBe("text");

    draft.variables[1].secret = false;
    await tick();
    expect(value().type).toBe("text");
    draft.variables[1].secret = true;
    await tick();

    expect(value().type).toBe("password");
    expect(within(row()).getByRole("button", { name: strings.table.show }).getAttribute("aria-pressed")).toBe("false");
  });

  it("is hidden again in the editor of an environment", async () => {
    const { api } = await openRequest();
    await tabs.openEnvironment(api, "env-dev");
    const table = await screen.findByRole("table", { name: strings.environment.variables });
    const row = within(table).getAllByRole("row")[2];
    const value = () => within(row).getByLabelText<HTMLInputElement>(strings.table.value);
    const secret = within(row).getByRole("checkbox", { name: strings.environment.secret });
    await fireEvent.click(within(row).getByRole("button", { name: strings.table.show }));
    expect(value().type).toBe("text");

    await fireEvent.click(secret);
    expect(value().type).toBe("text");
    await fireEvent.click(secret);

    expect(value().type).toBe("password");
    await tabs.environment("env-dev")!.flush(api);
  });
});

describe("the glow of typed letters in a field with variables", () => {
  afterEach(() => motion.choose("normal"));

  /** The letters of the layer that glow, piece by piece. */
  function typed(input: HTMLElement): string[] {
    return [...layerOf(input)!.querySelectorAll(".typed")].map((span) => span.textContent ?? "");
  }

  it("draws the field as before at the normal level: nothing glows", async () => {
    await openRequest({ method: "GET", url: "{{host}}/x" });
    await tokenHasClass(urlInput, "{{host}}", "var-environment");

    await type(urlInput(), "{{host}}/xy");

    expect(layerOf(urlInput())!.textContent).toBe("{{host}}/xy");
    expect(typed(urlInput())).toEqual([]);
    expect(layerOf(urlInput())!.querySelectorAll("span")).toHaveLength(2);
  });

  it("glows the letters typed at the rich level, each in its own piece, and the text stays the same", async () => {
    const { draft } = await openRequest({ method: "GET", url: "{{host}}/x" });
    motion.choose("rich");

    await type(urlInput(), "{{host}}/xy");
    expect(typed(urlInput())).toEqual(["y"]);
    await type(urlInput(), "{{host}}/xyz");
    expect(typed(urlInput())).toEqual(["y", "z"]);

    expect(layerOf(urlInput())!.textContent).toBe("{{host}}/xyz");
    expect(draft().url).toBe("{{host}}/xyz");
    // No style attribute: the glow is a class.
    expect(layerOf(urlInput())!.querySelector(".typed[style]")).toBeNull();
  });

  it("the letters are plain again when their glow is over, one piece at a time", async () => {
    await openRequest({ method: "GET", url: "a" });
    motion.choose("rich");
    await type(urlInput(), "ab");
    await type(urlInput(), "abc");
    const [first, second] = layerOf(urlInput())!.querySelectorAll(".typed");

    await fireEvent.animationEnd(first);
    expect(typed(urlInput())).toEqual(["c"]);
    // The piece that still glows is the same element: its glow goes on.
    expect(layerOf(urlInput())!.querySelector(".typed")).toBe(second);

    await fireEvent(second, new Event("animationcancel"));
    expect(typed(urlInput())).toEqual([]);
    expect(layerOf(urlInput())!.textContent).toBe("abc");
  });

  it("glows a pasted text as one piece, also inside a token, and keeps the token whole", async () => {
    await openRequest({ method: "GET", url: "" });
    motion.choose("rich");

    await type(urlInput(), "{{host}}/users");
    await tokenHasClass(urlInput, "{{host}}", "var-environment");

    expect(typed(urlInput())).toEqual(["{{host}}", "/users"]);
    expect(tokenOf(urlInput(), "{{host}}").dataset.name).toBe("host");
    expect(layerOf(urlInput())!.textContent).toBe("{{host}}/users");
  });

  it("drops the glow when the text of the field comes from outside", async () => {
    const { draft } = await openRequest({ method: "GET", url: "a" });
    motion.choose("rich");
    await type(urlInput(), "ab");
    expect(typed(urlInput())).toEqual(["b"]);

    draft().url = "zz";
    await tick();

    expect(layerOf(urlInput())!.textContent).toBe("zz");
    expect(typed(urlInput())).toEqual([]);
  });

  it("deleting a letter makes nothing glow", async () => {
    await openRequest({ method: "GET", url: "abc" });
    motion.choose("rich");

    await type(urlInput(), "ab");

    expect(typed(urlInput())).toEqual([]);
  });
});
