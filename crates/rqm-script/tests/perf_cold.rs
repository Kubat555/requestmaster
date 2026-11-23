//! A measurement of the first use of the engine in a process: the bytecode of the prelude and of chai is
//! compiled once per process. One test in the file — one process. Prints numbers and asserts nothing about
//! time; run by hand and only in a release build:
//!
//! `cargo test --release -p rqm-script --test perf_cold -- --ignored --nocapture --test-threads=1`

use rqm_script::{run, Job, Phase};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// The time of one `run` of this script, in milliseconds.
fn run_ms(script: &str) -> f64 {
    let job = Job::new(Phase::PreRequest, vec![script.to_string()]);
    let start = Instant::now();
    let report = run(job, &CancellationToken::new());
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(report.errors, vec![], "{script}");
    ms
}

#[test]
#[ignore = "a measurement: run by hand in a release build"]
fn first_use_in_the_process() {
    println!("engine_first_prelude_ms={:.2}", run_ms("0"));
    println!("engine_first_chai_ms={:.2}", run_ms("pm.expect(1).to.equal(1)"));
}
