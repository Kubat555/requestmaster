//! Data the window exchanges with the commands; JSON keys are `snake_case` (the command contract of the plan).

use rqm_model::NodeKind;
use serde::{Deserialize, Serialize};

/// A node as the tree shows it: everything except the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeRow {
    /// Id of the node.
    pub id: String,
    /// `None` for a collection.
    pub parent_id: Option<String>,
    /// Kind of the node.
    pub kind: NodeKind,
    /// Name shown in the tree.
    pub name: String,
    /// Position among the siblings, 0..n.
    pub sort_order: i64,
    /// HTTP method of a request; `None` for folders and collections.
    pub method: Option<String>,
}

impl From<rqm_store::TreeRow> for TreeRow {
    fn from(row: rqm_store::TreeRow) -> Self {
        TreeRow {
            id: row.id,
            parent_id: row.parent_id,
            kind: row.kind,
            name: row.name,
            sort_order: row.sort_order,
            method: row.method,
        }
    }
}

/// A node with its content, as an editor tab opens it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeDto {
    /// Id of the node.
    pub id: String,
    /// `None` for a collection.
    pub parent_id: Option<String>,
    /// Kind of the node; decides the shape of `content`.
    pub kind: NodeKind,
    /// Name shown in the tree.
    pub name: String,
    /// JSON of `NodeContent::to_json` without the top-level `postman_extra` key; a collection and a folder
    /// with a description also have it under the key `description`.
    pub content: serde_json::Value,
}

/// A file the user chose for import: the window reads it and passes the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportFile {
    /// File name without the folder; returned in the result so the window can tell the files apart.
    pub name: String,
    /// Contents of the file as text.
    pub text: String,
}

/// What a file turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    /// A Postman collection; it was added to the tree.
    Collection,
    /// A Postman environment; it was saved.
    Environment,
    /// A Postman globals file; merged unless `conflicts` is not empty.
    Globals,
    /// The file was not imported; `error` says why.
    Error,
}

/// Outcome of importing one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportResult {
    /// `ImportFile::name` of the file this result is about.
    pub file_name: String,
    /// What the file turned out to be.
    pub kind: ImportKind,
    /// Name of the imported collection, environment or globals set; `None` for an error.
    pub name: Option<String>,
    /// Id of the new collection node; `None` for anything but a collection.
    pub root_id: Option<String>,
    /// What was read from a collection but will not work as in Postman; empty for other kinds.
    pub report: Vec<ReportItem>,
    /// Globals only: names whose stored value differs from the file's. When not empty nothing was applied
    /// and the window asks the user, then calls `import_globals` with the chosen policy.
    pub conflicts: Vec<String>,
    /// Why the file was not imported; `Some` only for `ImportKind::Error`.
    pub error: Option<String>,
}

/// Kind of an import report record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    /// The node has a Postman authorization type the application does not apply.
    UnsupportedAuth,
    /// A setting is kept for export but has no effect in the application.
    SettingNotApplied,
    /// The node has a non-empty pre-request or test script.
    HasScripts,
    /// The body refers to a file on the computer the collection was exported from.
    FileReference,
}

/// One record of the import report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportItem {
    /// What the record is about.
    pub kind: ReportKind,
    /// The imported node the record belongs to.
    pub node_id: String,
    /// Names from the collection down to the node, joined with ` / `.
    pub node_path: String,
    /// Depends on `kind`: the Postman auth type, the setting key, the script events or the form field name.
    pub detail: String,
}

impl From<rqm_postman::ReportItem> for ReportItem {
    fn from(item: rqm_postman::ReportItem) -> Self {
        let kind = match item.kind {
            rqm_postman::ReportKind::UnsupportedAuth => ReportKind::UnsupportedAuth,
            rqm_postman::ReportKind::SettingNotApplied => ReportKind::SettingNotApplied,
            rqm_postman::ReportKind::HasScripts => ReportKind::HasScripts,
            rqm_postman::ReportKind::FileReference => ReportKind::FileReference,
        };
        ReportItem { kind, node_id: item.node_id, node_path: item.node_path, detail: item.detail }
    }
}

/// An environment in the switcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvSummary {
    /// Id of the environment.
    pub id: String,
    /// Name shown in the switcher.
    pub name: String,
}

/// Outcome of a send: a response or an error, never both, plus the warnings of the send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendResult {
    /// The response; `None` when the send failed.
    pub response: Option<ResponseDto>,
    /// Why there is no response; `None` when there is one.
    pub error: Option<SendErrorDto>,
    /// What the user should know about the send; present on a network error too.
    pub warnings: Vec<Warning>,
}

