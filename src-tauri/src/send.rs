//! The send scenario of [`AppCore`]: sending a request, cancelling a send, the stored response bodies.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rqm_http::{response_cookies, CookieJar, EngineConfig, HttpError, HttpResponse, ReqwestEngine};
use rqm_model::{NodeContent, NodeKind, RequestContent};
use rqm_runner::{body_view, load_cookies, load_send_input, save_cookies, PrepareError, SendError, SendWarning};
use rqm_store::Store;
use tokio_util::sync::CancellationToken;

use crate::core::{active_environment_in, message, AppCore};
use crate::dto::{BodyInfo, ResponseDto, SendErrorDto, SendErrorKind, SendResult, SentDto, Warning, WarningKind};
use crate::settings::app_settings_in;

/// What [`AppCore`] keeps for sending: the engine with its cookies, the sends in flight and the bodies
/// of the responses the window still shows.
pub(crate) struct SendState {
    /// Sends every request; stores response bodies in the core's bodies folder.
    engine: ReqwestEngine,
    /// Cookies of the application, shared with the engine; saved to the store after every send.
    jar: Arc<CookieJar>,
    /// Cancellation tokens of the sends in flight, by the `send_id` the window chose.
    running: Mutex<HashMap<String, CancellationToken>>,
    /// Files of the response bodies not released yet, by `body.id`.
    bodies: Mutex<HashMap<String, PathBuf>>,
}

impl SendState {
    /// Restores the cookies from `store` and creates the engine that writes bodies to `bodies_dir`.
    pub(crate) fn new(store: &Store, bodies_dir: PathBuf) -> Result<Self, String> {
        let jar = Arc::new(load_cookies(store).map_err(message)?);
        let config = EngineConfig {
            body_dir: bodies_dir,
            user_agent: format!("RequestMaster/{}", env!("CARGO_PKG_VERSION")),
        };
        let engine = ReqwestEngine::new(config, jar.clone()).map_err(message)?;
        Ok(SendState { engine, jar, running: Mutex::default(), bodies: Mutex::default() })
    }
}

/// Keeps a send cancellable by its id; dropping the value ends that, however the send finished.
struct RunningSend<'a> {
    running: &'a Mutex<HashMap<String, CancellationToken>>,
    send_id: &'a str,
}

impl<'a> RunningSend<'a> {
    fn register(
        running: &'a Mutex<HashMap<String, CancellationToken>>,
        send_id: &'a str,
        cancel: CancellationToken,
    ) -> Self {
        lock(running).insert(send_id.to_string(), cancel);
        RunningSend { running, send_id }
    }
}

impl Drop for RunningSend<'_> {
    fn drop(&mut self) {
        lock(self.running).remove(self.send_id);
    }
}

impl AppCore {
    /// Sends the request `request_id` with the active environment; `draft` (the content of the request as
    /// [`AppCore::get_node`] returns it) is sent instead of the stored content. `send_id` is chosen by the
    /// caller and lets [`AppCore::cancel_send`] stop this send.
    ///
    /// `Err` is a failure of the shell itself: no such request, an unreadable draft, the storage. A request
    /// that could not be built or delivered is an `Ok` result with `error` set.
    pub async fn send_request(
        &self,
        send_id: &str,
        request_id: &str,
        draft: Option<serde_json::Value>,
    ) -> Result<SendResult, String> {
        let draft = draft.map(request_from_draft).transpose()?;
        let input = {
            let store = self.store()?;
            let environment_id = active_environment_in(&store)?;
            load_send_input(&store, request_id, draft, environment_id.as_deref(), app_settings_in(&store))
                .map_err(message)?
        };

        // The moment of the send for its history entry; the name is always known to `system_dynamic`.
        let sent_at = rqm_vars::system_dynamic("isoTimestamp").unwrap_or_default();
        let cancel = CancellationToken::new();
        let outcome = {
            let _running = RunningSend::register(&self.send.running, send_id, cancel.clone());
            rqm_runner::send(&self.send.engine, &input, cancel).await
        };
        // Whatever the outcome, the send goes to the history: now, while the body file is still in place.
        self.record_history(request_id, &input, &sent_at, &outcome);

        // Cookies are saved after a failed send too: the redirects before the failure may have set some.
        let cookies_saved = self.store().and_then(|mut store| save_cookies(&mut store, &self.send.jar).map_err(message));
        let warnings = outcome.warnings.into_iter().map(warning_dto).collect();
        match outcome.result {
            Ok(response) => {
                let body_path = response.body_path.clone();
                // A failure to save the cookies is ignored here: the server has already executed the
                // request, and losing its response would be worse. The cookies stay in the in-memory jar
                // and are saved after the next send.
                let _ = cookies_saved;
                match self.response_dto(response) {
                    Ok(response) => Ok(SendResult { response: Some(response), error: None, warnings }),
                    Err(error) => {
                        // Nobody will ever ask for this body. A file that cannot be deleted now goes when
                        // the core empties its folder at the next start.
                        let _ = std::fs::remove_file(body_path);
                        Err(error)
                    }
                }
            }
            Err(error) => {
                cookies_saved?;
                Ok(SendResult { response: None, error: Some(error_dto(error)), warnings })
            }
        }
    }

