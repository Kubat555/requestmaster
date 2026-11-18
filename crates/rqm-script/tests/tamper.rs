//! A script that replaces built-in objects before it calls the API: the limits hold (Rust checks them, not the
//! prelude), and no primitive of the host comes into its hands.
//!
//! Extended by the later tasks, each time with the API of that task.

use rqm_script::{
    run, run_with_host, Host, Job, OutgoingRequest, Phase, Report, ResponseBody, ScriptErrorKind,
    ScriptResponse, Variables, CONSOLE_LINE_CHARS, CONSOLE_MAX_LINES, MAX_CHANGES,
    MAX_SEND_REQUESTS, MAX_TESTS, TEST_NAME_CHARS,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

fn response() -> ScriptResponse {
    ScriptResponse {
        code: 200,
        status: "OK".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        time_ms: 1.0,
        size: 7,
        body: ResponseBody::Bytes(br#"{"a":1}"#.to_vec()),
    }
}

fn job(script: &str) -> Job {
    let mut job = Job::new(Phase::Test, vec![script.to_string()]);
    job.variables = Variables {
        environment_name: Some("Dev".to_string()),
        ..Variables::default()
    };
    job.response = Some(response());
    job
}

/// Answers every request and counts them.
struct CountingHost(Arc<AtomicUsize>);

impl Host for CountingHost {
    fn send_request(&mut self, _request: OutgoingRequest) -> Result<ScriptResponse, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(response())
    }
}

/// The report, and how many requests of the script reached the host.
fn run_counting(script: &str) -> (Report, usize) {
    let requests = Arc::new(AtomicUsize::new(0));
    let host = CountingHost(Arc::clone(&requests));
    let report = run_with_host(job(script), Box::new(host), &CancellationToken::new());
    (report, requests.load(Ordering::SeqCst))
}

#[test]
fn tampered_builtins_do_not_lift_limits() {
    // Every replacement answers "nothing was cut, everything is allowed".
    let script = "
        const whole = function () { return String(this) };
        String.prototype.slice = whole;
        String.prototype.substring = whole;
        String.prototype.toWellFormed = whole;
        Array.prototype.push = function () { return 0 };
        Array.prototype.map = function () { return this };
        Array.prototype.forEach = function () {};
        Array.prototype.join = function () { return '' };
        JSON.stringify = function () { return '1' };
        Function.prototype.call = function () { return true };
        Function.prototype.apply = function () { return true };
        Function.prototype.bind = function () { return this };
        Reflect.apply = function () { return true };
        Promise.prototype.then = function () { return this };
        for (let i = 0; i < 5000; i++) console.log('x'.repeat(20000));
        // chai is loaded into a realm that is already broken: whatever comes of it, the limits hold.
        try { pm.expect(1).to.equal(1) } catch (error) {}
        try { pm.response.to.have.status(200) } catch (error) {}
        try { pm.test('later', function (done) {}) } catch (error) {}
        for (let i = 0; i < 1001; i++) {
            try { pm.test('n'.repeat(5000), function () {}) } catch (error) {}
        }
        // One call more than a script may make, each on its own: the count is kept by Rust.
        for (let i = 0; i < 51; i++) {
            try { pm.sendRequest('http://x/' + i, function () {}) } catch (error) {}
        }
        try { clearTimeout(setTimeout(function () {}, 0)); clearInterval(-1); clearImmediate(1e9) } catch (error) {}
        // The requests go first, then the timer: the limits of the host hold in a callback too.
        setTimeout(function () { for (;;) pm.environment.set('i', 1) }, 0)";
    let (report, requests) = run_counting(script);

    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Memory);
    assert_eq!(requests, MAX_SEND_REQUESTS);
    assert!(!report.console.is_empty());
    assert!(report.console.len() <= CONSOLE_MAX_LINES);
    assert!(report
        .console
        .iter()
        .all(|line| line.text.chars().count() <= CONSOLE_LINE_CHARS + 1));
    assert_eq!(report.changes.len(), MAX_CHANGES);
    assert!(!report.tests.is_empty());
    assert!(report.tests.len() <= MAX_TESTS);
    assert!(report
        .tests
        .iter()
        .all(|test| test.name.chars().count() <= TEST_NAME_CHARS));
}