/// A response as the response panel shows it; the body itself is fetched by `body.id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseDto {
    /// Status code.
    pub status: u16,
    /// Standard reason phrase of the status code; empty for a non-standard code.
    pub status_text: String,
    /// Protocol version of the response, e.g. `HTTP/1.1`.
    pub http_version: String,
    /// Response headers as received, in their order.
    pub headers: Vec<(String, String)>,
    /// Cookies the response set.
    pub cookies: Vec<CookieDto>,
    /// Redirects that were followed before this response.
    pub redirects: Vec<RedirectDto>,
    /// Milliseconds until the response headers arrived.
    pub ttfb_ms: f64,
    /// Milliseconds until the whole body was received and stored.
    pub total_ms: f64,
    /// Size of the body as received, before decoding.
    pub wire_size: u64,
    /// Size of the decoded body.
    pub decoded_size: u64,
    /// `Some` when the body could not be decoded; the body is then shown as received.
    pub decode_error: Option<String>,
    /// The last request actually sent, automatic headers included.
    pub sent: SentDto,
    /// Handle of the stored body.
    pub body: BodyInfo,
}

/// A cookie set by the response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieDto {
    /// Name of the cookie.
    pub name: String,
    /// Value as received.
    pub value: String,
    /// Host the cookie belongs to; a leading dot means subdomains too.
    pub domain: String,
    /// Path the cookie is sent to.
    pub path: String,
    /// Expiry as Unix seconds; `None` for a session cookie.
    pub expires_unix: Option<i64>,
    /// `Secure` attribute.
    pub secure: bool,
    /// `HttpOnly` attribute.
    pub http_only: bool,
}

impl From<rqm_http::Cookie> for CookieDto {
    fn from(cookie: rqm_http::Cookie) -> Self {
        CookieDto {
            name: cookie.name,
            value: cookie.value,
            domain: cookie.domain,
            path: cookie.path,
            expires_unix: cookie.expires_unix,
            secure: cookie.secure,
            http_only: cookie.http_only,
        }
    }
}

/// One redirect response that was followed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedirectDto {
    /// Status of the redirect response.
    pub status: u16,
    /// Method of the request that got the redirect.
    pub method: String,
    /// URL of the request that got the redirect.
    pub url: String,
    /// `Location` header of the redirect response, as received.
    pub location: String,
}

impl From<rqm_http::RedirectHop> for RedirectDto {
    fn from(hop: rqm_http::RedirectHop) -> Self {
        RedirectDto { status: hop.status, method: hop.method, url: hop.url, location: hop.location }
    }
}

/// What was put on the wire for the last request of a send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentDto {
    /// Method as sent.
    pub method: String,
    /// URL as sent.
    pub url: String,
    /// Automatic headers, then the user's headers.
    pub headers: Vec<(String, String)>,
}

/// Handle of a response body kept on disk until `release_body`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyInfo {
    /// Id for `response_body` and `release_body`.
    pub id: String,
    /// Size of the body file in bytes.
    pub total: u64,
    /// `true` when the body is too large to show whole: `response_body` returns only its beginning.
    pub truncated: bool,
    /// `true` when the body is valid JSON that `response_body` can return re-indented.
    pub json: bool,
}

/// Why a send produced no response, as the window tells the cases apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendErrorKind {
    /// The user cancelled the send.
    Cancelled,
    /// The time limit ran out.
    Timeout,
    /// The address cannot be used.
    InvalidUrl,
    /// No connection: DNS, refused, unreachable, proxy.
    Connect,
    /// TLS handshake or certificate failure.
    Tls,
    /// More redirects than allowed.
    TooManyRedirects,
    /// A local file could not be read or written.
    Io,
    /// The body refers to files that were not picked on this computer; `fields` names them.
    UnconfirmedFiles,
    /// Anything else.
    Other,
}

/// Why a send produced no response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendErrorDto {
    /// Which case it is.
    pub kind: SendErrorKind,
    /// Text of the core's error.
    pub message: String,
    /// `unconfirmed_files` only: names of the form fields (`binary` for a binary body); empty otherwise.
    pub fields: Vec<String>,
}

