//! The host side of a script: the state its primitives write to, the primitives as Rust functions, and the
//! object `__host` that hands them to the prelude.
//!
//! The script shares its realm with the prelude and may get hold of a primitive, so every limit is checked
//! here, never in JavaScript. A primitive never panics: a bad argument is a JavaScript exception.

use crate::budget::{Budget, Watch};
use crate::bundle::{self, Bundle};
use crate::text;
use crate::variables::{self, Book, Target, View};
use crate::{
    Info, Job, Phase, Report, ResponseBody, ScriptError, ScriptResponse, StoredScope, TestResult,
    Variables, MAX_VALUE_BYTES,
};
use rquickjs::function::{IntoJsFunc, Opt};
use rquickjs::{Ctx, Exception, Function, Object, Value};
use std::cell::Cell;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Instant;

pub const CONSOLE_MAX_LINES: usize = 1000;
pub const CONSOLE_MAX_BYTES: usize = 1_048_576;
pub const CONSOLE_LINE_CHARS: usize = 10_000;

/// The most of a response body a script can get: 10 MiB.
pub const BODY_LIMIT: u64 = 10 * 1024 * 1024;
/// The most tests one job records.
pub const MAX_TESTS: usize = 1000;
/// The longest name and error text of a test, in characters.
pub const TEST_NAME_CHARS: usize = 1000;
pub const TEST_ERROR_CHARS: usize = 10_000;
/// The most text (UTF-8 bytes of the names and the errors) the tests of one job hold.
pub const MAX_TEST_TEXT_BYTES: usize = 256 * 1024;

/// The longest text `log` reads out of JavaScript, in UTF-8 bytes: what the prelude sends at most —
/// `2 * CONSOLE_LINE_CHARS` UTF-16 units of three bytes each.
const CONSOLE_TEXT_BYTES: usize = 6 * CONSOLE_LINE_CHARS;
/// The same for the name and the error of a test.
const TEST_NAME_BYTES: usize = 6 * TEST_NAME_CHARS;
const TEST_ERROR_BYTES: usize = 6 * TEST_ERROR_CHARS;

/// Dropped from the start of a response body.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

const NO_ENVIRONMENT: &str =
    "pm.environment.set: no environment is selected, the value is not saved";
const NOT_RUNNING: &str = "the script is not running any more";
const BUSY: &str = "the script host is busy";
const NO_RESPONSE: &str = "there is no response";
const TOO_MANY_TESTS: &str = "too many tests: more than 1000";
const TOO_MUCH_TEST_TEXT: &str = "too many tests: their results are larger than 256 KB";
const UNKNOWN_BUNDLE: &str = "unknown bundle";

/// The bundles `load_bundle` knows, by the names the prelude asks for them.
const LOADABLE: [(&str, Bundle); 1] = [("chai", Bundle::Chai)];
/// No name in `LOADABLE` is longer, in UTF-8 bytes.
const BUNDLE_NAME_BYTES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleLine {
    /// `None` — a line of the application itself.
    pub script: Option<usize>,
    pub level: ConsoleLevel,
    pub text: String,
    /// Whole milliseconds since `Job::epoch`.
    pub at_ms: u64,
}

/// A limit of the memory of the host a script went over. The script is stopped: its `try / catch` does not
/// help, and every primitive refuses from then on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostLimit {
    VariableBytes,
    ChangeCount,
    ChangeBytes,
}

impl HostLimit {
    pub(crate) fn message(self) -> &'static str {
        match self {
            HostLimit::VariableBytes => "the script stored more than 16 MiB of variables",
            HostLimit::ChangeCount => "the script changed variables more than 10000 times",
            HostLimit::ChangeBytes => "the script wrote more than 16 MiB of variable values",
        }
    }
}

/// Why a primitive did not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// An ordinary error the script may catch.
    Error(String),
    Limit(HostLimit),
}

impl Refusal {
    fn message(&self) -> &str {
        match self {
            Refusal::Error(message) => message,
            Refusal::Limit(limit) => limit.message(),
        }
    }
}

