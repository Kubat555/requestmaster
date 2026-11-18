//! `pm.sendRequest`: what reaches the host, what comes back to the script, and the limits of both.
//!
//! The host of a test answers by the address and puts every request it got into a list it shares with the
//! test: the host itself moves into `run_with_host`.

use rqm_script::{
    abandoned_threads, run, run_with_host, ConsoleLevel, Host, Job, Limits, OutgoingBody,
    OutgoingRequest, Phase, Report, ResponseBody, ScriptErrorKind, ScriptResponse, Var, Variables,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

type Seen = Arc<Mutex<Vec<OutgoingRequest>>>;
type Answer = Box<dyn FnMut(&OutgoingRequest) -> Result<ScriptResponse, String> + Send>;

struct TestHost {
    seen: Seen,
    answer: Answer,
}

impl Host for TestHost {
    fn send_request(&mut self, request: OutgoingRequest) -> Result<ScriptResponse, String> {
        let answer = (self.answer)(&request);
        self.seen.lock().unwrap().push(request);
        answer
    }
}

fn response(code: u16, status: &str, body: &str) -> ScriptResponse {
    ScriptResponse {
        code,
        status: status.to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        time_ms: 3.0,
        size: body.len() as u64,
        body: ResponseBody::Bytes(body.as_bytes().to_vec()),
    }
}

/// `…/b` → 201 Created, anything else → 200 OK `{"v":1}`.
fn by_address(request: &OutgoingRequest) -> Result<ScriptResponse, String> {
    if request.url.ends_with("/b") {
        Ok(response(201, "Created", ""))
    } else {
        Ok(response(200, "OK", r#"{"v":1}"#))
    }
}

fn refused(_: &OutgoingRequest) -> Result<ScriptResponse, String> {
    Err("connect refused".to_string())
}

fn send_job(
    job: Job,
    answer: Answer,
    cancel: &CancellationToken,
) -> (Report, Vec<OutgoingRequest>) {
    let seen = Seen::default();
    let host = TestHost {
        seen: Arc::clone(&seen),
        answer,
    };
    let report = run_with_host(job, Box::new(host), cancel);
    let seen = seen.lock().unwrap().clone();
    (report, seen)
}

fn send_with(script: &str, answer: Answer) -> (Report, Vec<OutgoingRequest>) {
    let job = Job::new(Phase::Test, vec![script.to_string()]);
    send_job(job, answer, &CancellationToken::new())
}

fn send(script: &str) -> (Report, Vec<OutgoingRequest>) {
    send_with(script, Box::new(by_address))
}

fn console(report: &Report) -> Vec<&str> {
    report
        .console
        .iter()
        .filter(|line| line.level != ConsoleLevel::Network)
        .map(|line| line.text.as_str())
        .collect()
}

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn get(url: &str) -> OutgoingRequest {
    OutgoingRequest {
        method: "GET".to_string(),
        url: url.to_string(),
        headers: Vec::new(),
        body: OutgoingBody::None,
    }
}

#[test]
fn callback_and_promise_forms() {
    let (report, seen) = send(
        "const chained = pm.sendRequest('http://x/a', function (err, res) { \
             console.log(err === null, res.code, res.json().v) }); \
         const r = await pm.sendRequest({ url: 'http://x/b', method: 'POST', header: { 'X-K': 'v' }, \
             body: { mode: 'raw', raw: JSON.stringify({ a: 1 }) } }); \
         console.log(r.code, r.status, chained === pm)",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        seen,
        vec![
            get("http://x/a"),
            OutgoingRequest {
                method: "POST".to_string(),
                url: "http://x/b".to_string(),
                headers: pairs(&[("X-K", "v")]),
                body: OutgoingBody::Raw(r#"{"a":1}"#.to_string()),
            },
        ]
    );
    assert_eq!(console(&report), vec!["true 200 1", "201 Created true"]);
}

#[test]
fn header_and_body_forms() {
    let (report, seen) = send(
        "pm.sendRequest({ url: 'http://x/1', header: [{ key: 'A', value: '1' }, { key: 'B', value: '2', disabled: true }], \
             body: { mode: 'urlencoded', urlencoded: [{ key: 'a', value: '1' }, { key: 'b', value: '2' }, \
                                                      { key: 'c', value: '3', disabled: true }] } }); \
         pm.sendRequest({ url: { raw: 'http://x/2' }, header: 'A: 1\\nB: 2', \
             body: { mode: 'urlencoded', urlencoded: 'a=1&b=2' } }); \
         pm.sendRequest({ url: 'http://x/3', body: { mode: 'formdata', formdata: [{ key: 'f', value: 'v' }] } }); \
         pm.sendRequest({ url: 'http://x/4', body: { mode: 'raw', raw: { a: 1 } } })",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[0].headers, pairs(&[("A", "1")]));
    let urlencoded = OutgoingBody::UrlEncoded(pairs(&[("a", "1"), ("b", "2")]));
    assert_eq!(seen[0].body, urlencoded);
    assert_eq!(seen[1].url, "http://x/2");
    assert_eq!(seen[1].headers, pairs(&[("A", "1"), ("B", "2")]));
    assert_eq!(seen[1].body, urlencoded);
    assert_eq!(seen[2].body, OutgoingBody::FormData(pairs(&[("f", "v")])));
    assert_eq!(seen[3].body, OutgoingBody::Raw(r#"{"a":1}"#.to_string()));
}

#[test]
fn files_are_cut_out() {
    let (report, seen) = send(
        "pm.sendRequest({ url: 'http://x/1', body: { mode: 'file', file: { src: 'C:/secret.txt' } } }); \
         pm.sendRequest({ url: 'http://x/2', body: { mode: 'formdata', formdata: [ \
             { key: 'f', type: 'file', src: 'C:/x' }, { key: 't', value: '1' }] } })",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].body, OutgoingBody::None);
    assert_eq!(seen[1].body, OutgoingBody::FormData(pairs(&[("t", "1")])));
    let warnings: Vec<&str> = report
        .console
        .iter()
        .filter(|line| line.level == ConsoleLevel::Warn)
        .map(|line| line.text.as_str())
        .collect();
    assert_eq!(
        warnings,
        vec!["uploading files from scripts is not allowed"; 2]
    );
}

#[test]
fn variables_are_not_substituted() {
    let mut job = Job::new(
        Phase::Test,
        vec!["pm.variables.set('id', '7'); \
             pm.sendRequest({ url: 'http://h/u/{{id}}', header: { 'X-Id': '{{id}}' }, \
                 body: { mode: 'raw', raw: '{\"t\":\"{{id}}\"}' } }); \
             pm.sendRequest({ url: 'http://h/u', \
                 body: { mode: 'raw', raw: pm.variables.replaceIn('{\"t\":\"{{id}}\"}') } })"
            .to_string()],
    );
    job.variables = Variables {
        globals: vec![Var::new("host", "http://h")],
        ..Variables::default()
    };
    let (report, seen) = send_job(job, Box::new(by_address), &CancellationToken::new());
    assert_eq!(report.errors, vec![]);
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].url, "http://h/u/{{id}}");
    assert_eq!(seen[0].headers, pairs(&[("X-Id", "{{id}}")]));
    assert_eq!(
        seen[0].body,
        OutgoingBody::Raw(r#"{"t":"{{id}}"}"#.to_string())
    );
    // A script that wants the substitution asks for it.
    assert_eq!(seen[1].body, OutgoingBody::Raw(r#"{"t":"7"}"#.to_string()));
}

#[test]
fn too_big_request_is_refused() {
    let (report, seen) = send(
        "pm.sendRequest({ url: 'http://x/a', body: { mode: 'raw', raw: 'x'.repeat(11 * 1024 * 1024) } }, \
             (err) => console.log(err.message))",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        console(&report),
        vec!["pm.sendRequest: the request is larger than 10 MB"]
    );
    assert_eq!(seen, vec![]);

    let (report, seen) = send(
        "pm.sendRequest({ url: 'http://x/a', body: { mode: 'raw', raw: 'x'.repeat(9 * 1024 * 1024) } })",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].body, OutgoingBody::Raw("x".repeat(9 * 1024 * 1024)));
}

#[test]
fn queued_requests_are_capped() {
    let (report, seen) = send(
        "const body = 'x'.repeat(8 * 1024 * 1024); \
         for (let i = 0; i < 5; i++) pm.sendRequest({ url: 'http://x/a', body: { mode: 'raw', raw: body } })",
    );
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Memory);
    assert_eq!(
        report.errors[0].message,
        "the script queued more than 32 MiB of requests"
    );
    assert!(seen.len() <= 4);
}

#[test]
fn abandoned_script_does_not_reach_the_host() {
    let mut job = Job::new(
        Phase::Test,
        vec!["(10n ** 300000n).toString(); pm.sendRequest('http://x/late')".to_string()],
    );
    job.limits = Limits {
        time: Duration::from_millis(100),
        grace: Duration::from_millis(100),
        ..Limits::default()
    };
    let seen = Seen::default();
    let host = TestHost {
        seen: Arc::clone(&seen),
        answer: Box::new(by_address),
    };
    let started = Instant::now();
    let report = run_with_host(job, Box::new(host), &CancellationToken::new());
    let elapsed = started.elapsed();
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    assert!(elapsed < Duration::from_millis(1500), "{elapsed:?}");

    // The thread the watchdog gave up on runs on to the call of `pm.sendRequest` and ends there.
    let waiting = Instant::now();
    while abandoned_threads() != 0 {
        assert!(
            waiting.elapsed() < Duration::from_secs(30),
            "the abandoned thread has not ended"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(*seen.lock().unwrap(), vec![]);
}

#[test]
fn network_error_reaches_the_callback_and_rejects_the_promise() {
    let (report, _) = send_with(
        "pm.sendRequest('http://x/a', (err, res) => console.log(err.message, res))",
        Box::new(refused),
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(console(&report), vec!["connect refused undefined"]);

    let (report, _) = send_with("await pm.sendRequest('http://x/a')", Box::new(refused));
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Exception);
    assert_eq!(report.errors[0].message, "Error: connect refused");
}

#[test]
fn requests_run_one_at_a_time_and_hold_the_script() {
    let (report, seen) = send("pm.sendRequest('http://x/1'); pm.sendRequest('http://x/2')");
    assert_eq!(report.errors, vec![]);
    assert_eq!(seen, vec![get("http://x/1"), get("http://x/2")]);
}

#[test]
fn network_lines_in_the_console() {
    let network = |report: &Report| -> Vec<String> {
        report
            .console
            .iter()
            .filter(|line| line.level == ConsoleLevel::Network)
            .map(|line| {
                assert_eq!(line.script, Some(0));
                line.text.clone()
            })
            .collect()
    };

    let (report, _) = send("pm.sendRequest('http://x/a')");
    let lines = network(&report);
    assert_eq!(lines.len(), 1, "{:?}", report.console);
    // `GET http://x/a → 200 OK (N ms)`
    let millis = lines[0]
        .strip_prefix("GET http://x/a → 200 OK (")
        .and_then(|rest| rest.strip_suffix(" ms)"))
        .unwrap_or_else(|| panic!("{}", lines[0]));
    assert!(
        !millis.is_empty() && millis.bytes().all(|byte| byte.is_ascii_digit()),
        "{millis}"
    );

    let (report, _) = send_with("pm.sendRequest('http://x/a', () => {})", Box::new(refused));
    assert_eq!(
        network(&report),
        vec!["GET http://x/a → error: connect refused"]
    );
}

#[test]
fn too_many_requests() {
    let (report, seen) = send("for (let i = 0; i < 51; i++) pm.sendRequest('http://x/' + i)");
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Exception);
    assert_eq!(
        report.errors[0].message,
        "Error: pm.sendRequest: more than 50 requests in one script"
    );
    assert!(seen.len() <= 50);
}

#[test]
fn answer_is_a_response_object() {
    let (report, _) = send(
        "pm.sendRequest('http://x/a', (e, res) => { pm.test('inner', () => { \
             pm.expect(res).to.have.status(200); res.to.be.ok; \
             pm.expect(res.headers.count()).to.be.a('number'); \
             pm.expect(res.text()).to.equal('{\"v\":1}') }) })",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.tests.len(), 1);
    assert!(report.tests[0].passed, "{:?}", report.tests);
}

#[test]
fn waiting_for_the_host_is_not_execution_time() {
    let mut job = Job::new(
        Phase::Test,
        vec!["await pm.sendRequest('http://x/a'); console.log('ok')".to_string()],
    );
    job.limits.time = Duration::from_millis(200);
    let slow: Answer = Box::new(|request| {
        std::thread::sleep(Duration::from_millis(400));
        by_address(request)
    });
    let (report, _) = send_job(job, slow, &CancellationToken::new());
    assert_eq!(report.errors, vec![]);
    assert_eq!(console(&report), vec!["ok"]);
}

#[test]
fn cancel_while_the_host_answers() {
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let canceller = cancel.clone();
    // The host answers only when the send is cancelled, as the host of the application does.
    let waits: Answer = Box::new(move |_| {
        while !token.is_cancelled() {
            std::thread::sleep(Duration::from_millis(5));
        }
        Err("cancelled".to_string())
    });
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        canceller.cancel();
    });
    let job = Job::new(
        Phase::Test,
        vec!["pm.sendRequest('http://x/a', () => console.log('never'))".to_string()],
    );
    let started = Instant::now();
    let (report, seen) = send_job(job, waits, &cancel);
    let elapsed = started.elapsed();
    thread.join().unwrap();
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Cancelled);
    assert_eq!(seen.len(), 1);
    assert_eq!(console(&report), Vec::<&str>::new());
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn run_without_a_host_refuses() {
    let job = Job::new(
        Phase::Test,
        vec!["await pm.sendRequest('http://x')".to_string()],
    );
    let report = run(job, &CancellationToken::new());
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Exception);
    assert_eq!(
        report.errors[0].message,
        "Error: pm.sendRequest is not available"
    );
}