/// Kind of a send warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    /// Scripts exist but were not run; `items` are the names of the nodes that have one.
    ScriptsNotExecuted,
    /// The authorization type is not applied; `items` holds the Postman type.
    UnsupportedAuth,
    /// The body was not sent because of the method; `items` holds the method.
    BodyPruned,
    /// Variables that were not found; `items` are their names.
    UnresolvedVariables,
}

/// Something the user should know about a send; never blocks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    /// What the warning is about.
    pub kind: WarningKind,
    /// Depends on `kind`, see [`WarningKind`].
    pub items: Vec<String>,
}

/// `serde(default)` helper: a variable is enabled unless the window says otherwise.
fn enabled_by_default() -> bool {
    true
}

/// A variable in the environment editor. Unlike the model's `Variable`, which skips default values when it
/// is written to the store, all five keys are always present in the JSON the window receives. On the way
/// back a missing `secret`, `enabled` or `postman_extra` means `false`, `true` and `{}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDto {
    /// Name used in `{{name}}`.
    pub key: String,
    /// Value as text.
    pub value: String,
    /// `true` = the value is masked in the interface.
    #[serde(default)]
    pub secret: bool,
    /// `false` = the variable is kept but not substituted.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// Original Postman object of the variable; the window returns it untouched, so the export loses nothing.
    #[serde(default)]
    pub postman_extra: rqm_model::Extra,
}

impl From<rqm_model::Variable> for VariableDto {
    fn from(variable: rqm_model::Variable) -> Self {
        VariableDto {
            key: variable.key,
            value: variable.value,
            secret: variable.secret,
            enabled: variable.enabled,
            postman_extra: variable.postman_extra,
        }
    }
}

impl From<VariableDto> for rqm_model::Variable {
    fn from(variable: VariableDto) -> Self {
        rqm_model::Variable {
            key: variable.key,
            value: variable.value,
            secret: variable.secret,
            enabled: variable.enabled,
            postman_extra: variable.postman_extra,
        }
    }
}

/// An environment, or the globals set under the id `globals`, as the environment editor opens it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentDto {
    /// Id of the environment; `globals` for the globals set.
    pub id: String,
    /// Name shown in the switcher.
    pub name: String,
    /// Variables in their order.
    pub variables: Vec<VariableDto>,
}

/// Where the value of a `{{name}}` comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableScope {
    /// The globals set.
    Global,
    /// The collection of the node.
    Collection,
    /// The active environment.
    Environment,
    /// `{{$guid}}` and the like: a new value on every occurrence at the send.
    Dynamic,
}

impl From<rqm_runner::VariableOrigin> for VariableScope {
    fn from(origin: rqm_runner::VariableOrigin) -> Self {
        match origin {
            rqm_runner::VariableOrigin::Global => VariableScope::Global,
            rqm_runner::VariableOrigin::Collection => VariableScope::Collection,
            rqm_runner::VariableOrigin::Environment => VariableScope::Environment,
            rqm_runner::VariableOrigin::Dynamic => VariableScope::Dynamic,
        }
    }
}

/// One name that `{{name}}` resolves at a node, for the highlighting and the autocompletion in the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeVariable {
    /// The name without braces; a dynamic one starts with `$`.
    pub name: String,
    /// The scope that wins for this name.
    pub scope: VariableScope,
    /// The value after the full substitution; `null` for a secret and for a dynamic name.
    pub value: Option<String>,
    /// `true` — the variable is secret or its value refers to a secret one; the window shows a mask.
    pub secret: bool,
}

impl From<rqm_runner::ScopeVariable> for ScopeVariable {
    fn from(variable: rqm_runner::ScopeVariable) -> Self {
        ScopeVariable {
            name: variable.name,
            scope: variable.origin.into(),
            value: variable.value,
            secret: variable.secret,
        }
    }
}

/// The short facts of a history entry. Stored as JSON in the `summary` column of `history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistorySummary {
    /// Id of the request in the tree at the moment of the send; the node may be gone by now.
    pub node_id: Option<String>,
    /// Name of the request at the moment of the send.
    pub name: String,
    /// Method as it went to the network; as typed when the request could not be prepared.
    pub method: String,
    /// Address as it went to the network; as typed when the request could not be prepared.
    pub url: String,
    /// Status code of the response; `None` when there is no response.
    pub status: Option<u16>,
    /// Why there is no response; `None` when there is one.
    pub error_kind: Option<SendErrorKind>,
    /// Milliseconds of the whole exchange; `None` when there is no response.
    pub total_ms: Option<f64>,
    /// Size of the decoded response body in bytes; `None` when there is no response.
    pub size: Option<u64>,
}

