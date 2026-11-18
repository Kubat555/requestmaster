//! The variables of a script: the four scopes, the log of changes, the limits of what a script may store,
//! `pm.info`.

use rqm_script::{
    abandoned_threads, run, Info, Job, Limits, Phase, Report, ScriptErrorKind, StoredScope, Var,
    VariableChange, VariableOp, Variables, MAX_CHANGES, MAX_CHANGES_BYTES,
};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// globals `g=G, shared=g`; collection `c=C, shared=c`; environment "Dev" `e=E, shared=e`.
fn base() -> Variables {
    Variables {
        globals: vec![Var::new("g", "G"), Var::new("shared", "g")],
        collection: vec![Var::new("c", "C"), Var::new("shared", "c")],
        environment: vec![Var::new("e", "E"), Var::new("shared", "e")],
        environment_name: Some("Dev".to_string()),
        local: vec![],
    }
}

fn job(phase: Phase, variables: Variables, scripts: &[&str]) -> Job {
    let mut job = Job::new(
        phase,
        scripts.iter().map(|script| script.to_string()).collect(),
    );
    job.variables = variables;
    job.info = Info {
        request_id: "r-1".to_string(),
        request_name: "Login".to_string(),
    };
    job
}

fn run_with(variables: Variables, scripts: &[&str]) -> Report {
    run(
        job(Phase::Test, variables, scripts),
        &CancellationToken::new(),
    )
}

/// A run that has to end without an error.
fn run_clean(variables: Variables, scripts: &[&str]) -> Report {
    let report = run_with(variables, scripts);
    assert_eq!(report.errors, vec![], "{scripts:?}");
    report
}

fn console(report: &Report) -> Vec<&str> {
    report
        .console
        .iter()
        .map(|line| line.text.as_str())
        .collect()
}

fn set(scope: StoredScope, key: &str, value: &str) -> VariableChange {
    VariableChange {
        scope,
        op: VariableOp::Set {
            key: key.to_string(),
            value: value.to_string(),
        },
    }
}

fn one_error(report: &Report) -> (ScriptErrorKind, &str) {
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    (report.errors[0].kind, report.errors[0].message.as_str())
}

#[test]
fn narrowest_scope_wins() {
    let report = run_clean(
        base(),
        &["console.log(pm.variables.get('shared'), pm.globals.get('shared'), \
           pm.collectionVariables.get('shared'), pm.environment.get('shared'), pm.variables.get('nope'), \
           pm.variables.has('g'), pm.environment.has('g')); \
           pm.variables.set('shared', 'l'); \
           console.log(pm.variables.get('shared'), pm.environment.get('shared'))"],
    );
    assert_eq!(
        console(&report),
        vec!["e g c e undefined true false", "l e"]
    );
    assert_eq!(report.changes, vec![]);
}

#[test]
fn set_updates_the_last_entry_or_appends_and_is_logged() {
    let variables = Variables {
        environment: vec![Var::new("a", "1"), Var::new("a", "2")],
        environment_name: Some("Dev".to_string()),
        ..Variables::default()
    };
    let report = run_clean(
        variables,
        &[
            "pm.environment.set('a', 'x'); pm.environment.set('b', 'y'); pm.globals.set('g2', 'z'); \
             pm.collectionVariables.set('token', 't'); pm.variables.set('loc', 'v'); \
             pm.globals.set(7, 'k', 'string')",
        ],
    );
    assert_eq!(
        report.variables.environment,
        vec![Var::new("a", "1"), Var::new("a", "x"), Var::new("b", "y")]
    );
    assert_eq!(
        report.changes,
        vec![
            set(StoredScope::Environment, "a", "x"),
            set(StoredScope::Environment, "b", "y"),
            set(StoredScope::Globals, "g2", "z"),
            set(StoredScope::Collection, "token", "t"),
            set(StoredScope::Globals, "7", "k"),
        ]
    );
    assert_eq!(report.variables.local, vec![Var::new("loc", "v")]);
}

