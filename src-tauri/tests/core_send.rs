//! `AppCore` without a window: sending, cancelling, the response body. The server is `wiremock` on
//! `127.0.0.1`, the store lives in memory unless a test restarts the core; all Postman JSON is synthetic.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use requestmaster_lib::dto::{ImportFile, MotionLevel, ProxyDto, SettingsDto};
use requestmaster_lib::{AppCore, CoreConfig, ACTIVE_ENVIRONMENT_KEY, SETTINGS_KEY};
use rqm_model::NodeKind;
use serde_json::{json, Value};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SCHEMA_V21: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";
/// Nothing listens on port 1 of the local machine: a connection is refused at once.
const CLOSED_PORT: &str = "http://127.0.0.1:1";
const JSON_BODY: &str = "{\"ok\":true,\"list\":[1,2]}";
const JSON_BODY_PRETTY: &str = "{\n  \"ok\": true,\n  \"list\": [\n    1,\n    2\n  ]\n}";

/// Keeps the sends independent of the machine's system proxy. Every test sets the same value, so the
/// tests running in parallel do not disturb each other.
fn bypass_system_proxy() {
    std::env::set_var("NO_PROXY", "127.0.0.1");
}

/// A core over the database at `db_path` (`None` = in memory) with its bodies in `<dir>/bodies`.
fn core_at(dir: &TempDir, db_path: Option<PathBuf>) -> AppCore {
    bypass_system_proxy();
    AppCore::new(CoreConfig { db_path, bodies_dir: bodies_dir(dir), history_keep: None, measure_file: None }).unwrap()
}

/// A core over an in-memory store; the directory must outlive the core.
fn core() -> (AppCore, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let core = core_at(&dir, None);
    (core, dir)
}

fn bodies_dir(dir: &TempDir) -> PathBuf {
    dir.path().join("bodies")
}

fn files_in(dir: &Path) -> usize {
    std::fs::read_dir(dir).unwrap().count()
}

/// Imports a collection with these Postman items and returns nothing: requests are found by name.
fn import_items(core: &AppCore, items: Value) {
    let collection = json!({ "info": { "name": "Synthetic", "schema": SCHEMA_V21 }, "item": items });
    let results = core.import_files(vec![ImportFile { name: "c.json".to_string(), text: collection.to_string() }]);
    assert_eq!(results[0].error, None);
}

/// A Postman item: a `GET` request to `url`.
fn get_item(name: &str, url: &str) -> Value {
    json!({ "name": name, "request": { "method": "GET", "url": { "raw": url } } })
}

fn id_by_name(core: &AppCore, name: &str) -> String {
    core.tree().unwrap().into_iter().find(|row| row.name == name).unwrap().id
}

fn as_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

/// A server that answers `GET /x` with `200`, a JSON body and one cookie.
async fn json_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "sid=1; Path=/").set_body_string(JSON_BODY))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn send_returns_response_and_body() {
    let server = json_server().await;
    let (core, dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));

    let result = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert!(result.warnings.is_empty());
    let response = result.response.unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.json);
    assert!(!response.body.truncated);
    assert_eq!(response.body.total, JSON_BODY.len() as u64);
    assert_eq!(response.cookies.len(), 1);
    assert_eq!(response.cookies[0].name, "sid");
    assert_eq!(response.sent.method, "GET");
    assert!(response.total_ms >= response.ttfb_ms && response.ttfb_ms > 0.0);

    let pretty = core.response_body(&response.body.id, true).unwrap();
    assert_eq!(String::from_utf8(pretty).unwrap(), JSON_BODY_PRETTY);
    let raw = core.response_body(&response.body.id, false).unwrap();
    assert_eq!(raw, JSON_BODY.as_bytes());

    assert_eq!(files_in(&bodies_dir(&dir)), 1);
    core.release_body(&response.body.id);
    assert_eq!(files_in(&bodies_dir(&dir)), 0, "the body file is deleted");
    assert_eq!(core.response_body(&response.body.id, false), Err("response body is gone".to_string()));
    // releasing again, or an id that never existed, is not an error
    core.release_body(&response.body.id);
    core.release_body("never-existed");
}

