//! A measurement of the first send with scripts in a process: the bytecode of the prelude and of chai is
//! compiled once per process, so this file holds one test and nothing else (one file — one test binary —
//! one process). Prints a number and asserts nothing about time; run by hand and only in a release build:
//!
//! `cargo test --release -p requestmaster --test core_perf_cold -- --ignored --nocapture --test-threads=1`
//!
//! The server is `wiremock` on `127.0.0.1`.

#[path = "common/perf.rs"]
mod perf;

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use perf::{core_in, measured_send, scripted_request, TYPICAL_BODY, TYPICAL_PRE_REQUEST, TYPICAL_TEST};

#[tokio::test]
#[ignore = "a measurement: run by hand in a release build"]
async fn first_scripted_send_in_the_process() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string(TYPICAL_BODY))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let request_id = scripted_request(
        &core,
        &format!("{}/x", server.uri()),
        json!({ "pre_request": TYPICAL_PRE_REQUEST }),
        json!({ "test": TYPICAL_TEST }),
    );

    // The first and the only send of the process: the prelude and chai are both cold.
    let (overhead, result) = measured_send(&core, "first", &request_id).await;
    assert_eq!(result.script_errors, vec![]);
    assert_eq!(result.tests.iter().filter(|test| test.passed).count(), 3);
    println!("script_first_send_ms={overhead:.2}");
}
