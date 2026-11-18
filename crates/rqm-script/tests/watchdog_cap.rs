//! The watchdog: abandoned script threads are counted, and past `MAX_ABANDONED_THREADS` scripts do not run.
//!
//! One `#[test]` in the file, the steps go in order: the count is shared by the process.

use rqm_script::{
    abandoned_threads, run, Job, Limits, Phase, Report, ScriptErrorKind, MAX_ABANDONED_THREADS,
};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// A native loop of the engine that never ends and never calls the interrupt handler.
const ENDLESS: &str = "[].reverse.call({length: 2**53-1})";

fn limits(time: Duration, grace: Duration) -> Limits {
    Limits {
        time,
        grace,
        ..Limits::default()
    }
}

fn run_one(script: &str, limits: Limits, cancel: &CancellationToken) -> (Report, Duration) {
    let mut job = Job::new(Phase::Test, vec![script.to_string()]);
    job.limits = limits;
    let started = Instant::now();
    let report = run(job, cancel);
    (report, started.elapsed())
}

#[test]
fn abandoned_threads_are_counted_and_capped() {
    let short = limits(Duration::from_millis(100), Duration::from_millis(100));

    // (1) An abandoned thread that ends by itself takes its count back.
    let (report, elapsed) = run_one(
        "(10n ** 300000n).toString()",
        short,
        &CancellationToken::new(),
    );
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    assert!(elapsed < Duration::from_millis(1500), "{elapsed:?}");
    let waiting = Instant::now();
    while abandoned_threads() != 0 {
        assert!(
            waiting.elapsed() < Duration::from_secs(30),
            "the abandoned thread did not end"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    // (2) Cancel abandons a thread the engine cannot interrupt.
    let cancel = CancellationToken::new();
    let canceller = {
        let cancel = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel.cancel();
        })
    };
    let (report, elapsed) = run_one(
        ENDLESS,
        limits(Duration::from_secs(60), Duration::from_millis(300)),
        &cancel,
    );
    canceller.join().expect("canceller thread");
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Cancelled);
    assert!(elapsed <= Duration::from_millis(1500), "{elapsed:?}");
    assert_eq!(abandoned_threads(), 1);

    // (3) At the cap nothing runs any more.
    for _ in 0..3 {
        let (report, _) = run_one(ENDLESS, short, &CancellationToken::new());
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    }
    assert_eq!(abandoned_threads(), MAX_ABANDONED_THREADS);

    let (report, _) = run_one(
        "throw new Error('ran')",
        Limits::default(),
        &CancellationToken::new(),
    );
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Internal);
    assert_eq!(
        report.errors[0].message,
        "too many scripts are still running after their time limit; restart the application"
    );
}
