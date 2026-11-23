//! Script engine of RequestMaster: runs the scripts of one phase of a send in QuickJS, on a thread of its own,
//! under limits of time, memory and stack.
//!
//! No panics in this crate: the release profile aborts on panic, and that would close the application.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::string_slice,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod api;
mod budget;
mod bundle;
mod engine;
mod host;
mod text;
mod variables;

pub use api::unsupported_api;
pub use host::{
    ConsoleLevel, ConsoleLine, OutgoingBody, OutgoingRequest, BODY_LIMIT, CONSOLE_LINE_CHARS,
    CONSOLE_MAX_BYTES, CONSOLE_MAX_LINES, MAX_TESTS, MAX_TEST_TEXT_BYTES, TEST_ERROR_CHARS,
    TEST_NAME_CHARS,
};
pub use variables::{
    StoredScope, Var, VariableChange, VariableOp, Variables, MAX_CHANGES, MAX_CHANGES_BYTES,
    MAX_VALUE_BYTES, MAX_VARIABLES_BYTES,
};

use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Stack of the thread the scripts run on. Must stay at least twice `MAX_STACK_BYTES`.
pub const THREAD_STACK_BYTES: usize = 16 * 1024 * 1024;
/// Bounds `Limits::stack_bytes` is clamped to. Above 16 MiB rquickjs switches the stack check off; a limit
/// equal to the stack of the thread never fires before the real overflow, which kills the process — so the
/// upper bound stays at half of `THREAD_STACK_BYTES`.
pub const MIN_STACK_BYTES: usize = 1024 * 1024;
pub const MAX_STACK_BYTES: usize = 8 * 1024 * 1024;
const _: () = assert!(MAX_STACK_BYTES * 2 <= THREAD_STACK_BYTES);
/// How long `run` waits past the time limit, or past a cancel, for a script the engine cannot interrupt.
pub const WATCHDOG_GRACE: Duration = Duration::from_secs(2);
/// The most script threads that may still be running after `run` gave up on them.
pub const MAX_ABANDONED_THREADS: usize = 4;
/// The most a script may wait for its timers.
pub const TIMER_WAIT_LIMIT: Duration = Duration::from_secs(60);
/// The most `pm.sendRequest` calls of one script.
pub const MAX_SEND_REQUESTS: usize = 50;
/// The most one `pm.sendRequest` may carry: UTF-8 bytes of the method, the address, the headers and the body.
pub const MAX_REQUEST_BYTES: usize = 10 * 1_048_576;
/// The most the requests waiting in the queue of one script may hold together.
pub const MAX_QUEUED_REQUEST_BYTES: usize = 32 * 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// JavaScript execution time of one script; waiting for the host is not counted.
    pub time: Duration,
    pub memory_bytes: usize,
    pub stack_bytes: usize,
    /// How long past `time`, or past a cancel, the watchdog waits before it abandons the thread.
    pub grace: Duration,
    /// How long one script may wait for its timers, all waits together.
    pub timer_wait: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            time: Duration::from_secs(10),
            memory_bytes: 128 * 1024 * 1024,
            stack_bytes: MAX_STACK_BYTES,
            grace: WATCHDOG_GRACE,
            timer_wait: TIMER_WAIT_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    PreRequest,
    Test,
}

/// The request the scripts belong to, as `pm.info` shows it.
#[derive(Debug, Clone, Default)]
pub struct Info {
    pub request_id: String,
    pub request_name: String,
}

/// The response of the request, as `pm.response` shows it.
#[derive(Debug, Clone)]
pub struct ScriptResponse {
    pub code: u16,
    /// Reason phrase.
    pub status: String,
    pub headers: Vec<(String, String)>,
    pub time_ms: f64,
    /// Size of the decoded body in bytes.
    pub size: u64,
    pub body: ResponseBody,
}

