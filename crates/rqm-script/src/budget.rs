//! The time budget of one script — a stopwatch that runs only while JavaScript is executing — and what the
//! script thread shares with the watchdog in `run`: the deadline and the "abandoned" flag.

use crate::host::HostLimit;
use crate::Limits;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Takes the lock; a poisoned one is read as it is.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Shared by the script thread and the watchdog.
#[derive(Default)]
pub(crate) struct Watch {
    /// The moment the running script must have ended by; `None` while the stopwatch stands.
    deadline: Mutex<Option<Instant>>,
    /// Set by the watchdog when it gave up on the thread.
    abandoned: AtomicBool,
}

impl Watch {
    /// The watchdog decides under this lock, so the thread cannot move on to its next script meanwhile.
    pub(crate) fn deadline(&self) -> MutexGuard<'_, Option<Instant>> {
        lock(&self.deadline)
    }

    pub(crate) fn abandon(&self) {
        self.abandoned.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_abandoned(&self) -> bool {
        self.abandoned.load(Ordering::SeqCst)
    }
}

/// Why a script has to stop. Told by flags, never by the text of an exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    Cancelled,
    Timeout,
    /// The script went over a limit of the memory of the host.
    Memory(HostLimit),
    /// The watchdog gave up on the thread: whatever it reports now is dropped.
    Abandoned,
    /// The script waited for its timers longer than it may.
    TimerWait,
}

/// The longest sleep between two looks at the cancel token while a script waits for a timer.
const WAIT_STEP: Duration = Duration::from_millis(10);

pub(crate) struct Budget {
    limit: Duration,
    grace: Duration,
    used: Cell<Duration>,
    running_since: Cell<Option<Instant>>,
    expired: Cell<bool>,
    /// How long the script may wait for its timers, and how long it has.
    timer_wait: Duration,
    waited: Cell<Duration>,
    /// Set by a primitive; the interrupt handler stops the script by it.
    host_limit: Cell<Option<HostLimit>>,
    cancel: CancellationToken,
    watch: Arc<Watch>,
}

impl Budget {
    pub(crate) fn new(limits: &Limits, cancel: CancellationToken, watch: Arc<Watch>) -> Self {
        Budget {
            limit: limits.time,
            grace: limits.grace,
            used: Cell::new(Duration::ZERO),
            running_since: Cell::new(None),
            expired: Cell::new(false),
            timer_wait: limits.timer_wait,
            waited: Cell::new(Duration::ZERO),
            host_limit: Cell::new(None),
            cancel,
            watch,
        }
    }

    /// JavaScript of the script starts or goes on: the stopwatch runs, the watchdog gets a deadline.
    pub(crate) fn resume(&self) {
        let now = Instant::now();
        self.running_since.set(Some(now));
        let left = self.limit.saturating_sub(self.used.get());
        *self.watch.deadline() = now
            .checked_add(left)
            .and_then(|end| end.checked_add(self.grace));
    }

    /// JavaScript of the script stands: the stopwatch stops, the watchdog has no deadline.
    pub(crate) fn pause(&self) {
        if let Some(since) = self.running_since.take() {
            self.used
                .set(self.used.get().saturating_add(since.elapsed()));
        }
        *self.watch.deadline() = None;
    }

    /// The interrupt handler of the engine asks this; the order is the order of the error kinds.
    pub(crate) fn stop(&self) -> Option<Stop> {
        if self.cancel.is_cancelled() {
            Some(Stop::Cancelled)
        } else if self.is_expired() {
            Some(Stop::Timeout)
        } else if let Some(limit) = self.host_limit.get() {
            Some(Stop::Memory(limit))
        } else if self.watch.is_abandoned() {
            Some(Stop::Abandoned)
        } else {
            None
        }
    }

    /// Waits for the moment a timer of the script is due. The stopwatch stands meanwhile and the watchdog
    /// has no deadline: waiting is not execution. Returns why the script has to stop instead, if it has.
    pub(crate) fn wait_until(&self, due: Instant) -> Option<Stop> {
        self.pause();
        let stop = loop {
            if let Some(stop) = self.stop() {
                break Some(stop);
            }
            let left = due.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break None;
            }
            let allowed = self.timer_wait.saturating_sub(self.waited.get());
            if allowed.is_zero() {
                break Some(Stop::TimerWait);
            }
            let started = Instant::now();
            std::thread::sleep(left.min(allowed).min(WAIT_STEP));
            self.waited
                .set(self.waited.get().saturating_add(started.elapsed()));
        };
        self.resume();
        stop
    }

    pub(crate) fn host_limit(&self) -> Option<HostLimit> {
        self.host_limit.get()
    }

    /// Latches: the first limit crossed names the error.
    pub(crate) fn set_host_limit(&self, limit: HostLimit) {
        if self.host_limit.get().is_none() {
            self.host_limit.set(Some(limit));
        }
    }

    /// Latches once the time is spent.
    fn is_expired(&self) -> bool {
        if !self.expired.get() {
            let running = self
                .running_since
                .get()
                .map_or(Duration::ZERO, |since| since.elapsed());
            self.expired
                .set(self.used.get().saturating_add(running) > self.limit);
        }
        self.expired.get()
    }
}