#[test]
fn host_primitives_do_not_leak() {
    // The replaced functions note every `this` and every function among their arguments, then do their job.
    let script = "
        const seen = [];
        const note = (value) => { if (typeof value === 'function') seen[seen.length] = value; };
        const apply = Reflect.apply;
        const spy = (original) => function (...values) {
            note(this);
            for (let i = 0; i < values.length; i++) note(values[i]);
            return apply(original, this, values);
        };
        Reflect.apply = function (target, self, values) {
            note(target);
            note(self);
            for (let i = 0; i < values.length; i++) note(values[i]);
            return apply(target, self, values);
        };
        for (const name of ['call', 'apply', 'bind']) {
            Function.prototype[name] = spy(Function.prototype[name]);
        }
        for (const name of ['push', 'pop', 'map', 'forEach', 'join', 'filter', 'slice', 'concat', 'indexOf',
                            'includes', 'reduce', 'some', 'every', 'find']) {
            Array.prototype[name] = spy(Array.prototype[name]);
        }
        // The spies do see what goes through them.
        (function probe() {}).call(null);

        console.log('a', { b: [1] }, 1n, () => 1);
        console.info(1); console.warn(1); console.error(new Error('e')); console.debug(1); console.clear();
        for (const scope of [pm.globals, pm.collectionVariables, pm.environment, pm.variables]) {
            scope.set('k', { a: 1 });
            scope.set('s', 'text');
            scope.get('k');
            scope.get('nope');
            scope.has('k');
            scope.toObject();
            scope.replaceIn('{{s}}');
            scope.replaceIn({ a: ['{{s}}'] });
            scope.unset('k');
            scope.clear();
        }
        const info = [pm.info.eventName, pm.info.requestName, pm.info.requestId, pm.environment.name];
        const response = [pm.response.code, pm.response.status, pm.response.reason(), pm.response.responseTime,
                          pm.response.responseSize, pm.response.text(), pm.response.json()];
        const headers = pm.response.headers;
        headers.get('content-type'); headers.has('x'); headers.all(); headers.count(); headers.toObject();
        headers.each(function () {});
        const legacy = [responseBody, responseCode, responseHeaders, responseTime];
        pm.test('ok', function () {});
        pm.test('bad', function () { throw new Error('boom') });
        pm.test('no function');
        pm.test.skip('later');
        pm.test.index();
        // chai is loaded here, after the spies are in place, and uses them a lot.
        pm.test('expect', function () {
            pm.expect({ a: [1] }).to.deep.equal({ a: [1] });
            pm.expect([1, 2]).to.include(2).and.have.lengthOf(2);
            pm.expect(function () { throw new Error('x') }).to.throw('x');
        });
        pm.test('response', function () {
            pm.response.to.have.status(200);
            pm.response.to.be.ok;
            pm.response.to.be.success;
            pm.response.to.not.be.error;
            pm.response.to.have.header('Content-Type', 'application/json');
            pm.response.to.be.json;
            pm.response.to.be.withBody;
            pm.response.to.have.body({ a: 1 });
            pm.response.to.have.jsonBody('a', 1);
        });
        pm.test('done', function (done) { done() });
        pm.test('promise', function () { return Promise.resolve() });
        tests['old'] = true;
        setTimeout(function (a, b) {}, 1, 1, 2);
        const interval = setInterval(function () { clearInterval(interval) }, 1);
        setImmediate(function () {});
        clearTimeout(setTimeout(function () {}, 5));
        pm.sendRequest('http://x/a', function (error, answer) {
            const read = [answer.code, answer.headers.get('content-type'), answer.text(), answer.json()];
            answer.to.have.status(200);
        });
        pm.sendRequest({ url: { raw: 'http://x/b' }, method: 'POST', header: 'A: 1\\nB: 2',
                         body: { mode: 'urlencoded', urlencoded: 'a=1&b=2' } }).then(function () {});
        pm.sendRequest({ url: 'http://x/c', header: [{ key: 'A', value: '1' }],
                         body: { mode: 'formdata', formdata: [{ key: 'f', type: 'file', src: 'C:/x' }] } },
                       function () {});
        pm.sendRequest({ url: 'http://x/d', header: { A: '1' }, body: { mode: 'raw', raw: { a: 1 } } },
                       function () {});
        pm.sendRequest({}, function (error) {});

        // After the requests and the other timers: their callbacks have been through the spies too.
        setTimeout(function () {
            let leaked = 0;
            for (let i = 0; i < seen.length; i++) {
                if (String(seen[i].name).indexOf('rqm_host_') === 0) leaked++;
            }
            console.log(leaked, seen.length > 0);
        }, 30);";
    let (report, requests) = run_counting(script);
    assert_eq!(report.errors, vec![]);
    assert_eq!(requests, 4);
    assert_eq!(report.console.last().unwrap().text, "0 true");
    // The API did its job under the spies: only the test that throws has failed.
    let failed: Vec<&str> = report
        .tests
        .iter()
        .filter(|test| !test.passed)
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(failed, vec!["bad"], "{:?}", report.tests);
    assert_eq!(report.tests.len(), 9);
}

#[test]
fn chai_loading_into_a_tampered_realm_is_on_the_clock() {
    // chai runs in the realm of the script when it loads: a built-in function the script has replaced
    // with an endless loop is stopped by the time limit like any other code of the script.
    let script = "
        const loop = function () { for (;;) {} };
        Object.defineProperty = loop;
        Object.create = loop;
        Object.keys = loop;
        Array.prototype.forEach = loop;
        pm.expect(1).to.equal(1)";
    let mut job = job(script);
    job.limits.time = Duration::from_millis(300);
    let started = Instant::now();
    let report = run(job, &CancellationToken::new());
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Timeout);
    // The interrupt handler stopped it, not the watchdog two seconds later.
    assert!(started.elapsed() < Duration::from_millis(1500));
}
