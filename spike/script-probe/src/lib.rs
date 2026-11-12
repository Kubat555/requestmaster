//! Script engine probe: rquickjs (QuickJS-NG) on a dedicated OS thread with time and memory limits.
//!
//! Spike code: answers feasibility questions, not production-ready.

use rquickjs::context::EvalOptions;
use rquickjs::promise::PromiseState;
use rquickjs::{Context, Ctx, Error, Function, Promise, Runtime, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub struct Limits {
    pub time: Duration,
    pub memory_bytes: usize,
}

#[derive(Debug)]
pub enum ScriptOutcome {
    Ok(String),
    Threw(String),
    TimedOut,
    OutOfMemory,
}

/// Runs `source` after `prelude` in a fresh Runtime+Context on its own OS thread.
/// `Ok` holds the JSON of the last expression value (a returned Promise is awaited).
pub fn run_script(prelude: &str, source: &str, limits: Limits) -> ScriptOutcome {
    thread::scope(|s| s.spawn(|| execute(prelude, source, &limits, None)).join().unwrap())
}

/// Same as [`run_script`], plus a global `hostDelay(ms) -> Promise<string>` that a tokio
/// runtime on a separate thread resolves through channels. Time spent waiting for the host
/// is not charged to the script's time budget.
pub fn run_script_async(prelude: &str, source: &str, limits: Limits) -> ScriptOutcome {
    let (req_tx, mut req_rx) = tokio::sync::mpsc::unbounded_channel::<HostRequest>();
    let (resp_tx, resp_rx) = mpsc::channel::<HostResponse>();

    thread::scope(|s| {
        s.spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
            rt.block_on(async move {
                // Ends when the engine thread drops its request sender (runtime torn down).
                while let Some(req) = req_rx.recv().await {
                    let resp_tx = resp_tx.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(req.delay_ms)).await;
                        let _ = resp_tx.send(HostResponse { id: req.id, value: "done".to_string() });
                    });
                }
            });
        });
        s.spawn(move || execute(prelude, source, &limits, Some(Host { req_tx, resp_rx })))
            .join()
            .unwrap()
    })
}

struct HostRequest {
    id: u64,
    delay_ms: u64,
}

struct HostResponse {
    id: u64,
    value: String,
}

struct Host {
    req_tx: tokio::sync::mpsc::UnboundedSender<HostRequest>,
    resp_rx: mpsc::Receiver<HostResponse>,
}

/// Script time budget: a stopwatch that only runs while JavaScript is executing.
struct Budget {
    limit: Duration,
    used: Cell<Duration>,
    running_since: Cell<Option<Instant>>,
    expired: Cell<bool>,
}

impl Budget {
    fn new(limit: Duration) -> Self {
        Budget { limit, used: Cell::new(Duration::ZERO), running_since: Cell::new(None), expired: Cell::new(false) }
    }

    fn resume(&self) {
        self.running_since.set(Some(Instant::now()));
    }

    fn pause(&self) {
        if let Some(since) = self.running_since.take() {
            self.used.set(self.used.get() + since.elapsed());
        }
    }

    /// Called from the engine's interrupt handler; latches once the budget is spent.
    fn check(&self) -> bool {
        if !self.expired.get() {
            let running = self.running_since.get().map_or(Duration::ZERO, |s| s.elapsed());
            self.expired.set(self.used.get() + running > self.limit);
        }
        self.expired.get()
    }
}

fn execute(prelude: &str, source: &str, limits: &Limits, host: Option<Host>) -> ScriptOutcome {
    let rt = Runtime::new().unwrap();
    rt.set_memory_limit(limits.memory_bytes);
    let budget = Rc::new(Budget::new(limits.time));
    {
        let budget = budget.clone();
        rt.set_interrupt_handler(Some(Box::new(move || budget.check())));
    }
    let ctx = Context::full(&rt).unwrap();
    ctx.with(|ctx| {
        // The budget is paused while the prelude loads: only user code is charged.
        if let Err(e) = ctx.eval_with_options::<(), _>(prelude, sloppy()) {
            return match outcome_from_error(&ctx, e, &budget) {
                ScriptOutcome::Threw(m) => ScriptOutcome::Threw(format!("prelude failed: {m}")),
                other => other,
            };
        }
        drive(&ctx, source, &budget, host)
    })
}

fn sloppy() -> EvalOptions {
    let mut opts = EvalOptions::default();
    opts.strict = false;
    opts
}

