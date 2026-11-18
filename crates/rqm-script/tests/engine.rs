//! The engine of `rqm-script`: order and isolation of scripts, where an error points, the limits of time,
//! memory and stack, cancel.

use rqm_script::{run, Job, Limits, Phase, Report, ScriptError, ScriptErrorKind};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Runs the scripts and asserts that the engine itself did not fail.
fn run_scripts(phase: Phase, scripts: &[&str], limits: Limits) -> Report {
    let mut job = Job::new(
        phase,
        scripts.iter().map(|script| script.to_string()).collect(),
    );
    job.limits = limits;
    let report = run(job, &CancellationToken::new());
    assert!(
        report
            .errors
            .iter()
            .all(|error| error.kind != ScriptErrorKind::Internal),
        "engine failure: {:?}",
        report.errors
    );
    report
}

fn one_error(script: &str, limits: Limits) -> ScriptError {
    let mut report = run_scripts(Phase::Test, &[script], limits);
    assert_eq!(report.errors.len(), 1, "{script}: {:?}", report.errors);
    report.errors.remove(0)
}

fn time_limit(millis: u64) -> Limits {
    Limits {
        time: Duration::from_millis(millis),
        ..Limits::default()
    }
}

#[test]
fn scripts_run_in_order_and_do_not_share_globals() {
    let report = run_scripts(
        Phase::Test,
        &[
            "leak = 1; var v = 2; function f() {}; const dup = 1",
            "const dup = 2; if (typeof leak !== 'undefined' || typeof v !== 'undefined' || typeof f !== 'undefined') \
             throw new Error('leaked')",
        ],
        Limits::default(),
    );
    assert_eq!(report.errors, vec![]);
}

#[test]
fn sloppy_mode_top_level_await_and_return_work() {
    let report = run_scripts(
        Phase::Test,
        &[
            "with ({ a: 1 }) { if (a !== 1) throw new Error('with') }\nconst v = await Promise.resolve(2);\n\
             if (v !== 2) throw new Error('await');\nreturn;\nthrow new Error('after return')",
            "// only a comment",
        ],
        Limits::default(),
    );
    assert_eq!(report.errors, vec![]);
}

#[test]
fn error_names_the_line_and_column_of_the_editor() {
    let error = one_error("\n\nundefinedFn(1)", Limits::default());
    assert_eq!(error.kind, ScriptErrorKind::Exception);
    assert_eq!(error.message, "ReferenceError: undefinedFn is not defined");
    assert_eq!((error.line, error.column), (Some(3), Some(1)));

    // The first line is shifted by the wrapper the script runs in.
    let error = one_error("  boom()", Limits::default());
    assert_eq!((error.line, error.column), (Some(1), Some(3)));

    // A syntax error at the very end lands on the line of the wrapper.
    let error = one_error("x = 1;\nif (", Limits::default());
    assert!(
        error.message.starts_with("SyntaxError"),
        "{}",
        error.message
    );
    assert_eq!((error.line, error.column), (Some(2), None));

    let error = one_error("throw 'plain'", Limits::default());
    assert_eq!(error.message, "plain");
    assert_eq!(error.line, None);

    let error = one_error("await new Promise(() => {})", Limits::default());
    assert_eq!(error.kind, ScriptErrorKind::Exception);
    assert!(
        error.message.contains("never finishes"),
        "{}",
        error.message
    );
}

