import { afterEach, describe, expect, it, vi } from "vitest";
import { Tabs, tabsToClose, type Tab, type TabMenuAction } from "./tabs.svelte";
import { createFakeApi } from "./testing/fakeApi";
import { toasts } from "./toasts.svelte";
import {
  normalizeContent,
  type CollectionDraft,
  type FolderDraft,
  type NodeContent,
  type NodeDto,
  type RequestDraft,
  type SavedTab,
} from "./types";

// The notifications live in a module; every test starts without them.
afterEach(() => {
  toasts.items = [];
});

/** A collection with a folder and two requests; the first request has a parameter without `enabled`. */
function nodes(): NodeDto[] {
  return [
    { id: "col", parent_id: null, kind: "collection", name: "API", content: {} },
    { id: "fold", parent_id: "col", kind: "folder", name: "Folder", content: {} },
    {
      id: "req-a",
      parent_id: "col",
      kind: "request",
      name: "A",
      content: { method: "GET", url: "https://a.test", query: [{ key: "page", value: "1" }] },
    },
    { id: "req-b", parent_id: "col", kind: "request", name: "B", content: { method: "POST", url: "https://b.test" } },
  ];
}

async function openRequest(tabs: Tabs, api: ReturnType<typeof createFakeApi>, id: string): Promise<Tab<RequestDraft>> {
  await tabs.open(api, id);
  return tabs.list.find((tab) => tab.nodeId === id) as Tab<RequestDraft>;
}

describe("normalizeContent", () => {
  it("writes out every default the core leaves out of a request", () => {
    expect(normalizeContent("request", { method: "GET", url: "u", query: [{ key: "a", value: "b" }] })).toEqual({
      method: "GET",
      url: "u",
      query: [{ key: "a", value: "b", enabled: true, postman_extra: {} }],
      path_variables: [],
      headers: [],
      body: { mode: "none" },
      auth: null,
      scripts: { pre_request: "", test: "" },
      settings: { follow_redirects: null, max_redirects: null, strict_ssl: null, disable_body_pruning: null },
      disabled_auto_headers: [],
    });
  });

  it("writes out empty scripts and four settings that are not set", () => {
    const normalized = normalizeContent("request", { method: "GET", url: "" });
    expect(normalized.scripts).toEqual({ pre_request: "", test: "" });
    expect(normalized.settings).toEqual({ follow_redirects: null, max_redirects: null, strict_ssl: null, disable_body_pruning: null });
  });

  it("changes nothing when a request or a collection is normalized again", () => {
    const request = normalizeContent("request", {
      method: "POST",
      url: "u",
      query: [{ key: "a", value: "b" }],
      scripts: { test: "x" },
      settings: { max_redirects: 3, strict_ssl: false },
    });
    expect(normalizeContent("request", request)).toEqual(request);
    expect(request.settings).toEqual({ follow_redirects: null, max_redirects: 3, strict_ssl: false, disable_body_pruning: null });

    const collection = normalizeContent("collection", { variables: [{ key: "host", value: "h" }], scripts: { pre_request: "p" } });
    expect(normalizeContent("collection", collection)).toEqual(collection);
  });

  it("keeps what is set and fills the rows of an url-encoded body", () => {
    const normalized = normalizeContent("request", {
      method: "POST",
      url: "u",
      headers: [{ key: "h", value: "v", enabled: false, postman_extra: { description: "d" } }],
      body: { mode: "url_encoded", data: [{ key: "f", value: "1" }] },
      auth: { type: "bearer", token: "t" },
      scripts: { test: "x" },
    });
    expect(normalized.headers).toEqual([{ key: "h", value: "v", enabled: false, postman_extra: { description: "d" } }]);
    expect(normalized.body).toEqual({ mode: "url_encoded", data: [{ key: "f", value: "1", enabled: true, postman_extra: {} }] });
    expect(normalized.auth).toEqual({ type: "bearer", token: "t" });
    expect(normalized.scripts).toEqual({ pre_request: "", test: "x" });
  });

  it("writes out the defaults of the parts and of the files of a form-data body", () => {
    const normalized = normalizeContent("request", {
      method: "POST",
      url: "u",
      body: {
        mode: "form_data",
        data: [
          { key: "note", value: { text: "hello" } },
          {
            key: "doc",
            value: { files: [{ path: "C:/a.txt" }, { path: "C:/b.txt", confirmed: true }] },
            enabled: false,
            content_type: "text/plain",
            postman_extra: { description: "d" },
          },
        ],
      },
    });
    expect(normalized.body).toEqual({
      mode: "form_data",
      data: [
        { key: "note", value: { text: "hello" }, enabled: true, content_type: null, postman_extra: {} },
        {
          key: "doc",
          value: {
            files: [
              { path: "C:/a.txt", confirmed: false },
              { path: "C:/b.txt", confirmed: true },
            ],
          },
          enabled: false,
          content_type: "text/plain",
          postman_extra: { description: "d" },
        },
      ],
    });
    expect(normalizeContent("request", normalized)).toEqual(normalized);
  });

  it("writes out whether the file of a binary body is confirmed", () => {
    const withFile = normalizeContent("request", { method: "POST", url: "u", body: { mode: "binary", data: { file: { path: "C:/a.bin" } } } });
    expect(withFile.body).toEqual({ mode: "binary", data: { file: { path: "C:/a.bin", confirmed: false } } });
    expect(normalizeContent("request", withFile)).toEqual(withFile);

    const picked = normalizeContent("request", {
      method: "POST",
      url: "u",
      body: { mode: "binary", data: { file: { path: "C:/a.bin", confirmed: true } } },
    });
    expect(picked.body).toEqual({ mode: "binary", data: { file: { path: "C:/a.bin", confirmed: true } } });

    const empty = normalizeContent("request", { method: "POST", url: "u", body: { mode: "binary", data: { file: null } } });
    expect(empty.body).toEqual({ mode: "binary", data: { file: null } });
  });

  it("writes out the defaults of a folder and keeps what is set", () => {
    expect(normalizeContent("folder", {})).toEqual({ auth: null, scripts: { pre_request: "", test: "" }, description: "" });
    const folder = normalizeContent("folder", { auth: { type: "no_auth" }, scripts: { test: "t();" } });
    expect(folder).toEqual({ auth: { type: "no_auth" }, scripts: { pre_request: "", test: "t();" }, description: "" });
    expect(normalizeContent("folder", folder)).toEqual(folder);
  });

  it("writes out the defaults of a collection", () => {
    expect(normalizeContent("collection", { variables: [{ key: "host", value: "h" }] })).toEqual({
      auth: null,
      variables: [{ key: "host", value: "h", secret: false, enabled: true, postman_extra: {} }],
      scripts: { pre_request: "", test: "" },
      description: "",
    });
  });
});