#[test]
fn values_keep_their_type_within_a_send_and_are_stored_as_text() {
    let report = run_clean(
        base(),
        &[
            "pm.environment.set('n', 5); pm.environment.set('o', { a: [1] }); pm.environment.set('z', null); \
             pm.environment.set('u', undefined); pm.environment.set('b', true)",
            "console.log(typeof pm.environment.get('n'), pm.environment.get('o').a[0], \
             pm.environment.get('z') === null, pm.environment.get('u') === '', pm.environment.get('b') === true)",
        ],
    );
    assert_eq!(console(&report), vec!["number 1 true true true"]);
    assert_eq!(
        report.changes,
        vec![
            set(StoredScope::Environment, "n", "5"),
            set(StoredScope::Environment, "o", "[object Object]"),
            set(StoredScope::Environment, "z", "null"),
            set(StoredScope::Environment, "u", ""),
            set(StoredScope::Environment, "b", "true"),
        ]
    );
}

#[test]
fn unset_and_clear() {
    let report = run_clean(
        base(),
        &[
            "pm.environment.unset('e'); pm.environment.unset('nope'); pm.globals.clear(); \
             pm.collectionVariables.clear(); pm.collectionVariables.clear()",
        ],
    );
    let change = |scope, op| VariableChange { scope, op };
    assert_eq!(
        report.changes,
        vec![
            change(
                StoredScope::Environment,
                VariableOp::Unset {
                    key: "e".to_string()
                }
            ),
            change(StoredScope::Globals, VariableOp::Clear),
            change(StoredScope::Collection, VariableOp::Clear),
        ]
    );
    assert_eq!(
        report.variables,
        Variables {
            environment: vec![Var::new("shared", "e")],
            environment_name: Some("Dev".to_string()),
            ..Variables::default()
        }
    );
}

