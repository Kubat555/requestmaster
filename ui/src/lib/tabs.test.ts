import { describe, expect, it, vi } from "vitest";
import { Tabs, type Tab } from "./tabs.svelte";
import { createFakeApi } from "./testing/fakeApi";
import { normalizeContent, type CollectionDraft, type FolderDraft, type NodeDto, type RequestDraft } from "./types";

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

  it("writes out the defaults of a folder and keeps what is set", () => {
    expect(normalizeContent("folder", {})).toEqual({ auth: null, scripts: { pre_request: "", test: "" } });
    const folder = normalizeContent("folder", { auth: { type: "no_auth" }, scripts: { test: "t();" } });
    expect(folder).toEqual({ auth: { type: "no_auth" }, scripts: { pre_request: "", test: "t();" } });
    expect(normalizeContent("folder", folder)).toEqual(folder);
  });

  it("writes out the defaults of a collection", () => {
    expect(normalizeContent("collection", { variables: [{ key: "host", value: "h" }] })).toEqual({
      auth: null,
      variables: [{ key: "host", value: "h", secret: false, enabled: true, postman_extra: {} }],
      scripts: { pre_request: "", test: "" },
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
    expect(collection.draft).toEqual({ auth: null, variables: [], scripts: { pre_request: "", test: "" } });
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
    expect(folder.draft).toEqual({ auth: null, scripts: { pre_request: "", test: "" } });
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
