//! The scripts of a send: the chain of the nodes, the job for the script engine, and the report of the engine
//! in the terms of the tree (which node, which phase).

use std::time::Instant;

use rqm_http::{HttpEngine, HttpRequest, SendOptions};
use rqm_model::{
    Body, Extra, FormValue, NodeContent, NodeKind, Param, RawLanguage, RequestContent, Scripts, Variable,
};
use rqm_script::{
    Info, Job, Limits, Phase, Report, ScriptBody, ScriptPair, ScriptRequest, ScriptResponse, Var, Variables,
};
use tokio_util::sync::CancellationToken;

pub use rqm_script::{ConsoleLevel, ScriptErrorKind, StoredScope, VariableChange, VariableOp};

use crate::prepare::query_encode;
use crate::script_host::run_job;
use crate::{ScopeSources, SendInput};

/// When a script of a send runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPhase {
    /// Before the request is built.
    PreRequest,
    /// After the response has arrived.
    Test,
}

/// The script something came from: the node that holds it and the phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptSource {
    pub node_id: String,
    pub node_name: String,
    pub node_kind: NodeKind,
    pub phase: ScriptPhase,
}

/// One script of a phase.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptStep {
    pub source: ScriptSource,
    /// The text of the script as stored.
    pub code: String,
}

/// What one `pm.test` recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct TestOutcome {
    pub source: ScriptSource,
    pub name: String,
    pub passed: bool,
    pub skipped: bool,
    /// The text of the error of a failed test.
    pub error: Option<String>,
}

/// One line of the console of a send.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleEntry {
    /// `None` — a line of the application itself.
    pub source: Option<ScriptSource>,
    pub level: ConsoleLevel,
    pub text: String,
    /// Whole milliseconds since the start of the send.
    pub at_ms: u64,
}

/// An error that stopped a script.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptFailure {
    pub source: ScriptSource,
    pub kind: ScriptErrorKind,
    pub message: String,
    /// Line and column in the text of the script as the editor numbers them, from 1.
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// What the scripts of a send left, both phases in the order they ran.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScriptsReport {
    pub tests: Vec<TestOutcome>,
    pub console: Vec<ConsoleEntry>,
    /// Lines were dropped: the console of a phase is limited.
    pub console_truncated: bool,
    pub errors: Vec<ScriptFailure>,
    /// What the scripts changed in the stored scopes, in the order they did it.
    pub changes: Vec<VariableChange>,
}

/// The nodes of a send that may hold scripts, from the collection down to the request:
/// `(id, name, kind, scripts)`.
fn script_nodes(input: &SendInput) -> impl Iterator<Item = (&str, &str, NodeKind, &Scripts)> {
    let ancestors = input.ancestors.iter().map(|node| {
        let scripts = match &node.content {
            NodeContent::Collection(content) => &content.scripts,
            NodeContent::Folder(content) => &content.scripts,
            NodeContent::Request(content) => &content.scripts,
        };
        (node.id.as_str(), node.name.as_str(), node.kind, scripts)
    });
    let request =
        (input.request_id.as_str(), input.request_name.as_str(), NodeKind::Request, &input.request.scripts);
    ancestors.chain(std::iter::once(request))
}

/// The scripts of the phase that are not blank: the collection, the folders from the outer one, the request.
pub fn script_steps(input: &SendInput, phase: ScriptPhase) -> Vec<ScriptStep> {
    script_nodes(input)
        .filter_map(|(id, name, kind, scripts)| {
            let code = match phase {
                ScriptPhase::PreRequest => &scripts.pre_request,
                ScriptPhase::Test => &scripts.test,
            };
            (!code.trim().is_empty()).then(|| ScriptStep {
                source: ScriptSource { node_id: id.to_string(), node_name: name.to_string(), node_kind: kind, phase },
                code: code.clone(),
            })
        })
        .collect()
}

/// Names of the nodes that have a script of either phase, from the collection down to the request.
pub(crate) fn nodes_with_scripts(input: &SendInput) -> Vec<String> {
    let with_script = script_nodes(input).filter(|(_, _, _, scripts)| !scripts.is_empty());
    with_script.map(|(_, name, _, _)| name.to_string()).collect()
}

/// Byte length of the `{{name}}` token `text` starts with, as `rqm_vars::Resolver` understands a token.
fn token_len(text: &str) -> Option<usize> {
    let name = text.strip_prefix("{{")?;
    let end = name.find(['{', '}'])?;
    name.get(end..)?.starts_with("}}").then_some(end + 4)
}

