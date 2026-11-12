//! Content of a request node and the parts shared with collections, folders and environments.

use serde::{Deserialize, Serialize};

use crate::{is_default, Extra};

/// `serde(default)` helper for flags that are on unless stated otherwise.
fn enabled_by_default() -> bool {
    true
}

/// `skip_serializing_if` helper: `true` is the default of `enabled` and is not written.
fn is_true(value: &bool) -> bool {
    *value
}

/// `skip_serializing_if` helper: `false` is the default of a flag and is not written.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Data of a request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestContent {
    /// HTTP method as entered (Postman `request.method`).
    #[serde(default)]
    pub method: String,
    /// Address without the query part; a `#fragment`, if there was one, stays at the end.
    /// When the address is assembled the query is inserted before `#`.
    #[serde(default)]
    pub url: String,
    /// Query parameters, including the disabled ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query: Vec<Param>,
    /// Values for the `:name` segments of the path (Postman `url.variable`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_variables: Vec<Param>,
    /// Headers entered by the user, including the disabled ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Param>,
    /// Request body.
    #[serde(default, skip_serializing_if = "is_default")]
    pub body: Body,
    /// `None` = inherit from the parent; `Some(Auth::NoAuth)` = explicitly no authorization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    /// Scripts of the request (Postman `event`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub scripts: Scripts,
    /// Per-request overrides of the application-wide settings.
    #[serde(default, skip_serializing_if = "is_default")]
    pub settings: RequestSettings,
    /// Lower-case names of automatic headers switched off for this request (Headers tab).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled_auto_headers: Vec<String>,
    /// Original Postman item object, with the nested `request`, without `name` (rule P1).
    #[serde(default, skip_serializing_if = "Extra::is_empty")]
    pub postman_extra: Extra,
}

/// A key-value row: query parameter, path variable, header or url-encoded body field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    /// Name of the parameter or header.
    #[serde(default)]
    pub key: String,
    /// Value as text; may contain `{{variables}}`.
    #[serde(default)]
    pub value: String,
    /// `false` = the row is kept but not sent (Postman `disabled: true`).
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Original Postman JSON object of the row (rule P3).
    #[serde(default, skip_serializing_if = "Extra::is_empty")]
    pub postman_extra: Extra,
}

/// A variable of a collection, an environment or globals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    /// Name used in `{{name}}`.
    #[serde(default)]
    pub key: String,
    /// Value as text.
    #[serde(default)]
    pub value: String,
    /// `true` = the value is masked in the interface (Postman `type: "secret"`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub secret: bool,
    /// `false` = the variable is kept but not substituted.
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Original Postman JSON object of the variable (rule P3).
    #[serde(default, skip_serializing_if = "Extra::is_empty")]
    pub postman_extra: Extra,
}

/// Scripts of a node. Each one is a single string: Postman `exec` lines joined with `\n` (rule P4).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Scripts {
    /// Script of the Postman `prerequest` event.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pre_request: String,
    /// Script of the Postman `test` event.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub test: String,
}

impl Scripts {
    /// `true` when both scripts are blank after trimming.
    pub fn is_empty(&self) -> bool {
        self.pre_request.trim().is_empty() && self.test.trim().is_empty()
    }
}

/// Authorization set on a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Auth {
    /// Explicitly no authorization (Postman `noauth`); stops the inheritance.
    NoAuth,
    /// HTTP Basic.
    Basic {
        /// User name; may contain `{{variables}}`.
        username: String,
        /// Password; may contain `{{variables}}`.
        password: String,
    },
    /// `Authorization: Bearer <token>`.
    Bearer {
        /// Token; may contain `{{variables}}`.
        token: String,
    },
    /// A key-value pair added to the headers or to the query.
    ApiKey {
        /// Name of the header or query parameter.
        key: String,
        /// Value of the key.
        value: String,
        /// Where the pair is added.
        placement: ApiKeyPlacement,
    },
    /// OAuth 2.0 with a token that is already known; the token is not obtained by the application.
    #[serde(rename = "oauth2_token")]
    OAuth2Token {
        /// Access token (Postman `accessToken`).
        token: String,
        /// `header_prefix` is already trimmed; empty = send the bare token.
        header_prefix: String,
        /// Where the token is added.
        placement: ApiKeyPlacement,
    },
    /// A Postman authorization type the application does not apply; the details stay in `postman_extra`.
    Unsupported {
        /// Postman `auth.type` as it was in the source.
        auth_type: String,
    },
}

/// Where an API key or a token is added to the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyPlacement {
    /// As a request header.
    Header,
    /// As a query parameter.
    Query,
}

/// Request body.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", content = "data", rename_all = "snake_case")]
pub enum Body {
    /// No body.
    #[default]
    None,
    /// Text typed in the editor.
    Raw {
        /// Body text; may contain `{{variables}}`.
        text: String,
        /// Language chosen in the editor; decides the automatic `Content-Type`.
        language: RawLanguage,
    },
    /// `application/x-www-form-urlencoded` fields.
    UrlEncoded(Vec<Param>),
    /// `multipart/form-data` parts.
    FormData(Vec<FormPart>),
    /// Contents of a file sent as is.
    Binary {
        /// `None` = the file is not chosen yet.
        file: Option<FileRef>,
    },
    /// GraphQL query.
    #[serde(rename = "graphql")]
    GraphQl {
        /// Query text.
        query: String,
        /// Variables as JSON text.
        variables: String,
    },
}

/// Language of a raw body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawLanguage {
    /// Plain text.
    Text,
    /// JSON.
    Json,
    /// XML.
    Xml,
    /// HTML.
    Html,
    /// JavaScript.
    #[serde(rename = "javascript")]
    JavaScript,
}

/// One part of a `multipart/form-data` body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormPart {
    /// Name of the form field.
    #[serde(default)]
    pub key: String,
    /// Text or files of the part.
    pub value: FormValue,
    /// `false` = the part is kept but not sent.
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Explicit `Content-Type` of the part (Postman `contentType`); `None` = chosen automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Original Postman JSON object of the part (rule P3).
    #[serde(default, skip_serializing_if = "Extra::is_empty")]
    pub postman_extra: Extra,
}

/// Value of a form part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormValue {
    /// A text field; may contain `{{variables}}`.
    Text(String),
    /// A file field: zero or more files.
    Files(Vec<FileRef>),
}

/// Reference to a file on this computer used in a body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileRef {
    /// Path to the file as entered or imported.
    #[serde(default)]
    pub path: String,
    /// `true` = the user picked this file on this computer; an imported path starts as `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub confirmed: bool,
}

/// Per-request settings (Postman `protocolProfileBehavior`).
/// `None` = not set on the request, the application-wide setting applies.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RequestSettings {
    /// Follow redirects automatically (Postman `followRedirects`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    /// Limit of redirects in a chain (Postman `maxRedirects`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<u32>,
    /// Verify the TLS certificate of the server (Postman `strictSSL`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_ssl: Option<bool>,
    /// Send the body even with GET and HEAD (Postman `disableBodyPruning`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_body_pruning: Option<bool>,
}