/// What the scripts of a job have produced so far. Shared by the script thread and `run`, which takes it
/// when the thread has ended or is abandoned.
#[derive(Debug)]
pub(crate) struct State {
    pub(crate) errors: Vec<ScriptError>,
    console: Vec<ConsoleLine>,
    console_truncated: bool,
    console_bytes: usize,
    book: Book,
    tests: Vec<TestResult>,
    /// UTF-8 bytes of the names and the errors in `tests`.
    test_bytes: usize,
}

impl State {
    pub(crate) fn new(variables: Variables) -> Self {
        State {
            errors: Vec::new(),
            console: Vec::new(),
            console_truncated: false,
            console_bytes: 0,
            book: Book::new(variables),
            tests: Vec::new(),
            test_bytes: 0,
        }
    }

    pub(crate) fn into_report(self) -> Report {
        Report {
            errors: self.errors,
            console: self.console,
            console_truncated: self.console_truncated,
            variables: self.book.variables,
            changes: self.book.changes,
            tests: self.tests,
        }
    }

    /// Records a test: the name cut to `TEST_NAME_CHARS`, the error to `TEST_ERROR_CHARS`. Refuses a test
    /// over `MAX_TESTS` and one whose text does not fit into `MAX_TEST_TEXT_BYTES`; what is recorded stays.
    fn test(
        &mut self,
        script: usize,
        name: &str,
        passed: bool,
        skipped: bool,
        error: Option<&str>,
    ) -> Result<(), Refusal> {
        if self.tests.len() >= MAX_TESTS {
            return Err(Refusal::Error(TOO_MANY_TESTS.to_string()));
        }
        let name = text::clip(name, TEST_NAME_CHARS);
        let error = error.map(|error| text::clip(error, TEST_ERROR_CHARS));
        let bytes = self
            .test_bytes
            .saturating_add(name.len())
            .saturating_add(error.map_or(0, str::len));
        if bytes > MAX_TEST_TEXT_BYTES {
            return Err(Refusal::Error(TOO_MUCH_TEST_TEXT.to_string()));
        }
        self.test_bytes = bytes;
        self.tests.push(TestResult {
            script,
            name: name.to_string(),
            passed,
            skipped,
            error: error.map(str::to_string),
        });
        Ok(())
    }

    /// Adds a line cut to `CONSOLE_LINE_CHARS`. Once the console of the job is full, lines are dropped.
    fn log(&mut self, script: usize, level: ConsoleLevel, text: &str, at_ms: u64) {
        if self.console_truncated {
            return;
        }
        let clipped = text::clip(text, CONSOLE_LINE_CHARS);
        let line = if clipped.len() < text.len() {
            format!("{clipped}…")
        } else {
            text.to_string()
        };
        let bytes = self.console_bytes.saturating_add(line.len());
        if self.console.len() >= CONSOLE_MAX_LINES || bytes > CONSOLE_MAX_BYTES {
            self.console_truncated = true;
            return;
        }
        self.console_bytes = bytes;
        self.console.push(ConsoleLine {
            script: Some(script),
            level,
            text: line,
            at_ms,
        });
    }
}

/// The primitives of one script.
pub(crate) struct Host {
    state: Arc<Mutex<Option<State>>>,
    watch: Arc<Watch>,
    budget: Rc<Budget>,
    /// Index of the script in `Job::scripts`.
    script: usize,
    phase: Phase,
    info: Info,
    epoch: Instant,
    /// The response of the request; the scripts of a job share it.
    response: Option<Arc<ScriptResponse>>,
    /// The script has been told that `pm.environment.set` has nowhere to store.
    warned_no_environment: Cell<bool>,
    /// Which of `LOADABLE` this context has been asked for already: bit `n` — entry `n`.
    asked_bundles: Cell<u32>,
}