describe("Tabs", () => {
  it("opens a request and a collection with normalized content", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tabs = new Tabs();

    const tab = await openRequest(tabs, api, "req-a");
    expect(tab.kind).toBe("request");
    expect(tab.name).toBe("A");
    expect(tab.draft.query).toEqual([{ key: "page", value: "1", enabled: true, postman_extra: {} }]);
    expect(tab.draft).toEqual(tab.saved);
    expect(tab.dirty).toBe(false);
    expect(tabs.active).toBe(tab);

    await tabs.open(api, "col");
    const collection = tabs.active as Tab<CollectionDraft>;
    expect(collection.kind).toBe("collection");
    expect(collection.draft).toEqual({ auth: null, variables: [], scripts: { pre_request: "", test: "" }, description: "" });
  });

  it("does not open a second tab for the same node, only activates it", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const getNode = vi.spyOn(api, "getNode");
    const tabs = new Tabs();

    const first = await openRequest(tabs, api, "req-a");
    await tabs.open(api, "req-b");
    expect(tabs.active?.nodeId).toBe("req-b");

    await tabs.open(api, "req-a");
    expect(tabs.list).toHaveLength(2);
    expect(tabs.active).toBe(first);
    expect(getNode).toHaveBeenCalledTimes(2);
  });

  it("does not open a second tab when the same node is opened twice at once", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tabs = new Tabs();
    await Promise.all([tabs.open(api, "req-a"), tabs.open(api, "req-a")]);
    expect(tabs.list).toHaveLength(1);
  });

  it("opens a folder in a tab of its own with normalized content", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tabs = new Tabs();
    await tabs.open(api, "fold");

    expect(tabs.list).toHaveLength(1);
    const folder = tabs.active as Tab<FolderDraft>;
    expect(folder.kind).toBe("folder");
    expect(folder.nodeId).toBe("fold");
    expect(folder.name).toBe("Folder");
    expect(folder.draft).toEqual({ auth: null, scripts: { pre_request: "", test: "" }, description: "" });
    expect(folder.draft).toEqual(folder.saved);
    expect(folder.dirty).toBe(false);
  });

  it("becomes dirty when the draft is edited and clean when the value is put back", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tab = await openRequest(new Tabs(), api, "req-a");

    tab.draft.url = "https://changed.test";
    expect(tab.dirty).toBe(true);
    // The stored copy is not touched by the edit.
    expect(tab.saved.url).toBe("https://a.test");

    tab.draft.url = "https://a.test";
    expect(tab.dirty).toBe(false);
  });

  it("is clean after a parameter without an explicit `enabled` is switched off and on again", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tab = await openRequest(new Tabs(), api, "req-a");

    tab.draft.query[0].enabled = false;
    expect(tab.dirty).toBe(true);
    tab.draft.query[0].enabled = true;
    expect(tab.dirty).toBe(false);
  });

  it("is clean after a request whose script has Windows line ends is opened, and keeps the text as it came", async () => {
    const script = "const a = 1;\r\nconsole.log(a);\r\n";
    const stored: NodeDto[] = [
      { id: "col", parent_id: null, kind: "collection", name: "API", content: {} },
      { id: "req", parent_id: "col", kind: "request", name: "R", content: { method: "GET", url: "u", scripts: { pre_request: script } } },
    ];
    const tab = await openRequest(new Tabs(), createFakeApi({ nodes: stored }), "req");

    expect(tab.dirty).toBe(false);
    expect(tab.draft.scripts).toEqual({ pre_request: script, test: "" });
  });

  it("revert puts the stored content back", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tab = await openRequest(new Tabs(), api, "req-a");

    tab.draft.method = "DELETE";
    tab.draft.query.push({ key: "x", value: "y", enabled: true, postman_extra: {} });
    tab.revert();

    expect(tab.dirty).toBe(false);
    expect(tab.draft.method).toBe("GET");
    expect(tab.draft.query).toHaveLength(1);
  });

  it("save passes the draft to saveNodeContent and clears dirty", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const saveNodeContent = vi.spyOn(api, "saveNodeContent");
    const tree = vi.spyOn(api, "tree");
    const tab = await openRequest(new Tabs(), api, "req-a");
    tab.draft.url = "https://changed.test";
    tab.draft.method = "PUT";

    expect(await tab.save(api)).toBe(true);

    expect(saveNodeContent).toHaveBeenCalledTimes(1);
    expect(saveNodeContent).toHaveBeenCalledWith("req-a", {
      method: "PUT",
      url: "https://changed.test",
      query: [{ key: "page", value: "1", enabled: true, postman_extra: {} }],
      path_variables: [],
      headers: [],
      body: { mode: "none" },
      auth: null,
      scripts: { pre_request: "", test: "" },
      settings: { follow_redirects: null, max_redirects: null, strict_ssl: null, disable_body_pruning: null },
      disabled_auto_headers: [],
    });
    expect(tab.dirty).toBe(false);
    expect(tab.saved.url).toBe("https://changed.test");
    // The tree is re-read so that it shows the new method.
    expect(tree).toHaveBeenCalledTimes(1);
  });

  it("keeps the draft and stays dirty when the save fails", async () => {
    const api = createFakeApi({ nodes: nodes() });
    vi.spyOn(api, "saveNodeContent").mockRejectedValue("disk is full");
    const tab = await openRequest(new Tabs(), api, "req-a");
    tab.draft.url = "https://changed.test";

    expect(await tab.save(api)).toBe(false);

    expect(tab.dirty).toBe(true);
    expect(tab.draft.url).toBe("https://changed.test");
    expect(tab.saved.url).toBe("https://a.test");
  });

  it("an edit made while the save is running leaves the tab dirty", async () => {
    const api = createFakeApi({ nodes: nodes() });
    let finish = () => {};
    vi.spyOn(api, "saveNodeContent").mockImplementation(() => new Promise<void>((resolve) => (finish = resolve)));
    const tab = await openRequest(new Tabs(), api, "req-a");
    tab.draft.url = "https://first.test";

    const saving = tab.save(api);
    tab.draft.url = "https://second.test";
    finish();
    await saving;

    expect(tab.saved.url).toBe("https://first.test");
    expect(tab.dirty).toBe(true);
  });

  it("close activates the neighbouring tab", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tabs = new Tabs();
    await tabs.open(api, "req-a");
    await tabs.open(api, "req-b");
    await tabs.open(api, "col");
    const [a, b, col] = tabs.list;

    // The tab in the middle: the one that takes its place becomes active.
    tabs.activate(b.id);
    tabs.close(b.id);
    expect(tabs.list).toEqual([a, col]);
    expect(tabs.active).toBe(col);

    // The last tab: the previous one becomes active.
    tabs.close(col.id);
    expect(tabs.active).toBe(a);

    tabs.close(a.id);
    expect(tabs.list).toHaveLength(0);
    expect(tabs.active).toBeNull();
  });

  it("closing an inactive tab leaves the active one", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tabs = new Tabs();
    await tabs.open(api, "req-a");
    await tabs.open(api, "req-b");
    const [a, b] = tabs.list;

    tabs.close(a.id);
    expect(tabs.active).toBe(b);
  });
});