    /// Stops the send registered under `send_id`; its `send_request` then returns the error `cancelled`.
    /// A send that has finished, was cancelled already or never existed is not an error.
    pub fn cancel_send(&self, send_id: &str) {
        if let Some(cancel) = lock(&self.send.running).get(send_id) {
            cancel.cancel();
        }
    }

    /// Bytes of a response body by the rules of [`rqm_runner::body_view`]: the whole body, or its first
    /// part when it is too large; `pretty` re-indents a whole body that is valid JSON.
    pub fn response_body(&self, body_id: &str, pretty: bool) -> Result<Vec<u8>, String> {
        let path = lock(&self.send.bodies).get(body_id).cloned().ok_or("response body is gone")?;
        let view = body_view(&path, pretty).map_err(|error| format!("cannot read the response body: {error}"))?;
        Ok(view.bytes)
    }

    /// The text of a response body for the clipboard: what [`AppCore::response_body`] returns, so a body too
    /// large to show whole is copied as shown. Bytes that are not UTF-8 are replaced with U+FFFD.
    pub fn copy_response_body(&self, body_id: &str, pretty: bool) -> Result<String, String> {
        let bytes = self.response_body(body_id, pretty)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Deletes the file of a response body and forgets its id. An unknown id is not an error.
    pub fn release_body(&self, body_id: &str) {
        if let Some(path) = lock(&self.send.bodies).remove(body_id) {
            // A file that cannot be deleted now goes when the core empties its folder at the next start.
            let _ = std::fs::remove_file(path);
        }
    }

    /// The response for the window; its body file is registered under a new `body.id`.
    fn response_dto(&self, response: HttpResponse) -> Result<ResponseDto, String> {
        let body = self.register_body(&response.body_path)?;
        let cookies = response_cookies(&response).into_iter().map(Into::into).collect();
        Ok(ResponseDto {
            status: response.status,
            status_text: response.status_text,
            http_version: response.http_version,
            headers: response.headers,
            cookies,
            redirects: response.redirects.into_iter().map(Into::into).collect(),
            ttfb_ms: milliseconds(response.timings.ttfb),
            total_ms: milliseconds(response.timings.total),
            wire_size: response.wire_size,
            decoded_size: response.decoded_size,
            decode_error: response.decode_error,
            sent: SentDto { method: response.sent.method, url: response.sent.url, headers: response.sent.headers },
            body,
        })
    }

    /// Describes the body file at `path` and keeps it under a new id until [`AppCore::release_body`].
    fn register_body(&self, path: &Path) -> Result<BodyInfo, String> {
        let view = body_view(path, true).map_err(|error| format!("cannot read the response body: {error}"))?;
        let id = rqm_model::new_id();
        lock(&self.send.bodies).insert(id.clone(), path.to_path_buf());
        Ok(BodyInfo { id, total: view.total, truncated: view.truncated, json: view.pretty })
    }
}

/// Locks one of the send maps. A poisoned lock only means another thread panicked between two map
/// operations; the map itself is intact.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reads the draft of a request: the content of the node without the top-level `postman_extra`.
fn request_from_draft(draft: serde_json::Value) -> Result<RequestContent, String> {
    match NodeContent::from_json(NodeKind::Request, &draft.to_string()) {
        Ok(NodeContent::Request(request)) => Ok(request),
        Ok(_) => Err("invalid draft: not a request".to_string()),
        Err(error) => Err(format!("invalid draft: {error}")),
    }
}

/// Fractional milliseconds of a duration.
pub(crate) fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// The core's send error under the kind the window knows; `message` is the core's own text.
pub(crate) fn error_dto(error: SendError) -> SendErrorDto {
    let message = error.to_string();
    let (kind, fields) = match error {
        SendError::Prepare(PrepareError::UnconfirmedFiles { fields }) => (SendErrorKind::UnconfirmedFiles, fields),
        SendError::Prepare(PrepareError::InvalidUrl(_)) => (SendErrorKind::InvalidUrl, Vec::new()),
        SendError::Http(error) => {
            let kind = match error {
                HttpError::Cancelled => SendErrorKind::Cancelled,
                HttpError::Timeout => SendErrorKind::Timeout,
                HttpError::InvalidUrl(_) => SendErrorKind::InvalidUrl,
                HttpError::Connect(_) => SendErrorKind::Connect,
                HttpError::Tls(_) => SendErrorKind::Tls,
                HttpError::TooManyRedirects(_) => SendErrorKind::TooManyRedirects,
                HttpError::Io(_) => SendErrorKind::Io,
                HttpError::Other(_) => SendErrorKind::Other,
            };
            (kind, Vec::new())
        }
    };
    SendErrorDto { kind, message, fields }
}

/// The core's send warning under the kind the window knows.
pub(crate) fn warning_dto(warning: SendWarning) -> Warning {
    let (kind, items) = match warning {
        SendWarning::ScriptsNotExecuted { nodes } => (WarningKind::ScriptsNotExecuted, nodes),
        SendWarning::UnsupportedAuth { auth_type } => (WarningKind::UnsupportedAuth, vec![auth_type]),
        SendWarning::BodyPruned { method } => (WarningKind::BodyPruned, vec![method]),
        SendWarning::UnresolvedVariables { names } => (WarningKind::UnresolvedVariables, names),
    };
    Warning { kind, items }
}
