//! Sending: the pre-request scripts, [`prepare_with`] with the variables they left and the system dynamic
//! values, the HTTP engine, the test scripts.

use std::time::Instant;

use rqm_http::{HttpBody, HttpEngine, HttpError, HttpRequest, HttpResponse, PartValue};
use rqm_script::ResponseBody;
use tokio_util::sync::CancellationToken;

use crate::prepare::{prepare, prepare_with, unconfirmed_file_fields, PrepareError, Prepared, SendWarning};
use crate::script_host::script_response;
use crate::scripts::{nodes_with_scripts, script_steps, ScriptRun};
use crate::{ConsoleLevel, ScriptErrorKind, ScriptFailure, ScriptPhase, ScriptsMode, ScriptsReport, SendInput};

/// Why a send produced no response.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    /// The request could not be built; nothing was sent.
    #[error(transparent)]
    Prepare(#[from] PrepareError),
    /// The request was built but the exchange failed.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// A pre-request script failed; nothing was sent.
    #[error("script error: {}", .0.message)]
    Script(ScriptFailure),
    /// The send was cancelled while a pre-request script ran.
    #[error("cancelled")]
    Cancelled,
}

/// Result of [`send`].
#[derive(Debug, Clone)]
pub struct SendOutcome {
    /// The response, or why there is none.
    pub result: Result<HttpResponse, SendError>,
    /// Present on a network error too; empty when nothing was built: a pre-request script or `prepare` failed.
    pub warnings: Vec<SendWarning>,
    /// What was prepared for sending, for the history. `None` when nothing was built.
    pub request: Option<PreparedSummary>,
    /// What the scripts left: both phases in the order they ran. Kept whatever `result` is.
    pub scripts: ScriptsReport,
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

/// The scripts of `input` (when they are on) and its HTTP request, in the order of Postman: the pre-request
/// scripts, the request built with the variables they left (dynamic values from the system clock and random
/// generator), the exchange through `engine`, the test scripts. `cancel` stops a script and the exchange.
pub async fn send(engine: &dyn HttpEngine, input: &SendInput, cancel: CancellationToken) -> SendOutcome {
    let epoch = Instant::now();
    let failed = |error: SendError, scripts: ScriptsReport| SendOutcome {
        result: Err(error),
        warnings: Vec::new(),
        request: None,
        scripts,
    };
    // Before any script: a send that cannot happen must not change variables.
    let unconfirmed = unconfirmed_file_fields(&input.request.body);
    if !unconfirmed.is_empty() {
        return failed(PrepareError::UnconfirmedFiles { fields: unconfirmed }.into(), ScriptsReport::default());
    }

    let pre = script_steps(input, ScriptPhase::PreRequest);
    let tests = script_steps(input, ScriptPhase::Test);
    let has_scripts = !pre.is_empty() || !tests.is_empty();
    let mut warnings = Vec::new();
    // `None` — no script runs: they are off or there is none.
    let mut run = match &input.scripts {
        ScriptsMode::Run => has_scripts.then(|| ScriptRun::new(input, epoch, cancel.clone())),
        ScriptsMode::Off(reason) => {
            if has_scripts {
                let nodes = nodes_with_scripts(input);
                warnings.push(SendWarning::ScriptsNotRun { reason: reason.clone(), nodes });
            }
            None
        }
    };

    if let Some(run) = run.as_mut().filter(|_| !pre.is_empty()) {
        if let Some(failure) = run.run_phase(engine, &pre, ScriptPhase::PreRequest, None).await {
            let error = match failure.kind {
                ScriptErrorKind::Cancelled => SendError::Cancelled,
                _ => SendError::Script(failure),
            };
            return failed(error, std::mem::take(&mut run.report));
        }
    }

    let mut dynamic = |name: &str| rqm_vars::system_dynamic(name);
    let prepared = match &run {
        // With what the pre-request scripts left, the local variables included.
        Some(run) => prepare_with(input, run.variables.resolver(), &mut dynamic),
        None => prepare(input, &mut dynamic),
    };
    let Prepared { http, warnings: prepare_warnings } = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return failed(error.into(), run.map(|run| run.report).unwrap_or_default()),
    };
    warnings.extend(prepare_warnings);
    // The engine takes the request by value, so it is described first.
    let request = PreparedSummary::of(&http);
    let result = engine.send(http, cancel).await.map_err(SendError::from);
    if let Some(run) = run.as_mut() {
        let outcome = match &result {
            Ok(response) => {
                let millis = response.timings.total.as_millis();
                format!("{} {} ({millis} ms)", response.status, response.status_text)
            }
            Err(error) => format!("error: {error}"),
        };
        run.app_line(ConsoleLevel::Network, format!("{} {} → {outcome}", request.method, request.url));
    }
    let request = Some(request);

    if let (Some(run), Ok(response)) = (run.as_mut().filter(|_| !tests.is_empty()), &result) {
        let response = script_response(response, ResponseBody::File(response.body_path.clone()));
        // An error of a test script stays in the report: the response is the result all the same.
        run.run_phase(engine, &tests, ScriptPhase::Test, Some(response)).await;
    }
    SendOutcome { result, warnings, request, scripts: run.map(|run| run.report).unwrap_or_default() }
}
