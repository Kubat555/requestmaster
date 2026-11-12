//! Data types of the HTTP engine: the request to send, the response received and the errors.

use std::path::PathBuf;
use std::time::Duration;

/// One HTTP request, ready to be sent: variables are already substituted by the caller.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Method as typed by the user; any token is sent as is (`"PURGE"`, `"lower"`).
    pub method: String,
    /// Absolute URL with the `http` or `https` scheme (the scheme is added by `rqm-runner`).
    pub url: String,
    /// The user's headers in their order; a name may repeat. They override automatic headers.
    pub headers: Vec<(String, String)>,
    /// Request body.
    pub body: HttpBody,
    /// Settings of this send: application settings merged with the request's own by the caller.
    pub options: SendOptions,
}

/// Body of a request.
#[derive(Debug, Clone)]
pub enum HttpBody {
    /// No body.
    None,
    /// `default_content_type` is sent unless the request has its own `Content-Type` or the auto header is disabled.
    Bytes { data: Vec<u8>, default_content_type: Option<String> },
    /// `multipart/form-data` built from the parts in their order.
    Multipart(Vec<MultipartPart>),
    /// The content of a file, streamed from disk.
    /// `default_content_type` works as in [`HttpBody::Bytes`].
    File { path: PathBuf, default_content_type: Option<String> },
}

/// One part of a `multipart/form-data` body.
#[derive(Debug, Clone)]
pub struct MultipartPart {
    /// Field name.
    pub name: String,
    /// Text value or a file to upload.
    pub value: PartValue,
    /// `Content-Type` of the part, when the user set one.
    pub content_type: Option<String>,
}

/// Value of a multipart part.
#[derive(Debug, Clone)]
pub enum PartValue {
    /// Text field.
    Text(String),
    /// File streamed from disk; its file name is sent as `filename`.
    File(PathBuf),
}

/// Settings of one send.
#[derive(Debug, Clone)]
pub struct SendOptions {
    /// Limit for the whole exchange, headers and body; `None` — no limit.
    pub timeout: Option<Duration>,
    /// Whether redirects are followed (used from Task 7 on).
    pub follow_redirects: bool,
    /// Maximum number of redirects to follow (used from Task 7 on).
    pub max_redirects: u32,
    /// Whether the server's TLS certificate is verified.
    pub verify_tls: bool,
    /// Which proxy to use.
    pub proxy: ProxySetting,
    /// Lower-case names of automatic headers the user switched off.
    pub disabled_auto_headers: Vec<String>,
}

impl Default for SendOptions {
    /// No timeout, follow redirects, at most 10 of them, verify TLS, system proxy, nothing disabled.
    fn default() -> Self {
        SendOptions {
            timeout: None,
            follow_redirects: true,
            max_redirects: 10,
            verify_tls: true,
            proxy: ProxySetting::System,
            disabled_auto_headers: Vec::new(),
        }
    }
}

/// Proxy choice from the application settings. Part of the key of the engine's client cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProxySetting {
    /// Environment variables, then the static proxy of the OS settings (no PAC/WPAD).
    System,
    /// Direct connection.
    Off,
    /// Proxy URL typed by the user, used for every scheme.
    Manual(String),
}

/// The response to a request.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// Standard reason phrase of the status code; empty for a non-standard code.
    /// The phrase the server actually sent is not available from the HTTP stack.
    pub status_text: String,
    /// Protocol version of the response, e.g. `HTTP/1.1`.
    pub http_version: String,
    /// Response headers as received, `Content-Encoding` and `Content-Length` included.
    pub headers: Vec<(String, String)>,
    /// Decoded body on disk; the file is owned by the caller after `send` returns.
    pub body_path: PathBuf,
    /// Body bytes as received, before decoding.
    pub wire_size: u64,
    /// Size of the file at `body_path`.
    pub decoded_size: u64,
    /// `Some` when the body could not be decoded; the file then holds the body as received.
    pub decode_error: Option<String>,
    /// How long the exchange took.
    pub timings: Timings,
    /// Redirects that were followed (filled from Task 7 on).
    pub redirects: Vec<RedirectHop>,
    /// Method, URL and all headers of the last request actually sent, automatic ones included.
    pub sent: SentRequest,
}

/// Time measurements of one send, both counted from the moment the request starts to be sent.
#[derive(Debug, Clone)]
pub struct Timings {
    /// Until the response headers arrived.
    pub ttfb: Duration,
    /// Until the whole body was received and stored.
    pub total: Duration,
}

/// One redirect response that was followed.
#[derive(Debug, Clone)]
pub struct RedirectHop {
    /// Status of the redirect response.
    pub status: u16,
    /// Method of the request that got the redirect.
    pub method: String,
    /// URL of the request that got the redirect.
    pub url: String,
    /// `Location` header of the redirect response, as received.
    pub location: String,
}

/// What was put on the wire for the last request of a send.
#[derive(Debug, Clone)]
pub struct SentRequest {
    /// Method as sent.
    pub method: String,
    /// URL as sent (normalised by the URL parser).
    pub url: String,
    /// Automatic headers that were not overridden or disabled, then the user's headers.
    pub headers: Vec<(String, String)>,
}

/// Why a send produced no response.
#[derive(Debug, Clone, thiserror::Error)]
pub enum HttpError {
    /// The cancellation token fired.
    #[error("request cancelled")]
    Cancelled,
    /// `SendOptions::timeout` ran out.
    #[error("request timed out")]
    Timeout,
    /// The URL (or the manual proxy URL) cannot be used: not parseable, no scheme, not `http`/`https`.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    /// No connection: DNS, refused, unreachable, proxy.
    #[error("connection failed: {0}")]
    Connect(String),
    /// TLS handshake or certificate failure; the text comes from `rustls`.
    #[error("TLS error: {0}")]
    Tls(String),
    /// More redirects than `SendOptions::max_redirects`; holds the hops followed (Task 7).
    #[error("too many redirects ({} followed)", .0.len())]
    TooManyRedirects(Vec<RedirectHop>),
    /// A local file could not be read (request body) or written (response body).
    #[error("file error: {0}")]
    Io(String),
    /// Anything else: invalid method or header, a broken HTTP exchange.
    #[error("{0}")]
    Other(String),
}