impl Host {
    pub(crate) fn new(
        state: Arc<Mutex<Option<State>>>,
        watch: Arc<Watch>,
        budget: Rc<Budget>,
        script: usize,
        job: &Job,
        response: Option<Arc<ScriptResponse>>,
    ) -> Self {
        Host {
            state,
            watch,
            budget,
            script,
            phase: job.phase,
            info: job.info.clone(),
            epoch: job.epoch,
            response,
            warned_no_environment: Cell::new(false),
            asked_bundles: Cell::new(0),
        }
    }

    /// Every primitive goes through here. Refuses when the thread is abandoned or a host limit was crossed;
    /// never waits for the lock and never calls into JavaScript under it.
    fn with_state<T>(
        &self,
        action: impl FnOnce(&mut State) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        if self.watch.is_abandoned() {
            return Err(Refusal::Error(NOT_RUNNING.to_string()));
        }
        if let Some(limit) = self.budget.host_limit() {
            return Err(Refusal::Limit(limit));
        }
        let mut guard = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(Refusal::Error(BUSY.to_string())),
        };
        let Some(state) = guard.as_mut() else {
            return Err(Refusal::Error(NOT_RUNNING.to_string()));
        };
        let result = action(state);
        if let Err(Refusal::Limit(limit)) = &result {
            self.budget.set_host_limit(*limit);
        }
        result
    }

    fn at_ms(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn log(&self, level: ConsoleLevel, text: &str) -> Result<(), Refusal> {
        let at_ms = self.at_ms();
        self.with_state(|state| {
            state.log(self.script, level, text, at_ms);
            Ok(())
        })
    }

    /// JSON of what the prelude builds `pm.info` from.
    fn info(&self) -> Result<String, Refusal> {
        self.with_state(|_| {
            let event = match self.phase {
                Phase::PreRequest => "prerequest",
                Phase::Test => "test",
            };
            Ok(serde_json::json!({
                "eventName": event,
                "requestName": self.info.request_name,
                "requestId": self.info.request_id,
            })
            .to_string())
        })
    }

    fn environment_name(&self) -> Result<Option<String>, Refusal> {
        self.with_state(|state| Ok(state.book.variables.environment_name.clone()))
    }

    fn var_has(&self, view: View, key: &str) -> Result<bool, Refusal> {
        self.with_state(|state| Ok(state.book.find(view, key).is_some()))
    }

    fn var_text(&self, view: View, key: &str) -> Result<Option<String>, Refusal> {
        self.with_state(|state| Ok(state.book.find(view, key).map(|var| var.text.clone())))
    }

    fn var_typed(&self, view: View, key: &str) -> Result<Option<String>, Refusal> {
        self.with_state(|state| Ok(state.book.find(view, key).and_then(|var| var.typed.clone())))
    }

    /// JSON `[[key, text, typed or null], …]`, from the widest scope to the narrowest.
    fn var_list(&self, view: View) -> Result<String, Refusal> {
        self.with_state(|state| {
            let entries: Vec<(&str, &str, Option<&str>)> = state
                .book
                .entries(view)
                .into_iter()
                .map(|var| (var.key.as_str(), var.text.as_str(), var.typed.as_deref()))
                .collect();
            serde_json::to_string(&entries).map_err(|error| Refusal::Error(error.to_string()))
        })
    }

    fn var_replace(&self, view: View, input: &str) -> Result<String, Refusal> {
        self.with_state(|state| Ok(state.book.replace(view, input)))
    }

    fn var_set(
        &self,
        target: Target,
        key: String,
        text: String,
        typed: Option<String>,
    ) -> Result<(), Refusal> {
        let at_ms = self.at_ms();
        self.with_state(|state| {
            state.book.set(target, key, text, typed)?;
            let unsaved = target == Target::Stored(StoredScope::Environment)
                && state.book.logged_scope(target).is_none();
            if unsaved && !self.warned_no_environment.replace(true) {
                state.log(self.script, ConsoleLevel::Warn, NO_ENVIRONMENT, at_ms);
            }
            Ok(())
        })
    }

    fn var_unset(&self, target: Target, key: &str) -> Result<(), Refusal> {
        self.with_state(|state| state.book.unset(target, key))
    }

    fn var_clear(&self, target: Target) -> Result<(), Refusal> {
        self.with_state(|state| state.book.clear(target))
    }

    /// JSON of what the prelude builds `pm.response` from, without the body; `None` — there is no response.
    fn response(&self) -> Result<Option<String>, Refusal> {
        self.with_state(|_| {
            Ok(self.response.as_ref().map(|response| {
                serde_json::json!({
                    "code": response.code,
                    "status": response.status,
                    "headers": response.headers,
                    "time": response.time_ms,
                    "size": response.size,
                })
                .to_string()
            }))
        })
    }

    /// The body of the response as text; `None` — it is larger than `BODY_LIMIT`. Read anew on every call
    /// (the prelude remembers the text), with the stopwatch of the script standing and the state not locked.
    fn response_body(&self) -> Result<Option<String>, Refusal> {
        self.with_state(|_| Ok(()))?;
        let Some(response) = &self.response else {
            return Err(Refusal::Error(NO_RESPONSE.to_string()));
        };
        self.budget.pause();
        let text = body_text(&response.body);
        self.budget.resume();
        text
    }

    fn test(
        &self,
        name: &str,
        passed: bool,
        skipped: bool,
        error: Option<&str>,
    ) -> Result<(), Refusal> {
        self.with_state(|state| state.test(self.script, name, passed, skipped, error))
    }

    /// The bundle of that name when this context has to load it now; `None` — it was asked for before.
    /// A bundle counts as asked from here on, so a load that calls back into the script cannot start itself
    /// again. The lock of the state is not held when this returns: the caller runs JavaScript next.
    fn claim_bundle(&self, name: &str) -> Result<Option<Bundle>, Refusal> {
        self.with_state(|_| Ok(()))?;
        let known = LOADABLE
            .iter()
            .zip(0u32..)
            .find(|((known, _), _)| *known == name);
        let Some(((_, bundle), index)) = known else {
            return Err(Refusal::Error(format!("{UNKNOWN_BUNDLE}: {name}")));
        };
        let bit = 1u32.checked_shl(index).unwrap_or(0);
        let asked = self.asked_bundles.get();
        if asked & bit != 0 {
            return Ok(None);
        }
        self.asked_bundles.set(asked | bit);
        Ok(Some(*bundle))
    }
}

