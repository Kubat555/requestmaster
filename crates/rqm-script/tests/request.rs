//! `pm.request`, `pm.execution` and the old names: `postman.*`, `request`, `environment`, `globals`.
//!
//! The request of every test, unless it says otherwise: `GET {{host}}/users/7?page=1&q=a%20b#frag`, the
//! headers `Accept: json` and a disabled `X-Off: 1`, the raw body `{"a":1}`.

use rqm_script::{
    run, ConsoleLevel, Info, Job, Phase, Report, ResponseBody, ScriptBody, ScriptPair,
    ScriptRequest, ScriptResponse, StoredScope, Var, VariableChange, VariableOp, Variables,
};
use tokio_util::sync::CancellationToken;

const URL: &str = "{{host}}/users/7?page=1&q=a%20b#frag";

fn pair(key: &str, value: &str) -> ScriptPair {
    ScriptPair {
        key: key.to_string(),
        value: value.to_string(),
        disabled: false,
    }
}

fn off(key: &str, value: &str) -> ScriptPair {
    ScriptPair {
        disabled: true,
        ..pair(key, value)
    }
}

fn request() -> ScriptRequest {
    ScriptRequest {
        method: "GET".to_string(),
        url: URL.to_string(),
        headers: vec![pair("Accept", "json"), off("X-Off", "1")],
        body: ScriptBody::Raw("{\"a\":1}".to_string()),
    }
}

fn with_body(body: ScriptBody) -> ScriptRequest {
    ScriptRequest { body, ..request() }
}

/// globals `g=G`, environment "Dev" `e=E`; in `Phase::Test` a response with the header `Content-Type`.
fn job(phase: Phase, request: ScriptRequest, scripts: &[&str]) -> Job {
    let mut job = Job::new(
        phase,
        scripts.iter().map(|script| script.to_string()).collect(),
    );
    job.variables = Variables {
        globals: vec![Var::new("g", "G")],
        environment: vec![Var::new("e", "E")],
        environment_name: Some("Dev".to_string()),
        ..Variables::default()
    };
    job.info = Info {
        request_id: "r-1".to_string(),
        request_name: "Login".to_string(),
    };
    job.request = request;
    if phase == Phase::Test {
        job.response = Some(ScriptResponse {
            code: 200,
            status: "OK".to_string(),
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            time_ms: 1.0,
            size: 0,
            body: ResponseBody::Bytes(Vec::new()),
        });
    }
    job
}

/// A run that has to end without an error.
fn run_clean(phase: Phase, request: ScriptRequest, scripts: &[&str]) -> Report {
    let report = run(job(phase, request, scripts), &CancellationToken::new());
    assert_eq!(report.errors, vec![], "{scripts:?}");
    report
}

/// Before the request, with the request of the header of this file.
fn pre(script: &str) -> Report {
    run_clean(Phase::PreRequest, request(), &[script])
}

fn console(report: &Report) -> Vec<&str> {
    report
        .console
        .iter()
        .map(|line| line.text.as_str())
        .collect()
}

fn warnings(report: &Report) -> Vec<&str> {
    let warnings = report
        .console
        .iter()
        .filter(|line| line.level == ConsoleLevel::Warn);
    warnings.map(|line| line.text.as_str()).collect()
}

fn change(scope: StoredScope, op: VariableOp) -> VariableChange {
    VariableChange { scope, op }
}

#[test]
fn reads() {
    let report = pre("console.log(pm.request.method, pm.request.url.toString(), pm.request.url.getHost(), \
         pm.request.url.getPath(), pm.request.url.getQueryString(), pm.request.url.getPathWithQuery(), \
         pm.request.url.query.get('page'), pm.request.headers.get('accept'), pm.request.headers.has('X-Off'), \
         pm.request.body.mode, pm.request.body.raw, pm.request.body.isEmpty())");
    assert_eq!(
        console(&report),
        vec![
            "GET {{host}}/users/7?page=1&q=a%20b#frag {{host}} /users/7 page=1&q=a%20b \
             /users/7?page=1&q=a%20b 1 json false raw {\"a\":1} false"
        ]
    );
    // Reading changes nothing.
    assert_eq!(report.request, request());

    let with_port = ScriptRequest {
        url: "https://api.example.com:8443/a".to_string(),
        ..request()
    };
    let report = run_clean(
        Phase::PreRequest,
        with_port,
        &["console.log(pm.request.url.getHost())"],
    );
    assert_eq!(console(&report), vec!["api.example.com"]);
}

