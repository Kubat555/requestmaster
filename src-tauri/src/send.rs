//! The send scenario of [`AppCore`]: sending a request, cancelling a send, the stored response bodies.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rqm_http::{response_cookies, CookieJar, EngineConfig, HttpError, HttpResponse, ReqwestEngine};
use rqm_model::{NodeContent, NodeKind, RequestContent};
use rqm_runner::{
    apply_variable_changes, body_view, load_cookies, load_send_input, save_cookies, PrepareError, ScriptsOff,
    SendError, SendInput, SendWarning, VariableChange,
};
use rqm_store::Store;
use tokio_util::sync::CancellationToken;

use crate::core::{active_environment_in, message, AppCore};
use crate::dto::{
    BodyInfo, ResponseDto, SendErrorDto, SendErrorKind, SendResult, SentDto, VariableChangeDto, Warning, WarningKind,
};
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
    bodies: Mutex<HashMap<String, StoredBody>>,
}

/// A response body the window still shows.
struct StoredBody {
    /// The file with the body as received and decoded.
    path: PathBuf,
    /// Extension of the file name offered when the body is saved, see [`body_extension`].
    extension: &'static str,
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
        // What the scripts changed in the variables is written first: whatever happens next, the window
        // must learn the values that are already in the store.
        let mut warnings: Vec<Warning> = outcome.warnings.iter().cloned().map(warning_dto).collect();
        let mut variable_changes = Vec::new();
        if !outcome.scripts.changes.is_empty() {
            match self.apply_script_changes(&input, &outcome.scripts.changes) {
                Ok(applied) => variable_changes = applied,
                // A failure of the storage must not cost the user the response.
                Err(error) => warnings.push(Warning { kind: WarningKind::VariablesNotSaved, items: vec![error] }),
            }
        }
        // Whatever the outcome, the send goes to the history: now, while the body file is still in place.
        self.record_history(request_id, &input, &sent_at, &outcome, &warnings);

        // Cookies are saved after a failed send too: the redirects before the failure may have set some.
        let cookies_saved = self.store().and_then(|mut store| save_cookies(&mut store, &self.send.jar).map_err(message));
        // A failure after the send is an error of the command only while the scripts wrote no variables.
        // Otherwise the command answers with the changes: without them an open editor of the window would
        // keep the old rows and write them over the values of the script.
        let changes_written = !variable_changes.is_empty();
        let scripts = outcome.scripts;
        let result = |response: Option<ResponseDto>, error: Option<SendErrorDto>| SendResult {
            response,
            error,
            warnings,
            tests: scripts.tests.into_iter().map(Into::into).collect(),
            console: scripts.console.into_iter().map(Into::into).collect(),
            console_truncated: scripts.console_truncated,
            script_errors: scripts.errors.into_iter().map(Into::into).collect(),
            variable_changes,
        };
        match outcome.result {
            Ok(response) => {
                let body_path = response.body_path.clone();
                // A failure to save the cookies is ignored here: the server has already executed the
                // request, and losing its response would be worse. The cookies stay in the in-memory jar
                // and are saved after the next send.
                let _ = cookies_saved;
                match self.response_dto(response) {
                    Ok(response) => Ok(result(Some(response), None)),
                    Err(error) => {
                        // Nobody will ever ask for this body. A file that cannot be deleted now goes when
                        // the core empties its folder at the next start.
                        let _ = std::fs::remove_file(body_path);
                        if !changes_written {
                            return Err(error);
                        }
                        let error = SendErrorDto { kind: SendErrorKind::Io, message: error, fields: Vec::new() };
                        Ok(result(None, Some(error)))
                    }
                }
            }
            Err(error) => {
                if !changes_written {
                    cookies_saved?;
                }
                Ok(result(None, Some(error_dto(error))))
            }
        }
    }

    /// Writes what the scripts of a send changed in the variables over what the store holds now — under
    /// one lock, so that an edit saved while the send ran is kept — and returns what was written.
    fn apply_script_changes(
        &self,
        input: &SendInput,
        changes: &[VariableChange],
    ) -> Result<Vec<VariableChangeDto>, String> {
        // The tree rules give every request a collection; an id that names no node writes nothing.
        let collection_id = input.ancestors.first().map(|collection| collection.id.as_str()).unwrap_or_default();
        let environment_id = input.environment.as_ref().map(|environment| environment.id.as_str());
        let mut store = self.store()?;
        let applied =
            apply_variable_changes(&mut store, collection_id, environment_id, changes).map_err(message)?;
        Ok(applied.into_iter().map(Into::into).collect())
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
        let path = self.body_path(body_id)?;
        let view = body_view(&path, pretty).map_err(|error| format!("cannot read the response body: {error}"))?;
        Ok(view.bytes)
    }

    /// The text of a response body for the clipboard: what [`AppCore::response_body`] returns, so a body too
    /// large to show whole is copied as shown. Bytes that are not UTF-8 are replaced with U+FFFD.
    pub fn copy_response_body(&self, body_id: &str, pretty: bool) -> Result<String, String> {
        let bytes = self.response_body(body_id, pretty)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The file name offered when a response body is saved: `response.<extension>`, the extension chosen by
    /// [`body_extension`]. An unknown id is the error `response body is gone`.
    pub fn response_file_name(&self, body_id: &str) -> Result<String, String> {
        let bodies = lock(&self.send.bodies);
        let body = bodies.get(body_id).ok_or("response body is gone")?;
        Ok(format!("response.{}", body.extension))
    }

    /// Copies a response body, as received and decoded, to `target`. The file is copied on disk and never
    /// read into memory, so a body too large to show is saved whole. An unknown id is the error
    /// `response body is gone`.
    pub fn save_response_body(&self, body_id: &str, target: &Path) -> Result<(), String> {
        let path = self.body_path(body_id)?;
        // Copying a file onto itself would empty it. A target that does not exist yet cannot be resolved
        // and cannot be the body file.
        if let (Ok(source), Ok(target)) = (path.canonicalize(), target.canonicalize()) {
            if source == target {
                return Err("cannot save the response onto itself".to_string());
            }
        }
        std::fs::copy(&path, target).map_err(|error| format!("cannot write the file: {error}"))?;
        Ok(())
    }

    /// Deletes the file of a response body and forgets its id. An unknown id is not an error.
    pub fn release_body(&self, body_id: &str) {
        if let Some(body) = lock(&self.send.bodies).remove(body_id) {
            // A file that cannot be deleted now goes when the core empties its folder at the next start.
            let _ = std::fs::remove_file(body.path);
        }
    }

    /// The file of a response body not released yet.
    fn body_path(&self, body_id: &str) -> Result<PathBuf, String> {
        let bodies = lock(&self.send.bodies);
        Ok(bodies.get(body_id).ok_or("response body is gone")?.path.clone())
    }

    /// The response for the window; its body file is registered under a new `body.id`.
    fn response_dto(&self, response: HttpResponse) -> Result<ResponseDto, String> {
        let content_type = header_value(&response.headers, "content-type");
        let body = self.register_body(&response.body_path, content_type)?;
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

    /// Describes the body file at `path` and keeps it under a new id until [`AppCore::release_body`];
    /// `content_type` is the `Content-Type` header of the response.
    fn register_body(&self, path: &Path, content_type: Option<&str>) -> Result<BodyInfo, String> {
        let view = body_view(path, true).map_err(|error| format!("cannot read the response body: {error}"))?;
        let id = rqm_model::new_id();
        let stored = StoredBody { path: path.to_path_buf(), extension: body_extension(content_type, view.pretty) };
        lock(&self.send.bodies).insert(id.clone(), stored);
        Ok(BodyInfo { id, total: view.total, truncated: view.truncated, json: view.pretty })
    }
}

/// Locks one of the send maps. A poisoned lock only means another thread panicked between two map
/// operations; the map itself is intact.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The value of the first header with this name, whatever the case of its letters.
fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(header, _)| header.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
}