describe("tabsToClose", () => {
  /** Four tabs; `b` and `d` have unsaved edits. */
  const list = [
    { id: "a", dirty: false },
    { id: "b", dirty: true },
    { id: "c", dirty: false },
    { id: "d", dirty: true },
  ];
  const ids = (tabId: string, action: TabMenuAction, of = list) => tabsToClose(of, tabId, action).map((tab) => tab.id);

  it("closes the tab of the menu alone", () => {
    expect(ids("b", "close")).toEqual(["b"]);
  });

  it("closes every other tab, in the order of the bar", () => {
    expect(ids("b", "close_others")).toEqual(["a", "c", "d"]);
    expect(ids("a", "close_others", [list[0]])).toEqual([]);
  });

  it("closes the tabs on the right of the tab of the menu", () => {
    expect(ids("b", "close_right")).toEqual(["c", "d"]);
    expect(ids("a", "close_right")).toEqual(["b", "c", "d"]);
    expect(ids("d", "close_right")).toEqual([]);
  });

  it("closes the tabs without unsaved edits, the tab of the menu among them", () => {
    expect(ids("a", "close_saved")).toEqual(["a", "c"]);
    expect(ids("b", "close_saved")).toEqual(["a", "c"]);
    expect(ids("b", "close_saved", [list[1], list[3]])).toEqual([]);
  });

  it("closes all tabs", () => {
    expect(ids("c", "close_all")).toEqual(["a", "b", "c", "d"]);
  });

  it("closes nothing for a tab that is not open, and returns the tabs themselves", () => {
    for (const action of ["close", "close_others", "close_right", "close_saved", "close_all"] as const) {
      expect(ids("gone", action)).toEqual([]);
    }
    expect(tabsToClose(list, "b", "close")[0]).toBe(list[1]);
  });
});