#[tokio::test]
async fn draft_is_sent_instead_of_saved() {
    let server = json_server().await;
    let (core, _dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{CLOSED_PORT}/saved"))]));
    let id = id_by_name(&core, "Get");

    // the draft is the content the editor holds: what `get_node` returned, with the address changed
    let mut draft = core.get_node(&id).unwrap().content;
    draft["url"] = json!(format!("{}/x", server.uri()));

    let result = core.send_request("send-1", &id, Some(draft)).await.unwrap();
    assert_eq!(result.response.unwrap().status, 200);
    // the stored request is untouched
    assert_eq!(core.get_node(&id).unwrap().content["url"], format!("{CLOSED_PORT}/saved"));
}

#[tokio::test]
async fn environment_variables_are_applied() {
    let server = json_server().await;
    let (core, _dir) = core();
    import_items(&core, json!([get_item("Get", "{{base}}/x")]));
    let environment = json!({
        "name": "Local",
        "values": [{ "key": "base", "value": server.uri(), "enabled": true }],
        "_postman_variable_scope": "environment"
    });
    let results = core.import_files(vec![ImportFile { name: "e.json".to_string(), text: environment.to_string() }]);
    assert_eq!(results[0].error, None);
    let environment_id = core.environments().unwrap().remove(0).id;
    core.set_active_environment(Some(&environment_id)).unwrap();

    let result = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap();
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(result.response.unwrap().status, 200);
    assert_eq!(server.received_requests().await.unwrap().len(), 1, "the request reached the server");
}

#[tokio::test]
async fn warnings_and_network_error() {
    let (core, dir) = core();
    let mut item = get_item("Scripted", &format!("{CLOSED_PORT}/x"));
    item["event"] = json!([{ "listen": "prerequest", "script": { "type": "text/javascript", "exec": ["var a = 1;"] } }]);
    import_items(&core, json!([item]));
    // the user forbade the scripts of this collection
    core.set_scripts_allowed(&id_by_name(&core, "Synthetic"), false).unwrap();

    let result = core.send_request("send-1", &id_by_name(&core, "Scripted"), None).await.unwrap();
    let result = as_json(&result);
    assert_eq!(result["error"]["kind"], "connect");
    assert!(!result["error"]["message"].as_str().unwrap().is_empty());
    assert_eq!(result["error"]["fields"], json!([]));
    assert_eq!(result["warnings"][0]["kind"], "scripts_not_allowed");
    assert_eq!(result["warnings"][0]["items"], json!(["Synthetic"]));
    assert!(result["response"].is_null());
    assert_eq!(files_in(&bodies_dir(&dir)), 0);
}

#[tokio::test]
async fn cancel_stops_a_slow_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let (core, dir) = core();
    import_items(&core, json!([get_item("Slow", &format!("{}/slow", server.uri()))]));
    let id = id_by_name(&core, "Slow");

    let started = Instant::now();
    let (result, ()) = tokio::join!(core.send_request("send-1", &id, None), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        core.cancel_send("send-1");
    });
    assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
    let result = as_json(&result.unwrap());
    assert_eq!(result["error"]["kind"], "cancelled");
    assert!(result["response"].is_null());
    assert_eq!(files_in(&bodies_dir(&dir)), 0);

    // cancelling again, or a send that never existed, is not an error
    core.cancel_send("send-1");
    core.cancel_send("never-existed");
}

#[tokio::test]
async fn unconfirmed_file_blocks_send() {
    let (core, _dir) = core();
    let item = json!({
        "name": "Upload",
        "request": {
            "method": "POST",
            "url": { "raw": format!("{CLOSED_PORT}/upload") },
            "body": { "mode": "formdata", "formdata": [
                { "key": "note", "type": "text", "value": "hello" },
                { "key": "attachment", "type": "file", "src": "/synthetic/file.bin" }
            ] }
        }
    });
    import_items(&core, json!([item]));

    let result = core.send_request("send-1", &id_by_name(&core, "Upload"), None).await.unwrap();
    let result = as_json(&result);
    assert_eq!(result["error"]["kind"], "unconfirmed_files");
    assert_eq!(result["error"]["fields"], json!(["attachment"]));
    assert!(result["response"].is_null());
}

