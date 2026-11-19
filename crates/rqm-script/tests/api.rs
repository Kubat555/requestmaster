//! The table of the API (`prelude/src/api.json`): its shape, that the engine has every name the table calls
//! working, that every name for later throws its own name, `require` with lodash, and the search of the text
//! of a script for what is not supported.

use rqm_script::{
    run, run_with_host, unsupported_api, ConsoleLevel, Host, Job, OutgoingRequest, Phase, Report,
    ResponseBody, ScriptBody, ScriptErrorKind, ScriptPair, ScriptRequest, ScriptResponse,
    Variables,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq)]
struct Member {
    name: String,
    kind: String,
    status: String,
    phase: Option<String>,
}

fn table() -> Value {
    serde_json::from_str(include_str!("../prelude/src/api.json")).unwrap()
}

fn members() -> Vec<Member> {
    let text = |item: &Value, key: &str| item[key].as_str().map(str::to_string);
    table()["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| Member {
            name: text(item, "name").unwrap(),
            kind: text(item, "kind").unwrap(),
            status: text(item, "status").unwrap(),
            phase: text(item, "phase"),
        })
        .collect()
}

/// Module → status.
fn modules() -> BTreeMap<String, String> {
    table()["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["name"].as_str().unwrap().to_string(),
                item["status"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

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

/// An environment "Dev", the request `POST http://x/p?a=1` with a raw body; in `Phase::Test` a response 200.
fn job(phase: Phase, script: &str) -> Job {
    let mut job = Job::new(phase, vec![script.to_string()]);
    job.variables = Variables {
        environment_name: Some("Dev".to_string()),
        ..Variables::default()
    };
    job.request = ScriptRequest {
        method: "POST".to_string(),
        url: "http://x/p?a=1".to_string(),
        headers: vec![ScriptPair {
            key: "Accept".to_string(),
            value: "json".to_string(),
            disabled: false,
        }],
        body: ScriptBody::Raw("{}".to_string()),
    };
    if phase == Phase::Test {
        job.response = Some(response());
    }
    job
}

fn run_in(phase: Phase, script: &str) -> Report {
    run(job(phase, script), &CancellationToken::new())
}

fn lines(report: &Report) -> Vec<&str> {
    report
        .console
        .iter()
        .map(|line| line.text.as_str())
        .collect()
}

/// The members a script of the phase is checked against: `only` — those the table gives this phase alone.
fn of_phase<'a>(members: &'a [Member], phase: &str, only: bool) -> Vec<&'a Member> {
    members
        .iter()
        .filter(|member| match &member.phase {
            Some(own) => own == phase,
            None => !only,
        })
        .collect()
}

/// `[[name, kind], …]` as a JavaScript literal.
fn literal(members: &[&Member]) -> String {
    let pairs: Vec<[&str; 2]> = members
        .iter()
        .map(|member| [member.name.as_str(), member.kind.as_str()])
        .collect();
    serde_json::to_string(&pairs).unwrap()
}

/// Walks from `globalThis` and reads every segment of the name but the last; `check(parent, last, kind, name)`
/// says what is wrong with the member, or nothing. Prints the list of what was wrong.
fn walker(members: &[&Member], check: &str) -> String {
    format!(
        "const wrong = [];
         const check = {check};
         for (const [name, kind] of {list}) {{
             try {{
                 const parts = name.split('.');
                 let parent = globalThis;
                 for (let i = 0; i < parts.length - 1; i++) {{
                     parent = parent[parts[i]];
                     if (parent === undefined || parent === null) throw new Error('no ' + parts[i]);
                 }}
                 const found = check(parent, parts[parts.length - 1], kind, name);
                 if (found !== undefined) wrong.push(name + ': ' + found);
             }} catch (error) {{
                 wrong.push(name + ': ' + String(error));
             }}
         }}
         console.log(JSON.stringify(wrong));",
        list = literal(members)
    )
}

fn assert_nothing_wrong(phase: Phase, members: &[&Member], check: &str) {
    assert!(!members.is_empty());
    let report = run_in(phase, &walker(members, check));
    assert_eq!(report.errors, vec![]);
    assert_eq!(lines(&report), vec!["[]"], "{phase:?}");
}

#[test]
fn table_is_well_formed() {
    let members = members();
    let names: BTreeSet<&str> = members.iter().map(|member| member.name.as_str()).collect();
    assert_eq!(names.len(), members.len(), "a name repeats");
    for member in &members {
        assert!(
            ["v1", "noop", "unsupported"].contains(&member.status.as_str()),
            "{member:?}"
        );
        assert!(
            ["function", "property"].contains(&member.kind.as_str()),
            "{member:?}"
        );
        assert!(
            matches!(member.phase.as_deref(), None | Some("prerequest" | "test")),
            "{member:?}"
        );
        // A name under another name of the table: never under one for later, and with the same status.
        for other in &members {
            if other.name.starts_with(&format!("{}.", member.name)) {
                assert_ne!(member.status, "unsupported", "{} has members", member.name);
                assert_eq!(
                    member.status, other.status,
                    "{} and {}",
                    member.name, other.name
                );
            }
        }
    }

    let find = |name: &str| {
        members
            .iter()
            .find(|member| member.name == name)
            .unwrap_or_else(|| panic!("no {name} in the table"))
    };
    assert_eq!(find("pm.collectionVariables.set").status, "v1");
    assert_eq!(find("responseBody").status, "v1");
    assert_eq!(find("responseBody").phase.as_deref(), Some("test"));
    assert_eq!(find("pm.execution.skipRequest").status, "v1");
    assert_eq!(
        find("pm.execution.skipRequest").phase.as_deref(),
        Some("prerequest")
    );
    assert_eq!(find("pm.execution.setNextRequest").status, "noop");
    for name in [
        "pm.vault",
        "pm.cookies",
        "pm.request.to",
        "pm.response.to.have.jsonSchema",
    ] {
        assert_eq!(find(name).status, "unsupported", "{name}");
    }
    assert!(!names.contains("pm.cookies.jar"));

    let modules = modules();
    assert_eq!(modules.len(), table()["modules"].as_array().unwrap().len());
    assert!(modules
        .values()
        .all(|status| status == "v1" || status == "unsupported"));
    assert_eq!(modules["lodash"], "v1");
    assert_eq!(modules["moment"], "unsupported");
}

#[test]
fn every_v1_and_noop_member_exists_in_the_engine() {
    let all = members();
    let working: Vec<Member> = all
        .into_iter()
        .filter(|member| member.status != "unsupported")
        .collect();
    // The last segment is not read: a property assertion of chai runs its check when read. Only a function
    // is read, and that runs nothing.
    let check = "(parent, last, kind) => {
        if (!(last in parent)) return 'is missing';
        if (kind === 'function' && typeof parent[last] !== 'function') return 'is not a function';
    }";
    assert_nothing_wrong(Phase::Test, &of_phase(&working, "test", false), check);
    assert_nothing_wrong(
        Phase::PreRequest,
        &of_phase(&working, "prerequest", true),
        check,
    );
}