describe("Tabs.closeNodes", () => {
  it("closes the tabs of the deleted nodes and leaves the others, tabs that are not of a node included", async () => {
    const api = createFakeApi({ nodes: nodes() });
    const tabs = new Tabs();
    await tabs.open(api, "req-a");
    await tabs.open(api, "req-b");
    await tabs.open(api, "col");
    await tabs.openEnvironment(api, "globals");

    tabs.closeNodes(["req-a", "col", "not-open"]);

    expect(tabs.list.map((tab) => tab.id)).toEqual(["req-b", "env:globals"]);
  });
});

describe("Tabs.restore", () => {
  /** The tabs of {@link nodes} that were open: a request, the folder and an environment. */
  const SAVED: SavedTab[] = [
    { kind: "node", id: "req-a" },
    { kind: "node", id: "fold" },
    { kind: "environment", id: "env-dev" },
  ];

  /** Three requests in a row, for the cases about a draft of the second one. */
  const REQUESTS: SavedTab[] = [
    { kind: "node", id: "req-a" },
    { kind: "node", id: "req-b" },
    { kind: "node", id: "req-c" },
  ];

  function seeded(more: NodeDto[] = []) {
    return createFakeApi({
      nodes: [
        ...nodes(),
        { id: "req-c", parent_id: "col", kind: "request", name: "C", content: { method: "GET", url: "https://c.test" } },
        ...more,
      ],
      environments: [{ id: "env-dev", name: "Dev" }],
    });
  }

  /** Drafts as the storage gives them: any JSON object, whatever its shape. */
  function drafts(entries: Record<string, unknown>): Map<string, NodeContent> {
    return new Map(Object.entries(entries)) as Map<string, NodeContent>;
  }

  it("opens the saved tabs in their order and shows the saved active one", async () => {
    const api = seeded();
    const tabs = new Tabs();

    await tabs.restore(api, SAVED, new Map(), "fold");

    expect(tabs.list.map((tab) => [tab.kind, tab.id])).toEqual([
      ["request", "req-a"],
      ["folder", "fold"],
      ["environment", "env:env-dev"],
    ]);
    expect(tabs.activeId).toBe("fold");
    expect(tabs.active).toBe(tabs.list[1]);
    expect(tabs.list.some((tab) => tab.dirty)).toBe(false);
    expect(toasts.items).toEqual([]);
  });

  it("shows the first tab when the saved active one is not there", async () => {
    const api = seeded();
    const tabs = new Tabs();
    await tabs.restore(api, SAVED, new Map(), "gone");
    expect(tabs.activeId).toBe("req-a");

    const none = new Tabs();
    await none.restore(api, SAVED, new Map(), null);
    expect(none.activeId).toBe("req-a");
  });

  it("shows an environment tab that was the active one", async () => {
    const tabs = new Tabs();
    await tabs.restore(seeded(), SAVED, new Map(), "env:env-dev");
    expect(tabs.activeId).toBe("env:env-dev");
  });

  it("reads the nodes one at a time", async () => {
    const api = seeded();
    const getNode = api.getNode.bind(api);
    let running = 0;
    let most = 0;
    const spy = vi.spyOn(api, "getNode").mockImplementation(async (id) => {
      most = Math.max(most, ++running);
      // Another read started meanwhile would be counted before this one ends.
      await new Promise((resolve) => setTimeout(resolve, 1));
      const node = await getNode(id);
      running--;
      return node;
    });

    await new Tabs().restore(api, REQUESTS, new Map(), null);

    expect(spy.mock.calls.map(([id]) => id)).toEqual(["req-a", "req-b", "req-c"]);
    expect(most).toBe(1);
  });

  it("leaves out a node and an environment that cannot be read, without a notification", async () => {
    const api = seeded();
    const tabs = new Tabs();
    const saved: SavedTab[] = [
      { kind: "node", id: "gone" },
      { kind: "node", id: "req-b" },
      { kind: "environment", id: "env-gone" },
      { kind: "node", id: "col" },
    ];

    await tabs.restore(api, saved, new Map(), "gone");

    expect(tabs.list.map((tab) => tab.id)).toEqual(["req-b", "col"]);
    expect(tabs.activeId).toBe("req-b");
    expect(toasts.items).toEqual([]);
  });

  it("opens nothing and shows nothing when nothing was saved", async () => {
    const tabs = new Tabs();
    await tabs.restore(seeded(), [], new Map(), null);
    expect(tabs.list).toEqual([]);
    expect(tabs.activeId).toBeNull();
  });

  it("opens a tab with a draft as changed and shows the address of the draft", async () => {
    const api = seeded();
    const tabs = new Tabs();
    const draft = normalizeContent("request", { method: "GET", url: "https://draft.test" });

    await tabs.restore(api, REQUESTS, drafts({ "req-b": draft }), "req-b");

    const tab = tabs.list[1] as Tab<RequestDraft>;
    expect(tab.dirty).toBe(true);
    expect(tab.draft.url).toBe("https://draft.test");
    expect(tab.draft.method).toBe("GET");
    // What is stored is still the content of the node.
    expect(tab.saved.url).toBe("https://b.test");
    expect(tab.saved.method).toBe("POST");
    expect(tabs.list[0].dirty).toBe(false);
    expect(tabs.list[2].dirty).toBe(false);
    expect(api.draftCalls).toEqual([]);
  });

  it("does not take a draft that equals the stored content for a change", async () => {
    const api = seeded();
    const tabs = new Tabs();

    await tabs.restore(api, REQUESTS, drafts({ "req-b": { method: "POST", url: "https://b.test" } }), null);

    expect(tabs.list[1].dirty).toBe(false);
    expect(toasts.items).toEqual([]);
  });

  it("normalizes a draft of an older shape and takes the keys it does not have from the node", async () => {
    const api = seeded([
      {
        id: "req-s",
        parent_id: "col",
        kind: "request",
        name: "S",
        content: { method: "GET", url: "https://s.test", scripts: { test: "check();" }, settings: { max_redirects: 3 } },
      },
    ]);
    const tabs = new Tabs();
    // Written by a build that knew neither the scripts nor the settings.
    const same = { method: "GET", url: "https://s.test", query: [], path_variables: [], headers: [], body: { mode: "none" }, auth: null };

    await tabs.restore(api, [{ kind: "node", id: "req-s" }], drafts({ "req-s": same }), null);

    const tab = tabs.list[0] as Tab<RequestDraft>;
    expect(tab.draft.scripts).toEqual({ pre_request: "", test: "check();" });
    expect(tab.draft.settings).toEqual({ follow_redirects: null, max_redirects: 3, strict_ssl: null, disable_body_pruning: null });
    expect(tab.draft.disabled_auto_headers).toEqual([]);
    expect(tab.dirty).toBe(false);

    const edited = new Tabs();
    await edited.restore(api, [{ kind: "node", id: "req-s" }], drafts({ "req-s": { ...same, url: "https://old.test" } }), null);
    const old = edited.list[0] as Tab<RequestDraft>;
    expect(old.dirty).toBe(true);
    expect(old.draft.url).toBe("https://old.test");
    expect(old.draft.scripts).toEqual({ pre_request: "", test: "check();" });
  });

  it("gives a tab at the end to a draft whose node was not among the saved tabs", async () => {
    const api = seeded();
    const tabs = new Tabs();

    await tabs.restore(api, [{ kind: "node", id: "req-a" }], drafts({ "req-c": { method: "GET", url: "https://orphan.test" } }), "req-a");

    expect(tabs.list.map((tab) => tab.id)).toEqual(["req-a", "req-c"]);
    const orphan = tabs.list[1] as Tab<RequestDraft>;
    expect(orphan.dirty).toBe(true);
    expect(orphan.draft.url).toBe("https://orphan.test");
    expect(tabs.activeId).toBe("req-a");
  });

  it.each([
    ["headers that are not a list", { headers: 5 }],
    ["a body without its rows", { body: { mode: "url_encoded" } }],
  ])("drops a draft that cannot be laid over the node: %s", async (_name, broken) => {
    const api = seeded();
    const tabs = new Tabs();

    await tabs.restore(api, REQUESTS, drafts({ "req-b": broken }), "req-c");

    expect(tabs.list.map((tab) => tab.id)).toEqual(["req-a", "req-b", "req-c"]);
    const tab = tabs.list[1] as Tab<RequestDraft>;
    expect(tab.dirty).toBe(false);
    expect(tab.draft.url).toBe("https://b.test");
    expect(tab.draft).toEqual(tab.saved);
    expect(toasts.items).toEqual([]);
    expect(api.draftCalls).toContainEqual(["req-b", null]);
    expect(tabs.activeId).toBe("req-c");
  });

  it("drops a draft of a collection whose variables are not a list", async () => {
    const api = seeded();
    const tabs = new Tabs();
    const saved: SavedTab[] = [
      { kind: "node", id: "req-a" },
      { kind: "node", id: "col" },
      { kind: "node", id: "req-c" },
    ];

    await tabs.restore(api, saved, drafts({ col: { variables: 1 } }), "req-c");

    expect(tabs.list.map((tab) => tab.id)).toEqual(["req-a", "col", "req-c"]);
    const tab = tabs.list[1] as Tab<CollectionDraft>;
    expect(tab.dirty).toBe(false);
    expect(tab.draft).toEqual({ auth: null, variables: [], scripts: { pre_request: "", test: "" }, description: "" });
    expect(toasts.items).toEqual([]);
    expect(api.draftCalls).toContainEqual(["col", null]);
    expect(tabs.activeId).toBe("req-c");
  });

  it("gives a tab at the end to an orphan draft that cannot be used, and deletes the draft", async () => {
    const api = seeded();
    const tabs = new Tabs();

    await tabs.restore(api, [{ kind: "node", id: "req-a" }], drafts({ "req-c": { headers: 5 } }), "req-a");

    expect(tabs.list.map((tab) => tab.id)).toEqual(["req-a", "req-c"]);
    const orphan = tabs.list[1] as Tab<RequestDraft>;
    expect(orphan.dirty).toBe(false);
    expect(orphan.draft.url).toBe("https://c.test");
    expect(toasts.items).toEqual([]);
    expect(api.draftCalls).toEqual([["req-c", null]]);
  });

  it("goes on when the deletion of a draft that cannot be used fails", async () => {
    const api = seeded();
    const saveDraft = vi.spyOn(api, "saveDraft").mockRejectedValue("storage is locked");
    const tabs = new Tabs();

    await expect(tabs.restore(api, REQUESTS, drafts({ "req-b": { headers: 5 } }), "req-c")).resolves.toBeUndefined();

    expect(saveDraft).toHaveBeenCalledWith("req-b", null);
    expect(tabs.list.map((tab) => tab.id)).toEqual(["req-a", "req-b", "req-c"]);
    expect(tabs.list.some((tab) => tab.dirty)).toBe(false);
    expect(tabs.activeId).toBe("req-c");
    expect(toasts.items).toEqual([]);
  });
});