#[test]
fn pre_request_stops_at_the_first_error_and_tests_go_on() {
    let scripts = ["throw new Error('a')", "throw new Error('b')"];
    let report = run_scripts(Phase::PreRequest, &scripts, Limits::default());
    assert_eq!(
        report
            .errors
            .iter()
            .map(|error| error.script)
            .collect::<Vec<_>>(),
        vec![0]
    );
    let report = run_scripts(Phase::Test, &scripts, Limits::default());
    assert_eq!(
        report
            .errors
            .iter()
            .map(|error| error.script)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn infinite_loop_is_a_timeout() {
    let started = Instant::now();
    let report = run_scripts(
        Phase::Test,
        &[
            "try { while (true) {} } catch (e) {} finally {}",
            "throw new Error('next')",
        ],
        time_limit(200),
    );
    assert_eq!(report.errors.len(), 2, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    assert_eq!(
        report.errors[0].message,
        "the script ran longer than 200 ms"
    );
    assert_eq!(report.errors[1].message, "Error: next");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn catastrophic_regex_is_a_timeout() {
    let error = one_error("/(a+)+$/.test('a'.repeat(40) + '!')", time_limit(200));
    assert_eq!(error.kind, ScriptErrorKind::Timeout);
}

#[test]
fn time_limit_is_per_script() {
    let script = "const t = Date.now(); while (Date.now() - t < 120) {}";
    let report = run_scripts(Phase::Test, &[script, script, script], time_limit(300));
    assert_eq!(report.errors, vec![]);
}

#[test]
fn memory_limit_is_a_memory_error() {
    let limits = Limits {
        memory_bytes: 16 << 20,
        ..Limits::default()
    };
    let error = one_error(
        "const a = []; while (true) a.push('x'.repeat(1 << 20) + a.length)",
        limits,
    );
    assert_eq!(error.kind, ScriptErrorKind::Memory);
    assert_eq!(error.message, "the script used more than 16 MiB of memory");
}

#[test]
fn infinite_recursion_is_an_error_of_the_script() {
    let still_works = || {
        assert_eq!(
            run_scripts(Phase::Test, &["1"], Limits::default()).errors,
            vec![]
        )
    };
    // 64 MiB and 16 MiB are clamped to `MAX_STACK_BYTES`: unclamped, the second one kills the test process.
    for stack_bytes in [Limits::default().stack_bytes, 64 << 20, 16 << 20] {
        let error = one_error(
            "function f() { f() } f()",
            Limits {
                stack_bytes,
                ..Limits::default()
            },
        );
        assert_eq!(
            error.kind,
            ScriptErrorKind::Exception,
            "stack {stack_bytes}"
        );
        assert_eq!(
            error.message, "RangeError: Maximum call stack size exceeded",
            "stack {stack_bytes}"
        );
        still_works();
    }
    let error = one_error("JSON.parse('['.repeat(100000))", Limits::default());
    assert_eq!(error.kind, ScriptErrorKind::Exception);
    still_works();
}

#[test]
fn error_text_is_read_whatever_it_holds() {
    // A lone surrogate does not become a Rust string as it is.
    let error = one_error("throw new Error('a\\ud800b')", Limits::default());
    assert_eq!(error.kind, ScriptErrorKind::Exception);
    assert_eq!(error.message, "Error: a\u{FFFD}b");

    // The text is cut by characters, never inside one.
    let error = one_error("throw new Error('😀'.repeat(20000))", Limits::default());
    assert_eq!(error.kind, ScriptErrorKind::Exception);
    assert_eq!(error.message.chars().count(), 10_000);
    assert!(error.message.starts_with("Error: 😀"));
    assert_eq!(error.message.chars().last(), Some('😀'));

    let error = one_error("throw '\\ud83d'", Limits::default());
    assert_eq!(error.kind, ScriptErrorKind::Exception);
    assert_eq!(error.message, "\u{FFFD}");
}

#[test]
fn cancel_stops_a_running_script() {
    for phase in [Phase::PreRequest, Phase::Test] {
        let mut job = Job::new(
            phase,
            vec![
                "while (true) {}".to_string(),
                "throw new Error('next')".to_string(),
            ],
        );
        job.limits = Limits {
            time: Duration::from_secs(60),
            ..Limits::default()
        };
        let cancel = CancellationToken::new();
        let canceller = {
            let cancel = cancel.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                cancel.cancel();
            })
        };
        let started = Instant::now();
        let report = run(job, &cancel);
        let elapsed = started.elapsed();
        canceller.join().expect("canceller thread");
        assert_eq!(report.errors.len(), 1, "{phase:?}: {:?}", report.errors);
        assert_eq!(report.errors[0].kind, ScriptErrorKind::Cancelled);
        assert_eq!(report.errors[0].script, 0);
        assert!(elapsed < Duration::from_secs(2), "{phase:?}: {elapsed:?}");
    }
}

#[test]
fn a_cancelled_token_runs_nothing() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let job = Job::new(
        Phase::Test,
        vec!["throw new Error('ran')".to_string(), "1".to_string()],
    );
    let report = run(job, &cancel);
    assert_eq!(
        report.errors,
        vec![ScriptError {
            script: 0,
            kind: ScriptErrorKind::Cancelled,
            message: "cancelled".to_string(),
            line: None,
            column: None
        }]
    );
}

#[test]
fn no_ambient_io_globals() {
    let script = "for (const name of ['fetch', 'XMLHttpRequest', 'WebSocket', 'process', 'require', 'module', 'std', \
                  'os']) { if (typeof globalThis[name] !== 'undefined') throw new Error(name + ' is defined') }";
    assert_eq!(
        run_scripts(Phase::Test, &[script], Limits::default()).errors,
        vec![]
    );
}