#[derive(Debug, Clone)]
pub enum ResponseBody {
    /// Read from the file when a script asks for the body; the file is not deleted.
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// The request of the send, as `pm.request` shows it. There is no file in it, and there will be none.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScriptRequest {
    pub method: String,
    /// The address with its query (the query stands before the `#fragment`). Pre-request: as typed,
    /// variables are not substituted. Test: the address that was sent.
    pub url: String,
    pub headers: Vec<ScriptPair>,
    pub body: ScriptBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptPair {
    pub key: String,
    pub value: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ScriptBody {
    #[default]
    None,
    Raw(String),
    UrlEncoded(Vec<ScriptPair>),
    /// The text fields of a form-data body (file parts are left out): a script reads them in the legacy
    /// `request.data`, sees the mode `formdata` and cannot edit the body.
    FormData(Vec<ScriptPair>),
    /// A body a script sees only the mode of and cannot edit: `file`, `graphql`.
    Other(String),
}

/// The scripts of one phase of one send, in the order they run.
#[derive(Debug, Clone)]
pub struct Job {
    pub phase: Phase,
    pub scripts: Vec<String>,
    pub limits: Limits,
    /// The start of the send: console lines are timed from it.
    pub epoch: Instant,
    pub info: Info,
    /// What the scripts see; what they leave comes back in `Report::variables`.
    pub variables: Variables,
    /// `Some` only in `Phase::Test`: without it a script has no `pm.response`.
    pub response: Option<ScriptResponse>,
    /// What `pm.request` shows; what the scripts leave of it comes back in `Report::request`.
    pub request: ScriptRequest,
}

impl Job {
    /// Default limits, `epoch` = now, everything later tasks add — empty.
    pub fn new(phase: Phase, scripts: Vec<String>) -> Self {
        Job {
            phase,
            scripts,
            limits: Limits::default(),
            epoch: Instant::now(),
            info: Info::default(),
            variables: Variables::default(),
            response: None,
            request: ScriptRequest::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub errors: Vec<ScriptError>,
    pub console: Vec<ConsoleLine>,
    /// Lines were dropped: the console of a job holds `CONSOLE_MAX_LINES` lines and `CONSOLE_MAX_BYTES` bytes.
    pub console_truncated: bool,
    /// The variables after the last script that ran.
    pub variables: Variables,
    /// What the scripts changed in the stored scopes, in the order they did it.
    pub changes: Vec<VariableChange>,
    /// What `pm.test` recorded, in the order the tests ran.
    pub tests: Vec<TestResult>,
    /// The request after the last script that ran; the caller reads it after `Phase::PreRequest` only.
    pub request: ScriptRequest,
    /// `pm.execution.skipRequest()` was called; never true in `Phase::Test`.
    pub skip_request: bool,
    /// The largest `memory_used_size` a runtime of a script of the job had at the end of its script.
    #[cfg(feature = "measure")]
    pub memory_used_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestResult {
    /// Index in `Job::scripts`.
    pub script: usize,
    pub name: String,
    pub passed: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptError {
    /// Index in `Job::scripts`.
    pub script: usize,
    pub kind: ScriptErrorKind,
    pub message: String,
    /// Line and column in the text of the script as the editor numbers them, from 1.
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptErrorKind {
    Exception,
    Timeout,
    Memory,
    Cancelled,
    Internal,
}

/// What the scripts ask of the application. The host moves to the thread of the scripts and may outlive
/// `run_with_host` there (a thread the watchdog gave up on), hence `'static`; an abandoned script never
/// reaches it.
pub trait Host {
    /// Sends the request and blocks until the answer; `Err` — the text of the network error.
    /// Must return soon after the token of the run is cancelled.
    fn send_request(&mut self, request: OutgoingRequest) -> Result<ScriptResponse, String>;
}

/// The host of `run`: it has no network.
struct NoHost;

impl Host for NoHost {
    fn send_request(&mut self, _request: OutgoingRequest) -> Result<ScriptResponse, String> {
        Err("pm.sendRequest is not available".to_string())
    }
}

/// Runs the scripts one after another on a thread of its own (stack `THREAD_STACK_BYTES`) and blocks until
/// they have finished — or until the watchdog gives up on a script the engine cannot interrupt: never longer
/// than the time the scripts are allowed (waiting for timers and for the host apart) plus `Limits::grace`
/// per script. An empty job starts nothing.
pub fn run_with_host(job: Job, host: Box<dyn Host + Send>, cancel: &CancellationToken) -> Report {
    engine::run(job, host, cancel)
}

/// `run_with_host` with a host that answers every request with the error `pm.sendRequest is not available`.
pub fn run(job: Job, cancel: &CancellationToken) -> Report {
    run_with_host(job, Box::new(NoHost), cancel)
}

/// Script threads `run` gave up on that have not ended yet. For tests and measurements.
pub fn abandoned_threads() -> usize {
    engine::abandoned_threads()
}
