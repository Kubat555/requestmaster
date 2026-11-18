//! The time budget of one script — a stopwatch that runs only while JavaScript is executing — and what the
//! script thread shares with the watchdog in `run`: the deadline and the "abandoned" flag.

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
    /// The watchdog gave up on the thread: whatever it reports now is dropped.
    Abandoned,
}

pub(crate) struct Budget {
    limit: Duration,
    grace: Duration,
    used: Cell<Duration>,
    running_since: Cell<Option<Instant>>,
    expired: Cell<bool>,
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
        } else if self.watch.is_abandoned() {
            Some(Stop::Abandoned)
        } else {
            None
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
