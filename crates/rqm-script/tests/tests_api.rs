//! `pm.test`: what a test records, and the limits of what the tests of a job may hold.

use rqm_script::{
    run, Job, Phase, Report, ScriptErrorKind, TestResult, MAX_TESTS, MAX_TEST_TEXT_BYTES,
    TEST_ERROR_CHARS, TEST_NAME_CHARS,
};
use tokio_util::sync::CancellationToken;

fn run_scripts(scripts: &[&str]) -> Report {
    let scripts = scripts.iter().map(|script| script.to_string()).collect();
    run(Job::new(Phase::Test, scripts), &CancellationToken::new())
}

fn test(name: &str, passed: bool, skipped: bool, error: Option<&str>) -> TestResult {
    TestResult {
        script: 0,
        name: name.to_string(),
        passed,
        skipped,
        error: error.map(str::to_string),
    }
}

#[test]
fn tests_pass_fail_skip_and_do_not_stop_the_script() {
    let report = run_scripts(&["pm.test('ok', function () {}); \
         pm.test('bad', function () { throw new Error('boom') }); \
         pm.test('str', () => { throw 'plain' }); \
         pm.test('no function'); \
         pm.test.skip('later', () => { throw new Error('never') }); \
         console.log(pm.test.index(), pm.test('chain', () => {}) === pm)"]);
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        report.tests,
        vec![
            test("ok", true, false, None),
            test("bad", false, false, Some("Error: boom")),
            test("str", false, false, Some("plain")),
            test("no function", true, false, None),
            test("later", true, true, None),
            test("chain", true, false, None),
        ]
    );
    assert_eq!(report.console[0].text, "5 true");
}

#[test]
fn tests_name_their_script() {
    let script = "pm.test('same', () => {})";
    let report = run_scripts(&[script, script]);
    assert_eq!(report.errors, vec![]);
    let scripts: Vec<usize> = report.tests.iter().map(|test| test.script).collect();
    assert_eq!(scripts, vec![0, 1]);
}

#[test]
fn too_many_tests_is_an_error_of_the_script() {
    let report = run_scripts(&["for (let i = 0; i < 1001; i++) pm.test('t' + i, () => {})"]);
    assert_eq!(report.tests.len(), MAX_TESTS);
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Exception);
    assert_eq!(
        report.errors[0].message,
        "Error: too many tests: more than 1000"
    );
}

#[test]
fn names_and_errors_are_capped() {
    let report = run_scripts(&[
        "pm.test('n'.repeat(5000), () => { throw new Error('m'.repeat(20000)) }); \
         pm.test('я'.repeat(5000), () => {}); \
         pm.test('😀'.repeat(5000), () => { throw new Error('😀'.repeat(20000)) });",
    ]);
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.tests.len(), 3);
    for test in &report.tests {
        assert_eq!(test.name.chars().count(), TEST_NAME_CHARS);
    }
    assert_eq!(report.tests[1].name, "я".repeat(TEST_NAME_CHARS));
    // The cut never splits a character.
    assert_eq!(report.tests[2].name, "😀".repeat(TEST_NAME_CHARS));
    for test in [&report.tests[0], &report.tests[2]] {
        let error = test.error.as_deref().unwrap();
        assert_eq!(error.chars().count(), TEST_ERROR_CHARS);
        assert!(error.starts_with("Error: "));
    }
}

#[test]
fn lone_surrogates_in_tests() {
    let report = run_scripts(&[
        r"pm.test('\ud83d', () => {}); pm.test('t', () => { throw new Error('\ud800') })",
    ]);
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        report.tests,
        vec![
            test("\u{FFFD}", true, false, None),
            test("t", false, false, Some("Error: \u{FFFD}")),
        ]
    );
}

#[test]
fn test_text_is_capped_as_a_whole() {
    let report = run_scripts(&[
        "for (let i = 0; i < 1000; i++) pm.test('t' + i, () => { throw new Error('я'.repeat(10000)) })",
    ]);
    let bytes: usize = report
        .tests
        .iter()
        .map(|test| test.name.len() + test.error.as_ref().map_or(0, String::len))
        .sum();
    assert!(bytes <= MAX_TEST_TEXT_BYTES, "{bytes}");
    assert!(!report.tests.is_empty());
    assert!(report.tests.len() < 1000);
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Exception);
    assert_eq!(
        report.errors[0].message,
        "Error: too many tests: their results are larger than 256 KB"
    );
}