/// The editor of the window writes every key of a part out: `content_type: null`, `enabled`, `postman_extra`.
#[tokio::test]
async fn form_data_from_the_window_is_sent() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/upload")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let (core, dir) = core();
    let item = json!({ "name": "Upload", "request": { "method": "POST", "url": { "raw": format!("{}/upload", server.uri()) } } });
    import_items(&core, json!([item]));
    let id = id_by_name(&core, "Upload");
    let file = dir.path().join("picked.txt");
    std::fs::write(&file, "synthetic file content").unwrap();

    let mut draft = core.get_node(&id).unwrap().content;
    draft["body"] = json!({ "mode": "form_data", "data": [
        { "key": "note", "value": { "text": "hello" }, "enabled": true, "content_type": null, "postman_extra": {} },
        { "key": "attachment", "value": { "files": [{ "path": file.to_str().unwrap(), "confirmed": true }] },
          "enabled": true, "content_type": null, "postman_extra": {} }
    ] });

    let result = core.send_request("send-1", &id, Some(draft)).await.unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.response.unwrap().status, 200);

    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    let content_type = received[0].headers.get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.starts_with("multipart/form-data; boundary="), "{content_type}");
    let body = String::from_utf8_lossy(&received[0].body);
    assert!(body.contains("name=\"note\""), "{body}");
    assert!(body.contains("hello"), "{body}");
    assert!(body.contains("name=\"attachment\""), "{body}");
    assert!(body.contains("filename=\"picked.txt\""), "{body}");
    assert!(body.contains("synthetic file content"), "{body}");
}

#[tokio::test]
async fn cookies_survive_restart() {
    let server = json_server().await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("requestmaster.db");

    let first = core_at(&dir, Some(db_path.clone()));
    import_items(&first, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let id = id_by_name(&first, "Get");
    let result = first.send_request("send-1", &id, None).await.unwrap();
    assert_eq!(result.response.unwrap().cookies.len(), 1);
    drop(first);

    let second = core_at(&dir, Some(db_path));
    let result = second.send_request("send-2", &id, None).await.unwrap();
    assert_eq!(result.response.unwrap().status, 200);

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].headers.get("cookie").is_none(), "the first request has no cookie yet");
    assert_eq!(requests[1].headers.get("cookie").unwrap(), "sid=1");
}

#[tokio::test]
async fn bad_draft_and_unknown_request_are_errors() {
    let (core, dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{CLOSED_PORT}/x"))]));
    let id = id_by_name(&core, "Get");

    assert!(core.send_request("send-1", &id, Some(json!(5))).await.is_err());
    assert!(core.send_request("send-2", "no-such-request", None).await.is_err());
    // a node that is not a request cannot be sent either
    assert!(core.send_request("send-3", &id_by_name(&core, "Synthetic"), None).await.is_err());
    assert_eq!(files_in(&bodies_dir(&dir)), 0);
}

/// Writes a value of `app_state` behind the back of the core.
fn raw_state_set(db_path: &Path, key: &str, value: &str) {
    rusqlite::Connection::open(db_path)
        .unwrap()
        .execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )
        .unwrap();
}

#[tokio::test]
async fn saved_timeout_stops_a_slow_server() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .mount(&server)
        .await;
    let (core, dir) = core();
    import_items(&core, json!([get_item("Slow", &format!("{}/slow", server.uri()))]));
    let id = id_by_name(&core, "Slow");
    core.save_settings(SettingsDto { timeout_ms: 200, verify_tls: true, proxy: ProxyDto::System, motion: MotionLevel::Normal }).unwrap();

    let started = Instant::now();
    let result = as_json(&core.send_request("send-1", &id, None).await.unwrap());
    assert!(started.elapsed() < Duration::from_millis(1500), "took {:?}", started.elapsed());
    assert_eq!(result["error"]["kind"], "timeout");
    assert!(result["response"].is_null());
    assert_eq!(files_in(&bodies_dir(&dir)), 0);

    // the setting works from the next send on, without a restart: no limit again — the response arrives
    core.save_settings(SettingsDto { timeout_ms: 0, verify_tls: true, proxy: ProxyDto::System, motion: MotionLevel::Normal }).unwrap();
    let result = core.send_request("send-2", &id, None).await.unwrap();
    assert_eq!(result.response.unwrap().status, 200);
}

