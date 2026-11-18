//! `console.*` of a script: what a line looks like, the limits of the console, text that is not ASCII.

use rqm_script::{
    run, ConsoleLevel, Job, Limits, Phase, Report, ScriptErrorKind, CONSOLE_LINE_CHARS,
    CONSOLE_MAX_BYTES, CONSOLE_MAX_LINES,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn run_scripts(scripts: &[&str]) -> Report {
    let scripts = scripts.iter().map(|script| script.to_string()).collect();
    run(Job::new(Phase::Test, scripts), &CancellationToken::new())
}

/// The lines of a run that has to end without an error.
fn lines(script: &str) -> Vec<String> {
    let report = run_scripts(&[script]);
    assert_eq!(report.errors, vec![], "{script}");
    report.console.into_iter().map(|line| line.text).collect()
}

#[test]
fn console_levels_and_formatting() {
    let report = run_scripts(&[
        "console.log('a', 1, true, null, undefined, { b: [1, { c: 'я' }] }); \
         console.info(new TypeError('t')); console.warn(function named() {}); console.error('e'); \
         console.debug('d'); const o = {}; o.self = o; console.log(o); console.clear();",
    ]);
    assert_eq!(report.errors, vec![]);
    let lines: Vec<(ConsoleLevel, &str)> = report
        .console
        .iter()
        .map(|line| (line.level, line.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        vec![
            (
                ConsoleLevel::Log,
                r#"a 1 true null undefined {"b":[1,{"c":"я"}]}"#
            ),
            (ConsoleLevel::Info, "TypeError: t"),
            (ConsoleLevel::Warn, "[Function: named]"),
            (ConsoleLevel::Error, "e"),
            (ConsoleLevel::Debug, "d"),
            (ConsoleLevel::Log, r#"{"self":"[Circular]"}"#),
        ]
    );
    assert!(report.console.iter().all(|line| line.script == Some(0)));
    assert!(report
        .console
        .windows(2)
        .all(|pair| pair[0].at_ms <= pair[1].at_ms));
    assert!(!report.console_truncated);
}

#[test]
fn console_is_capped() {
    let many = run_scripts(&["for (let i = 0; i < 5000; i++) console.log(i)"]);
    assert_eq!(many.errors, vec![]);
    assert_eq!(many.console.len(), CONSOLE_MAX_LINES);
    assert!(many.console_truncated);

    let long = lines("console.log('x'.repeat(20000))");
    assert_eq!(long[0].chars().count(), CONSOLE_LINE_CHARS + 1);
    assert!(long[0].ends_with('…'));

    // The limit of bytes is of the job, not of a script.
    let script = "for (let i = 0; i < 120; i++) console.log('x'.repeat(5000))";
    let big = run_scripts(&[script, script, script]);
    assert_eq!(big.errors, vec![]);
    let bytes: usize = big.console.iter().map(|line| line.text.len()).sum();
    assert!(bytes <= CONSOLE_MAX_BYTES, "{bytes}");
    assert!(big.console.len() > 120);
    assert!(big.console_truncated);
}

#[test]
fn endless_logging_ends_by_the_time_limit() {
    let mut job = Job::new(Phase::Test, vec!["while (true) console.log(1)".to_string()]);
    job.limits = Limits {
        time: Duration::from_millis(200),
        ..Limits::default()
    };
    let report = run(job, &CancellationToken::new());
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    assert_eq!(report.console.len(), CONSOLE_MAX_LINES);
    assert!(report.console_truncated);
}

#[test]
fn non_ascii_and_lone_surrogates_in_the_console() {
    // An emoji is one character of the limit, and the cut never splits it.
    let whole = lines("console.log('😀'.repeat(6000))");
    assert_eq!(whole, vec!["😀".repeat(6000)]);

    let cut = lines("console.log('a' + '😀'.repeat(20000))");
    let characters: Vec<char> = cut[0].chars().collect();
    assert_eq!(characters.len(), CONSOLE_LINE_CHARS + 1);
    assert_eq!(characters[CONSOLE_LINE_CHARS], '…');
    assert_eq!(characters[CONSOLE_LINE_CHARS - 1], '😀');

    // A lone surrogate does not turn into a Rust string: it comes as U+FFFD.
    assert_eq!(
        lines("console.log(String.fromCharCode(0xd800)); console.log('x😀'.slice(0, 2))"),
        vec!["\u{FFFD}", "x\u{FFFD}"]
    );
}