#[test]
fn to_object_and_replace_in() {
    let report = run_clean(
        base(),
        &["console.log(JSON.stringify(pm.variables.toObject())); \
           console.log(pm.variables.replaceIn('{{g}}-{{c}}-{{e}}-{{nope}}')); \
           console.log(pm.environment.replaceIn('{{g}}{{e}}')); \
           console.log(JSON.stringify(pm.variables.replaceIn({ a: ['{{g}}', 1] }))); \
           console.log(pm.variables.replaceIn('{{$guid}}'))"],
    );
    let lines = console(&report);
    assert_eq!(
        lines[..4],
        [
            r#"{"g":"G","shared":"e","c":"C","e":"E"}"#,
            "G-C-E-{{nope}}",
            "{{g}}E",
            r#"{"a":["G",1]}"#,
        ]
    );
    let guid: Vec<char> = lines[4].chars().collect();
    assert_eq!(guid.len(), 36, "{}", lines[4]);
    assert!(
        guid.iter().enumerate().all(|(index, character)| {
            if [8, 13, 18, 23].contains(&index) {
                *character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        }),
        "{}",
        lines[4]
    );
}

#[test]
fn environment_set_without_an_environment_lives_for_the_send_only() {
    let variables = Variables {
        environment_name: None,
        ..base()
    };
    let report = run_clean(
        variables,
        &[
            "pm.environment.set('t', '1'); pm.environment.set('t2', '2')",
            "console.log(pm.environment.get('t'), pm.environment.name)",
        ],
    );
    assert_eq!(report.changes, vec![]);
    assert_eq!(
        console(&report),
        vec![
            "pm.environment.set: no environment is selected, the value is not saved",
            "1 undefined"
        ]
    );
    assert_eq!(report.console[0].level, rqm_script::ConsoleLevel::Warn);
}

#[test]
fn info_of_the_run() {
    let job = job(
        Phase::PreRequest,
        base(),
        &["console.log(pm.info.eventName, pm.info.requestName, pm.info.requestId, pm.info.iteration, \
           pm.info.iterationCount)"],
    );
    let report = run(job, &CancellationToken::new());
    assert_eq!(report.errors, vec![]);
    assert_eq!(console(&report), vec!["prerequest Login r-1 0 1"]);
}

#[test]
fn changes_made_before_an_error_are_kept() {
    let report = run_with(base(), &["pm.globals.set('a', '1'); throw new Error('x')"]);
    assert_eq!(one_error(&report), (ScriptErrorKind::Exception, "Error: x"));
    assert_eq!(report.changes, vec![set(StoredScope::Globals, "a", "1")]);
    assert!(report.variables.globals.contains(&Var::new("a", "1")));
}

#[test]
fn endless_set_is_a_memory_error() {
    // One key: the list does not grow, the log does. The `catch` of the script does not stop the error.
    let started = Instant::now();
    let report = run_with(
        base(),
        &["try { while (true) pm.globals.set('k', 'x'.repeat(1 << 19)) } catch (e) {}"],
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        one_error(&report),
        (
            ScriptErrorKind::Memory,
            "the script wrote more than 16 MiB of variable values"
        )
    );
    assert!(report.changes.len() <= MAX_CHANGES);
    let bytes: usize = report
        .changes
        .iter()
        .map(|change| match &change.op {
            VariableOp::Set { value, .. } => value.len(),
            _ => 0,
        })
        .sum();
    assert!(bytes <= MAX_CHANGES_BYTES, "{bytes}");
    assert!(bytes > MAX_CHANGES_BYTES / 2, "{bytes}");

    let next = run_clean(base(), &["pm.globals.set('k', 'v')"]);
    assert_eq!(next.changes, vec![set(StoredScope::Globals, "k", "v")]);
}

#[test]
fn endless_small_sets_hit_the_change_count() {
    let report = run_with(base(), &["for (;;) pm.environment.set('i', 1)"]);
    assert_eq!(
        one_error(&report),
        (
            ScriptErrorKind::Memory,
            "the script changed variables more than 10000 times"
        )
    );
    assert_eq!(report.changes.len(), MAX_CHANGES);
}

#[test]
fn local_variables_are_capped() {
    let report = run_with(
        base(),
        &["for (let i = 0; ; i++) pm.variables.set('k' + i, 'x'.repeat(1 << 19))"],
    );
    assert_eq!(
        one_error(&report),
        (
            ScriptErrorKind::Memory,
            "the script stored more than 16 MiB of variables"
        )
    );
    assert_eq!(report.changes, vec![]);
    assert_eq!(report.variables.local.len(), 31);
}

#[test]
fn too_long_value_is_a_catchable_error() {
    let report = run_clean(
        base(),
        &[
            "try { pm.environment.set('e', 'x'.repeat((1 << 20) + 1)) } catch (err) { console.log(err.message) } \
             console.log(pm.environment.get('e')); \
             try { pm.environment.set('e', { long: 'x'.repeat(1 << 20) }) } catch (err) { console.log(err.message) } \
             console.log(pm.environment.get('e'))",
        ],
    );
    let refused = "pm.environment.set: the value of 'e' is larger than 1 MB";
    assert_eq!(console(&report), vec![refused, "E", refused, "E"]);
    assert_eq!(report.changes, vec![]);

    // Exactly 1 MiB is taken.
    let exact = run_clean(base(), &["pm.environment.set('e', 'x'.repeat(1 << 20))"]);
    assert_eq!(exact.changes.len(), 1);
}

#[test]
fn lone_surrogates_in_keys_and_values() {
    let report = run_clean(
        base(),
        &["pm.environment.set('k', 'a\\ud800b'); pm.globals.set('\\ud83d', 'v')"],
    );
    assert_eq!(
        report.changes,
        vec![
            set(StoredScope::Environment, "k", "a\u{FFFD}b"),
            set(StoredScope::Globals, "\u{FFFD}", "v"),
        ]
    );
}

#[test]
fn changes_made_before_the_watchdog_are_reported() {
    // The conversion of a huge BigInt to text is a native loop the engine does not interrupt.
    let mut job = job(
        Phase::Test,
        base(),
        &["pm.globals.set('a', '1'); (10n ** 300000n).toString(); pm.globals.set('late', '1')"],
    );
    job.limits = Limits {
        time: Duration::from_millis(100),
        grace: Duration::from_millis(100),
        ..Limits::default()
    };
    let started = Instant::now();
    let report = run(job, &CancellationToken::new());
    assert!(started.elapsed() < Duration::from_millis(1500));
    assert_eq!(one_error(&report).0, ScriptErrorKind::Timeout);
    assert_eq!(report.changes, vec![set(StoredScope::Globals, "a", "1")]);

    // The abandoned thread ends by itself; what it does on the way is not reported anywhere.
    let waiting = Instant::now();
    while abandoned_threads() > 0 {
        assert!(waiting.elapsed() < Duration::from_secs(120));
        std::thread::sleep(Duration::from_millis(20));
    }
}
