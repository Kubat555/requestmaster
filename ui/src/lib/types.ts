// Data exchanged with the Tauri commands (the command contract of the plan) and the content of tree nodes.
// The node content mirrors the JSON of `crates/rqm-model`: a field that is absent has its default value,
// because the core skips defaults when it writes JSON.

/** Any JSON value. */
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

/** Original Postman JSON object of a row; the interface returns it unchanged. */
export type Extra = { [key: string]: Json };

// ---------------------------------------------------------------------------------------------------------
// Node content (mirror of rqm-model)
// ---------------------------------------------------------------------------------------------------------

/** Kind of a tree node. */
export type NodeKind = "collection" | "folder" | "request";

/** A key-value row: query parameter, path variable, header or url-encoded body field. */
export interface Param {
  key: string;
  value: string;
  /** Absent = `true`; `false` = the row is kept but not sent. */
  enabled?: boolean;
  postman_extra?: Extra;
}

/** A variable of a collection. */
export interface Variable {
  key: string;
  value: string;
  /** Absent = `false`; `true` = the value is masked. */
  secret?: boolean;
  /** Absent = `true`; `false` = the variable is kept but not substituted. */
  enabled?: boolean;
  postman_extra?: Extra;
}

/** Scripts of a node; an absent script is empty. */
export interface Scripts {
  pre_request?: string;
  test?: string;
}

/** Where an API key or a token is added to the request. */
export type ApiKeyPlacement = "header" | "query";

/** Authorization set on a node. */
export type Auth =
  | { type: "no_auth" }
  | { type: "basic"; username: string; password: string }
  | { type: "bearer"; token: string }
  | { type: "api_key"; key: string; value: string; placement: ApiKeyPlacement }
  | { type: "oauth2_token"; token: string; header_prefix: string; placement: ApiKeyPlacement }
  | { type: "unsupported"; auth_type: string };

/** Language of a raw body. */
export type RawLanguage = "text" | "json" | "xml" | "html" | "javascript";

/** Reference to a file on this computer used in a body. */
export interface FileRef {
  path: string;
  /** Absent = `false`; `true` = the user picked the file on this computer. */
  confirmed?: boolean;
}

/** Value of a form part: a text field or a list of files. */
export type FormValue = { text: string } | { files: FileRef[] };

/** One part of a `multipart/form-data` body. */
export interface FormPart {
  key: string;
  value: FormValue;
  /** Absent = `true`. */
  enabled?: boolean;
  /** Explicit `Content-Type` of the part; absent or `null` = chosen automatically. */
  content_type?: string | null;
  postman_extra?: Extra;
}

/** Request body; `mode` decides the shape of `data`. */
export type Body =
  | { mode: "none" }
  | { mode: "raw"; data: { text: string; language: RawLanguage } }
  | { mode: "url_encoded"; data: Param[] }
  | { mode: "form_data"; data: FormPart[] }
  | { mode: "binary"; data: { file: FileRef | null } }
  | { mode: "graphql"; data: { query: string; variables: string } };

/** Per-request settings; one that is absent or `null` is not set: the default applies. */
export interface RequestSettings {
  follow_redirects?: boolean | null;
  max_redirects?: number | null;
  strict_ssl?: boolean | null;
  disable_body_pruning?: boolean | null;
}

/** Content of a request node. */
export interface RequestContent {
  method: string;
  /** Address without the query part. */
  url: string;
  query?: Param[];
  path_variables?: Param[];
  headers?: Param[];
  /** Absent = no body. */
  body?: Body;
  /** Absent or `null` = inherit from the parent; `no_auth` = explicitly none. */
  auth?: Auth | null;
  scripts?: Scripts;
  settings?: RequestSettings;
  /** Lower-case names of automatic headers switched off for this request. */
  disabled_auto_headers?: string[];
}

/** Content of a collection node. */
export interface CollectionContent {
  /** Absent or `null` = nothing set at this level. */
  auth?: Auth | null;
  variables?: Variable[];
  scripts?: Scripts;
  /** The source text of the description (Markdown is not rendered); absent = there is none. */
  description?: string;
}

/** Content of a folder node. */
export interface FolderContent {
  /** Absent or `null` = inherit from the parent. */
  auth?: Auth | null;
  scripts?: Scripts;
  /** The source text of the description (Markdown is not rendered); absent = there is none. */
  description?: string;
}

/** Content of a node of any kind. */
export type NodeContent = CollectionContent | FolderContent | RequestContent;

// ---------------------------------------------------------------------------------------------------------
// Content as the editors hold it
// ---------------------------------------------------------------------------------------------------------