#[tokio::test]
async fn saved_manual_proxy_is_used() {
    // the "proxy" is the test server itself: it answers whatever address the request names
    let proxy = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_string("via proxy")).mount(&proxy).await;
    let (core, _dir) = core();
    import_items(&core, json!([get_item("Far", "http://example.invalid/x")]));
    let id = id_by_name(&core, "Far");

    core.save_settings(SettingsDto { timeout_ms: 0, verify_tls: true, proxy: ProxyDto::Manual { url: proxy.uri() }, motion: MotionLevel::Normal }).unwrap();
    let result = core.send_request("send-1", &id, None).await.unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    let response = result.response.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(core.response_body(&response.body.id, false).unwrap(), b"via proxy");
    assert_eq!(proxy.received_requests().await.unwrap().len(), 1, "the request went to the proxy");
}

#[tokio::test]
async fn broken_settings_do_not_stop_a_send() {
    let server = json_server().await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("requestmaster.db");
    let core = core_at(&dir, Some(db_path.clone()));
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let id = id_by_name(&core, "Get");

    for (number, broken) in ["{broken", "[]", r#"{ "timeout_ms": "soon" }"#].into_iter().enumerate() {
        raw_state_set(&db_path, SETTINGS_KEY, broken);
        let result = core.send_request(&format!("send-{number}"), &id, None).await.unwrap();
        assert!(result.error.is_none(), "{broken}: {:?}", result.error);
        assert_eq!(result.response.unwrap().status, 200, "{broken}");
    }
}

#[tokio::test]
async fn unreadable_active_environment_counts_as_none() {
    let server = json_server().await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("requestmaster.db");
    let core = core_at(&dir, Some(db_path.clone()));
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let id = id_by_name(&core, "Get");

    for (number, broken) in ["{broken", "", "5", "[]"].into_iter().enumerate() {
        raw_state_set(&db_path, ACTIVE_ENVIRONMENT_KEY, broken);
        assert_eq!(core.active_environment(), Ok(None), "{broken}");
        let result = core.send_request(&format!("send-{number}"), &id, None).await.unwrap();
        assert_eq!(result.response.unwrap().status, 200, "{broken}");
    }
}

/// A server that answers `GET /x` with `200` and this body under this `Content-Type`.
async fn typed_server(body: Vec<u8>, content_type: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, content_type))
        .mount(&server)
        .await;
    server
}

/// The only file in the bodies folder: the body of the one response the core holds.
fn only_body_file(dir: &TempDir) -> PathBuf {
    let mut files: Vec<PathBuf> = std::fs::read_dir(bodies_dir(dir)).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(files.len(), 1);
    files.remove(0)
}

