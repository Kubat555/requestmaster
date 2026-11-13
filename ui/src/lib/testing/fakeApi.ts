// In-memory `Api` for component tests. Not imported by the application, so it is not part of its build.

import type { Api } from "../api";
import type { EnvSummary, NodeDto, RequestContent, SendResult } from "../types";

/** Answers `sendRequest` in a test. */
export type SendHandler = (sendId: string, requestId: string, draft: RequestContent | null) => Promise<SendResult>;

/** What a test can set instead of the defaults. */
export interface FakeSeed {
  /** Nodes of the tree with their content; default — {@link defaultNodes}. */
  nodes?: NodeDto[];
  /** Environments of the switcher; default — none. */
  environments?: EnvSummary[];
  /** Id of the active environment; default — `null`. */
  activeEnvironment?: string | null;
  /** Answers `sendRequest`; without it a send rejects. */
  onSend?: SendHandler;
  /** Response bodies by id; `responseBody` and `copyResponseBody` return them whatever `pretty` is. */
  bodies?: Record<string, Uint8Array>;
}

/** `Api` in memory plus what a test inspects or changes after creation. */
export interface FakeApi extends Api {
  /** Answers `sendRequest`; a test may replace it at any time. */
  onSend: SendHandler;
  /** Response bodies by id; `releaseBody` removes from here. */
  bodies: Map<string, Uint8Array>;
  /** Ids passed to `cancelSend`, in order. */
  cancelled: string[];
  /** Ids passed to `releaseBody`, in order. */
  released: string[];
  /**
   * The user asks to close the window: calls what was given to `onCloseRequested` and resolves to whether the
   * window would close.
   */
  requestClose(): Promise<boolean>;
}

/** The default tree: one collection with two folders and three requests. */
export function defaultNodes(): NodeDto[] {
  return [
    { id: "col-1", parent_id: null, kind: "collection", name: "Demo API", content: {} },
    { id: "fold-users", parent_id: "col-1", kind: "folder", name: "Users", content: {} },
    { id: "fold-orders", parent_id: "col-1", kind: "folder", name: "Orders", content: {} },
    {
      id: "req-list-users",
      parent_id: "fold-users",
      kind: "request",
      name: "List users",
      content: { method: "GET", url: "https://api.example.test/users" },
    },
    {
      id: "req-create-user",
      parent_id: "fold-users",
      kind: "request",
      name: "Create user",
      content: { method: "POST", url: "https://api.example.test/users" },
    },
    {
      id: "req-get-order",
      parent_id: "fold-orders",
      kind: "request",
      name: "Get order",
      content: { method: "GET", url: "https://api.example.test/orders/1" },
    },
  ];
}

/** Creates an in-memory `Api`. Like the real one, a failed call rejects with a string. */
export function createFakeApi(seed: FakeSeed = {}): FakeApi {
  const nodes = structuredClone(seed.nodes ?? defaultNodes());
  const environments = seed.environments ?? [];
  let activeEnvironment = seed.activeEnvironment ?? null;
  const closeHandlers: (() => Promise<boolean> | boolean)[] = [];

  const findNode = (id: string): NodeDto => {
    const node = nodes.find((n) => n.id === id);
    if (!node) throw `node not found: ${id}`;
    return node;
  };

  const api: FakeApi = {
    onSend: seed.onSend ?? (() => Promise.reject("fakeApi: the test did not set a sendRequest answer")),
    bodies: new Map(Object.entries(seed.bodies ?? {})),
    cancelled: [],
    released: [],

    async tree() {
      // `sort_order` is the position among the siblings, 0..n, in the order of the seed.
      const nextOrder = new Map<string | null, number>();
      return nodes.map((node) => {
        const sortOrder = nextOrder.get(node.parent_id) ?? 0;
        nextOrder.set(node.parent_id, sortOrder + 1);
        return {
          id: node.id,
          parent_id: node.parent_id,
          kind: node.kind,
          name: node.name,
          sort_order: sortOrder,
          method: node.kind === "request" ? node.content.method : null,
        };
      });
    },
    async getNode(id) {
      return structuredClone(findNode(id));
    },
    async saveNodeContent(id, content) {
      // The kind comes from the stored node, as in the core; the content is taken as is.
      const node = findNode(id);
      (node as { content: unknown }).content = structuredClone(content);
    },
    async importFiles() {
      return [];
    },
    async importGlobals() {},
    async environments() {
      return [...environments];
    },
    async activeEnvironment() {
      return activeEnvironment;
    },
    async setActiveEnvironment(id) {
      activeEnvironment = id;
    },
    sendRequest(sendId, requestId, draft) {
      return api.onSend(sendId, requestId, draft);
    },
    async cancelSend(sendId) {
      api.cancelled.push(sendId);
    },
    async responseBody(bodyId) {
      const body = api.bodies.get(bodyId);
      if (!body) throw `response body not found: ${bodyId}`;
      return body;
    },
    async copyResponseBody(bodyId) {
      return new TextDecoder().decode(await api.responseBody(bodyId, false));
    },
    async releaseBody(bodyId) {
      api.released.push(bodyId);
      api.bodies.delete(bodyId);
    },
    async formatJson(text) {
      // Enough for a test double; the application formats in the core, which keeps every token as written.
      try {
        return JSON.stringify(JSON.parse(text), null, 2);
      } catch {
        throw "not valid JSON";
      }
    },
    async onCloseRequested(handler) {
      closeHandlers.push(handler);
      return () => void closeHandlers.splice(closeHandlers.indexOf(handler), 1);
    },
    async requestClose() {
      const answers = await Promise.all(closeHandlers.map((handler) => handler()));
      return answers.every(Boolean);
    },
  };
  return api;
}
