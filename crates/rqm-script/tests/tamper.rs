//! A script that replaces built-in objects before it calls the API: the limits hold (Rust checks them, not the
//! prelude), and no primitive of the host comes into its hands.
//!
//! Extended by the later tasks, each time with the API of that task.

use rqm_script::{
    run, Job, Phase, ScriptErrorKind, Variables, CONSOLE_LINE_CHARS, CONSOLE_MAX_LINES, MAX_CHANGES,
};
use tokio_util::sync::CancellationToken;

fn job(script: &str) -> Job {
    let mut job = Job::new(Phase::Test, vec![script.to_string()]);
    job.variables = Variables {
        environment_name: Some("Dev".to_string()),
        ..Variables::default()
    };
    job
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
        for (;;) pm.environment.set('i', 1)";
    let report = run(job(script), &CancellationToken::new());

    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].kind, ScriptErrorKind::Memory);
    assert!(!report.console.is_empty());
    assert!(report.console.len() <= CONSOLE_MAX_LINES);
    assert!(report
        .console
        .iter()
        .all(|line| line.text.chars().count() <= CONSOLE_LINE_CHARS + 1));
    assert_eq!(report.changes.len(), MAX_CHANGES);
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

        let leaked = 0;
        for (let i = 0; i < seen.length; i++) {
            if (String(seen[i].name).indexOf('rqm_host_') === 0) leaked++;
        }
        console.log(leaked, seen.length > 0);";
    let report = run(job(script), &CancellationToken::new());
    assert_eq!(report.errors, vec![]);
    assert_eq!(report.console.last().unwrap().text, "0 true");
}
