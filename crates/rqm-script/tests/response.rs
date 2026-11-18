//! `pm.response` and the old names of the response: the fields, the list of headers, and a body a script
//! cannot take — too large, unreadable, not UTF-8, empty.

use rqm_script::{
    run, Job, Phase, Report, ResponseBody, ScriptErrorKind, ScriptResponse, StoredScope,
    VariableChange, VariableOp, BODY_LIMIT,
};
use std::fs;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

/// 200 `OK`; headers `Content-Type: application/json`, `X-A: 1`, `x-a: 2`; 12.5 ms.
fn response(body: ResponseBody) -> ScriptResponse {
    let size = match &body {
        ResponseBody::Bytes(bytes) => bytes.len() as u64,
        ResponseBody::File(_) => 0,
    };
    ScriptResponse {
        code: 200,
        status: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-A".to_string(), "1".to_string()),
            ("x-a".to_string(), "2".to_string()),
        ],
        time_ms: 12.5,
        size,
        body,
    }
}

fn run_with(body: ResponseBody, script: &str) -> Report {
    let mut job = Job::new(Phase::Test, vec![script.to_string()]);
    job.response = Some(response(body));
    run(job, &CancellationToken::new())
}

fn run_bytes(body: &[u8], script: &str) -> Report {
    run_with(ResponseBody::Bytes(body.to_vec()), script)
}

/// The console of a run that has to end without an error.
fn lines(body: &[u8], script: &str) -> Vec<String> {
    let report = run_bytes(body, script);
    assert_eq!(report.errors, vec![], "{script}");
    report.console.into_iter().map(|line| line.text).collect()
}

/// A file of `length` bytes `a` in the folder cargo gives the tests.
fn file_of(name: &str, length: u64) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::write(&path, vec![b'a'; usize::try_from(length).unwrap()]).unwrap();
    path
}

#[test]
fn response_fields() {
    let body = br#"{"token":"abc","n":[1,2]}"#;
    assert_eq!(body.len(), 25);
    assert_eq!(
        lines(
            body,
            r#"console.log(pm.response.code, pm.response.status, pm.response.reason(),
                pm.response.responseTime, pm.response.responseSize, pm.response.headers.get('content-type'),
                pm.response.headers.get('X-A'), pm.response.headers.has('nope'), pm.response.headers.count(),
                pm.response.json().n[1], pm.response.text() === '{"token":"abc","n":[1,2]}')"#
        ),
        vec!["200 OK OK 12.5 25 application/json 2 false 3 2 true"]
    );
}

#[test]
fn headers_to_object_lowercases_and_groups() {
    assert_eq!(
        lines(
            b"{}",
            "console.log(JSON.stringify(pm.response.headers.toObject())); \
             console.log(JSON.stringify(pm.response.headers.all()[1]))"
        ),
        vec![
            r#"{"content-type":"application/json","x-a":["1","2"]}"#,
            r#"{"key":"X-A","value":"1"}"#,
        ]
    );
}

#[test]
fn owner_scenario_parses_the_body_and_sets_a_collection_variable() {
    let report = run_bytes(
        br#"{"access_token":"T-1"}"#,
        "var data = JSON.parse(responseBody);\npm.collectionVariables.set(\"token\", data.access_token);",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        report.changes,
        vec![VariableChange {
            scope: StoredScope::Collection,
            op: VariableOp::Set {
                key: "token".to_string(),
                value: "T-1".to_string(),
            },
        }]
    );
}

#[test]
fn legacy_response_names() {
    assert_eq!(
        lines(
            b"{}",
            "console.log(responseCode.code, responseCode.name, responseHeaders['Content-Type'], \
             responseHeaders['x-a'], responseTime)"
        ),
        vec!["200 OK application/json 2 12.5"]
    );
}

#[test]
fn bom_bad_utf8_and_empty_bodies() {
    let mut with_bom = vec![0xEF, 0xBB, 0xBF];
    with_bom.extend_from_slice(br#"{"a":1}"#);
    assert_eq!(
        lines(&with_bom, "console.log(pm.response.json().a === 1)"),
        vec!["true"]
    );
    assert_eq!(
        lines(&[0xFF, 0x41], r"console.log(pm.response.text() === '�A')"),
        vec!["true"]
    );

    let empty = run_bytes(
        b"",
        "console.log(pm.response.text() === ''); pm.test('j', () => pm.response.json())",
    );
    assert_eq!(empty.errors, vec![]);
    assert_eq!(empty.console[0].text, "true");
    assert_eq!(empty.tests.len(), 1);
    assert!(!empty.tests[0].passed);
    let error = empty.tests[0].error.as_deref().unwrap();
    assert!(error.starts_with("JSONError"), "{error}");
}

#[test]
fn body_file_is_read_only_when_asked() {
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rqm-no-such-body.bin");
    let untouched = run_with(
        ResponseBody::File(missing.clone()),
        "console.log(pm.response.code)",
    );
    assert_eq!(untouched.errors, vec![]);
    assert_eq!(untouched.console[0].text, "200");

    let asked = run_with(ResponseBody::File(missing), "pm.response.text()");
    assert_eq!(asked.errors.len(), 1, "{:?}", asked.errors);
    assert_eq!(asked.errors[0].kind, ScriptErrorKind::Exception);
    let message = &asked.errors[0].message;
    assert!(
        message.starts_with("Error: response body cannot be read"),
        "{message}"
    );
}

#[test]
fn body_over_the_limit_is_refused() {
    let over = file_of("rqm-body-over-the-limit.bin", BODY_LIMIT + 1);
    let report = run_with(
        ResponseBody::File(over.clone()),
        "pm.test('t', () => { pm.response.json() }); console.log(typeof responseBody)",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.tests.len(), 1);
    assert!(!report.tests[0].passed);
    assert_eq!(
        report.tests[0].error.as_deref(),
        Some("Error: response body is larger than 10 MB and is not available to scripts")
    );
    assert_eq!(report.console[0].text, "undefined");
    // The file is the response the user sees: a script never deletes it.
    assert!(over.exists());
    fs::remove_file(over).unwrap();

    let exact = file_of("rqm-body-at-the-limit.bin", BODY_LIMIT);
    let report = run_with(
        ResponseBody::File(exact.clone()),
        "console.log(pm.response.text().length === 10485760)",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.console[0].text, "true");
    fs::remove_file(exact).unwrap();
}

#[test]
fn no_response_before_the_request() {
    let script = "console.log(typeof pm.response, typeof responseBody, typeof responseCode, \
                  typeof responseHeaders, typeof responseTime)";
    let report = run(
        Job::new(Phase::PreRequest, vec![script.to_string()]),
        &CancellationToken::new(),
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        report.console[0].text,
        "undefined undefined undefined undefined undefined"
    );
}

#[test]
fn legacy_names_can_be_assigned_without_var() {
    assert_eq!(
        lines(
            b"{}",
            "responseBody = 'x'; responseTime = 1; console.log(responseBody, responseTime); \
             var responseCode = 2; console.log(responseCode)"
        ),
        vec!["x 1", "2"]
    );
}