#[test]
fn every_unsupported_member_throws_its_name() {
    let all = members();
    let later: Vec<Member> = all
        .iter()
        .filter(|member| member.status == "unsupported")
        .cloned()
        .collect();
    // A function throws when called, a property when read.
    let check = "(parent, last, kind, name) => {
        try {
            if (kind === 'function') parent[last](); else parent[last];
        } catch (error) {
            const text = error.name + ': ' + error.message;
            return text === 'Error: ' + name + ' is not supported in RequestMaster' ? undefined : text;
        }
        return 'did not throw';
    }";
    assert_nothing_wrong(Phase::Test, &of_phase(&later, "test", false), check);
    assert_nothing_wrong(
        Phase::PreRequest,
        &of_phase(&later, "prerequest", false),
        check,
    );

    let report = run_in(
        Phase::Test,
        "for (const call of [() => pm.response.to.have.jsonSchema({}), () => pm.response.to.not.have.jsonSchema({})]) {
             try { call() } catch (error) { console.log(error.name + ': ' + error.message) }
         }",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        lines(&report),
        vec!["Error: pm.response.to.have.jsonSchema is not supported in RequestMaster"; 2]
    );

    // The three assertions the chai plugin stubs are the ones the table has for later (each of them threw
    // its name above).
    let stubbed: Vec<&str> = later
        .iter()
        .filter_map(|member| member.name.strip_prefix("pm.response.to.have."))
        .collect();
    assert_eq!(stubbed, vec!["jsonSchema", "responseTime", "responseSize"]);
    assert!(!all
        .iter()
        .any(|member| member.status == "unsupported"
            && member.name.starts_with("pm.response.to.be.")));
}

