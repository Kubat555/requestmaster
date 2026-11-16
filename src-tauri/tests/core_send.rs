//! `AppCore` without a window: sending, cancelling, the response body. The server is `wiremock` on
//! `127.0.0.1`, the store lives in memory unless a test restarts the core; all Postman JSON is synthetic.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use requestmaster_lib::dto::{ImportFile, ProxyDto, SettingsDto};
use requestmaster_lib::{AppCore, CoreConfig, ACTIVE_ENVIRONMENT_KEY, SETTINGS_KEY};
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
    AppCore::new(CoreConfig { db_path, bodies_dir: bodies_dir(dir), history_keep: None }).unwrap()
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

    let result = core.send_request("send-1", &id_by_name(&core, "Scripted"), None).await.unwrap();
    let result = as_json(&result);
    assert_eq!(result["error"]["kind"], "connect");
    assert!(!result["error"]["message"].as_str().unwrap().is_empty());
    assert_eq!(result["error"]["fields"], json!([]));
    assert_eq!(result["warnings"][0]["kind"], "scripts_not_executed");
    assert_eq!(result["warnings"][0]["items"], json!(["Scripted"]));
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
    core.save_settings(SettingsDto { timeout_ms: 200, verify_tls: true, proxy: ProxyDto::System }).unwrap();

    let started = Instant::now();
    let result = as_json(&core.send_request("send-1", &id, None).await.unwrap());
    assert!(started.elapsed() < Duration::from_millis(1500), "took {:?}", started.elapsed());
    assert_eq!(result["error"]["kind"], "timeout");
    assert!(result["response"].is_null());
    assert_eq!(files_in(&bodies_dir(&dir)), 0);

    // the setting works from the next send on, without a restart: no limit again — the response arrives
    core.save_settings(SettingsDto { timeout_ms: 0, verify_tls: true, proxy: ProxyDto::System }).unwrap();
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

    core.save_settings(SettingsDto { timeout_ms: 0, verify_tls: true, proxy: ProxyDto::Manual { url: proxy.uri() } }).unwrap();
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