/// Extension of the file name offered when a response body is saved, by the `Content-Type` header of the
/// response; `json` tells that the body itself is valid JSON, whatever the header says.
pub(crate) fn body_extension(content_type: Option<&str>, json: bool) -> &'static str {
    let content_type = content_type.unwrap_or_default().trim().to_ascii_lowercase();
    if json || content_type.contains("json") {
        "json"
    } else if content_type.starts_with("text/html") {
        "html"
    } else if content_type.contains("xml") {
        "xml"
    } else if content_type.starts_with("text/") {
        "txt"
    } else {
        "bin"
    }
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

/// The core's send error under the kind the window knows; `message` is the core's own text — of a failed
/// pre-request script the text of its error, of a skipped request nothing.
pub(crate) fn error_dto(error: SendError) -> SendErrorDto {
    let message = match &error {
        SendError::Script(failure) => failure.message.clone(),
        SendError::Skipped => String::new(),
        other => other.to_string(),
    };
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
        SendError::Script(_) => (SendErrorKind::Script, Vec::new()),
        SendError::Skipped => (SendErrorKind::Skipped, Vec::new()),
        SendError::Cancelled => (SendErrorKind::Cancelled, Vec::new()),
    };
    SendErrorDto { kind, message, fields }
}

/// The core's send warning under the kind the window knows.
fn warning_dto(warning: SendWarning) -> Warning {
    let (kind, items) = match warning {
        SendWarning::ScriptsNotRun { reason: ScriptsOff::Disabled, .. } => (WarningKind::ScriptsDisabled, Vec::new()),
        SendWarning::ScriptsNotRun { reason: ScriptsOff::NotAllowed { collection }, .. } => {
            (WarningKind::ScriptsNotAllowed, vec![collection])
        }
        SendWarning::UnsupportedAuth { auth_type } => (WarningKind::UnsupportedAuth, vec![auth_type]),
        SendWarning::BodyPruned { method } => (WarningKind::BodyPruned, vec![method]),
        SendWarning::UnresolvedVariables { names } => (WarningKind::UnresolvedVariables, names),
    };
    Warning { kind, items }
}

#[cfg(test)]
mod tests {
    use super::body_extension;

    #[test]
    fn body_extension_table() {
        let table = [
            (Some("application/json; charset=utf-8"), false, "json"),
            (None, true, "json"),
            (Some("application/problem+json"), false, "json"),
            (Some("text/html; charset=utf-8"), false, "html"),
            (Some("text/xml"), false, "xml"),
            (Some("text/plain"), false, "txt"),
            (Some("image/png"), false, "bin"),
            (None, false, "bin"),
        ];
        for (content_type, json, extension) in table {
            assert_eq!(body_extension(content_type, json), extension, "{content_type:?}, json = {json}");
        }
    }
}
