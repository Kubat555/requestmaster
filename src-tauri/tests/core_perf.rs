//! Measurements of section 10 of the spec: they print numbers and assert nothing about time. Every test is
//! ignored in the usual run; run them by hand and only in a release build (a debug build is several times
//! slower and cannot be compared with the goal of the spec):
//!
//! `cargo test --release -p requestmaster --test core_perf -- --ignored --nocapture --test-threads=1`
//!
//! The server is `wiremock` on `127.0.0.1`; all Postman JSON is synthetic.

#[path = "common/big_collection.rs"]
mod big_collection;
#[path = "common/perf.rs"]
mod perf;

use std::path::Path;
use std::time::Instant;

use requestmaster_lib::dto::ImportFile;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use big_collection::big_collection;
use perf::{
    core_in, measured_send, ms_since, overhead_ms, scripted_request, TYPICAL_BODY, TYPICAL_PRE_REQUEST,
    TYPICAL_TEST,
};

/// How many sends the overhead is measured over, after the warm-up one.
const SENDS: usize = 30;

#[tokio::test]
#[ignore = "a measurement: run by hand in a release build"]
async fn request_overhead_report() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"ok\":true}"))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let collection = json!({
        "info": {
            "name": "Synthetic",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": [{ "name": "Get", "request": { "method": "GET", "url": { "raw": format!("{}/x", server.uri()) } } }]
    });
    let results = core.import_files(vec![ImportFile { name: "c.json".to_string(), text: collection.to_string() }]);
    assert_eq!(results[0].error, None);
    let request_id = core.tree().unwrap().into_iter().find(|row| row.name == "Get").unwrap().id;

    // The first send opens the connection and warms everything up; it is not part of the sample.
    let first = overhead_ms(&core, "warm-up", &request_id).await;
    println!("first_request_overhead_ms={first:.2}");

    let mut overheads = Vec::with_capacity(SENDS);
    for index in 0..SENDS {
        overheads.push(overhead_ms(&core, &format!("send-{index}"), &request_id).await);
    }
    overheads.sort_by(f64::total_cmp);
    let median = (overheads[SENDS / 2 - 1] + overheads[SENDS / 2]) / 2.0;
    let max = overheads[SENDS - 1];
    println!("request_overhead_ms median={median:.2} max={max:.2} n={SENDS}");
}

/// One case of `script_overhead_report`.
struct ScriptCase {
    name: &'static str,
    /// Path on the server.
    path: &'static str,
    /// The `scripts` of the contents of the collection and of the request.
    collection_scripts: Value,
    request_scripts: Value,
    /// How many passed `pm.test` every send has to bring.
    tests: usize,
    /// How many changes of stored variables every send has to bring.
    variable_changes: usize,
}

