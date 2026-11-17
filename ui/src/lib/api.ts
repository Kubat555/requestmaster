// The commands of the Tauri shell behind a typed interface. Components take `Api` from the Svelte context
// (`getContext("api")`) and never import `tauriApi` directly, so that tests can pass an in-memory one.

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  EnvSummary,
  EnvironmentDto,
  GlobalsPolicy,
  HistoryEntry,
  HistoryItem,
  ImportFile,
  ImportResult,
  NodeContent,
  NodeDto,
  NodeKind,
  RequestContent,
  ScopeVariable,
  SendResult,
  SessionDto,
  SettingsDto,
  TimingSample,
  TreeRow,
  VariableRow,
  WindowState,
} from "./types";

/** Everything the window asks the core for. A failed call rejects with a string in English. */
export interface Api {
  /** All nodes of the tree without their content. */
  tree(): Promise<TreeRow[]>;
  /** The node with its content. */
  getNode(id: string): Promise<NodeDto>;
  /** Replaces the content of the node. */
  saveNodeContent(id: string, content: NodeContent): Promise<void>;
  /**
   * Creates a node as the last child of `parentId` (`null` — as the last collection) and resolves to its id.
   * The name is trimmed; an empty one is rejected.
   */
  createNode(parentId: string | null, kind: NodeKind, name: string): Promise<string>;
  /** Changes the name of the node; the name is trimmed, an empty one is rejected. */
  renameNode(id: string, name: string): Promise<void>;
  /** Copies the node with everything inside right after the original; resolves to the id of the copy. */
  duplicateNode(id: string): Promise<string>;
  /**
   * Moves the node under `parentId` (`null` — among the collections) to the place `index`, counted among the
   * children of that parent without the moved node; an index past the end means the last place. A place the
   * rules of the tree forbid is rejected, and nothing changes.
   */
  moveNode(id: string, parentId: string | null, index: number): Promise<void>;
  /** Deletes the node with everything inside. */
  deleteNode(id: string): Promise<void>;
  /** Imports Postman files; every file gets its own result. */
  importFiles(files: ImportFile[]): Promise<ImportResult[]>;
  /** Merges a globals file with the policy the user chose for the conflicts. */
  importGlobals(text: string, policy: GlobalsPolicy): Promise<void>;
  /** Environments of the switcher, by name. */
  environments(): Promise<EnvSummary[]>;
  /** Id of the active environment. */
  activeEnvironment(): Promise<string | null>;
  /** Chooses the active environment; `null` clears the choice. */
  setActiveEnvironment(id: string | null): Promise<void>;
  /** The environment with its variables; the id `globals` — the set of global variables. An unknown id is rejected. */
  getEnvironment(id: string): Promise<EnvironmentDto>;
  /**
   * Replaces the name and the variables of the environment. The name is trimmed, an empty one is rejected;
   * for the id `globals` it is ignored. An unknown id is rejected: nothing is created.
   */
  saveEnvironment(id: string, name: string, variables: VariableRow[]): Promise<void>;
  /** Creates an environment without variables and resolves to its id. The name is trimmed; an empty one is rejected. */
  createEnvironment(name: string): Promise<string>;
  /** Deletes the environment; the active one stops being active. The globals cannot be deleted. */
  deleteEnvironment(id: string): Promise<void>;
  /**
   * The names `{{name}}` resolves at the node (a request, a folder or a collection) with the active environment,
   * by what is in the storage. The value of a secret variable does not come.
   */
  variablesInScope(nodeId: string): Promise<ScopeVariable[]>;
  /**
   * Asks where to save and writes the collection as a Postman file. Resolves to the path of the file; `null` —
   * the user closed the window that asks, and nothing was written. A node that is not a collection is rejected.
   */
  exportCollection(id: string): Promise<string | null>;
  /** The same for an environment; the id `globals` — the set of global variables. */
  exportEnvironment(id: string): Promise<string | null>;
  /** Sends the request, or its unsaved `draft`. `sendId` is made by the caller so that it can cancel. */
  sendRequest(sendId: string, requestId: string, draft: RequestContent | null): Promise<SendResult>;
  /** Stops the send; an unknown id is not an error. */
  cancelSend(sendId: string): Promise<void>;
  /** Bytes of a response body: whole or its beginning, re-indented when `pretty` and the body is JSON. */
  responseBody(bodyId: string, pretty: boolean): Promise<Uint8Array>;
  /** Text of a response body for the clipboard: what `responseBody` gives, with bytes that are not UTF-8 replaced. */
  copyResponseBody(bodyId: string, pretty: boolean): Promise<string>;
  /**
   * Writes a response body, as received and unpacked, to a file the user chooses. Resolves to the path of the
   * file, or to `null` when the user closed the window that asks where to save.
   */
  saveResponseBody(bodyId: string): Promise<string | null>;
  /** Deletes the stored body; an unknown id is not an error. */
  releaseBody(bodyId: string): Promise<void>;
  /**
   * Asks the user for a file of this computer, or for several when `multiple`, with the system window. Resolves
   * to the full paths; an empty list — the user closed the window.
   */
  pickFiles(multiple: boolean): Promise<string[]>;
  /** Re-indents JSON text; rejects when the text is not valid JSON. */
  formatJson(text: string): Promise<string>;
  /**
   * Sends of the history, the newest first: at most `limit` (1 to 200) of them, with `beforeId` — only the ones
   * older than that entry.
   */
  historyList(limit: number, beforeId: number | null): Promise<HistoryItem[]>;
  /** One entry of the history with the request and the response as they were kept; an unknown id is rejected. */
  historyGet(id: number): Promise<HistoryEntry>;
  /** Deletes the whole history. */
  historyClear(): Promise<void>;
  /**
   * The layout of the window with its open tabs, and the unsaved drafts, as they were remembered. Tabs and
   * drafts of nodes and environments that no longer exist do not come.
   */
  loadSession(): Promise<SessionDto>;
  /** Remembers the layout of the window and its open tabs. */
  saveWindowState(state: WindowState): Promise<void>;
  /** Remembers the unsaved draft of the node; `null` forgets it. */
  saveDraft(nodeId: string, content: NodeContent | null): Promise<void>;
  /** The application settings; the defaults while nothing is saved. */
  getSettings(): Promise<SettingsDto>;
  /**
   * Saves the application settings; they are used from the next send on. A timeout out of range and a manual
   * proxy address that is not an `http://` or `https://` URL are rejected, and the saved settings stay.
   */
  saveSettings(settings: SettingsDto): Promise<void>;
  /** Appends the measurements to the file of timings; a program that is not being measured does nothing. */
  reportTimings(samples: TimingSample[]): Promise<void>;
  /**
   * Calls `handler` when the user asks to close the window; the window closes only when the handler answers
   * `true`. Resolves to the function that stops the listening.
   */
  onCloseRequested(handler: () => Promise<boolean> | boolean): Promise<() => void>;
}

