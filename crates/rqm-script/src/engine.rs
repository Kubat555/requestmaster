//! The script thread, the watchdog that stops waiting for it, a `Runtime` and a `Context` per script, the
//! wrapper a script runs in, and the reading of its error.

use crate::budget::{lock, Budget, Stop, Watch};
use crate::bundle::{self, Bundle};
use crate::host::{self, Host, State};
use crate::text;
use crate::{
    Job, Limits, Phase, Report, ScriptError, ScriptErrorKind, ScriptResponse, Variables,
    MAX_ABANDONED_THREADS, MAX_STACK_BYTES, MIN_STACK_BYTES, THREAD_STACK_BYTES,
};
use rquickjs::context::EvalOptions;
use rquickjs::convert::Coerced;
use rquickjs::promise::PromiseState;
use rquickjs::{Context, Ctx, Error, FromJs, Function, Object, Runtime, Value};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// The name the engine knows the text of a script by; the stack of an error names it.
const SCRIPT_FILE: &str = "script.js";
/// The wrapper makes `await` and `return` work at the top level of a script. The text of the script starts
/// on the first line of the wrapper, so the line numbers are the ones of the editor.
const WRAPPER_START: &str = "(async()=>{";
const WRAPPER_END: &str = "\n})()";
/// The longest text of an error, in characters.
const MAX_ERROR_CHARS: usize = 10_000;
/// The watchdog looks at the thread at least this often.
const WATCH_STEP: Duration = Duration::from_millis(20);

const THREAD_FAILED: &str = "script thread failed";
const TOO_MANY_ABANDONED: &str =
    "too many scripts are still running after their time limit; restart the application";
const NEVER_FINISHES: &str =
    "the script never finishes: its promise is pending and nothing can settle it";

/// Script threads `run` gave up on that have not ended yet.
static ABANDONED_THREADS: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn abandoned_threads() -> usize {
    ABANDONED_THREADS.load(Ordering::SeqCst)
}

/// What `run` shares with the script thread.
struct Shared {
    /// What the scripts have produced so far; the primitives of a script write here too. `None` once `run`
    /// has taken it: what the thread writes later is dropped.
    state: Arc<Mutex<Option<State>>>,
    watch: Arc<Watch>,
    /// Index of the script the thread is at.
    current: AtomicUsize,
    /// The thread went through all its scripts (it did not panic on the way).
    completed: AtomicBool,
    /// The thread has ended. Written under the lock of `state`.
    ended: AtomicBool,
}

impl Shared {
    fn push(&self, error: ScriptError) {
        if let Some(state) = lock(&self.state).as_mut() {
            state.errors.push(error);
        }
    }
}

/// Lives on the script thread and is dropped at its very end, on a panic too.
struct ThreadEnd {
    shared: Arc<Shared>,
    /// Dropped after `drop` below: `run` sees the channel closed only when the flags are final.
    _ended: mpsc::Sender<()>,
}

impl Drop for ThreadEnd {
    fn drop(&mut self) {
        let state = lock(&self.shared.state);
        self.shared.ended.store(true, Ordering::SeqCst);
        // The state is taken from a living thread only when the thread is abandoned.
        if state.is_none() {
            let _ = ABANDONED_THREADS.try_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                count.checked_sub(1)
            });
        }
    }
}

pub(crate) fn run(mut job: Job, cancel: &CancellationToken) -> Report {
    let variables = std::mem::take(&mut job.variables);
    if job.scripts.is_empty() {
        return Report {
            variables,
            ..Report::default()
        };
    }
    if abandoned_threads() >= MAX_ABANDONED_THREADS {
        return internal_report(TOO_MANY_ABANDONED, variables);
    }
    let limits = job.limits;
    // Shared by the hosts of all the scripts: the body may be large and is never copied.
    let response = job.response.take().map(Arc::new);
    let shared = Arc::new(Shared {
        state: Arc::new(Mutex::new(Some(State::new(variables)))),
        watch: Arc::new(Watch::default()),
        current: AtomicUsize::new(0),
        completed: AtomicBool::new(false),
        ended: AtomicBool::new(false),
    });
    let (ended_sender, ended_receiver) = mpsc::channel::<()>();
    let end = ThreadEnd {
        shared: Arc::clone(&shared),
        _ended: ended_sender,
    };
    let token = cancel.clone();
    // Not a scoped thread and never joined: an abandoned thread has to outlive this call.
    let spawned = std::thread::Builder::new()
        .name("rqm-script".to_string())
        .stack_size(THREAD_STACK_BYTES)
        .spawn(move || {
            let end = end;
            run_scripts(&job, response.as_ref(), &token, &end.shared);
        });
    if spawned.is_err() {
        // The closure, and `end` in it, is dropped: the state is free to take.
        return failed_report(&shared);
    }

    let mut cancelled_since: Option<Instant> = None;
    loop {
        match ended_receiver.recv_timeout(WATCH_STEP) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return finished_report(&shared),
            Err(RecvTimeoutError::Timeout) => {}
        }
        let now = Instant::now();
        let cancelled = cancel.is_cancelled();
        if cancelled && cancelled_since.is_none() {
            cancelled_since = Some(now);
        }
        let cancel_overdue = cancelled_since
            .is_some_and(|since| now.saturating_duration_since(since) >= limits.grace);
        if let Some(report) = abandon(&shared, now, cancel_overdue, cancelled, &limits) {
            return report;
        }
    }
}