/// The overhead of a send with scripts: the warm-up send (`first`), then `SENDS` measured ones. The numbers
/// include the writing of the variables and of the history. The bytecode of the prelude and of chai is
/// compiled once per process and the cases run one after another, so `first` of the cases after the first
/// one with a script is not a cold start — `core_perf_cold.rs` measures that.
#[tokio::test]
#[ignore = "a measurement: run by hand in a release build"]
async fn script_overhead_report() {
    let server = MockServer::start().await;
    let big = json!({ "token": "abc", "filler": "x".repeat(15 * 1024) }).to_string();
    for (at, body) in [("/x", TYPICAL_BODY.to_string()), ("/big", big)] {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }
    let none = || json!({});
    let blank = || json!({ "pre_request": "  \n\t ", "test": " \n " });
    let cases = [
        ScriptCase {
            name: "no_scripts",
            path: "/x",
            collection_scripts: none(),
            request_scripts: none(),
            tests: 0,
            variable_changes: 0,
        },
        ScriptCase {
            name: "blank_scripts",
            path: "/x",
            collection_scripts: blank(),
            request_scripts: blank(),
            tests: 0,
            variable_changes: 0,
        },
        ScriptCase {
            name: "owner_like",
            path: "/big",
            collection_scripts: none(),
            request_scripts: json!({
                "test": r#"var d = JSON.parse(responseBody); pm.collectionVariables.set("token", d.token);"#
            }),
            tests: 0,
            variable_changes: 1,
        },
        ScriptCase {
            name: "typical",
            path: "/x",
            collection_scripts: json!({ "pre_request": TYPICAL_PRE_REQUEST }),
            request_scripts: json!({ "test": TYPICAL_TEST }),
            tests: 3,
            variable_changes: 0,
        },
    ];

    for case in cases {
        // A database and a core of its own: a case does not see what the ones before it stored.
        let dir = tempfile::tempdir().unwrap();
        let core = core_in(&dir);
        let url = format!("{}{}", server.uri(), case.path);
        let request_id = scripted_request(&core, &url, case.collection_scripts, case.request_scripts);

        let mut overheads = Vec::with_capacity(SENDS);
        let mut first = 0.0;
        for index in 0..=SENDS {
            let (overhead, result) = measured_send(&core, &format!("send-{index}"), &request_id).await;
            assert_eq!(result.script_errors, vec![], "{}", case.name);
            assert_eq!(result.warnings, vec![], "{}", case.name);
            assert_eq!(result.tests.iter().filter(|test| test.passed).count(), case.tests, "{}", case.name);
            assert_eq!(result.variable_changes.len(), case.variable_changes, "{}", case.name);
            // The first send warms everything up; it is not part of the sample.
            if index == 0 {
                first = overhead;
            } else {
                overheads.push(overhead);
            }
        }
        overheads.sort_by(f64::total_cmp);
        let median = (overheads[SENDS / 2 - 1] + overheads[SENDS / 2]) / 2.0;
        let max = overheads[SENDS - 1];
        println!("script_overhead_ms case={} first={first:.2} median={median:.2} max={max:.2} n={SENDS}", case.name);
    }
}

#[test]
#[ignore = "a measurement: run by hand in a release build"]
fn tree_of_3000_requests_report() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let text = big_collection(30, 100).to_string();

    let start = Instant::now();
    let results = core.import_files(vec![ImportFile { name: "big.json".to_string(), text }]);
    let import_ms = ms_since(start);
    assert_eq!(results[0].error, None);

    let start = Instant::now();
    let rows = core.tree().unwrap();
    let tree_ms = ms_since(start);
    // The collection, 30 folders and 3000 requests.
    assert_eq!(rows.len(), 3031);

    println!("import_3000_ms={import_ms:.2}");
    println!("tree_3000_ms={tree_ms:.2}");
}

#[test]
#[ignore = "writes a file for the owner: run by hand"]
fn write_big_collection_file() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("target").join("measure");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("collection-3000.postman_collection.json");
    std::fs::write(&file, serde_json::to_string_pretty(&big_collection(30, 100)).unwrap()).unwrap();
    println!("written: {}", file.display());
}

/// Imports a synthetic collection and returns how long `delete_node` of the node called `name` takes.
fn delete_ms(folders: usize, requests_per_folder: usize, name: &str) -> f64 {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    let text = big_collection(folders, requests_per_folder).to_string();
    let results = core.import_files(vec![ImportFile { name: "big.json".to_string(), text }]);
    assert_eq!(results[0].error, None);
    let id = core.tree().unwrap().into_iter().find(|row| row.name == name).unwrap().id;

    let start = Instant::now();
    core.delete_node(&id).unwrap();
    let ms = ms_since(start);
    assert!(core.tree().unwrap().iter().all(|row| row.id != id));
    ms
}

/// Unlike the measurements above, this one is telling in a debug build too: it shows whether the deletion
/// grows with the square of the number of nodes (seconds) or with the number itself (milliseconds).
#[test]
#[ignore = "a measurement: run by hand"]
fn delete_of_3000_requests_report() {
    // One folder with all the requests, then the whole collection of 30 folders.
    println!("delete_folder_3000_ms={:.2}", delete_ms(1, 3000, "Folder 000"));
    println!("delete_collection_3000_ms={:.2}", delete_ms(30, 100, "Synthetic 3000"));
}