#[tokio::test]
async fn response_body_is_saved_as_received() {
    let server = json_server().await;
    let (core, dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let response = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap().response.unwrap();
    assert!(response.body.json);

    let target = dir.path().join("saved.json");
    assert_eq!(core.save_response_body(&response.body.id, &target), Ok(()));
    // the bytes of the server, not the re-indented view
    assert_eq!(std::fs::read(&target).unwrap(), JSON_BODY.as_bytes());
    // the body stays with the core: it is still shown and can be saved again, over the same file too
    assert_eq!(core.response_body(&response.body.id, false).unwrap(), JSON_BODY.as_bytes());
    assert_eq!(core.save_response_body(&response.body.id, &target), Ok(()));
    assert_eq!(std::fs::read(&target).unwrap(), JSON_BODY.as_bytes());

    // a place that cannot be written is an error with the reason
    let nowhere = dir.path().join("no-such-folder").join("saved.json");
    let error = core.save_response_body(&response.body.id, &nowhere).unwrap_err();
    assert!(error.starts_with("cannot write the file: "), "{error}");
}

#[tokio::test]
async fn body_above_the_inline_limit_is_saved_whole() {
    const LENGTH: usize = 22_020_096; // 21 MiB
    let body: Vec<u8> = (0..LENGTH).map(|index| b'a' + (index % 26) as u8).collect();
    let server = typed_server(body.clone(), "application/octet-stream").await;
    let (core, dir) = core();
    import_items(&core, json!([get_item("Big", &format!("{}/x", server.uri()))]));
    let response = core.send_request("send-1", &id_by_name(&core, "Big"), None).await.unwrap().response.unwrap();
    assert!(response.body.truncated);

    let target = dir.path().join("saved.bin");
    assert_eq!(core.save_response_body(&response.body.id, &target), Ok(()));
    assert_eq!(std::fs::metadata(&target).unwrap().len(), 22_020_096);
    assert!(std::fs::read(&target).unwrap() == body, "the saved file differs from the body of the server");
}

#[tokio::test]
async fn saving_a_released_body_is_an_error() {
    let server = json_server().await;
    let (core, dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let response = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap().response.unwrap();
    core.release_body(&response.body.id);

    let target = dir.path().join("saved.json");
    assert_eq!(core.save_response_body(&response.body.id, &target), Err("response body is gone".to_string()));
    assert!(!target.exists(), "nothing is written");
    assert_eq!(core.response_file_name(&response.body.id), Err("response body is gone".to_string()));
    assert_eq!(core.save_response_body("never-existed", &target), Err("response body is gone".to_string()));
    assert_eq!(core.response_file_name("never-existed"), Err("response body is gone".to_string()));
}

#[tokio::test]
async fn saving_onto_the_body_file_itself_is_refused_and_keeps_it() {
    let server = json_server().await;
    let (core, dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let response = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap().response.unwrap();
    let body_file = only_body_file(&dir);

    let refused = Err("cannot save the response onto itself".to_string());
    assert_eq!(core.save_response_body(&response.body.id, &body_file), refused);
    // the same file under another spelling of its path
    let name = body_file.file_name().unwrap();
    let respelled = bodies_dir(&dir).join("..").join("bodies").join(name);
    assert_eq!(core.save_response_body(&response.body.id, &respelled), refused);

    assert_eq!(std::fs::read(&body_file).unwrap(), JSON_BODY.as_bytes(), "the body file is intact");
    assert_eq!(core.response_body(&response.body.id, false).unwrap(), JSON_BODY.as_bytes());
}

#[tokio::test]
async fn response_file_name_follows_the_content_type() {
    let cases: [(&[u8], &str, &str); 6] = [
        (b"<html></html>", "text/html; charset=utf-8", "response.html"),
        (b"<a/>", "application/xml", "response.xml"),
        (b"hello", "text/plain", "response.txt"),
        (b"\x89PNG", "image/png", "response.bin"),
        (b"{\"title\":\"x\"}", "application/problem+json", "response.json"),
        // a JSON body wins over the type the server named
        (JSON_BODY.as_bytes(), "text/plain", "response.json"),
    ];
    let (core, _dir) = core();
    for (number, (body, content_type, file_name)) in cases.into_iter().enumerate() {
        let server = typed_server(body.to_vec(), content_type).await;
        let name = format!("Get {number}");
        import_items(&core, json!([get_item(&name, &format!("{}/x", server.uri()))]));
        let result = core.send_request(&format!("send-{number}"), &id_by_name(&core, &name), None).await.unwrap();
        let response = result.response.unwrap();
        assert_eq!(core.response_file_name(&response.body.id), Ok(file_name.to_string()), "{content_type}");
    }
}

/// A collection created in the application — its scripts may run — with the request `R`: a `GET` to `url`
/// with these scripts. Returns the ids of the collection and of the request.
fn scripted(core: &AppCore, url: &str, pre_request: &str, test: &str) -> (String, String) {
    let collection = core.create_node(None, NodeKind::Collection, "Made here").unwrap();
    let request = core.create_node(Some(&collection), NodeKind::Request, "R").unwrap();
    let mut content = core.get_node(&request).unwrap().content;
    content["url"] = json!(url);
    content["scripts"] = json!({ "pre_request": pre_request, "test": test });
    core.save_node_content(&request, content).unwrap();
    (collection, request)
}

/// A server that answers `GET /x` with `{"t":"T"}` after `delay`.
async fn token_server(delay: Duration) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_string("{\"t\":\"T\"}"))
        .mount(&server)
        .await;
    server
}

/// The enabled variables of an environment (`globals` — of the globals set) as `key = value`.
fn variables_of(core: &AppCore, environment_id: &str) -> Vec<String> {
    let variables = core.get_environment(environment_id).unwrap().variables;
    variables.into_iter().map(|variable| format!("{} = {}", variable.key, variable.value)).collect()
}

/// Renames a table behind the back of the core: from then on the core cannot read or write it.
fn break_table(db_path: &Path, table: &str) {
    rusqlite::Connection::open(db_path)
        .unwrap()
        .execute_batch(&format!("ALTER TABLE {table} RENAME TO {table}_gone"))
        .unwrap();
}

#[tokio::test]
async fn scripts_run_and_their_results_come_back() {
    let server = token_server(Duration::ZERO).await;
    let (core, _dir) = core();
    let (collection, request) = scripted(
        &core,
        &format!("{}/x", server.uri()),
        "pm.environment.set('stage', 'pre'); console.log('hi')",
        "pm.test('ok', () => {}); pm.collectionVariables.set('token', JSON.parse(responseBody).t)",
    );
    let environment = core.create_environment("Local").unwrap();
    core.set_active_environment(Some(&environment)).unwrap();

    let result = as_json(&core.send_request("send-1", &request, None).await.unwrap());
    let mut keys: Vec<&str> = result.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["console", "console_truncated", "error", "response", "script_errors", "tests", "variable_changes", "warnings"]
    );
    assert!(result["error"].is_null(), "{}", result["error"]);
    assert_eq!(result["response"]["status"], 200);
    assert_eq!(result["warnings"], json!([]));
    assert_eq!(result["script_errors"], json!([]));
    assert_eq!(result["console_truncated"], false);

    assert_eq!(
        result["tests"],
        json!([{
            "source": { "node_id": request, "node_name": "R", "node_kind": "request", "phase": "test" },
            "name": "ok", "passed": true, "skipped": false, "error": null
        }])
    );
    let console = result["console"].as_array().unwrap();
    let hi = console.iter().find(|line| line["text"] == "hi").expect("the line of the script");
    assert_eq!(hi["level"], "log");
    assert_eq!(hi["source"]["phase"], "pre_request");
    assert!(hi["at_ms"].is_u64());
    let network = console.iter().find(|line| line["level"] == "network").expect("the line of the request");
    assert!(network["source"].is_null());

    assert_eq!(
        result["variable_changes"],
        json!([
            { "scope": "environment", "target_id": environment, "op": "set", "key": "stage", "value": "pre" },
            { "scope": "collection", "target_id": collection, "op": "set", "key": "token", "value": "T" }
        ])
    );
    // and the store holds them
    assert_eq!(variables_of(&core, &environment), ["stage = pre"]);
    let variables = &core.get_node(&collection).unwrap().content["variables"];
    assert_eq!(variables[0]["key"], "token");
    assert_eq!(variables[0]["value"], "T");
}