/// The report of a thread that has ended.
fn finished_report(shared: &Shared) -> Report {
    if shared.completed.load(Ordering::SeqCst) {
        let state = lock(&shared.state).take();
        state.map(State::into_report).unwrap_or_default()
    } else {
        // The thread panicked. Reachable in the dev and test profiles only: release aborts on panic.
        failed_report(shared)
    }
}

/// The report of a thread that did not start or did not get through its scripts: one error of the engine;
/// the console, the variables and their changes stay as far as they got.
fn failed_report(shared: &Shared) -> Report {
    let state = lock(&shared.state).take();
    let mut report = state.map(State::into_report).unwrap_or_default();
    report.errors = internal_report(THREAD_FAILED, Variables::default()).errors;
    report
}

/// Gives up on the thread when its deadline has passed or a cancel has waited too long: takes the report,
/// names the error of the script the thread is stuck in, and leaves the thread to end by itself.
fn abandon(
    shared: &Shared,
    now: Instant,
    cancel_overdue: bool,
    cancelled: bool,
    limits: &Limits,
) -> Option<Report> {
    let deadline = shared.watch.deadline();
    if !cancel_overdue && !deadline.is_some_and(|deadline| now >= deadline) {
        return None;
    }
    let mut slot = lock(&shared.state);
    if shared.ended.load(Ordering::SeqCst) {
        return None;
    }
    let mut report = slot.take().map(State::into_report).unwrap_or_default();
    shared.watch.abandon();
    ABANDONED_THREADS.fetch_add(1, Ordering::SeqCst);
    drop(slot);
    drop(deadline);

    let script = shared.current.load(Ordering::SeqCst);
    let stop = if cancelled {
        Stop::Cancelled
    } else {
        Stop::Timeout
    };
    report
        .errors
        .push(Failure::stopped(stop, limits).into_error(script));
    Some(report)
}

/// No script ran: the variables go back as they came.
fn internal_report(message: &str, variables: Variables) -> Report {
    Report {
        errors: vec![ScriptError {
            script: 0,
            kind: ScriptErrorKind::Internal,
            message: message.to_string(),
            line: None,
            column: None,
        }],
        variables,
        ..Report::default()
    }
}

/// The body of the script thread.
fn run_scripts(
    job: &Job,
    response: Option<&Arc<ScriptResponse>>,
    cancel: &CancellationToken,
    shared: &Shared,
) {
    for (index, code) in job.scripts.iter().enumerate() {
        if shared.watch.is_abandoned() {
            break;
        }
        shared.current.store(index, Ordering::SeqCst);
        let failure = if cancel.is_cancelled() {
            Some(Failure::stopped(Stop::Cancelled, &job.limits))
        } else {
            run_script(index, code, job, response, cancel, shared)
        };
        if let Some(failure) = failure {
            let kind = failure.kind;
            shared.push(failure.into_error(index));
            if job.phase == Phase::PreRequest || kind == ScriptErrorKind::Cancelled {
                break;
            }
        }
    }
    shared.completed.store(true, Ordering::SeqCst);
}

/// An error of one script, before it knows its index.
struct Failure {
    kind: ScriptErrorKind,
    message: String,
    line: Option<u32>,
    column: Option<u32>,
}

impl Failure {
    fn new(kind: ScriptErrorKind, message: String) -> Self {
        Failure {
            kind,
            message,
            line: None,
            column: None,
        }
    }

