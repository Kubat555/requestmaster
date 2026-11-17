//! Measurements of section 10 of the spec: they print numbers and assert nothing about time. Every test is
//! ignored in the usual run; run them by hand and only in a release build (a debug build is several times
//! slower and cannot be compared with the goal of the spec):
//!
//! `cargo test --release -p requestmaster --test core_perf -- --ignored --nocapture --test-threads=1`
//!
//! The server is `wiremock` on `127.0.0.1`; all Postman JSON is synthetic.

#[path = "common/big_collection.rs"]
mod big_collection;

use std::path::Path;
use std::time::Instant;

use requestmaster_lib::dto::ImportFile;
use requestmaster_lib::{AppCore, CoreConfig};
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use big_collection::big_collection;

/// How many sends the overhead is measured over, after the warm-up one.
const SENDS: usize = 30;

/// A core over a database file in `dir`, as the application has it.
fn core_in(dir: &TempDir) -> AppCore {
    // Keeps the sends independent of the machine's system proxy.
    std::env::set_var("NO_PROXY", "127.0.0.1");
    AppCore::new(CoreConfig {
        db_path: Some(dir.path().join("app.db")),
        bodies_dir: dir.path().join("bodies"),
        history_keep: None,
        measure_file: None,
    })
    .unwrap()
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// Sends the request and returns what the application adds to the time of the response: the time around
/// `send_request` minus `total_ms` of the response.
async fn overhead_ms(core: &AppCore, send_id: &str, request_id: &str) -> f64 {
    let start = Instant::now();
    let result = core.send_request(send_id, request_id, None).await.unwrap();
    let around = ms_since(start);
    around - result.response.expect("the server answers").total_ms
}

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