/// Scripts are allowed unless the user forbids them for a collection: an imported collection runs its
/// scripts at once.
#[tokio::test]
async fn imported_collection_runs_scripts_until_forbidden() {
    let server = token_server(Duration::ZERO).await;
    let (core, _dir) = core();
    let mut item = get_item("Get", &format!("{}/x", server.uri()));
    let script = "pm.test('ran', () => {}); pm.globals.set('seen', 'yes');";
    item["event"] = json!([{ "listen": "test", "script": { "type": "text/javascript", "exec": [script] } }]);
    import_items(&core, json!([item]));
    let (collection, request) = (id_by_name(&core, "Synthetic"), id_by_name(&core, "Get"));

    assert_eq!(core.scripts_allowed(&collection), Ok(true));
    let result = as_json(&core.send_request("send-1", &request, None).await.unwrap());
    assert_eq!(result["warnings"], json!([]));
    assert_eq!(result["tests"].as_array().unwrap().len(), 1);
    assert_eq!(variables_of(&core, "globals"), ["seen = yes"]);

    // forbidden through any node of the collection: not a line of its scripts runs
    core.set_scripts_allowed(&request, false).unwrap();
    assert_eq!(core.scripts_allowed(&collection), Ok(false));
    core.save_environment("globals", "", Vec::new()).unwrap();
    let result = as_json(&core.send_request("send-2", &request, None).await.unwrap());
    assert_eq!(result["response"]["status"], 200);
    assert_eq!(result["warnings"][0], json!({ "kind": "scripts_not_allowed", "items": ["Synthetic"] }));
    assert_eq!(result["tests"], json!([]));
    assert_eq!(result["console"], json!([]));
    assert_eq!(result["variable_changes"], json!([]));
    assert!(variables_of(&core, "globals").is_empty());

    core.set_scripts_allowed(&request, true).unwrap();
    assert_eq!(core.scripts_allowed(&request), Ok(true));
    let result = as_json(&core.send_request("send-3", &request, None).await.unwrap());
    assert_eq!(result["warnings"], json!([]));
    assert_eq!(variables_of(&core, "globals"), ["seen = yes"]);
}

