//! Measurements of the engine: they print numbers and assert nothing about time. The test is ignored in the
//! usual run; run it by hand and only in a release build:
//!
//! `cargo test --release -p rqm-script --features measure --test perf -- --ignored --nocapture --test-threads=1`
#![cfg(feature = "measure")]

use rqm_script::{run, Job, Phase, ResponseBody, ScriptResponse};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// How many times every case runs.
const RUNS: usize = 200;

const OWNER_LIKE: &str =
    r#"var d = JSON.parse(responseBody); pm.collectionVariables.set("token", d.token);"#;

const TYPICAL: &str = r#"
pm.test("status is 200", function () { pm.expect(pm.response.code).to.equal(200); });
pm.test("has a token", function () { pm.expect(pm.response.json().token).to.be.a("string"); });
pm.test("has items", function () { pm.expect(pm.response.json().items).to.have.lengthOf(3); });
console.log("done", pm.response.code);
"#;

/// A JSON body of about `bytes` bytes with the fields `token`, `items` and `filler`.
fn json_body(bytes: usize) -> Vec<u8> {
    let filler = "x".repeat(bytes);
    format!(r#"{{"token":"abc","items":[1,2,3],"filler":"{filler}"}}"#).into_bytes()
}

/// A job of the phase test with a 200 response that has this body.
fn test_job(script: &str, body: Vec<u8>) -> Job {
    let mut job = Job::new(Phase::Test, vec![script.to_string()]);
    job.response = Some(ScriptResponse {
        code: 200,
        status: "OK".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        time_ms: 12.5,
        size: body.len() as u64,
        body: ResponseBody::Bytes(body),
    });
    job
}

/// Runs the job `RUNS` times and prints the time of `run` and the largest memory of a script.
fn measure(case: &str, job: &Job) {
    let cancel = CancellationToken::new();
    let mut times = Vec::with_capacity(RUNS);
    let mut memory_bytes = 0;
    for _ in 0..RUNS {
        let job = job.clone();
        let start = Instant::now();
        let report = run(job, &cancel);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(report.errors, vec![], "{case}");
        memory_bytes = memory_bytes.max(report.memory_used_bytes);
    }
    times.sort_by(f64::total_cmp);
    let median = (times[RUNS / 2 - 1] + times[RUNS / 2]) / 2.0;
    let max = times[RUNS - 1];
    let memory_kb = memory_bytes / 1024;
    println!("engine_ms case={case} median={median:.3} max={max:.3} n={RUNS} memory_kb={memory_kb}");
}

#[test]
#[ignore = "a measurement: run by hand in a release build"]
fn engine_report() {
    // The thread, the runtime, the context and the prelude.
    measure("engine_start", &Job::new(Phase::PreRequest, vec!["0".to_string()]));
    measure("owner_like", &test_job(OWNER_LIKE, json_body(15 * 1024)));
    measure("typical", &test_job(TYPICAL, json_body(0)));
    let six = vec![r#"pm.variables.set("a", "1")"#.to_string(); 6];
    measure("chain_of_six", &Job::new(Phase::PreRequest, six));
}
