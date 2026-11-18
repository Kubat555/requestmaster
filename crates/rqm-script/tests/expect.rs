//! chai: `pm.expect`, the assertions of `pm.response.to`, tests that end later (`done`, a promise), and the
//! old `tests[...]` object.

use rqm_script::{run, Job, Phase, Report, ResponseBody, ScriptResponse, TestResult};
use tokio_util::sync::CancellationToken;

fn response(code: u16, status: &str) -> ScriptResponse {
    let body = br#"{"user":{"id":7,"tags":["a"]}}"#.to_vec();
    ScriptResponse {
        code,
        status: status.to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        time_ms: 1.0,
        size: body.len() as u64,
        body: ResponseBody::Bytes(body),
    }
}

fn run_with(response: ScriptResponse, scripts: &[&str]) -> Report {
    let scripts = scripts.iter().map(|script| script.to_string()).collect();
    let mut job = Job::new(Phase::Test, scripts);
    job.response = Some(response);
    run(job, &CancellationToken::new())
}

fn run_ok(script: &str) -> Report {
    run_with(response(200, "OK"), &[script])
}

fn find<'a>(report: &'a Report, name: &str) -> &'a TestResult {
    report
        .tests
        .iter()
        .find(|test| test.name == name)
        .unwrap_or_else(|| panic!("no test {name:?} in {:?}", report.tests))
}

/// `(name, passed, error)` of every test, in the order they were recorded.
fn outcomes(report: &Report) -> Vec<(&str, bool, Option<&str>)> {
    report
        .tests
        .iter()
        .map(|test| (test.name.as_str(), test.passed, test.error.as_deref()))
        .collect()
}

#[test]
fn chai_messages_match_postman() {
    let report = run_ok(
        "pm.test('equal', () => { pm.expect(1).to.equal(2) });
         pm.test('deep', () => { pm.expect({ a: [1, { b: 2 }] }).to.deep.equal({ a: [1, { b: 3 }] }) });
         pm.test('type', () => { pm.expect('abc').to.be.a('number') });
         pm.test('property', () => { pm.expect({ a: 1 }).to.have.property('b') });
         pm.test('include', () => { pm.expect([1, 2]).to.include(3) });
         pm.test('oneOf', () => { pm.expect(200).to.be.oneOf([201, 202]) });
         pm.test('message', () => { pm.expect(1, 'with message').to.be.below(0) });
         pm.test('ok', () => { pm.expect('').to.be.ok });",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        outcomes(&report),
        vec![
            ("equal", false, Some("AssertionError: expected 1 to equal 2")),
            (
                "deep",
                false,
                Some("AssertionError: expected { a: [ 1, { b: 2 } ] } to deeply equal { a: [ 1, { b: 3 } ] }")
            ),
            ("type", false, Some("AssertionError: expected 'abc' to be a number")),
            (
                "property",
                false,
                Some("AssertionError: expected { a: 1 } to have property 'b'")
            ),
            ("include", false, Some("AssertionError: expected [ 1, 2 ] to include 3")),
            (
                "oneOf",
                false,
                Some("AssertionError: expected 200 to be one of [ 201, 202 ]")
            ),
            (
                "message",
                false,
                Some("AssertionError: with message: expected 1 to be below +0")
            ),
            ("ok", false, Some("AssertionError: expected '' to be truthy")),
        ]
    );
}

#[test]
fn common_chains_pass() {
    let report = run_ok(
        "pm.test('chains', () => {
             pm.expect({ a: [1] }).to.eql({ a: [1] });
             pm.expect([1]).to.be.an('array');
             pm.expect({ a: 1 }).to.have.property('a', 1);
             pm.expect(true).to.be.true;
             pm.expect(null).to.be.null;
             pm.expect(0).to.exist;
             pm.expect(2).to.be.above(1);
             pm.expect(2).to.be.at.least(2);
             pm.expect(2).to.be.within(1, 3);
             pm.expect([1, 2]).to.have.lengthOf(2);
             pm.expect('abc').to.match(/b/);
             pm.expect({ a: 1, b: 2 }).to.have.keys('a', 'b');
             pm.expect([1, 2]).to.have.members([2, 1]);
             pm.expect([1]).to.not.be.empty;
             pm.expect(() => { throw new Error('x') }).to.throw('x');
         })",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(outcomes(&report), vec![("chains", true, None)]);
}