#[tokio::test]
async fn pre_request_error_is_a_script_error() {
    let (core, _dir) = core();
    let (_, request) = scripted(&core, &format!("{CLOSED_PORT}/x"), "const a = 1;\nthrow new Error('nope');", "");

    let result = as_json(&core.send_request("send-1", &request, None).await.unwrap());
    assert_eq!(result["error"]["kind"], "script");
    assert_eq!(result["error"]["message"], "Error: nope");
    assert_eq!(result["error"]["fields"], json!([]));
    assert!(result["response"].is_null());
    assert_eq!(result["script_errors"][0]["kind"], "exception");
    assert_eq!(result["script_errors"][0]["line"], 2);
    assert_eq!(result["script_errors"][0]["source"]["phase"], "pre_request");

    let item = core.history_list(1, None).unwrap().remove(0);
    let entry = as_json(&core.history_get(item.id).unwrap());
    assert_eq!(entry["error_kind"], "script");
    assert_eq!(entry["error_message"], "Error: nope");
}

#[tokio::test]
async fn skip_request_is_reported() {
    let (core, _dir) = core();
    let (_, request) = scripted(&core, &format!("{CLOSED_PORT}/x"), "pm.execution.skipRequest();", "");

    let result = as_json(&core.send_request("send-1", &request, None).await.unwrap());
    assert_eq!(result["error"]["kind"], "skipped");
    assert_eq!(result["error"]["message"], "");
    assert!(result["response"].is_null());
    assert_eq!(result["script_errors"], json!([]));
    assert_eq!(as_json(&core.history_list(1, None).unwrap())[0]["error_kind"], "skipped");
}

#[tokio::test]
async fn changes_are_saved_when_the_send_fails_or_is_cancelled() {
    let (core, _dir) = core();
    let (_, failing) = scripted(&core, &format!("{CLOSED_PORT}/x"), "pm.globals.set('failed', '1');", "");
    let result = as_json(&core.send_request("send-1", &failing, None).await.unwrap());
    assert_eq!(result["error"]["kind"], "connect");
    let change = json!({ "scope": "global", "target_id": "globals", "op": "set", "key": "failed", "value": "1" });
    assert_eq!(result["variable_changes"], json!([change]));
    assert_eq!(variables_of(&core, "globals"), ["failed = 1"]);

    let waiting = "pm.globals.set('cancelled', '2'); await new Promise((r) => setTimeout(r, 30000));";
    let (_, cancelled) = scripted(&core, &format!("{CLOSED_PORT}/x"), waiting, "");
    let started = Instant::now();
    let (result, ()) = tokio::join!(core.send_request("send-2", &cancelled, None), async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        core.cancel_send("send-2");
    });
    assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    let result = as_json(&result.unwrap());
    assert_eq!(result["error"]["kind"], "cancelled");
    assert_eq!(result["variable_changes"][0]["key"], "cancelled");
    assert_eq!(variables_of(&core, "globals"), ["failed = 1", "cancelled = 2"]);
}

#[tokio::test]
async fn edits_made_during_the_send_are_not_lost() {
    let server = token_server(Duration::from_millis(300)).await;
    let (core, _dir) = core();
    let (_, request) = scripted(&core, &format!("{}/x", server.uri()), "", "pm.environment.set('b', 'script');");
    let environment = core.create_environment("Local").unwrap();
    core.set_active_environment(Some(&environment)).unwrap();

    let (result, ()) = tokio::join!(core.send_request("send-1", &request, None), async {
        // the user edits the environment while the request is on its way
        tokio::time::sleep(Duration::from_millis(100)).await;
        let edited = serde_json::from_value(json!([{ "key": "a", "value": "user" }])).unwrap();
        core.save_environment(&environment, "Local", edited).unwrap();
    });
    assert_eq!(result.unwrap().response.unwrap().status, 200);
    assert_eq!(variables_of(&core, &environment), ["a = user", "b = script"]);
}

