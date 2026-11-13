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
  /** Explicit `Content-Type` of the part; absent = chosen automatically. */
  content_type?: string;
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

/** Per-request settings; an absent one means the application-wide setting applies. */
export interface RequestSettings {
  follow_redirects?: boolean;
  max_redirects?: number;
  strict_ssl?: boolean;
  disable_body_pruning?: boolean;
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
}

/** Content of a folder node. */
export interface FolderContent {
  /** Absent or `null` = inherit from the parent. */
  auth?: Auth | null;
  scripts?: Scripts;
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

/** A body whose url-encoded rows have every default written out. */
export type DraftBody = Exclude<Body, { mode: "url_encoded" }> | { mode: "url_encoded"; data: ParamRow[] };

/** Content of a request after {@link normalizeContent}. */
export interface RequestDraft extends RequestContent {
  query: ParamRow[];
  path_variables: ParamRow[];
  headers: ParamRow[];
  body: DraftBody;
  /** `null` = inherit from the parent. */
  auth: Auth | null;
  disabled_auto_headers: string[];
}

/** Content of a collection after {@link normalizeContent}. */
export interface CollectionDraft extends CollectionContent {
  /** `null` = nothing set at this level. */
  auth: Auth | null;
  variables: VariableRow[];
}

function paramRow(param: Param): ParamRow {
  return { ...param, enabled: param.enabled ?? true, postman_extra: param.postman_extra ?? {} };
}

function draftBody(body: Body | undefined): DraftBody {
  if (!body) return { mode: "none" };
  switch (body.mode) {
    case "url_encoded":
      return { mode: "url_encoded", data: body.data.map(paramRow) };
    case "form_data":
      return {
        mode: "form_data",
        data: body.data.map((part) => ({ ...part, enabled: part.enabled ?? true, postman_extra: part.postman_extra ?? {} })),
      };
    default:
      return body;
  }
}

/**
 * Writes out every default the core leaves out of the JSON of a node: `enabled: true`, `secret: false`,
 * empty lists, the body "none", `auth: null`, an empty `postman_extra` of a row. Without it a value put back
 * by the user (a checkbox switched off and on) would differ from the stored content. The core reads the
 * result as it is. Nothing else is touched; the input is not changed.
 */
export function normalizeContent(kind: "request", content: RequestContent): RequestDraft;
export function normalizeContent(kind: "collection", content: CollectionContent): CollectionDraft;
export function normalizeContent(
  kind: "request" | "collection",
  content: RequestContent | CollectionContent,
): RequestDraft | CollectionDraft {
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

/** Why a send produced no response. */
export interface SendErrorDto {
  kind:
    | "cancelled"
    | "timeout"
    | "invalid_url"
    | "connect"
    | "tls"
    | "too_many_redirects"
    | "io"
    | "unconfirmed_files"
    | "other";
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
