// In-memory `Api` for component tests. Not imported by the application, so it is not part of its build.

import type { Api } from "../api";
import {
  GLOBALS_ID,
  type EnvSummary,
  type HistoryEntry,
  type HistoryItem,
  type NodeContent,
  type NodeDto,
  type NodeKind,
  type RequestContent,
  type ScopeVariable,
  type SendResult,
  type SessionDto,
  type SettingsDto,
  type TimingSample,
  type Variable,
  type VariableRow,
  type WindowState,
} from "../types";

/** An entry of the history as the storage keeps it: whether its request still exists is told by the tree. */
export type StoredHistoryEntry = Omit<HistoryEntry, "node_exists">;

/** Answers `sendRequest` in a test. */
export type SendHandler = (sendId: string, requestId: string, draft: RequestContent | null) => Promise<SendResult>;

/** The keys of a `SendResult` that hold what the scripts left, for a send without scripts. */
export function noScriptResults(): Pick<
  SendResult,
  "tests" | "console" | "console_truncated" | "script_errors" | "variable_changes"
> {
  return { tests: [], console: [], console_truncated: false, script_errors: [], variable_changes: [] };
}

/** What a test can set instead of the defaults. */
export interface FakeSeed {
  /** Nodes of the tree with their content; default — {@link defaultNodes}. */
  nodes?: NodeDto[];
  /** Environments of the switcher; default — none. */
  environments?: EnvSummary[];
  /** Variables by the id of their environment, `globals` for the global ones; default — none. */
  variables?: Record<string, Variable[]>;
  /** Id of the active environment; default — `null`. */
  activeEnvironment?: string | null;
  /** Answers `sendRequest`; without it a send rejects. */
  onSend?: SendHandler;
  /** Response bodies by id; `responseBody` and `copyResponseBody` return them whatever `pretty` is. */
  bodies?: Record<string, Uint8Array>;
  /** Entries of the history, in any order; default — none. A send does not add one: a test does. */
  history?: StoredHistoryEntry[];
  /**
   * What `loadSession` gives; default — the window of a new database: no tabs, no drafts. Without `measure`
   * the program is not being measured.
   */
  session?: Omit<SessionDto, "measure"> & { measure?: boolean };
  /** The saved application settings; default — the ones of a new database. */
  settings?: SettingsDto;
  /** Ids of the collections whose scripts may run; default — every collection of the seed. */
  scriptsAllowed?: string[];
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
  /** Entries of the history, in any order; a test may add to them at any time. */
  history: StoredHistoryEntry[];
  /** What `pickFiles` resolves to; empty — the user closes the window. A test may replace it at any time. */
  filesToPick: string[];
  /** What was passed to `saveWindowState`, in order. */
  windowStates: WindowState[];
  /** What was passed to `saveDraft`, in order: the id of the node and the draft, `null` for a deletion. */
  draftCalls: [string, NodeContent | null][];
  /** What was passed to `reportTimings`, call by call, in order. */
  timingCalls: TimingSample[][];
  /** What was passed to `setScriptsAllowed`, in order: the id of the node and the consent. */
  allowCalls: [string, boolean][];
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
  /** A variable as the core returns it: with every key. */
  const variableRow = (variable: Variable): VariableRow => ({
    ...variable,
    secret: variable.secret ?? false,
    enabled: variable.enabled ?? true,
    postman_extra: variable.postman_extra ?? {},
  });
  const variablesOf = (id: string): VariableRow[] => structuredClone(seed.variables?.[id] ?? []).map(variableRow);
  // The globals are kept next to the environments, as in the storage, but are not one of them.
  const environments = [{ id: GLOBALS_ID, name: "Globals" }, ...(seed.environments ?? [])].map((environment) => ({
    ...environment,
    variables: variablesOf(environment.id),
  }));
  let activeEnvironment = seed.activeEnvironment ?? null;
  let settings: SettingsDto = structuredClone(
    seed.settings ?? { timeout_ms: 0, verify_tls: true, proxy: { mode: "system" }, motion: "normal" },
  );
  const allowed = new Set(
    seed.scriptsAllowed ?? nodes.filter((node) => node.kind === "collection").map((node) => node.id),
  );
  const closeHandlers: (() => Promise<boolean> | boolean)[] = [];
  let lastId = 0;
  const newId = () => `new-${++lastId}`;

  const findNode = (id: string): NodeDto => {
    const node = nodes.find((n) => n.id === id);
    if (!node) throw `node not found: ${id}`;
    return node;
  };

  const findEnvironment = (id: string) => {
    const environment = environments.find((candidate) => candidate.id === id);
    if (!environment) throw "environment not found";
    return environment;
  };

  /** The id of the collection of the node: the node itself or its topmost ancestor. */
  const collectionOf = (id: string): string => {
    let node = findNode(id);
    while (node.parent_id !== null) node = findNode(node.parent_id);
    return node.id;
  };

