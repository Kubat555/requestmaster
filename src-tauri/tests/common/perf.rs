//! What the measurements of `core_perf.rs` and `core_perf_cold.rs` share.

use std::time::Instant;

use requestmaster_lib::dto::SendResult;
use requestmaster_lib::{AppCore, CoreConfig};
use rqm_model::NodeKind;
use serde_json::{json, Value};
use tempfile::TempDir;

/// A core over a database file in `dir`, as the application has it.
pub fn core_in(dir: &TempDir) -> AppCore {
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

pub fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// Sends the request and returns what the application adds to the time of the response — the time around
/// `send_request` minus `total_ms` of the response — and the result of the send.
pub async fn measured_send(core: &AppCore, send_id: &str, request_id: &str) -> (f64, SendResult) {
    let start = Instant::now();
    let result = core.send_request(send_id, request_id, None).await.unwrap();
    let around = ms_since(start);
    let total_ms = result.response.as_ref().expect("the server answers").total_ms;
    (around - total_ms, result)
}

/// `measured_send` without the result.
// Not every test binary this module is part of calls it.
#[allow(dead_code)]
pub async fn overhead_ms(core: &AppCore, send_id: &str, request_id: &str) -> f64 {
    measured_send(core, send_id, request_id).await.0
}

/// A collection created in the application — its scripts may run — with one request, a `GET` to `url`.
/// `collection_scripts` and `request_scripts` are the `scripts` of their contents: `{ "pre_request", "test" }`.
/// Returns the id of the request.
pub fn scripted_request(core: &AppCore, url: &str, collection_scripts: Value, request_scripts: Value) -> String {
    let collection = core.create_node(None, NodeKind::Collection, "Measured").unwrap();
    let mut content = core.get_node(&collection).unwrap().content;
    content["scripts"] = collection_scripts;
    core.save_node_content(&collection, content).unwrap();

    let request = core.create_node(Some(&collection), NodeKind::Request, "R").unwrap();
    let mut content = core.get_node(&request).unwrap().content;
    content["url"] = json!(url);
    content["scripts"] = request_scripts;
    core.save_node_content(&request, content).unwrap();
    request
}

/// The body the server gives the case `typical`.
pub const TYPICAL_BODY: &str = r#"{"token":"abc","items":[1,2,3]}"#;
/// The pre-request script of the collection and the test script of the request of the case `typical`.
pub const TYPICAL_PRE_REQUEST: &str = r#"pm.variables.set("started", "1");"#;
pub const TYPICAL_TEST: &str = r#"
pm.test("status is 200", function () { pm.expect(pm.response.code).to.equal(200); });
pm.test("has a token", function () { pm.expect(pm.response.json().token).to.be.a("string"); });
pm.test("has items", function () { pm.expect(pm.response.json().items).to.have.lengthOf(3); });
"#;
