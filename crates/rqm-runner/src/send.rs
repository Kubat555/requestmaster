//! Sending: [`prepare`] with the system dynamic values, then the HTTP engine.

use rqm_http::{HttpBody, HttpEngine, HttpError, HttpRequest, HttpResponse, PartValue};
use tokio_util::sync::CancellationToken;

use crate::prepare::{prepare, PrepareError, Prepared, SendWarning};
use crate::SendInput;

/// Why a send produced no response.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    /// The request could not be built; nothing was sent.
    #[error(transparent)]
    Prepare(#[from] PrepareError),
    /// The request was built but the exchange failed.
    #[error(transparent)]
    Http(#[from] HttpError),
}

/// Result of [`send`].
#[derive(Debug, Clone)]
pub struct SendOutcome {
    /// The response, or why there is none.
    pub result: Result<HttpResponse, SendError>,
    /// Warnings of [`prepare`]; present on a network error too, empty when `prepare` itself failed.
    pub warnings: Vec<SendWarning>,
    /// What was prepared for sending, for the history. `None` only when `prepare` failed.
    pub request: Option<PreparedSummary>,
}

/// The request [`prepare`] built, in the form the history keeps: variables are already substituted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSummary {
    /// Method as typed.
    pub method: String,
    /// Final address, query included.
    pub url: String,
    /// The request's own headers and the auth header — without the automatic ones.
    pub headers: Vec<(String, String)>,
    /// Text form of the body: bytes as lossy UTF-8; for multipart one line per part (`name: value` or
    /// `name: <file path>`); for a file body `<file: path>`; `None` when no body is sent (also when it was pruned).
    pub body: Option<String>,
}

impl PreparedSummary {
    /// Describes `http` before it is handed to the engine.
    fn of(http: &HttpRequest) -> Self {
        let body = match &http.body {
            HttpBody::None => None,
            HttpBody::Bytes { data, .. } => Some(String::from_utf8_lossy(data).into_owned()),
            HttpBody::Multipart(parts) => {
                let lines: Vec<String> = parts
                    .iter()
                    .map(|part| match &part.value {
                        PartValue::Text(text) => format!("{}: {text}", part.name),
                        PartValue::File(path) => format!("{}: <{}>", part.name, path.display()),
                    })
                    .collect();
                Some(lines.join("\n"))
            }
            HttpBody::File { path, .. } => Some(format!("<file: {}>", path.display())),
        };
        PreparedSummary { method: http.method.clone(), url: http.url.clone(), headers: http.headers.clone(), body }
    }
}

/// Builds the HTTP request from `input` (dynamic values from the system clock and random generator) and
/// sends it with `engine`. `cancel` aborts the exchange.
pub async fn send(engine: &dyn HttpEngine, input: &SendInput, cancel: CancellationToken) -> SendOutcome {
    let prepared = prepare(input, &mut |name| rqm_vars::system_dynamic(name));
    let Prepared { http, warnings } = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return SendOutcome { result: Err(error.into()), warnings: Vec::new(), request: None },
    };
    // The engine takes the request by value, so it is described first.
    let request = Some(PreparedSummary::of(&http));
    let result = engine.send(http, cancel).await.map_err(SendError::from);
    SendOutcome { result, warnings, request }
}