/// `text` percent-encoded for a query ([`query_encode`]) everywhere but inside its `{{tokens}}`, which stay
/// as typed.
fn encode_outside_tokens(text: &str, is_key: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    // `plain` — how much of `rest` is known to hold no token.
    let mut plain = 0;
    while let Some(tail) = rest.get(plain..).filter(|tail| !tail.is_empty()) {
        match token_len(tail) {
            Some(length) => {
                let (before, from_token) = rest.split_at(plain);
                let (token, after) = from_token.split_at(length);
                out.push_str(&query_encode(before, is_key));
                out.push_str(token);
                rest = after;
                plain = 0;
            }
            None => plain += tail.chars().next().map_or(1, char::len_utf8),
        }
    }
    out.push_str(&query_encode(rest, is_key));
    out
}

/// Where the first `wanted` of `text` stands that is not inside a `{{token}}`.
fn find_outside_tokens(text: &str, wanted: char) -> Option<usize> {
    let mut at = 0;
    while let Some(tail) = text.get(at..) {
        let first = tail.chars().next()?;
        match token_len(tail) {
            Some(length) => at += length,
            None if first == wanted => return Some(at),
            None => at += first.len_utf8(),
        }
    }
    None
}

/// The address a pre-request script sees: the address as typed with the enabled rows of the query written
/// into it — after the `?`, before the `#fragment`. A row is percent-encoded like the sent one, but only
/// outside its `{{tokens}}`: a literal `&`, `#`, `=` or space of a row does not break the address apart, and
/// a variable stays visible as `{{name}}`.
fn script_url(request: &RequestContent) -> String {
    let (start, fragment) = match request.url.split_once('#') {
        Some((start, fragment)) => (start, Some(fragment)),
        None => (request.url.as_str(), None),
    };
    let mut url = start.to_string();
    let mut separator = if start.contains('?') { '&' } else { '?' };
    for row in request.query.iter().filter(|row| row.enabled) {
        url.push(separator);
        url.push_str(&encode_outside_tokens(&row.key, true));
        url.push('=');
        url.push_str(&encode_outside_tokens(&row.value, false));
        separator = '&';
    }
    if let Some(fragment) = fragment {
        url.push('#');
        url.push_str(fragment);
    }
    url
}

/// The address a script left, back in the form of the model: the address without its query (the `#fragment`
/// stays at its end) and the query as rows. A `{{token}}` is never cut in two; empty pieces between `&` are
/// left out. The rows are not decoded: sending encodes them again, and that leaves `%` alone.
fn model_url(url: &str) -> (String, Vec<Param>) {
    let (start, fragment) = match url.split_once('#') {
        Some((start, fragment)) => (start, Some(fragment)),
        None => (url, None),
    };
    let (address, mut query) = match start.split_once('?') {
        Some((address, query)) => (address, query),
        None => (start, ""),
    };
    let mut address = address.to_string();
    if let Some(fragment) = fragment {
        address.push('#');
        address.push_str(fragment);
    }
    let mut rows = Vec::new();
    while !query.is_empty() {
        let (piece, rest) = match find_outside_tokens(query, '&') {
            Some(at) => (query.get(..at).unwrap_or_default(), query.get(at + 1..).unwrap_or_default()),
            None => (query, ""),
        };
        query = rest;
        if piece.is_empty() {
            continue;
        }
        let (key, value) = match find_outside_tokens(piece, '=') {
            Some(at) => (piece.get(..at).unwrap_or_default(), piece.get(at + 1..).unwrap_or_default()),
            None => (piece, ""),
        };
        rows.push(Param { key: key.to_string(), value: value.to_string(), enabled: true, postman_extra: Extra::new() });
    }
    (address, rows)
}

fn script_pairs(rows: &[Param]) -> Vec<ScriptPair> {
    let pair = |row: &Param| ScriptPair { key: row.key.clone(), value: row.value.clone(), disabled: !row.enabled };
    rows.iter().map(pair).collect()
}

fn model_rows(pairs: Vec<ScriptPair>) -> Vec<Param> {
    let row = |pair: ScriptPair| Param {
        key: pair.key,
        value: pair.value,
        enabled: !pair.disabled,
        postman_extra: Extra::new(),
    };
    pairs.into_iter().map(row).collect()
}