/// A history entry as the list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryItem {
    /// Number of the entry; a newer entry has a greater one.
    pub id: i64,
    /// Moment the send started, UTC, like `2026-11-13T10:42:07.123Z`.
    pub sent_at: String,
    /// The short facts, as keys of this object.
    #[serde(flatten)]
    pub summary: HistorySummary,
}

/// What a history entry holds beyond the list. Stored as JSON in the `detail` column of `history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryDetail {
    /// Headers of the request as they went to the network (automatic ones included when there is a
    /// response); empty when the request could not be prepared.
    pub request_headers: Vec<(String, String)>,
    /// Text form of the request body, at most 1 MiB of it; `None` when no body was sent.
    pub request_body: Option<String>,
    /// `true` when the request body was longer than 1 MiB and `request_body` is its beginning.
    pub request_body_truncated: bool,
    /// Response headers as received; empty when there is no response.
    pub response_headers: Vec<(String, String)>,
    /// The response body as text, at most 1 MiB of it; `None` when there is no response or it is not text.
    pub response_body: Option<String>,
    /// `true` when the response body was longer than 1 MiB and `response_body` is its beginning.
    pub response_body_truncated: bool,
    /// `true` when the response body is not UTF-8 text and is therefore not kept.
    pub response_body_binary: bool,
    /// Text of the send error; `None` when there is a response.
    pub error_message: Option<String>,
    /// Warnings of the send.
    pub warnings: Vec<Warning>,
}

/// A whole history entry: the list item, the detail and whether the request still exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// What the list shows, as keys of this object.
    #[serde(flatten)]
    pub item: HistoryItem,
    /// What was stored beyond the list, as keys of this object.
    #[serde(flatten)]
    pub detail: HistoryDetail,
    /// Whether the node `node_id` is in the tree now.
    pub node_exists: bool,
}

/// What the window restores at start: its layout with the open tabs, and the unsaved drafts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDto {
    /// The layout and the tabs; the defaults while nothing was saved.
    pub window: WindowState,
    /// The unsaved drafts of the nodes that still exist, ordered by the id of the node.
    pub drafts: Vec<DraftDto>,
    /// Whether the window measures itself and reports the timings with `report_timings`.
    pub measure: bool,
}

/// One measurement of the window: what was measured and how long it took.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingSample {
    /// What was measured: `ready`, or `<event>:<place>` with an optional `:proc` at the end.
    pub name: String,
    /// Milliseconds.
    pub ms: f64,
}

/// The layout of the window and its open tabs. Stored as JSON under the `window_state` key of `app_state`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    /// Width of the side panel in pixels.
    pub sidebar_width: f64,
    /// Where the response is placed relative to the request; the window chooses the word and reads it back.
    pub placement: String,
    /// Share of the request part in the request/response split, in percent.
    pub split: f64,
    /// The open tabs, left to right.
    pub tabs: Vec<SavedTab>,
    /// Key of the active tab: the id of the node, or `env:<id>` for an environment; `None` — no tab is active.
    pub active: Option<String>,
}

/// What an open tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedTabKind {
    /// A node of the tree: a request, a collection or a folder.
    Node,
    /// An environment, or the globals set under the id `globals`.
    Environment,
}

/// An open tab as it is remembered between two runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTab {
    /// Whether `id` names a node or an environment.
    pub kind: SavedTabKind,
    /// Id of the node or of the environment.
    pub id: String,
}

/// The unsaved draft of an open tab of a node. Its content is stored as JSON under the `draft:<node_id>` key
/// of `app_state`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftDto {
    /// Id of the node the draft belongs to.
    pub node_id: String,
    /// The content of the node as the editor of the window holds it (the shape of [`NodeDto::content`]);
    /// always a JSON object, its fields are not checked.
    pub content: serde_json::Value,
}

/// The application settings of a send. Stored as JSON under the `settings` key of `app_state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsDto {
    /// Limit for the whole exchange in milliseconds; `0` — no limit.
    pub timeout_ms: u64,
    /// Whether the server's TLS certificate is verified.
    pub verify_tls: bool,
    /// Which proxy to use.
    pub proxy: ProxyDto,
}

/// The proxy choice of the application settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ProxyDto {
    /// The proxy of the operating system.
    System,
    /// Direct connection.
    Off,
    /// The proxy at this address, for every request.
    Manual {
        /// Address of the proxy: an `http://` or `https://` URL.
        url: String,
    },
}
