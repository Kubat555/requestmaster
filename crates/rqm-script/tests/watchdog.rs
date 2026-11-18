//! The watchdog: a script the engine cannot interrupt does not hold `run` for ever.
//!
//! One `#[test]` in the file: the count of abandoned threads is shared by the process, and the script below
//! keeps a processor core busy until the test process ends.

use rqm_script::{abandoned_threads, run, Job, Limits, Phase, ScriptErrorKind, WATCHDOG_GRACE};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[test]
fn native_loops_end_by_the_watchdog() {
    let mut job = Job::new(Phase::Test, vec!["new Array(2**32-1).join('')".to_string()]);
    job.limits = Limits {
        time: Duration::from_millis(200),
        ..Limits::default()
    };
    let started = Instant::now();
    let report = run(job, &CancellationToken::new());
    let elapsed = started.elapsed();

    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    assert_eq!(
        report.errors[0].message,
        "the script ran longer than 200 ms"
    );
    assert!(
        elapsed <= Duration::from_millis(200) + WATCHDOG_GRACE + Duration::from_secs(1),
        "{elapsed:?}"
    );

    // The next script runs although the abandoned thread is still busy.
    let next = run(
        Job::new(Phase::Test, vec!["1".to_string()]),
        &CancellationToken::new(),
    );
    assert_eq!(next.errors, vec![]);
    assert_eq!(abandoned_threads(), 1);
}