/// At most `BODY_LIMIT + 1` bytes of the body are taken into memory. The text is UTF-8: bytes that are not
/// become U+FFFD, a BOM at the start is dropped.
fn body_text(body: &ResponseBody) -> Result<Option<String>, Refusal> {
    let mut read = Vec::new();
    let bytes: &[u8] = match body {
        ResponseBody::Bytes(bytes) => bytes,
        ResponseBody::File(path) => {
            File::open(path)
                .and_then(|file| {
                    file.take(BODY_LIMIT.saturating_add(1))
                        .read_to_end(&mut read)
                })
                .map_err(|error| {
                    Refusal::Error(format!("response body cannot be read: {error}"))
                })?;
            &read
        }
    };
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > BODY_LIMIT {
        return Ok(None);
    }
    let text = bytes.strip_prefix(&BOM).unwrap_or(bytes);
    Ok(Some(String::from_utf8_lossy(text).into_owned()))
}

/// Puts the object `__host` into the global scope and returns it. The prelude takes it away from there
/// first thing and leaves on it the function `finish` (see `finish` below).
pub(crate) fn install<'js>(ctx: &Ctx<'js>, host: &Rc<Host>) -> rquickjs::Result<Object<'js>> {
    let object = Object::new(ctx.clone())?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "log",
        move |ctx: Ctx<'js>, level: i32, text: rquickjs::String<'js>| -> rquickjs::Result<()> {
            let level = match level {
                0 => ConsoleLevel::Log,
                1 => ConsoleLevel::Info,
                2 => ConsoleLevel::Warn,
                3 => ConsoleLevel::Error,
                4 => ConsoleLevel::Debug,
                _ => return Err(Exception::throw_range(&ctx, "unknown console level")),
            };
            let text = read(&ctx, text, CONSOLE_TEXT_BYTES, || {
                "console: the text is too long".to_string()
            })?;
            this.log(level, &text)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "info",
        move |ctx: Ctx<'js>| -> rquickjs::Result<String> {
            this.info().map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "env_name",
        move |ctx: Ctx<'js>| -> rquickjs::Result<Option<String>> {
            this.environment_name()
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_has",
        move |ctx: Ctx<'js>, view: i32, key: rquickjs::String<'js>| -> rquickjs::Result<bool> {
            let view = view_of(&ctx, view)?;
            let key = read_key(&ctx, key, view, "has")?;
            this.var_has(view, &key)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_text",
        move |ctx: Ctx<'js>,
              view: i32,
              key: rquickjs::String<'js>|
              -> rquickjs::Result<Option<String>> {
            let view = view_of(&ctx, view)?;
            let key = read_key(&ctx, key, view, "get")?;
            this.var_text(view, &key)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_typed",
        move |ctx: Ctx<'js>,
              view: i32,
              key: rquickjs::String<'js>|
              -> rquickjs::Result<Option<String>> {
            let view = view_of(&ctx, view)?;
            let key = read_key(&ctx, key, view, "get")?;
            this.var_typed(view, &key)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_list",
        move |ctx: Ctx<'js>, view: i32| -> rquickjs::Result<String> {
            let view = view_of(&ctx, view)?;
            this.var_list(view).map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_replace",
        move |ctx: Ctx<'js>, view: i32, input: rquickjs::String<'js>| -> rquickjs::Result<String> {
            let view = view_of(&ctx, view)?;
            // The length is bounded by the memory limit of the engine only: a template may be a whole body.
            let input = input.to_string()?;
            this.var_replace(view, &input)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_set",
        move |ctx: Ctx<'js>,
              target: i32,
              key: rquickjs::String<'js>,
              text: rquickjs::String<'js>,
              typed: Opt<Value<'js>>|
              -> rquickjs::Result<()> {
            let target = target_of(&ctx, target)?;
            let typed = optional_string(&ctx, typed, "typed is not a string")?;
            // The sizes are asked before anything is copied out of JavaScript.
            let key_bytes = byte_length(&key)?;
            let sizes = (
                key_bytes,
                byte_length(&text)?,
                typed.as_ref().map(byte_length).transpose()?.unwrap_or(0),
            );
            let key = if key_bytes > MAX_VALUE_BYTES {
                None
            } else {
                Some(key.to_string()?)
            };
            if let Some(error) =
                variables::set_size_error(target, key.as_deref(), sizes.0, sizes.1, sizes.2)
            {
                return Err(Exception::throw_message(&ctx, &error));
            }
            let typed = typed.map(|typed| typed.to_string()).transpose()?;
            this.var_set(target, key.unwrap_or_default(), text.to_string()?, typed)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_unset",
        move |ctx: Ctx<'js>, target: i32, key: rquickjs::String<'js>| -> rquickjs::Result<()> {
            let target = target_of(&ctx, target)?;
            let key = read_key(&ctx, key, View::One(target), "unset")?;
            this.var_unset(target, &key)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "var_clear",
        move |ctx: Ctx<'js>, target: i32| -> rquickjs::Result<()> {
            let target = target_of(&ctx, target)?;
            this.var_clear(target)
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "response",
        move |ctx: Ctx<'js>| -> rquickjs::Result<Option<String>> {
            this.response().map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "response_body",
        move |ctx: Ctx<'js>| -> rquickjs::Result<Option<String>> {
            this.response_body()
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "test",
        move |ctx: Ctx<'js>,
              name: rquickjs::String<'js>,
              passed: bool,
              skipped: bool,
              error: Opt<Value<'js>>|
              -> rquickjs::Result<()> {
            let error = optional_string(&ctx, error, "error is not a string")?;
            let name = read(&ctx, name, TEST_NAME_BYTES, || {
                "pm.test: the name is too long".to_string()
            })?;
            let error = error
                .map(|error| {
                    read(&ctx, error, TEST_ERROR_BYTES, || {
                        "pm.test: the error text is too long".to_string()
                    })
                })
                .transpose()?;
            this.test(&name, passed, skipped, error.as_deref())
                .map_err(|refusal| throw(&ctx, &refusal))
        },
    )?;

    let this = Rc::clone(host);
    add(
        ctx,
        &object,
        "load_bundle",
        move |ctx: Ctx<'js>, name: rquickjs::String<'js>| -> rquickjs::Result<()> {
            let name = read(&ctx, name, BUNDLE_NAME_BYTES, || UNKNOWN_BUNDLE.to_string())?;
            let Some(bundle) = this
                .claim_bundle(&name)
                .map_err(|refusal| throw(&ctx, &refusal))?
            else {
                return Ok(());
            };
            // Compiling runs no code of the script: the stopwatch stands. Running the bundle does — the
            // script may have replaced the built-in objects the bundle uses — so that is on the clock.
            this.budget.pause();
            let compiled = bundle::compile(&ctx, bundle);
            this.budget.resume();
            compiled?;
            bundle::load(&ctx, bundle)
        },
    )?;

    ctx.globals().set("__host", object.clone())?;
    Ok(object)
}

/// Tells the prelude that the text of the script has ended: the tests that are still waiting fail, the
/// pairs of the old `tests` object become tests. `hooks` is the object `install` returned; it lives
/// inside one `Context::with`, as every JavaScript value does. Runs code of the script (getters of `tests`).
pub(crate) fn finish<'js>(hooks: &Object<'js>) -> rquickjs::Result<()> {
    let finish: Function<'js> = hooks.get("finish")?;
    finish.call(())
}

/// An argument that may be left out: a string, or nothing.
fn optional_string<'js>(
    ctx: &Ctx<'js>,
    value: Opt<Value<'js>>,
    not_a_string: &str,
) -> rquickjs::Result<Option<rquickjs::String<'js>>> {
    match value.0 {
        None => Ok(None),
        Some(value) if value.is_undefined() => Ok(None),
        Some(value) => match value.into_string() {
            Some(string) => Ok(Some(string)),
            None => Err(Exception::throw_type(ctx, not_a_string)),
        },
    }
}

/// Every primitive is named `rqm_host_<name>`: by that name a test tells a primitive that leaked to the script.
fn add<'js, P, F>(
    ctx: &Ctx<'js>,
    object: &Object<'js>,
    name: &str,
    function: F,
) -> rquickjs::Result<()>
where
    F: IntoJsFunc<'js, P> + 'js,
{
    let function = Function::new(ctx.clone(), function)?.with_name(format!("rqm_host_{name}"))?;
    object.set(name, function)
}

fn throw(ctx: &Ctx<'_>, refusal: &Refusal) -> rquickjs::Error {
    Exception::throw_message(ctx, refusal.message())
}

/// The numbers the prelude names the scopes by (`prelude/src/variables.js`).
fn target_of(ctx: &Ctx<'_>, number: i32) -> rquickjs::Result<Target> {
    match number {
        0 => Ok(Target::Stored(StoredScope::Globals)),
        1 => Ok(Target::Stored(StoredScope::Collection)),
        2 => Ok(Target::Stored(StoredScope::Environment)),
        3 => Ok(Target::Local),
        _ => Err(Exception::throw_range(ctx, "unknown variable scope")),
    }
}

fn view_of(ctx: &Ctx<'_>, number: i32) -> rquickjs::Result<View> {
    match number {
        4 => Ok(View::Effective),
        other => target_of(ctx, other).map(View::One),
    }
}

/// UTF-8 length of a JavaScript string, taken without copying it into Rust (the copy the engine makes lies
/// in its own heap, under its memory limit).
fn byte_length<'js>(string: &rquickjs::String<'js>) -> rquickjs::Result<usize> {
    Ok(string.clone().to_cstring()?.len())
}