fn drive<'js>(ctx: &Ctx<'js>, source: &str, budget: &Budget, host: Option<Host>) -> ScriptOutcome {
    let pending: Rc<RefCell<HashMap<u64, Function<'js>>>> = Rc::default();
    let resp_rx = match host {
        Some(Host { req_tx, resp_rx }) => {
            install_host_delay(ctx, req_tx, pending.clone()).unwrap();
            Some(resp_rx)
        }
        None => None,
    };

    budget.resume();
    let outcome = run_to_completion(ctx, source, budget, &pending, resp_rx.as_ref());
    budget.pause();
    // Resolve functions held on the Rust side are invisible to the JS GC; drop them before the
    // runtime is freed, otherwise QuickJS reports leaked objects on teardown.
    pending.borrow_mut().clear();
    outcome
}

fn install_host_delay<'js>(
    ctx: &Ctx<'js>,
    req_tx: tokio::sync::mpsc::UnboundedSender<HostRequest>,
    pending: Rc<RefCell<HashMap<u64, Function<'js>>>>,
) -> rquickjs::Result<()> {
    let next_id = Cell::new(0u64);
    let host_delay = Function::new(ctx.clone(), move |ctx: Ctx<'js>, ms: f64| -> rquickjs::Result<Promise<'js>> {
        let (promise, resolve, _reject) = Promise::new(&ctx)?;
        let id = next_id.get();
        next_id.set(id + 1);
        pending.borrow_mut().insert(id, resolve);
        let _ = req_tx.send(HostRequest { id, delay_ms: ms.max(0.0) as u64 });
        Ok(promise)
    })?;
    ctx.globals().set("hostDelay", host_delay)
}

fn run_to_completion<'js>(
    ctx: &Ctx<'js>,
    source: &str,
    budget: &Budget,
    pending: &RefCell<HashMap<u64, Function<'js>>>,
    resp_rx: Option<&mpsc::Receiver<HostResponse>>,
) -> ScriptOutcome {
    let value: Value<'js> = match ctx.eval_with_options(source, sloppy()) {
        Ok(v) => v,
        Err(e) => return outcome_from_error(ctx, e, budget),
    };
    let Some(promise) = value.as_promise().cloned() else {
        return to_json(ctx, value, budget);
    };
    loop {
        while ctx.execute_pending_job() {
            if budget.expired.get() {
                return ScriptOutcome::TimedOut;
            }
        }
        if budget.expired.get() {
            return ScriptOutcome::TimedOut;
        }
        match promise.state() {
            PromiseState::Resolved => return to_json(ctx, promise.result::<Value>().unwrap().unwrap(), budget),
            PromiseState::Rejected => {
                let err = promise.result::<Value>().unwrap().unwrap_err();
                return outcome_from_error(ctx, err, budget);
            }
            PromiseState::Pending => {}
        }
        let rx = match resp_rx {
            Some(rx) if !pending.borrow().is_empty() => rx,
            _ => return ScriptOutcome::Threw("promise never settles: no pending host work".into()),
        };
        budget.pause();
        let resp = rx.recv().unwrap();
        budget.resume();
        let resolve = pending.borrow_mut().remove(&resp.id).unwrap();
        if let Err(e) = resolve.call::<_, ()>((resp.value,)) {
            return outcome_from_error(ctx, e, budget);
        }
    }
}

fn to_json<'js>(ctx: &Ctx<'js>, value: Value<'js>, budget: &Budget) -> ScriptOutcome {
    match ctx.json_stringify(value) {
        Ok(Some(s)) => ScriptOutcome::Ok(s.to_string().unwrap()),
        Ok(None) => ScriptOutcome::Ok("undefined".into()),
        Err(e) => outcome_from_error(ctx, e, budget),
    }
}

fn outcome_from_error(ctx: &Ctx<'_>, err: Error, budget: &Budget) -> ScriptOutcome {
    let thrown = matches!(err, Error::Exception).then(|| ctx.catch());
    if budget.expired.get() {
        return ScriptOutcome::TimedOut;
    }
    if matches!(err, Error::Allocation) {
        return ScriptOutcome::OutOfMemory;
    }
    let Some(thrown) = thrown else {
        return ScriptOutcome::Threw(err.to_string());
    };
    // Read name/message as plain properties: chai's AssertionError (assertion-error 1.x) is an
    // object with Error.prototype in its chain, not a native Error, so as_exception() misses it.
    let message = match thrown.as_object() {
        Some(obj) => {
            let name: Option<String> = obj.get("name").ok();
            let message: Option<String> = obj.get("message").ok();
            format!("{}: {}", name.unwrap_or_else(|| "Error".into()), message.unwrap_or_default())
        }
        None => match thrown.as_string() {
            Some(s) => s.to_string().unwrap_or_default(),
            None => format!("{thrown:?}"),
        },
    };
    if message.contains("out of memory") {
        ScriptOutcome::OutOfMemory
    } else {
        ScriptOutcome::Threw(message)
    }
}