/** A key-value row with every default written out. */
export type ParamRow = Required<Param>;

/** A variable with every default written out. */
export type VariableRow = Required<Variable>;

/** A file of a body with every default written out; `confirmed` is `true` only for a path the system window gave. */
export type FileRow = Required<FileRef>;

/** Value of a form part whose files have every default written out. */
export type FormValueRow = { text: string } | { files: FileRow[] };

/** A part of a form-data body with every default written out; `content_type: null` = chosen automatically. */
export interface FormPartRow {
  key: string;
  value: FormValueRow;
  enabled: boolean;
  content_type: string | null;
  postman_extra: Extra;
}

/** A body whose rows, parts and files have every default written out. */
export type DraftBody =
  | Exclude<Body, { mode: "url_encoded" | "form_data" | "binary" }>
  | { mode: "url_encoded"; data: ParamRow[] }
  | { mode: "form_data"; data: FormPartRow[] }
  | { mode: "binary"; data: { file: FileRow | null } };

/** Scripts with both texts written out; an empty text is a script that is absent. */
export type ScriptsDraft = Required<Scripts>;

/** Per-request settings with every one written out; `null` = not set, the default applies. */
export interface RequestSettingsDraft {
  follow_redirects: boolean | null;
  max_redirects: number | null;
  strict_ssl: boolean | null;
  disable_body_pruning: boolean | null;
}

/** Content of a request after {@link normalizeContent}. */
export interface RequestDraft extends RequestContent {
  query: ParamRow[];
  path_variables: ParamRow[];
  headers: ParamRow[];
  body: DraftBody;
  /** `null` = inherit from the parent. */
  auth: Auth | null;
  scripts: ScriptsDraft;
  settings: RequestSettingsDraft;
  disabled_auto_headers: string[];
}

/** Content of a collection after {@link normalizeContent}. */
export interface CollectionDraft extends CollectionContent {
  /** `null` = nothing set at this level. */
  auth: Auth | null;
  variables: VariableRow[];
  scripts: ScriptsDraft;
  /** Empty = there is none. */
  description: string;
}

/** Content of a folder after {@link normalizeContent}. */
export interface FolderDraft extends FolderContent {
  /** `null` = inherit from the parent. */
  auth: Auth | null;
  scripts: ScriptsDraft;
  /** Empty = there is none. */
  description: string;
}

function paramRow(param: Param): ParamRow {
  return { ...param, enabled: param.enabled ?? true, postman_extra: param.postman_extra ?? {} };
}

/** The text of a script is taken as it is, line ends included. */
function scriptsDraft(scripts: Scripts | undefined): ScriptsDraft {
  return { pre_request: scripts?.pre_request ?? "", test: scripts?.test ?? "" };
}

function settingsDraft(settings: RequestSettings | undefined): RequestSettingsDraft {
  return {
    follow_redirects: settings?.follow_redirects ?? null,
    max_redirects: settings?.max_redirects ?? null,
    strict_ssl: settings?.strict_ssl ?? null,
    disable_body_pruning: settings?.disable_body_pruning ?? null,
  };
}

function fileRow(file: FileRef): FileRow {
  return { ...file, confirmed: file.confirmed ?? false };
}

function formPartRow(part: FormPart): FormPartRow {
  return {
    ...part,
    value: "text" in part.value ? part.value : { files: part.value.files.map(fileRow) },
    enabled: part.enabled ?? true,
    content_type: part.content_type ?? null,
    postman_extra: part.postman_extra ?? {},
  };
}

function draftBody(body: Body | undefined): DraftBody {
  if (!body) return { mode: "none" };
  switch (body.mode) {
    case "url_encoded":
      return { mode: "url_encoded", data: body.data.map(paramRow) };
    case "form_data":
      return { mode: "form_data", data: body.data.map(formPartRow) };
    case "binary":
      return { mode: "binary", data: { file: body.data.file ? fileRow(body.data.file) : null } };
    default:
      return body;
  }
}

/**
 * Writes out every default the core leaves out of the JSON of a node: `enabled: true`, `secret: false`,
 * empty lists, the body "none", `auth: null`, an empty `postman_extra` of a row, empty scripts, `null` for a
 * setting that is not set, `content_type: null` of a form part, `confirmed: false` of a file, an empty description of a collection and of a folder. Without it a value put back by the user (a checkbox switched off and on) would
 * differ from the stored content. The core reads the result as it is. Nothing else is touched; the input is
 * not changed.
 */
