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

mod budget;
mod engine;
mod text;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// JavaScript execution time of one script; waiting for the host is not counted.
    pub time: Duration,
    pub memory_bytes: usize,
    pub stack_bytes: usize,
    /// How long past `time`, or past a cancel, the watchdog waits before it abandons the thread.
    pub grace: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            time: Duration::from_secs(10),
            memory_bytes: 128 * 1024 * 1024,
            stack_bytes: MAX_STACK_BYTES,
            grace: WATCHDOG_GRACE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    PreRequest,
    Test,
}

/// The scripts of one phase of one send, in the order they run.
#[derive(Debug, Clone)]
pub struct Job {
    pub phase: Phase,
    pub scripts: Vec<String>,
    pub limits: Limits,
    /// The start of the send: console lines are timed from it.
    pub epoch: Instant,
}

impl Job {
    /// Default limits, `epoch` = now, everything later tasks add — empty.
    pub fn new(phase: Phase, scripts: Vec<String>) -> Self {
        Job {
            phase,
            scripts,
            limits: Limits::default(),
            epoch: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub errors: Vec<ScriptError>,
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

/// Runs the scripts one after another on a thread of its own (stack `THREAD_STACK_BYTES`) and blocks until
/// they have finished — or until the watchdog gives up on a script the engine cannot interrupt: never longer
/// than the time the scripts are allowed plus `Limits::grace` per script. An empty job starts nothing.
pub fn run(job: Job, cancel: &CancellationToken) -> Report {
    engine::run(job, cancel)
}

/// Script threads `run` gave up on that have not ended yet. For tests and measurements.
pub fn abandoned_threads() -> usize {
    engine::abandoned_threads()
}