#[test]
fn mutations_come_back() {
    let report = pre("pm.request.method = 'POST'; \
         pm.request.url.query.add({ key: 'x', value: '1' }); \
         pm.request.url.removeQueryParams('page'); \
         pm.request.headers.add({ key: 'X-New', value: 'n' }); \
         pm.request.headers.upsert({ key: 'accept', value: 'xml' }); \
         pm.request.removeHeader('X-Off'); \
         pm.request.addHeader('X-S: s'); \
         pm.request.body.raw = '{\"b\":2}'");
    assert_eq!(
        report.request,
        ScriptRequest {
            method: "POST".to_string(),
            url: "{{host}}/users/7?q=a%20b&x=1#frag".to_string(),
            headers: vec![pair("Accept", "xml"), pair("X-New", "n"), pair("X-S", "s")],
            body: ScriptBody::Raw("{\"b\":2}".to_string()),
        }
    );
    assert!(!report.skip_request);
}

#[test]
fn url_assignment_and_body_update() {
    let report = pre("const url = pm.request.url; \
         pm.request.url = 'http://other/p?z=1'; \
         pm.request.body.update({ mode: 'urlencoded', urlencoded: [{ key: 'a', value: '1' }] }); \
         console.log(url === pm.request.url, url.query.get('z'), pm.request.body.mode)");
    assert_eq!(report.request.url, "http://other/p?z=1");
    assert_eq!(
        report.request.body,
        ScriptBody::UrlEncoded(vec![pair("a", "1")])
    );
    // The object of the address is the same one: the assignment changed what it holds.
    assert_eq!(console(&report), vec!["true 1 urlencoded"]);

    let report = pre("pm.request.body.update('plain')");
    assert_eq!(report.request.body, ScriptBody::Raw("plain".to_string()));
}

#[test]
fn file_modes_cannot_be_set() {
    let report = pre(
        "pm.request.body.update({ mode: 'file', file: { src: 'C:/x' } }); \
         pm.request.body.update({ mode: 'formdata', formdata: [] })",
    );
    assert_eq!(report.request, request());
    assert_eq!(
        warnings(&report),
        vec![
            "pm.request.body: mode 'file' cannot be set from a script",
            "pm.request.body: mode 'formdata' cannot be set from a script"
        ]
    );

    // A body a script only sees the mode of: neither `raw` nor the fields take an edit.
    let script = "console.log(pm.request.body.mode, pm.request.body.toString() === ''); \
                  pm.request.body.raw = 'x'; \
                  pm.request.body.urlencoded.add({ key: 'k', value: 'v' })";
    let cannot = |mode: &str| format!("pm.request.body: a {mode} body cannot be changed from a script");
    for (body, mode) in [
        (ScriptBody::FormData(vec![pair("a", "1")]), "formdata"),
        (ScriptBody::Other("file".to_string()), "file"),
    ] {
        let report = run_clean(Phase::PreRequest, with_body(body.clone()), &[script]);
        assert_eq!(report.request.body, body);
        assert_eq!(report.console[0].text, format!("{mode} true"));
        assert_eq!(warnings(&report), vec![cannot(mode), cannot(mode)]);
    }
}

#[test]
fn changes_pass_to_the_next_script() {
    let report = run_clean(
        Phase::PreRequest,
        request(),
        &[
            "pm.request.headers.add({ key: 'X-One', value: '1' })",
            "console.log(pm.request.headers.get('x-one'))",
        ],
    );
    assert_eq!(console(&report), vec!["1"]);
}

#[test]
fn skip_request() {
    let report = run_clean(
        Phase::PreRequest,
        request(),
        &[
            "console.log('a'); pm.execution.skipRequest(); console.log('b')",
            "console.log('c')",
        ],
    );
    assert_eq!(console(&report), vec!["a"]);
    assert!(report.skip_request);

    // A script cannot go on by catching it: the host does nothing for it any more.
    let report = pre(
        "try { pm.execution.skipRequest() } catch (error) {} \
         try { console.log('b') } catch (error) {} \
         pm.request.method = 'POST'",
    );
    assert_eq!(console(&report), Vec::<&str>::new());
    assert!(report.skip_request);
    assert_eq!(report.request, request());

    // After the request there is nothing to skip, as in Postman.
    let report = run_clean(
        Phase::Test,
        request(),
        &["console.log(typeof pm.execution.skipRequest)"],
    );
    assert_eq!(console(&report), vec!["undefined"]);
    assert!(!report.skip_request);
}