#[test]
fn stubs_keep_phases_apart() {
    let report = run_in(
        Phase::PreRequest,
        "console.log(typeof pm.response, typeof responseBody)",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(lines(&report), vec!["undefined undefined"]);
}

/// Answers every request with the response 200.
struct Answering;

impl Host for Answering {
    fn send_request(&mut self, _request: OutgoingRequest) -> Result<ScriptResponse, String> {
        Ok(response())
    }
}

#[test]
fn sendrequest_response_has_stubs() {
    let script = "pm.sendRequest('http://x/a', (e, res) => {
        try { res.cookies } catch (err) { console.log(err.message) }
    })";
    let report = run_with_host(
        job(Phase::PreRequest, script),
        Box::new(Answering),
        &CancellationToken::new(),
    );
    assert_eq!(report.errors, vec![]);
    let printed: Vec<&str> = report
        .console
        .iter()
        .filter(|line| line.level != ConsoleLevel::Network)
        .map(|line| line.text.as_str())
        .collect();
    assert_eq!(
        printed,
        vec!["pm.response.cookies is not supported in RequestMaster"]
    );
}

#[test]
fn stubs_do_not_break_printing() {
    let report = run_in(
        Phase::Test,
        "console.log(pm.response);
         console.log(Object.keys(pm.response).length >= 0, Object.keys(pm.request).includes('auth'));
         console.log(pm.request);
         console.log(pm.environment)",
    );
    assert_eq!(report.errors, vec![]);
    let lines = lines(&report);
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(lines[0].starts_with("{\"code\":200,"), "{}", lines[0]);
    assert_eq!(lines[1], "true false");
    // `toJSON` of the request and of a scope are for later, and `JSON.stringify` calls them: the console
    // still prints the objects.
    assert!(lines[2].contains("\"method\":\"POST\""), "{}", lines[2]);
    assert_eq!(lines[3], "{\"name\":\"Dev\"}");
}

#[test]
fn stubs_survive_url_assignment() {
    let report = run_in(
        Phase::PreRequest,
        "pm.request.url = 'http://other/p';
         try { pm.request.url.host } catch (err) { console.log(err.message) }",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(
        lines(&report),
        vec!["pm.request.url.host is not supported in RequestMaster"]
    );
}

#[test]
fn require_modules() {
    let report = run_in(
        Phase::Test,
        "const l = require('lodash');
         console.log(l.VERSION, l.get({ a: [{ b: 1 }] }, 'a[0].b'), _.chunk([1, 2, 3], 2).length,
                     require('chai').expect === pm.expect, require('atob')('YQ=='), btoa('a'))",
    );
    assert_eq!(report.errors, vec![]);
    assert_eq!(lines(&report), vec!["4.18.1 1 2 true a YQ=="]);

    // Every module of the table: one that works is given, one for later is "not found".
    for (module, status) in modules() {
        let report = run_in(Phase::Test, &format!("require('{module}')"));
        if status == "v1" {
            assert_eq!(report.errors, vec![], "{module}");
        } else {
            assert_eq!(report.errors.len(), 1, "{module}: {:?}", report.errors);
            assert_eq!(report.errors[0].kind, ScriptErrorKind::Exception);
            assert_eq!(
                report.errors[0].message,
                format!("Error: Cannot find module '{module}' (not supported in RequestMaster)")
            );
        }
    }
    let report = run_in(Phase::Test, "require('fs')");
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(
        report.errors[0].message,
        "Error: Cannot find module 'fs' (not supported in RequestMaster)"
    );
}

#[test]
fn scanner() {
    let none: Vec<String> = vec![];
    assert_eq!(
        unsupported_api(
            "pm.environment.set('a', 1); pm.response.json().data.items; \
             pm.expect(pm.response.code).to.equal(200); require(\"lodash\")"
        ),
        none
    );
    assert_eq!(
        unsupported_api(
            "pm.vault.get('x'); const m = require('moment'); CryptoJS.MD5('a'); pm.foo.bar(); \
             postman.getResponseCookie('a'); x.Buffer; pm.vault.set('y', 1)"
        ),
        vec![
            "CryptoJS",
            "pm.foo",
            "pm.vault",
            "postman.getResponseCookie",
            "require('moment')"
        ]
    );
    assert_eq!(unsupported_api("// pm.vault"), vec!["pm.vault"]);
    assert_eq!(unsupported_api(""), none);
    // Namespaces, a negated assertion and the free chain of chai are not findings.
    assert_eq!(
        unsupported_api(
            "pm.expect(pm.response).to.have.status(200); const r = pm.response; const e = pm.environment; \
             pm.request.url = 'x'; pm.response.to.not.be.error; pm.response.to.not.have.status(404); \
             pm.response.to.not.be.ok; pm.response.to.be.an('object'); pm.response.to.have.property('code')"
        ),
        none
    );
    assert_eq!(
        unsupported_api(
            "pm.response.to.not.have.jsonSchema(s); pm.response.foo(); pm.request.to.have.header('a'); \
             pm.cookies.jar()"
        ),
        vec![
            "pm.cookies",
            "pm.request.to",
            "pm.response.foo",
            "pm.response.to.have.jsonSchema"
        ]
    );
}
