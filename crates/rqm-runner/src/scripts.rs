//! The scripts of a send: the chain of the nodes, the job for the script engine, and the report of the engine
//! in the terms of the tree (which node, which phase).

use std::time::Instant;

use rqm_http::{HttpEngine, SendOptions};
use rqm_model::{NodeContent, NodeKind, Scripts, Variable};
use rqm_script::{Info, Job, Limits, Phase, Report, ScriptResponse, Var, Variables};
use tokio_util::sync::CancellationToken;

pub use rqm_script::{ConsoleLevel, ScriptErrorKind, StoredScope, VariableChange, VariableOp};

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

/// The scripts of one send while it runs: the variables go from phase to phase, the report gathers both.
pub(crate) struct ScriptRun {
    /// The variables after the last script that ran; the local ones live for this send only.
    pub(crate) variables: Variables,
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
        ScriptRun { variables, report: ScriptsReport::default(), info, epoch, cancel, options }
    }

    /// Runs the scripts of one phase on a blocking thread and adds what they left to the report; their
    /// `pm.sendRequest` goes through `engine`.
    /// Returns the error that stops a pre-request phase: the cancel if there is one, else the first error.
    pub(crate) async fn run_phase(
        &mut self,
        engine: &dyn HttpEngine,
        steps: &[ScriptStep],
        phase: ScriptPhase,
        response: Option<ScriptResponse>,
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