#[tokio::test]
async fn storage_failure_while_saving_variables_is_a_warning() {
    let server = token_server(Duration::from_millis(300)).await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("requestmaster.db");
    let core = core_at(&dir, Some(db_path.clone()));
    let (_, request) = scripted(&core, &format!("{}/x", server.uri()), "", "pm.globals.set('g', '1');");

    let (result, ()) = tokio::join!(core.send_request("send-1", &request, None), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        break_table(&db_path, "environment");
    });
    let result = as_json(&result.unwrap());
    assert_eq!(result["response"]["status"], 200, "the response is not lost");
    assert_eq!(result["variable_changes"], json!([]));
    let warnings = result["warnings"].as_array().unwrap();
    let warning = warnings.iter().find(|warning| warning["kind"] == "variables_not_saved").expect("the warning");
    assert!(!warning["items"][0].as_str().unwrap().is_empty());
    // the history entry of the send names it too
    let entry = as_json(&core.history_get(core.history_list(1, None).unwrap()[0].id).unwrap());
    assert_eq!(entry["warnings"][0]["kind"], "variables_not_saved");
}

/// The window must learn what a script wrote even when the command would otherwise fail after the send.
#[tokio::test]
async fn script_changes_come_back_when_the_cookies_cannot_be_saved() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("requestmaster.db");
    let core = core_at(&dir, Some(db_path.clone()));
    let waiting = "pm.globals.set('g', '1'); await new Promise((r) => setTimeout(r, 300));";
    let (_, request) = scripted(&core, &format!("{CLOSED_PORT}/x"), waiting, "");

    let (result, ()) = tokio::join!(core.send_request("send-1", &request, None), async {
        // while the script waits: from now on the cookies cannot be saved
        tokio::time::sleep(Duration::from_millis(100)).await;
        break_table(&db_path, "app_state");
    });
    let result = as_json(&result.expect("the command answers with the changes"));
    assert_eq!(result["error"]["kind"], "connect");
    assert_eq!(
        result["variable_changes"],
        json!([{ "scope": "global", "target_id": "globals", "op": "set", "key": "g", "value": "1" }])
    );
    assert_eq!(variables_of(&core, "globals"), ["g = 1"]);
}

#[tokio::test]
async fn test_results_do_not_bloat_the_history() {
    /// The most of a body a history entry keeps.
    const HISTORY_BODY_LIMIT: usize = 1_048_576;
    let server = token_server(Duration::ZERO).await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("requestmaster.db");
    let core = core_at(&dir, Some(db_path.clone()));
    // twelve results of about 20 KB each stay under the limit of a phase: the send goes on
    let pre = "for (let i = 0; i < 12; i++) pm.test('p' + i, () => { throw new Error('я'.repeat(10000)) })";
    let test = "for (let i = 0; i < 1000; i++) pm.test('t' + i, () => { throw new Error('я'.repeat(10000)) })";
    let (_, request) = scripted(&core, &format!("{}/x", server.uri()), pre, test);

    let result = as_json(&core.send_request("send-1", &request, None).await.unwrap());
    assert_eq!(result["response"]["status"], 200);
    let errors = result["script_errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    let message = errors[0]["message"].as_str().unwrap();
    assert!(message.contains("their results are larger than 256 KB"), "{message}");
    assert!(result["tests"].as_array().unwrap().len() > 12);

    let connection = rusqlite::Connection::open(&db_path).unwrap();
    let stored: i64 = connection
        .query_row("SELECT length(CAST(detail AS BLOB)) FROM history ORDER BY id DESC LIMIT 1", [], |row| row.get(0))
        .unwrap();
    assert!(stored as usize <= 2 * HISTORY_BODY_LIMIT + 1_048_576, "the entry takes {stored} bytes");
}