    fn stopped(stop: Stop, limits: &Limits) -> Self {
        match stop {
            Stop::Cancelled => Failure::new(ScriptErrorKind::Cancelled, "cancelled".to_string()),
            Stop::Memory(limit) => {
                Failure::new(ScriptErrorKind::Memory, limit.message().to_string())
            }
            // The watchdog has already reported an abandoned script: what its thread says is dropped.
            Stop::Timeout | Stop::Abandoned => Failure::new(
                ScriptErrorKind::Timeout,
                format!("the script ran longer than {} ms", limits.time.as_millis()),
            ),
        }
    }

    fn memory(limits: &Limits) -> Self {
        Failure::new(
            ScriptErrorKind::Memory,
            format!(
                "the script used more than {} MiB of memory",
                limits.memory_bytes >> 20
            ),
        )
    }

    /// An error of the engine itself, outside the text of the script.
    fn engine(error: &Error, limits: &Limits) -> Self {
        match error {
            Error::Allocation => Failure::memory(limits),
            other => Failure::new(
                ScriptErrorKind::Internal,
                format!("script engine failed: {other}"),
            ),
        }
    }

    fn into_error(self, script: usize) -> ScriptError {
        ScriptError {
            script,
            kind: self.kind,
            message: self.message,
            line: self.line,
            column: self.column,
        }
    }
}

/// One script: a `Runtime` and a `Context` of its own, both created and dropped on this thread.
fn run_script(
    index: usize,
    code: &str,
    job: &Job,
    response: Option<&Arc<ScriptResponse>>,
    cancel: &CancellationToken,
    shared: &Shared,
) -> Option<Failure> {
    let limits = &job.limits;
    let watch = &shared.watch;
    let budget = Rc::new(Budget::new(limits, cancel.clone(), Arc::clone(watch)));
    let host = Rc::new(Host::new(
        Arc::clone(&shared.state),
        Arc::clone(watch),
        Rc::clone(&budget),
        index,
        job,
        response.cloned(),
    ));
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => return Some(Failure::engine(&error, limits)),
    };
    runtime.set_memory_limit(limits.memory_bytes);
    runtime.set_max_stack_size(limits.stack_bytes.clamp(MIN_STACK_BYTES, MAX_STACK_BYTES));
    let interrupt = Rc::clone(&budget);
    runtime.set_interrupt_handler(Some(Box::new(move || interrupt.stop().is_some())));
    let context = match Context::full(&runtime) {
        Ok(context) => context,
        Err(error) => return Some(Failure::engine(&error, limits)),
    };
    context.with(|ctx| {
        let script = Script {
            ctx: &ctx,
            code,
            limits,
            budget: &budget,
            well_formed: text::well_formed_function(&ctx),
        };
        // `__host`, then the prelude, then the text of the script. The stopwatch of the script stands
        // while the prelude loads.
        let prepared =
            host::install(&ctx, &host).and_then(|()| bundle::load(&ctx, Bundle::Prelude));
        if let Err(error) = prepared {
            return Some(script.failure(error, Origin::Engine));
        }
        budget.resume();
        let failure = script.execute();
        budget.pause();
        failure
    })
}

/// A script inside its context. Holds JavaScript values, so it lives inside one `Context::with` only.
struct Script<'a, 'js> {
    ctx: &'a Ctx<'js>,
    code: &'a str,
    limits: &'a Limits,
    budget: &'a Budget,
    well_formed: Option<Function<'js>>,
}