/// Copies a JavaScript string of at most `max_bytes` into Rust.
fn read<'js>(
    ctx: &Ctx<'js>,
    string: rquickjs::String<'js>,
    max_bytes: usize,
    too_long: impl FnOnce() -> String,
) -> rquickjs::Result<String> {
    if byte_length(&string)? > max_bytes {
        return Err(Exception::throw_message(ctx, &too_long()));
    }
    string.to_string()
}

fn read_key<'js>(
    ctx: &Ctx<'js>,
    key: rquickjs::String<'js>,
    view: View,
    method: &str,
) -> rquickjs::Result<String> {
    read(ctx, key, MAX_VALUE_BYTES, || view.key_too_long(method))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, Var};
    use tokio_util::sync::CancellationToken;

    const ENVIRONMENT: Target = Target::Stored(StoredScope::Environment);

    fn host(variables: Variables) -> Host {
        let watch = Arc::new(Watch::default());
        let budget = Rc::new(Budget::new(
            &Limits::default(),
            CancellationToken::new(),
            Arc::clone(&watch),
        ));
        let job = Job::new(Phase::Test, Vec::new());
        Host::new(
            Arc::new(Mutex::new(Some(State::new(variables)))),
            watch,
            budget,
            0,
            &job,
            None,
        )
    }

    fn report(host: &Host) -> Report {
        let state = host.state.lock().unwrap().take().unwrap();
        state.into_report()
    }

    #[test]
    fn console_primitive_enforces_its_limits() {
        // A line is cut by characters, not by bytes.
        let long = host(Variables::default());
        long.log(ConsoleLevel::Log, &"я".repeat(20_000)).unwrap();
        let lines = report(&long).console;
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text.chars().count(), CONSOLE_LINE_CHARS + 1);
        assert!(lines[0].text.ends_with('…'));

        let many = host(Variables::default());
        for _ in 0..5000 {
            many.log(ConsoleLevel::Log, "1").unwrap();
        }
        let report_of_many = report(&many);
        assert_eq!(report_of_many.console.len(), CONSOLE_MAX_LINES);
        assert!(report_of_many.console_truncated);

        let big = host(Variables::default());
        for _ in 0..300 {
            big.log(ConsoleLevel::Log, &"x".repeat(5000)).unwrap();
        }
        let report_of_big = report(&big);
        let bytes: usize = report_of_big
            .console
            .iter()
            .map(|line| line.text.len())
            .sum();
        assert_eq!(report_of_big.console.len(), CONSOLE_MAX_BYTES / 5000);
        assert!(bytes <= CONSOLE_MAX_BYTES);
        assert!(report_of_big.console_truncated);
    }

    #[test]
    fn test_primitive_enforces_its_limits() {
        // A name is cut by characters, not by bytes.
        let long = host(Variables::default());
        long.test(&"я".repeat(5000), true, false, None).unwrap();
        assert_eq!(report(&long).tests[0].name, "я".repeat(TEST_NAME_CHARS));

        let many = host(Variables::default());
        for _ in 0..MAX_TESTS {
            many.test("t", true, false, None).unwrap();
        }
        assert_eq!(
            many.test("t", true, false, None),
            Err(Refusal::Error(TOO_MANY_TESTS.to_string()))
        );
        assert_eq!(report(&many).tests.len(), MAX_TESTS);

        // Exactly the limit fits; one byte more does not, and what is recorded stays.
        let big = host(Variables::default());
        let error = "e".repeat(TEST_ERROR_CHARS);
        let whole = MAX_TEST_TEXT_BYTES / (1 + TEST_ERROR_CHARS);
        for _ in 0..whole {
            big.test("t", false, false, Some(&error)).unwrap();
        }
        let left = MAX_TEST_TEXT_BYTES - whole * (1 + TEST_ERROR_CHARS);
        assert_eq!(
            big.test("t", false, false, Some(&"e".repeat(left))),
            Err(Refusal::Error(TOO_MUCH_TEST_TEXT.to_string()))
        );
        big.test("t", false, false, Some(&"e".repeat(left - 1)))
            .unwrap();
        let tests = report(&big).tests;
        assert_eq!(tests.len(), whole + 1);
        let bytes: usize = tests
            .iter()
            .map(|test| test.name.len() + test.error.as_ref().map_or(0, String::len))
            .sum();
        assert_eq!(bytes, MAX_TEST_TEXT_BYTES);
    }

    #[test]
    fn host_limit_stops_every_later_primitive() {
        let host = host(Variables {
            environment_name: Some("Dev".to_string()),
            ..Variables::default()
        });
        for _ in 0..crate::MAX_CHANGES {
            host.var_set(ENVIRONMENT, "i".to_string(), "1".to_string(), None)
                .unwrap();
        }
        let limit = Refusal::Limit(HostLimit::ChangeCount);
        assert_eq!(
            host.var_set(ENVIRONMENT, "i".to_string(), "1".to_string(), None),
            Err(limit.clone())
        );
        assert_eq!(host.budget.host_limit(), Some(HostLimit::ChangeCount));
        // The flag, not the state, refuses now: reading is refused too.
        assert_eq!(host.var_has(View::Effective, "i"), Err(limit.clone()));
        assert_eq!(host.log(ConsoleLevel::Log, "late"), Err(limit));
        let report = report(&host);
        assert_eq!(report.changes.len(), crate::MAX_CHANGES);
        assert_eq!(report.console, vec![]);
    }

    /// Calls every primitive; each has to refuse.
    fn all_refuse(host: &Host) {
        let view = View::Effective;
        assert!(host.log(ConsoleLevel::Log, "x").is_err());
        assert!(host.info().is_err());
        assert!(host.environment_name().is_err());
        assert!(host.var_has(view, "g").is_err());
        assert!(host.var_text(view, "g").is_err());
        assert!(host.var_typed(view, "g").is_err());
        assert!(host.var_list(view).is_err());
        assert!(host.var_replace(view, "{{g}}").is_err());
        assert!(host
            .var_set(ENVIRONMENT, "k".to_string(), "v".to_string(), None)
            .is_err());
        assert!(host
            .var_unset(Target::Stored(StoredScope::Globals), "g")
            .is_err());
        assert!(host
            .var_clear(Target::Stored(StoredScope::Globals))
            .is_err());
        assert!(host.response().is_err());
        assert!(host.response_body().is_err());
        assert!(host.test("t", true, false, None).is_err());
        assert!(host.claim_bundle("chai").is_err());
    }

    fn with_a_global() -> Variables {
        Variables {
            globals: vec![Var::new("g", "G")],
            ..Variables::default()
        }
    }

    #[test]
    fn load_bundle_refuses_unknown_names() {
        use rquickjs::{Context, Runtime};

        let host = Rc::new(host(Variables::default()));
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx, &host).unwrap();
            let refused: String = ctx
                .eval("try { __host.load_bundle('fs'); 'loaded' } catch (error) { String(error.message) }")
                .unwrap();
            assert_eq!(refused, "unknown bundle: fs");
            // The context is whole: the next call loads a known bundle, and only once.
            let loaded: String = ctx
                .eval(
                    "__host.load_bundle('chai'); const first = typeof __rqm_chai; delete globalThis.__rqm_chai;
                     __host.load_bundle('chai'); first + ' ' + typeof __rqm_chai",
                )
                .unwrap();
            assert_eq!(loaded, "function undefined");
        });
    }

    #[test]
    fn primitives_refuse_when_abandoned() {
        let host = host(with_a_global());
        host.watch.abandon();
        all_refuse(&host);
        let report = report(&host);
        assert_eq!(report.variables, with_a_global());
        assert_eq!(report.changes, vec![]);
        assert_eq!(report.console, vec![]);
        assert!(!report.console_truncated);
    }

    #[test]
    fn busy_state_is_an_error_not_a_panic() {
        let host = host(with_a_global());
        let state = Arc::clone(&host.state);
        let guard = state.lock().unwrap();
        all_refuse(&host);
        assert_eq!(
            host.log(ConsoleLevel::Log, "x"),
            Err(Refusal::Error(BUSY.to_string()))
        );
        drop(guard);
        // And a state `run` has already taken.
        report(&host);
        all_refuse(&host);
    }
}
