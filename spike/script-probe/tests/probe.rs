use script_probe::{run_script, run_script_async, Limits, ScriptOutcome};
use std::time::{Duration, Instant};

const PRELUDE: &str = include_str!("../js/dist/prelude.js");
fn limits() -> Limits { Limits { time: Duration::from_millis(500), memory_bytes: 64 * 1024 * 1024 } }

#[test] fn chai_and_lodash_work() {
    let out = run_script(PRELUDE, "chai.expect(_.sum([1,2])).to.equal(3); 'ok'", limits());
    assert!(matches!(out, ScriptOutcome::Ok(s) if s == "\"ok\""));
}
#[test] fn chai_failure_is_reported() {
    let out = run_script(PRELUDE, "chai.expect(1).to.equal(2)", limits());
    assert!(matches!(out, ScriptOutcome::Threw(m) if m.contains("expected 1 to equal 2")));
}
#[test] fn infinite_loop_is_interrupted() {
    let t = Instant::now();
    assert!(matches!(run_script(PRELUDE, "while(true){}", limits()), ScriptOutcome::TimedOut));
    assert!(t.elapsed() < Duration::from_millis(1500));
}
#[test] fn catastrophic_regex_is_interrupted() {
    let t = Instant::now();
    let out = run_script(PRELUDE, "/(a+)+$/.test('a'.repeat(40)+'b')", limits());
    assert!(matches!(out, ScriptOutcome::TimedOut));
    assert!(t.elapsed() < Duration::from_millis(1500));
}
#[test] fn memory_limit_is_enforced() {
    let out = run_script(PRELUDE, "const a=[]; while(true){a.push('x'.repeat(1<<20))}", limits());
    assert!(matches!(out, ScriptOutcome::OutOfMemory | ScriptOutcome::Threw(_)));
}
#[test] fn promise_resolved_from_rust() {
    let out = run_script_async(PRELUDE, "(async()=>{ return await hostDelay(50) })()", limits());
    assert!(matches!(out, ScriptOutcome::Ok(s) if s == "\"done\""));
}
#[test] fn host_wait_does_not_count_as_script_time() {
    // hostDelay(800) дольше лимита 500 мс: ожидание хоста не должно давать TimedOut
    let out = run_script_async(PRELUDE, "(async()=>{ return await hostDelay(800) })()", limits());
    assert!(matches!(out, ScriptOutcome::Ok(_)));
}
#[test] fn no_ambient_io_globals() {
    let out = run_script(PRELUDE, "[typeof fetch, typeof process, typeof require].join()", limits());
    assert!(matches!(out, ScriptOutcome::Ok(s) if s == "\"undefined,undefined,undefined\""));
}
#[test] fn host_wait_is_excluded_while_interrupt_handler_runs() {
    // 800 ms host wait, then ~150 ms of CPU so the interrupt handler actually fires:
    // without the pause the charged time would be ~950 ms > 500 ms limit.
    let src = "(async()=>{ await hostDelay(800); const t=Date.now(); while(Date.now()-t<150){} return 'ok' })()";
    assert!(matches!(run_script_async(PRELUDE, src, limits()), ScriptOutcome::Ok(s) if s == "\"ok\""));
}
#[test] fn script_time_accumulates_across_host_wait() {
    // ~300 ms CPU + host wait + ~300 ms CPU: charged time ~600 ms > 500 ms limit.
    let src = "(async()=>{ let t=Date.now(); while(Date.now()-t<300){} await hostDelay(100); t=Date.now(); while(Date.now()-t<300){} return 'ok' })()";
    assert!(matches!(run_script_async(PRELUDE, src, limits()), ScriptOutcome::TimedOut));
}