  /** The name as the core stores it: trimmed and not empty. */
  const nodeName = (name: string): string => {
    const trimmed = name.trim();
    if (trimmed === "") throw "name is empty";
    return trimmed;
  };

  /** The nodes under `id` at any depth, parents before children. */
  const descendants = (id: string): NodeDto[] =>
    nodes.filter((node) => node.parent_id === id).flatMap((child) => [child, ...descendants(child.id)]);

  /** A new node of the kind with the content the core gives it. */
  const blankNode = (parentId: string | null, kind: NodeKind, name: string): NodeDto => {
    const base = { id: newId(), parent_id: parentId, name };
    return kind === "request" ? { ...base, kind, content: { method: "GET", url: "" } } : { ...base, kind, content: {} };
  };

  const api: FakeApi = {
    onSend: seed.onSend ?? (() => Promise.reject("fakeApi: the test did not set a sendRequest answer")),
    bodies: new Map(Object.entries(seed.bodies ?? {})),
    cancelled: [],
    released: [],
    history: structuredClone(seed.history ?? []),
    filesToPick: [],
    windowStates: [],
    draftCalls: [],
    timingCalls: [],
    allowCalls: [],

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
    async createNode(parentId, kind, name) {
      // The rules of the tree, as the core has them: collections are the roots, a request has no children.
      const parent = parentId === null ? null : findNode(parentId);
      if ((kind === "collection") !== (parent === null)) throw "a collection is a root, anything else is not";
      if (parent?.kind === "request") throw "a request cannot have children";
      // The position among the siblings is the order of the list, so the last place is its end.
      const node = blankNode(parentId, kind, nodeName(name));
      nodes.push(node);
      // The scripts of a collection made in the application may run at once.
      if (kind === "collection") allowed.add(node.id);
      return node.id;
    },
    async renameNode(id, name) {
      findNode(id).name = nodeName(name);
    },
    async duplicateNode(id) {
      const original = findNode(id);
      const ids = new Map<string, string>([[id, newId()]]);
      const copies = descendants(id).map((node) => {
        ids.set(node.id, newId());
        return { ...structuredClone(node), id: ids.get(node.id)!, parent_id: ids.get(node.parent_id!)! };
      });
      const copy = { ...structuredClone(original), id: ids.get(id)!, name: `${original.name} Copy` };
      // Right after the original among its siblings; where the descendants stand in the list does not matter.
      nodes.splice(nodes.indexOf(original) + 1, 0, copy, ...copies);
      // The copy of a collection whose scripts may run may run its scripts too.
      if (allowed.has(id)) allowed.add(copy.id);
      return copy.id;
    },
    async moveNode(id, parentId, index) {
      // The rules of the tree, in the order and in the words of the core.
      const node = findNode(id);
      const parent = parentId === null ? null : findNode(parentId);
      if (node.kind === "collection" && parent !== null) throw "a collection can only be a root";
      if (node.kind !== "collection" && parent === null) throw "only a collection can be a root";
      if (parent?.kind === "request") throw "a request cannot have children";
      let cursor: NodeDto | null = parent;
      while (cursor !== null) {
        if (cursor.id === id) throw "a node cannot be moved into itself or its descendant";
        cursor = cursor.parent_id === null ? null : findNode(cursor.parent_id);
      }
      // The position among the siblings is the order of the list: the node is taken out and put back right
      // before the sibling that stands at `index` now, or after the last one.
      nodes.splice(nodes.indexOf(node), 1);
      node.parent_id = parentId;
      const siblings = nodes.filter((candidate) => candidate.parent_id === parentId);
      const at = index < siblings.length ? nodes.indexOf(siblings[index]) : nodes.length;
      nodes.splice(at, 0, node);
    },
    async deleteNode(id) {
      const removed = new Set([findNode(id), ...descendants(id)]);
      for (let index = nodes.length - 1; index >= 0; index--) {
        if (removed.has(nodes[index])) nodes.splice(index, 1);
      }
      allowed.delete(id);
    },
    async importFiles() {
      return [];
    },
    async importGlobals() {},
    async environments() {
      // By the lower-cased name, as the core returns them; the globals are not an environment.
      const key = (environment: EnvSummary) => environment.name.toLowerCase();
      return environments
        .filter((environment) => environment.id !== GLOBALS_ID)
        .map(({ id, name }) => ({ id, name }))
        .sort((a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0));
    },
    async activeEnvironment() {
      return activeEnvironment;
    },
    async setActiveEnvironment(id) {
      activeEnvironment = id;
    },
    async getEnvironment(id) {
      const { name, variables } = findEnvironment(id);
      return { id, name, variables: structuredClone(variables) };
    },
    async saveEnvironment(id, name, variables) {
      const environment = findEnvironment(id);
      if (id !== GLOBALS_ID) environment.name = nodeName(name);
      environment.variables = structuredClone(variables).map(variableRow);
    },
    async createEnvironment(name) {
      const environment = { id: newId(), name: nodeName(name), variables: [] };
      environments.push(environment);
      return environment.id;
    },
    async deleteEnvironment(id) {
      if (id === GLOBALS_ID) throw "globals cannot be deleted";
      environments.splice(environments.indexOf(findEnvironment(id)), 1);
      if (activeEnvironment === id) activeEnvironment = null;
    },
    async variablesInScope(nodeId) {
      // The scopes from the widest to the narrowest, as the core has them; a later one wins a name. What the
      // core does besides is left out: a value that refers to another variable is given as it is written.
      let root = findNode(nodeId);
      while (root.parent_id !== null) root = findNode(root.parent_id);
      const scopes: [ScopeVariable["scope"], Variable[]][] = [
        ["global", findEnvironment(GLOBALS_ID).variables],
        ["collection", root.kind === "collection" ? (root.content.variables ?? []) : []],
        ["environment", activeEnvironment === null ? [] : findEnvironment(activeEnvironment).variables],
      ];
      const found = new Map<string, ScopeVariable>();
      for (const [scope, variables] of scopes) {
        for (const variable of variables.filter((candidate) => candidate.enabled !== false)) {
          const secret = variable.secret === true;
          // The value of a secret variable never leaves the core.
          found.set(variable.key, { name: variable.key, scope, value: secret ? null : variable.value, secret });
        }
      }
      const byName = [...found.values()].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
      const dynamic = ["$guid", "$randomUUID", "$timestamp", "$isoTimestamp", "$randomInt"]
        .filter((name) => !found.has(name))
        .map((name): ScopeVariable => ({ name, scope: "dynamic", value: null, secret: false }));
      return [...byName, ...dynamic];
    },
    // No file is written: the path is made of the name the core gives the file, as if the user had kept it.
    async exportCollection(id) {
      const node = findNode(id);
      if (node.kind !== "collection") throw "not a collection";
      return `C:\\Exports\\${node.name}.postman_collection.json`;
    },
    async exportEnvironment(id) {
      const { name } = findEnvironment(id);
      return id === GLOBALS_ID
        ? "C:\\Exports\\globals.postman_globals.json"
        : `C:\\Exports\\${name}.postman_environment.json`;
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
    async saveResponseBody(bodyId) {
      if (!api.bodies.has(bodyId)) throw `response body not found: ${bodyId}`;
      return "C:\\Exports\\response.txt";
    },
    async releaseBody(bodyId) {
      api.released.push(bodyId);
      api.bodies.delete(bodyId);
    },
    async pickFiles(multiple) {
      // Like the system window: one file at most unless several were asked for.
      return multiple ? [...api.filesToPick] : api.filesToPick.slice(0, 1);
    },
    async formatJson(text) {
      // Enough for a test double; the application formats in the core, which keeps every token as written.
      try {
        return JSON.stringify(JSON.parse(text), null, 2);
      } catch {
        throw "not valid JSON";
      }
    },
    async historyList(limit, beforeId) {
      if (limit < 1 || limit > 200) throw "limit must be from 1 to 200";
      return api.history
        .filter((entry) => beforeId === null || entry.id < beforeId)
        .sort((a, b) => b.id - a.id)
        .slice(0, limit)
        .map(
          ({ id, sent_at, node_id, name, method, url, status, error_kind, total_ms, size }): HistoryItem => ({
            id,
            sent_at,
            node_id,
            name,
            method,
            url,
            status,
            error_kind,
            total_ms,
            size,
          }),
        );
    },
    async historyGet(id) {
      const entry = api.history.find((candidate) => candidate.id === id);
      if (!entry) throw `history entry not found: ${id}`;
      return { ...structuredClone(entry), node_exists: nodes.some((node) => node.id === entry.node_id) };
    },
    async historyClear() {
      api.history.length = 0;
    },
    async loadSession() {
      return structuredClone({
        measure: false,
        ...(seed.session ?? { window: { sidebar_width: 280, placement: "auto", split: 50, tabs: [], active: null }, drafts: [] }),
      });
    },
    async saveWindowState(state) {
      api.windowStates.push(structuredClone(state));
    },
    async saveDraft(nodeId, content) {
      api.draftCalls.push([nodeId, structuredClone(content)]);
    },
    async getSettings() {
      return structuredClone(settings);
    },
    // The core checks the values as well; a test of a refusal makes this call reject.
    async saveSettings(saved) {
      settings = structuredClone(saved);
    },
    async reportTimings(samples) {
      api.timingCalls.push(structuredClone(samples));
    },
    async scriptsAllowed(nodeId) {
      return allowed.has(collectionOf(nodeId));
    },
    async setScriptsAllowed(nodeId, consent) {
      const collection = collectionOf(nodeId);
      api.allowCalls.push([nodeId, consent]);
      if (consent) allowed.add(collection);
      else allowed.delete(collection);
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