impl<'js> Script<'_, 'js> {
    fn execute(&self) -> Option<Failure> {
        let mut options = EvalOptions::default();
        options.strict = false;
        options.filename = Some(SCRIPT_FILE.to_string());
        let wrapped = format!("{WRAPPER_START}{}{WRAPPER_END}", self.code);
        let value: Value<'js> = match self.ctx.eval_with_options(wrapped, options) {
            Ok(value) => value,
            Err(error) => return Some(self.failure(error, Origin::Script)),
        };
        let promise = value.into_promise()?;
        while self.budget.stop().is_none() && self.ctx.execute_pending_job() {}
        if let Some(stop) = self.budget.stop() {
            return Some(Failure::stopped(stop, self.limits));
        }
        match promise.state() {
            PromiseState::Resolved => None,
            PromiseState::Rejected => match promise.result::<Value<'js>>() {
                Some(Err(error)) => Some(self.failure(error, Origin::Script)),
                _ => None,
            },
            // No job is left that could settle it.
            PromiseState::Pending => Some(Failure::new(
                ScriptErrorKind::Exception,
                NEVER_FINISHES.to_string(),
            )),
        }
    }

    /// What kind of error it is, is told by the flags first and by the error itself last.
    fn failure(&self, error: Error, origin: Origin) -> Failure {
        let thrown = matches!(error, Error::Exception).then(|| self.ctx.catch());
        if let Some(stop) = self.budget.stop() {
            return Failure::stopped(stop, self.limits);
        }
        let Some(thrown) = thrown else {
            return Failure::engine(&error, self.limits);
        };
        let read = self.read_thrown(&thrown);
        // Reading runs getters of the script; they are stopped like the script itself.
        if let Some(stop) = self.budget.stop() {
            return Failure::stopped(stop, self.limits);
        }
        if read.name.as_deref() == Some("InternalError") && read.text.contains("out of memory") {
            return Failure::memory(self.limits);
        }
        if origin == Origin::Engine {
            return Failure::new(
                ScriptErrorKind::Internal,
                format!("script engine failed: {}", read.text),
            );
        }
        let (line, column) = position(read.stack.as_deref(), self.code);
        Failure {
            kind: ScriptErrorKind::Exception,
            message: text::clip(&read.text, MAX_ERROR_CHARS).to_string(),
            line,
            column,
        }
    }

    /// `name`, `message` and `stack` are read as plain properties: an error of chai is not a native one.
    fn read_thrown(&self, thrown: &Value<'js>) -> Thrown {
        let Some(object) = thrown.as_object() else {
            return Thrown {
                name: None,
                text: self.string_form(thrown),
                stack: None,
            };
        };
        let name = self.property(object, "name");
        let message = self.property(object, "message");
        let stack = self.property(object, "stack");
        let text = match (&name, message) {
            (Some(name), Some(message)) => format!("{name}: {message}"),
            (Some(name), None) => name.clone(),
            (None, Some(message)) => message,
            (None, None) => self.string_form(thrown),
        };
        Thrown { name, text, stack }
    }

    /// A property that holds a string which is not empty.
    fn property(&self, object: &Object<'js>, key: &str) -> Option<String> {
        let value = match object.get::<_, Value<'js>>(key) {
            Ok(value) => value,
            Err(_) => {
                // A getter threw: its exception must not stay pending in the context.
                let _ = self.ctx.catch();
                return None;
            }
        };
        let text = text::read(value.as_string()?, self.well_formed.as_ref());
        (!text.is_empty()).then_some(text)
    }

    /// What `String(value)` gives.
    fn string_form(&self, value: &Value<'js>) -> String {
        if let Some(string) = value.as_string() {
            return text::read(string, self.well_formed.as_ref());
        }
        match Coerced::<rquickjs::String<'js>>::from_js(self.ctx, value.clone()) {
            Ok(string) => text::read(&string.0, self.well_formed.as_ref()),
            Err(_) => {
                let _ = self.ctx.catch();
                text::UNREADABLE.to_string()
            }
        }
    }
}

/// Whose code threw.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// The text of the script.
    Script,
    /// The code of the application that prepares the context: its error is not the user's.
    Engine,
}

/// What was thrown, as text.
struct Thrown {
    name: Option<String>,
    text: String,
    stack: Option<String>,
}

/// Line and column of the error in the text of the script, from the first frame of `stack` that names the
/// script.
fn position(stack: Option<&str>, code: &str) -> (Option<u32>, Option<u32>) {
    let Some((line, column)) = stack.and_then(|stack| stack.lines().find_map(frame_position))
    else {
        return (None, None);
    };
    let last_line = u32::try_from(code.matches('\n').count())
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    if line > last_line {
        // A syntax error at the very end of the text lands on the line of the wrapper.
        return (Some(last_line), None);
    }
    if line == 1 {
        let shift = u32::try_from(WRAPPER_START.len()).unwrap_or(0);
        return (Some(line), Some(column.saturating_sub(shift).max(1)));
    }
    (Some(line), Some(column))
}

/// Reads `at <name> (script.js:L:C)` and `at script.js:L:C`.
fn frame_position(frame: &str) -> Option<(u32, u32)> {
    let (_, rest) = frame.split_once(SCRIPT_FILE)?;
    let rest = rest.strip_prefix(':')?;
    let (line, rest) = rest.split_once(':')?;
    let column = rest
        .split(|character: char| !character.is_ascii_digit())
        .next()?;
    Some((line.parse().ok()?, column.parse().ok()?))
}