#[test]
fn test_phase_changes_nothing() {
    let report = run_clean(
        Phase::Test,
        request(),
        &["pm.request.method = 'DELETE'; \
           pm.request.headers.add({ key: 'X', value: '1' }); \
           pm.request.url = 'http://other'; \
           pm.request.body.raw = 'x'; \
           console.log(pm.request.method, pm.request.headers.get('x'), pm.request.url.toString())"],
    );
    // The script sees its own copy change; the request does not.
    assert_eq!(console(&report), vec!["DELETE 1 http://other"]);
    assert_eq!(report.request, request());
    assert!(!report.skip_request);
}

#[test]
fn legacy_postman_object() {
    let report = pre("postman.setEnvironmentVariable('n', 5); \
         postman.setGlobalVariable('g', 'v'); \
         console.log(typeof postman.getEnvironmentVariable('n'), postman.getGlobalVariable('g'), \
                     environment.n, globals.g); \
         postman.clearEnvironmentVariable('n'); \
         console.log(environment.n); \
         postman.clearGlobalVariables(); \
         console.log(Object.keys(globals).length); \
         postman.setNextRequest(null); pm.execution.setNextRequest('x')");
    assert_eq!(console(&report), vec!["string v 5 v", "undefined", "0"]);
    let set = |key: &str, value: &str| VariableOp::Set {
        key: key.to_string(),
        value: value.to_string(),
    };
    assert_eq!(
        report.changes,
        vec![
            change(StoredScope::Environment, set("n", "5")),
            change(StoredScope::Globals, set("g", "v")),
            change(
                StoredScope::Environment,
                VariableOp::Unset {
                    key: "n".to_string()
                }
            ),
            change(StoredScope::Globals, VariableOp::Clear),
        ]
    );

    // As in Postman: only a truthy value and `false` become text.
    let report = pre("postman.setEnvironmentVariable('z', 0); \
         postman.setEnvironmentVariable('f', false); \
         console.log(typeof pm.environment.get('z'), typeof pm.environment.get('f'))");
    assert_eq!(console(&report), vec!["number string"]);

    // The scenario of an old script: the old object shows what `postman.set…` has just stored.
    let report = pre("postman.setEnvironmentVariable('token', 'T'); \
         tests['saved'] = environment.token === 'T';");
    assert_eq!(
        report
            .tests
            .iter()
            .map(|test| (test.name.as_str(), test.passed))
            .collect::<Vec<_>>(),
        vec![("saved", true)]
    );

    // A value of the script under the old name is not touched.
    let report = pre("var environment = { mine: 1 }; \
         postman.setEnvironmentVariable('q', '1'); \
         console.log(environment.q, environment.mine)");
    assert_eq!(console(&report), vec!["undefined 1"]);

    let report = run_clean(
        Phase::Test,
        request(),
        &["console.log(postman.getResponseHeader('content-type'))"],
    );
    assert_eq!(console(&report), vec!["text/plain"]);
}

#[test]
fn legacy_snapshots() {
    let report = pre(
        "console.log(environment.e, globals.g, request.method, request.url, \
         request.headers.accept, request.headers.Accept, request.data, typeof data, iteration, \
         request.id, request.name)",
    );
    assert_eq!(
        console(&report),
        vec!["E G GET {{host}}/users/7?page=1&q=a%20b#frag json undefined {\"a\":1} object 0 r-1 Login"]
    );

    // A name that repeats gives an array of its values; a disabled pair is not there.
    let repeated = ScriptRequest {
        headers: vec![pair("X-A", "1"), pair("x-a", "2")],
        body: ScriptBody::UrlEncoded(vec![pair("u", "1"), pair("u", "2"), off("off", "3")]),
        ..request()
    };
    let report = run_clean(
        Phase::PreRequest,
        repeated,
        &["console.log(JSON.stringify(request.headers['x-a']), JSON.stringify(request.data))"],
    );
    assert_eq!(console(&report), vec![r#"["1","2"] {"u":["1","2"]}"#]);

    let form = ScriptBody::FormData(vec![pair("user", "bob"), pair("ts", "5")]);
    let report = run_clean(
        Phase::PreRequest,
        with_body(form),
        &["console.log(request.data.user + request.data.ts)"],
    );
    assert_eq!(console(&report), vec!["bob5"]);

    let report = run_clean(
        Phase::PreRequest,
        with_body(ScriptBody::Other("file".to_string())),
        &["console.log(JSON.stringify(request.data))"],
    );
    assert_eq!(console(&report), vec!["{}"]);

    // The old names give way to a declaration and to an assignment without one.
    let report = pre("var data = 1; var request = 2; console.log(data + request)");
    assert_eq!(console(&report), vec!["3"]);
    let report = pre("data = { a: 1 }; request = 'r'; environment = 'e'; \
         console.log(data.a, request, environment)");
    assert_eq!(console(&report), vec!["1 r e"]);
}
