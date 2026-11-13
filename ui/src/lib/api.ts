// The commands of the Tauri shell behind a typed interface. Components take `Api` from the Svelte context
// (`getContext("api")`) and never import `tauriApi` directly, so that tests can pass an in-memory one.

import { invoke } from "@tauri-apps/api/core";
import type {
  EnvSummary,
  GlobalsPolicy,
  ImportFile,
  ImportResult,
  NodeContent,
  NodeDto,
  RequestContent,
  SendResult,
  TreeRow,
} from "./types";

/** Everything the window asks the core for. A failed call rejects with a string in English. */
export interface Api {
  /** All nodes of the tree without their content. */
  tree(): Promise<TreeRow[]>;
  /** The node with its content. */
  getNode(id: string): Promise<NodeDto>;
  /** Replaces the content of the node. */
  saveNodeContent(id: string, content: NodeContent): Promise<void>;
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
  /** Sends the request, or its unsaved `draft`. `sendId` is made by the caller so that it can cancel. */
  sendRequest(sendId: string, requestId: string, draft: RequestContent | null): Promise<SendResult>;
  /** Stops the send; an unknown id is not an error. */
  cancelSend(sendId: string): Promise<void>;
  /** Bytes of a response body: whole or its beginning, re-indented when `pretty` and the body is JSON. */
  responseBody(bodyId: string, pretty: boolean): Promise<Uint8Array>;
  /** Deletes the stored body; an unknown id is not an error. */
  releaseBody(bodyId: string): Promise<void>;
}

/** `Api` over the commands of the Tauri shell; argument keys are `snake_case`, as the commands declare. */
export const tauriApi: Api = {
  tree: () => invoke<TreeRow[]>("tree"),
  getNode: (id) => invoke<NodeDto>("get_node", { id }),
  saveNodeContent: (id, content) => invoke<void>("save_node_content", { id, content }),
  importFiles: (files) => invoke<ImportResult[]>("import_files", { files }),
  importGlobals: (text, policy) => invoke<void>("import_globals", { text, policy }),
  environments: () => invoke<EnvSummary[]>("environments"),
  activeEnvironment: () => invoke<string | null>("active_environment"),
  setActiveEnvironment: (id) => invoke<void>("set_active_environment", { id }),
  sendRequest: (sendId, requestId, draft) =>
    invoke<SendResult>("send_request", { send_id: sendId, request_id: requestId, draft }),
  cancelSend: (sendId) => invoke<void>("cancel_send", { send_id: sendId }),
  // Raw bytes arrive as an ArrayBuffer.
  responseBody: async (bodyId, pretty) =>
    new Uint8Array(await invoke<ArrayBuffer>("response_body", { body_id: bodyId, pretty })),
  releaseBody: (bodyId) => invoke<void>("release_body", { body_id: bodyId }),
};