export function normalizeContent(kind: "request", content: RequestContent): RequestDraft;
export function normalizeContent(kind: "collection", content: CollectionContent): CollectionDraft;
export function normalizeContent(kind: "folder", content: FolderContent): FolderDraft;
export function normalizeContent(kind: NodeKind, content: NodeContent): RequestDraft | CollectionDraft | FolderDraft {
  switch (kind) {
    case "request": {
      const request = content as RequestContent;
      return {
        ...request,
        query: (request.query ?? []).map(paramRow),
        path_variables: (request.path_variables ?? []).map(paramRow),
        headers: (request.headers ?? []).map(paramRow),
        body: draftBody(request.body),
        auth: request.auth ?? null,
        scripts: scriptsDraft(request.scripts),
        settings: settingsDraft(request.settings),
        disabled_auto_headers: request.disabled_auto_headers ?? [],
      };
    }
    case "collection": {
      const collection = content as CollectionContent;
      return {
        ...collection,
        auth: collection.auth ?? null,
        variables: (collection.variables ?? []).map((variable) => ({
          ...variable,
          secret: variable.secret ?? false,
          enabled: variable.enabled ?? true,
          postman_extra: variable.postman_extra ?? {},
        })),
        scripts: scriptsDraft(collection.scripts),
        description: collection.description ?? "",
      };
    }
    case "folder": {
      const folder = content as FolderContent;
      return {
        ...folder,
        auth: folder.auth ?? null,
        scripts: scriptsDraft(folder.scripts),
        description: folder.description ?? "",
      };
    }
  }
}

// ---------------------------------------------------------------------------------------------------------
// Command contract
// ---------------------------------------------------------------------------------------------------------

/** A node as the tree shows it. */
export interface TreeRow {
  id: string;
  /** `null` for a collection. */
  parent_id: string | null;
  kind: NodeKind;
  name: string;
  sort_order: number;
  /** HTTP method of a request; `null` for folders and collections. */
  method: string | null;
}

interface NodeDtoBase {
  id: string;
  parent_id: string | null;
  name: string;
}

/** A node with its content; `kind` decides the shape of `content`. */
export type NodeDto =
  | (NodeDtoBase & { kind: "collection"; content: CollectionContent })
  | (NodeDtoBase & { kind: "folder"; content: FolderContent })
  | (NodeDtoBase & { kind: "request"; content: RequestContent });

/** A file chosen for import: its name without the folder and its text. */
export interface ImportFile {
  name: string;
  text: string;
}

/** One record of the import report. */
export interface ReportItem {
  kind: "unsupported_auth" | "setting_not_applied" | "has_scripts" | "file_reference";
  node_id: string;
  node_path: string;
  detail: string;
}

/** Outcome of importing one file. */
export interface ImportResult {
  file_name: string;
  kind: "collection" | "environment" | "globals" | "error";
  name: string | null;
  root_id: string | null;
  report: ReportItem[];
  /** Globals only: when not empty nothing was applied and the user has to choose a policy. */
  conflicts: string[];
  error: string | null;
}

/** What to do with globals whose stored value differs from the file's. */
export type GlobalsPolicy = "take_incoming" | "keep_current";

/** An environment in the switcher. */
export interface EnvSummary {
  id: string;
  name: string;
}

/** Id under which the commands of the environments take the set of global variables. */
export const GLOBALS_ID = "globals";

/** An environment, or the set of global variables, with its variables; every key of a variable is present. */
export interface EnvironmentDto {
  id: string;
  name: string;
  variables: VariableRow[];
}

/** Where the value of a variable comes from; `dynamic` — a `{{$name}}` the core makes a value for at every send. */
export type VariableScopeName = "global" | "collection" | "environment" | "dynamic";

/** One name that `{{name}}` resolves at a node. */
export interface ScopeVariable {
  /** The name without braces; a dynamic one starts with `$`. */
  name: string;
  /** The scope that wins for this name. */
  scope: VariableScopeName;
  /** The value after the full substitution; `null` for a secret and for a dynamic name. */
  value: string | null;
  /** The variable is secret or its value refers to a secret one: a mask is shown instead of the value. */
  secret: boolean;
}

/** A cookie set by the response. */
export interface CookieDto {
  name: string;
  value: string;
  domain: string;
  path: string;
  /** Unix seconds; `null` for a session cookie. */
  expires_unix: number | null;
  secure: boolean;
  http_only: boolean;
}

/** One redirect response that was followed. */
export interface RedirectDto {
  status: number;
  method: string;
  url: string;
  location: string;
}

/** A response; the body itself is fetched by `body.id`. */
export interface ResponseDto {
  status: number;
  status_text: string;
  http_version: string;
  headers: [string, string][];
  cookies: CookieDto[];
  redirects: RedirectDto[];
  ttfb_ms: number;
  total_ms: number;
  wire_size: number;
  decoded_size: number;
  decode_error: string | null;
  /** The last request actually sent. */
  sent: { method: string; url: string; headers: [string, string][] };
  body: { id: string; total: number; truncated: boolean; json: boolean };
}