/// The request as a pre-request script sees it: as typed, nothing is substituted. There is no file in it: of
/// a form-data body the enabled text fields, of a file or a GraphQL body only the kind.
pub(crate) fn script_request(request: &RequestContent) -> ScriptRequest {
    let body = match &request.body {
        Body::None | Body::Binary { file: None } => ScriptBody::None,
        Body::Raw { text, .. } => ScriptBody::Raw(text.clone()),
        Body::UrlEncoded(fields) => ScriptBody::UrlEncoded(script_pairs(fields)),
        Body::FormData(parts) => ScriptBody::FormData(
            parts
                .iter()
                .filter(|part| part.enabled)
                .filter_map(|part| match &part.value {
                    FormValue::Text(text) => {
                        Some(ScriptPair { key: part.key.clone(), value: text.clone(), disabled: false })
                    }
                    FormValue::Files(_) => None,
                })
                .collect(),
        ),
        Body::Binary { file: Some(_) } => ScriptBody::Other("file".to_string()),
        Body::GraphQl { .. } => ScriptBody::Other("graphql".to_string()),
    };
    ScriptRequest {
        method: request.method.clone(),
        url: script_url(request),
        headers: script_pairs(&request.headers),
        body,
    }
}

/// The request of this send after the pre-request scripts: `request` with what they changed. `before` is
/// what the scripts were given ([`script_request`] of `request`), `after` — what they left. `None` — they
/// changed nothing. A part the scripts did not touch stays exactly as it is in `request`; the result is
/// sent and never stored.
pub(crate) fn edited_request(
    request: &RequestContent,
    before: &ScriptRequest,
    after: ScriptRequest,
) -> Option<RequestContent> {
    if *before == after {
        return None;
    }
    let mut edited = request.clone();
    edited.method = after.method;
    if after.url != before.url {
        // The query stays rows: each is substituted and encoded on its own, and the query parameter of
        // the authorization still replaces the row of its name.
        (edited.url, edited.query) = model_url(&after.url);
    }
    if after.headers != before.headers {
        edited.headers = model_rows(after.headers);
    }
    if after.body != before.body {
        match after.body {
            ScriptBody::Raw(text) => {
                let language = match &request.body {
                    Body::Raw { language, .. } => *language,
                    _ => RawLanguage::Text,
                };
                edited.body = Body::Raw { text, language };
            }
            ScriptBody::UrlEncoded(fields) => edited.body = Body::UrlEncoded(model_rows(fields)),
            ScriptBody::None => edited.body = Body::None,
            // A script cannot make such a body: the body of the request stays.
            ScriptBody::FormData(_) | ScriptBody::Other(_) => {}
        }
    }
    Some(edited)
}

/// The request as a test script sees it — the one that was sent: the method and the final address, the
/// headers that were sent (the own enabled ones and the one of the authorization; the automatic ones are
/// not known here) and after them the disabled rows of `request`, and `body` — the substituted body before
/// pruning. Built before `http` is handed to the engine.
pub(crate) fn sent_request(http: &HttpRequest, body: ScriptBody, request: &RequestContent) -> ScriptRequest {
    let sent = http.headers.iter().map(|(key, value)| ScriptPair {
        key: key.clone(),
        value: value.clone(),
        disabled: false,
    });
    let disabled = script_pairs(&request.headers).into_iter().filter(|pair| pair.disabled);
    ScriptRequest { method: http.method.clone(), url: http.url.clone(), headers: sent.chain(disabled).collect(), body }
}

/// The scripts of one send while it runs: the variables go from phase to phase, the report gathers both.
pub(crate) struct ScriptRun {
    /// The variables after the last script that ran; the local ones live for this send only.
    pub(crate) variables: Variables,
    /// The request after the last script of the last phase; means something after the pre-request phase only.
    pub(crate) request: ScriptRequest,
    /// A pre-request script called `pm.execution.skipRequest()`.
    pub(crate) skip_request: bool,
    pub(crate) report: ScriptsReport,
    info: Info,
    /// The start of the send: the console lines of both phases are timed from it.
    epoch: Instant,
    cancel: CancellationToken,
    /// The settings of the requests the scripts send: those of the application, not of the request.
    options: SendOptions,
}