/** `Api` over the commands of the Tauri shell; argument keys are `snake_case`, as the commands declare. */
export const tauriApi: Api = {
  tree: () => invoke<TreeRow[]>("tree"),
  getNode: (id) => invoke<NodeDto>("get_node", { id }),
  saveNodeContent: (id, content) => invoke<void>("save_node_content", { id, content }),
  createNode: (parentId, kind, name) => invoke<string>("create_node", { parent_id: parentId, kind, name }),
  renameNode: (id, name) => invoke<void>("rename_node", { id, name }),
  duplicateNode: (id) => invoke<string>("duplicate_node", { id }),
  moveNode: (id, parentId, index) => invoke<void>("move_node", { id, parent_id: parentId, index }),
  deleteNode: (id) => invoke<void>("delete_node", { id }),
  importFiles: (files) => invoke<ImportResult[]>("import_files", { files }),
  importGlobals: (text, policy) => invoke<void>("import_globals", { text, policy }),
  environments: () => invoke<EnvSummary[]>("environments"),
  activeEnvironment: () => invoke<string | null>("active_environment"),
  setActiveEnvironment: (id) => invoke<void>("set_active_environment", { id }),
  getEnvironment: (id) => invoke<EnvironmentDto>("get_environment", { id }),
  saveEnvironment: (id, name, variables) => invoke<void>("save_environment", { id, name, variables }),
  createEnvironment: (name) => invoke<string>("create_environment", { name }),
  deleteEnvironment: (id) => invoke<void>("delete_environment", { id }),
  variablesInScope: (nodeId) => invoke<ScopeVariable[]>("variables_in_scope", { node_id: nodeId }),
  exportCollection: (id) => invoke<string | null>("export_collection", { id }),
  exportEnvironment: (id) => invoke<string | null>("export_environment", { id }),
  sendRequest: (sendId, requestId, draft) =>
    invoke<SendResult>("send_request", { send_id: sendId, request_id: requestId, draft }),
  cancelSend: (sendId) => invoke<void>("cancel_send", { send_id: sendId }),
  // Raw bytes arrive as an ArrayBuffer.
  responseBody: async (bodyId, pretty) =>
    new Uint8Array(await invoke<ArrayBuffer>("response_body", { body_id: bodyId, pretty })),
  copyResponseBody: (bodyId, pretty) => invoke<string>("copy_response_body", { body_id: bodyId, pretty }),
  saveResponseBody: (bodyId) => invoke<string | null>("save_response_body", { body_id: bodyId }),
  releaseBody: (bodyId) => invoke<void>("release_body", { body_id: bodyId }),
  pickFiles: (multiple) => invoke<string[]>("pick_files", { multiple }),
  formatJson: (text) => invoke<string>("format_json", { text }),
  historyList: (limit, beforeId) => invoke<HistoryItem[]>("history_list", { limit, before_id: beforeId }),
  historyGet: (id) => invoke<HistoryEntry>("history_get", { id }),
  historyClear: () => invoke<void>("history_clear"),
  loadSession: () => invoke<SessionDto>("load_session"),
  saveWindowState: (state) => invoke<void>("save_window_state", { state }),
  saveDraft: (nodeId, content) => invoke<void>("save_draft", { node_id: nodeId, content }),
  getSettings: () => invoke<SettingsDto>("get_settings"),
  saveSettings: (settings) => invoke<void>("save_settings", { settings }),
  reportTimings: (samples) => invoke<void>("report_timings", { samples }),
  // `Window.onCloseRequested` waits for the handler, asynchronous or not, and then destroys the window itself
  // unless `preventDefault` was called; nothing has to be called afterwards. It needs the permissions
  // `core:event:allow-listen`, `core:event:allow-unlisten` and `core:window:allow-destroy` of the capability.
  onCloseRequested: (handler) =>
    getCurrentWindow().onCloseRequested(async (event) => {
      if (!(await handler())) event.preventDefault();
    }),
};
