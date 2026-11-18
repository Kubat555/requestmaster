//! The asynchronous part of a script: timers, when a script is over, and what becomes of an error nobody
//! handled.

use rqm_script::{run, Job, Limits, Phase, Report, ScriptErrorKind, VariableOp};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

fn run_scripts(scripts: &[&str], limits: Limits) -> Report {
    let mut job = Job::new(
        Phase::Test,
        scripts.iter().map(|script| script.to_string()).collect(),
    );
    job.limits = limits;
    run(job, &CancellationToken::new())
}

fn run_one(script: &str) -> Report {
    run_scripts(&[script], Limits::default())
}

fn console(report: &Report) -> Vec<&str> {
    report
        .console
        .iter()
        .map(|line| line.text.as_str())
        .collect()
}

#[test]
fn timers_fire_in_order() {
    let report = run_one(
        "setTimeout(() => console.log('b'), 20); setTimeout(() => console.log('a'), 5); \
         setImmediate(() => console.log('i')); console.log('sync'); \
         await new Promise((r) => setTimeout(r, 40)); console.log('end')",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(console(&report), vec!["sync", "i", "a", "b", "end"]);
}

#[test]
fn script_waits_for_its_timers() {
    let report = run_scripts(
        &[
            "setTimeout(() => pm.globals.set('late', '1'), 30)",
            "console.log(pm.globals.get('late'))",
        ],
        Limits::default(),
    );
    assert_eq!(report.errors, vec![]);
    assert!(report.changes.iter().any(|change| change.op
        == VariableOp::Set {
            key: "late".to_string(),
            value: "1".to_string()
        }));
    assert_eq!(console(&report), vec!["1"]);
}

#[test]
fn interval_and_clear() {
    let report = run_one(
        "let n = 0; const id = setInterval(() => { n++; console.log(n); if (n === 3) clearInterval(id) }, 5); \
         clearTimeout(setTimeout(() => console.log('no'), 5))",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(console(&report), vec!["1", "2", "3"]);
}

#[test]
fn waiting_is_not_execution_time() {
    let report = run_scripts(
        &["await new Promise((r) => setTimeout(r, 400)); console.log('ok')"],
        Limits {
            time: Duration::from_millis(200),
            ..Limits::default()
        },
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(console(&report), vec!["ok"]);
}

#[test]
fn endless_interval_hits_the_timer_wait_limit() {
    let started = Instant::now();
    let report = run_scripts(
        &["setInterval(() => {}, 10)"],
        Limits {
            timer_wait: Duration::from_millis(300),
            ..Limits::default()
        },
    );
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    assert_eq!(
        report.errors[0].message,
        "the script waited for timers longer than 300 ms"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn cancel_while_waiting_for_a_timer() {
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        canceller.cancel();
    });
    let job = Job::new(
        Phase::Test,
        vec!["await new Promise((r) => setTimeout(r, 30000))".to_string()],
    );
    let started = Instant::now();
    let report = run(job, &cancel);
    let elapsed = started.elapsed();
    thread.join().unwrap();
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Cancelled);
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn error_in_a_timer_is_the_error_of_the_script() {
    let report = run_one(
        "setTimeout(() => { throw new Error('in timer') }, 5);\nsetTimeout(() => console.log('never'), 50)",
    );
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Exception);
    assert_eq!(report.errors[0].message, "Error: in timer");
    assert_eq!(report.errors[0].line, Some(1));
    assert_eq!(console(&report), Vec::<&str>::new());
}

#[test]
fn unhandled_rejection_is_an_error_and_a_handled_one_is_not() {
    let lost = run_one("Promise.reject(new Error('lost'))");
    assert_eq!(lost.errors.len(), 1, "{:?}", lost.errors);
    assert_eq!(lost.errors[0].kind, ScriptErrorKind::Exception);
    assert_eq!(lost.errors[0].message, "Error: lost");

    // The handler comes later than the rejection: the engine tells of the promise twice.
    let caught = run_one(
        "const p = Promise.reject(new Error('caught')); await Promise.resolve(); p.catch(() => {})",
    );
    assert_eq!(caught.errors, vec![]);
}

#[test]
fn done_from_a_timer() {
    let report = run_one("pm.test('later', function (done) { setTimeout(done, 10) })");
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.tests.len(), 1);
    assert_eq!(report.tests[0].name, "later");
    assert!(report.tests[0].passed, "{:?}", report.tests);
}