impl ScriptRun {
    /// The variables are the enabled ones of the stored scopes of `input`; `epoch` is the start of the send.
    pub(crate) fn new(input: &SendInput, epoch: Instant, cancel: CancellationToken) -> Self {
        let sources = ScopeSources::of_input(input);
        let enabled = |variables: &[Variable]| -> Vec<Var> {
            let enabled = variables.iter().filter(|variable| variable.enabled);
            enabled.map(|variable| Var::new(variable.key.as_str(), variable.value.as_str())).collect()
        };
        let variables = Variables {
            globals: enabled(&sources.globals),
            collection: enabled(&sources.collection),
            environment: enabled(&sources.environment),
            environment_name: input.environment.as_ref().map(|environment| environment.name.clone()),
            local: Vec::new(),
        };
        let info = Info { request_id: input.request_id.clone(), request_name: input.request_name.clone() };
        let options = SendOptions {
            timeout: input.settings.timeout,
            verify_tls: input.settings.verify_tls,
            proxy: input.settings.proxy.clone(),
            ..SendOptions::default()
        };
        ScriptRun {
            variables,
            request: ScriptRequest::default(),
            skip_request: false,
            report: ScriptsReport::default(),
            info,
            epoch,
            cancel,
            options,
        }
    }

    /// Runs the scripts of one phase on a blocking thread and adds what they left to the report; their
    /// `pm.sendRequest` goes through `engine`. `request` is what `pm.request` shows them.
    /// Returns the error that stops a pre-request phase: the cancel if there is one, else the first error.
    pub(crate) async fn run_phase(
        &mut self,
        engine: &dyn HttpEngine,
        steps: &[ScriptStep],
        phase: ScriptPhase,
        response: Option<ScriptResponse>,
        request: ScriptRequest,
    ) -> Option<ScriptFailure> {
        let job = Job {
            phase: match phase {
                ScriptPhase::PreRequest => Phase::PreRequest,
                ScriptPhase::Test => Phase::Test,
            },
            scripts: steps.iter().map(|step| step.code.clone()).collect(),
            limits: Limits::default(),
            epoch: self.epoch,
            info: self.info.clone(),
            // Moved, not copied: after a failure of the thread nothing reads the variables any more.
            variables: std::mem::take(&mut self.variables),
            response,
            request,
        };
        let first_new = self.report.errors.len();
        match run_job(engine, job, &self.options, &self.cancel).await {
            Some(report) => self.take(steps, report),
            // Reachable in the dev and test profiles only: the release profile aborts on a panic.
            None => self.report.errors.extend(steps.first().map(|step| ScriptFailure {
                source: step.source.clone(),
                kind: ScriptErrorKind::Internal,
                message: "script thread failed".to_string(),
                line: None,
                column: None,
            })),
        }
        let new = self.report.errors.get(first_new..).unwrap_or_default();
        let cancelled = new.iter().find(|error| error.kind == ScriptErrorKind::Cancelled);
        cancelled.or(new.first()).cloned()
    }

    /// Adds a console line of the application itself, timed like the lines of the scripts.
    pub(crate) fn app_line(&mut self, level: ConsoleLevel, text: String) {
        let at_ms = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.report.console.push(ConsoleEntry { source: None, level, text, at_ms });
    }

    /// Adds the report of the engine for `steps` to the report of the send and keeps the variables.
    fn take(&mut self, steps: &[ScriptStep], report: Report) {
        // The engine numbers scripts by their place in the job; a number out of range cannot come from it.
        let source = |script: usize| steps.get(script).or(steps.last()).map(|step| step.source.clone());
        self.variables = report.variables;
        self.request = report.request;
        self.skip_request = report.skip_request;
        self.report.console_truncated |= report.console_truncated;
        self.report.changes.extend(report.changes);
        self.report.console.extend(report.console.into_iter().map(|line| ConsoleEntry {
            source: line.script.and_then(source),
            level: line.level,
            text: line.text,
            at_ms: line.at_ms,
        }));
        self.report.tests.extend(report.tests.into_iter().filter_map(|test| {
            Some(TestOutcome {
                source: source(test.script)?,
                name: test.name,
                passed: test.passed,
                skipped: test.skipped,
                error: test.error,
            })
        }));
        self.report.errors.extend(report.errors.into_iter().filter_map(|error| {
            Some(ScriptFailure {
                source: source(error.script)?,
                kind: error.kind,
                message: error.message,
                line: error.line,
                column: error.column,
            })
        }));
    }
}