/** Kind of the reason a send produced no response. */
export type SendErrorKind =
  | "cancelled"
  | "timeout"
  | "invalid_url"
  | "connect"
  | "tls"
  | "too_many_redirects"
  | "io"
  | "unconfirmed_files"
  | "other";

/** Why a send produced no response. */
export interface SendErrorDto {
  kind: SendErrorKind;
  message: string;
  /** `unconfirmed_files` only: names of the form fields. */
  fields: string[];
}

/** Something the user should know about a send; never blocks it. */
export interface Warning {
  kind: "scripts_not_executed" | "unsupported_auth" | "body_pruned" | "unresolved_variables";
  items: string[];
}

/** Outcome of a send: a response or an error, never both. */
export interface SendResult {
  response: ResponseDto | null;
  error: SendErrorDto | null;
  warnings: Warning[];
}

/** A send as the list of the history shows it. */
export interface HistoryItem {
  /** Number of the entry; a newer entry has a greater one. */
  id: number;
  /** Moment the send started, UTC, like `2026-11-13T10:42:07.123Z`. */
  sent_at: string;
  /** Id of the request in the tree at the moment of the send; the node may be gone by now. */
  node_id: string | null;
  /** Name of the request at the moment of the send. */
  name: string;
  /** Method and address as they went to the network; as typed when the request could not be prepared. */
  method: string;
  url: string;
  /** Status code of the response; `null` when there is no response. */
  status: number | null;
  /** Why there is no response; `null` when there is one. */
  error_kind: SendErrorKind | null;
  /** Milliseconds of the whole exchange; `null` when there is no response. */
  total_ms: number | null;
  /** Size of the decoded response body in bytes; `null` when there is no response. */
  size: number | null;
}

/** A whole entry of the history: the request and the response as they were kept. */
export interface HistoryEntry extends HistoryItem {
  /** Headers of the request as they went to the network; empty when the request could not be prepared. */
  request_headers: [string, string][];
  /** Text form of the request body, at most 1 MiB of it; `null` when no body was sent. */
  request_body: string | null;
  /** The request body was longer than 1 MiB and `request_body` is its beginning. */
  request_body_truncated: boolean;
  /** Response headers as received; empty when there is no response. */
  response_headers: [string, string][];
  /** The response body as text, at most 1 MiB of it; `null` when there is no response or it is not text. */
  response_body: string | null;
  /** The response body was longer than 1 MiB and `response_body` is its beginning. */
  response_body_truncated: boolean;
  /** The response body is not text and is therefore not kept. */
  response_body_binary: boolean;
  /** Text of the send error; `null` when there is a response. */
  error_message: string | null;
  /** Warnings of the send. */
  warnings: Warning[];
  /** Whether the node `node_id` is in the tree now. */
  node_exists: boolean;
}

/** A tab that was open: of a node (a request, a collection or a folder) or of an environment. */
export interface SavedTab {
  kind: "node" | "environment";
  /** Id of the node or of the environment (`globals` — the global variables). */
  id: string;
}

/** The layout of the window and its open tabs, as they are remembered between two launches. */
export interface WindowState {
  /** Width of the sidebar in pixels. */
  sidebar_width: number;
  /** Side of the editor the response stands on; anything unknown is taken for `auto`. */
  placement: string;
  /** Share of the editor in percent. */
  split: number;
  /** The open tabs in the order of the tab bar; a tab of a history entry is not remembered. */
  tabs: SavedTab[];
  /** Id of the shown tab as the tab bar knows it (`env:` + id for an environment); `null` — none. */
  active: string | null;
}

/** The unsaved draft of a node: its content as the editor held it. */
export interface DraftDto {
  node_id: string;
  /** A JSON object; written by any build of the program, so its shape is not guaranteed. */
  content: NodeContent;
}

/** The proxy of every send: the one of the system, none, or the one at `url` (`http://` or `https://`). */
export type ProxyDto = { mode: "system" } | { mode: "off" } | { mode: "manual"; url: string };

/** The application settings: what every send takes. */
export interface SettingsDto {
  /** How long a send may take, in milliseconds; 0 — no limit. */
  timeout_ms: number;
  /** Whether the certificate of the server is verified, unless the request says otherwise. */
  verify_tls: boolean;
  proxy: ProxyDto;
}

/** What the window remembers between two launches. */
export interface SessionDto {
  window: WindowState;
  drafts: DraftDto[];
}