#[test]
fn response_assertions_pass() {
    let report = run_ok(
        "pm.test('200', () => {
             pm.response.to.have.status(200);
             pm.response.to.have.status('OK');
             pm.response.to.be.ok;
             pm.response.to.be.success;
             pm.response.to.not.be.error;
             pm.response.to.have.header('content-type');
             pm.response.to.have.header('Content-Type', 'application/json');
             pm.response.to.be.json;
             pm.response.to.be.withBody;
             pm.response.to.have.body(/user/);
             pm.response.to.have.jsonBody('user.id', 7);
             pm.response.to.have.jsonBody('user.tags[0]', 'a');
             pm.response.to.have.statusCode(200);
             pm.response.to.have.statusCodeClass(2);
             pm.response.to.not.have.status(404);
         })",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(outcomes(&report), vec![("200", true, None)]);

    let report = run_with(
        response(404, "Not Found"),
        &["pm.test('404', () => {
               pm.response.to.be.notFound;
               pm.response.to.be.clientError;
               pm.response.to.be.error;
               pm.response.to.not.be.ok;
           })"],
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(outcomes(&report), vec![("404", true, None)]);
}

#[test]
fn response_assertions_fail_with_postman_messages() {
    let report = run_ok(
        "pm.test('code', () => { pm.response.to.have.status(201) });
         pm.test('reason', () => { pm.response.to.have.status('Created') });
         pm.test('class', () => { pm.response.to.be.clientError });
         pm.test('header', () => { pm.response.to.have.header('X-No') });
         pm.test('named', () => { pm.response.to.be.notFound });
         pm.test('path', () => { pm.response.to.have.jsonBody('user.name') });
         pm.test('other', () => { pm.expect({}).to.have.status(200) });",
    );
    assert_eq!(report.errors, vec![]);
    let error = |name: &str| find(&report, name).error.clone().unwrap();
    assert_eq!(
        error("code"),
        "AssertionError: expected response to have status code 201 but got 200"
    );
    assert_eq!(
        error("reason"),
        "AssertionError: expected response to have status reason 'Created' but got 'OK'"
    );
    assert_eq!(
        error("class"),
        "AssertionError: expected response code to be 4XX but found 200"
    );
    assert_eq!(
        error("header"),
        "AssertionError: expected response to have header with key 'X-No'"
    );
    assert_eq!(
        error("named"),
        "AssertionError: expected response to have status reason 'NOT FOUND' but got 'OK'"
    );
    assert!(
        error("path").contains("in response to contain property 'user.name'"),
        "{}",
        error("path")
    );
    assert!(
        error("other").contains("expecting a postman response object"),
        "{}",
        error("other")
    );
    assert!(report.tests.iter().all(|test| !test.passed));
}

#[test]
fn async_tests() {
    let report = run_ok(
        "pm.test('done ok', function (done) { Promise.resolve().then(() => done()) });
         pm.test('done err', function (done) { done(new Error('late')) });
         pm.test('never', function (done) {});
         pm.test('promise ok', async () => { await Promise.resolve() });
         pm.test('promise bad', async () => { throw new Error('rejected') });",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.tests.len(), 5, "{:?}", report.tests);
    let outcome = |name: &str| {
        let test = find(&report, name);
        (test.passed, test.error.clone())
    };
    assert_eq!(outcome("done ok"), (true, None));
    assert_eq!(outcome("done err"), (false, Some("Error: late".to_string())));
    assert_eq!(outcome("promise ok"), (true, None));
    assert_eq!(
        outcome("promise bad"),
        (false, Some("Error: rejected".to_string()))
    );
    assert_eq!(
        outcome("never"),
        (false, Some("done() was not called".to_string()))
    );
    // A test is listed when it is recorded: the one that never ends comes last.
    assert_eq!(report.tests.last().unwrap().name, "never");
}

#[test]
fn legacy_tests_object() {
    let report = run_ok(
        "tests['is ok'] = responseCode.code === 200; tests.zero = 0; tests['empty'] = '';
         pm.test('first', () => {});",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        outcomes(&report),
        vec![
            ("first", true, None),
            ("is ok", true, None),
            ("zero", false, Some("AssertionError: expected 0 to be truthy")),
            ("empty", false, Some("AssertionError: expected '' to be truthy")),
        ]
    );
}

#[test]
fn legacy_tests_survive_an_error() {
    let report = run_ok("tests.a = true; throw new Error('x')");
    assert_eq!(outcomes(&report), vec![("a", true, None)]);
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].message, "Error: x");
}

#[test]
fn legacy_tests_object_can_be_replaced() {
    let report = run_ok("tests = { a: true }; tests.b = false");
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        outcomes(&report),
        vec![
            ("a", true, None),
            ("b", false, Some("AssertionError: expected false to be truthy")),
        ]
    );
}

#[test]
fn chai_loads_wherever_it_is_first_touched() {
    // Every script has a context of its own, so each of the three loads chai anew — from inside a call the
    // prelude made, from a job of a promise, and through the response.
    let report = run_with(
        response(200, "OK"),
        &[
            "pm.test('in a test', () => { pm.expect(1).to.equal(1) })",
            "await Promise.resolve().then(() => { pm.expect(2).to.equal(2) }); pm.test('in a job', () => {})",
            "pm.test('through the response', () => { pm.response.to.have.status(200) })",
        ],
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        outcomes(&report),
        vec![
            ("in a test", true, None),
            ("in a job", true, None),
            ("through the response", true, None),
        ]
    );
}
