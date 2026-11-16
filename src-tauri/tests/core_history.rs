//! `AppCore` without a window: the history of sends and copying a response body. The server is `wiremock`
//! on `127.0.0.1`; all Postman JSON is synthetic.

use std::path::PathBuf;
use std::time::Duration;

use requestmaster_lib::dto::ImportFile;
use requestmaster_lib::{AppCore, CoreConfig};
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
/// The most of a body the history keeps.
const MIB: usize = 1_048_576;

/// A core with its bodies in `<dir>/bodies`; the sends do not depend on the system proxy of the machine.
fn core_with(dir: &TempDir, db_path: Option<PathBuf>, history_keep: Option<usize>) -> AppCore {
    std::env::set_var("NO_PROXY", "127.0.0.1");
    AppCore::new(CoreConfig { db_path, bodies_dir: dir.path().join("bodies"), history_keep }).unwrap()
}

/// A core over an in-memory store; the directory must outlive the core.
fn core() -> (AppCore, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let core = core_with(&dir, None, None);
    (core, dir)
}

fn import(core: &AppCore, json: &Value) {
    let results = core.import_files(vec![ImportFile { name: "file.json".to_string(), text: json.to_string() }]);
    assert_eq!(results[0].error, None);
}

/// Imports a collection named `Synthetic` with these Postman items; requests are found by name.
fn import_items(core: &AppCore, items: Value) {
    import(core, &json!({ "info": { "name": "Synthetic", "schema": SCHEMA_V21 }, "item": items }));
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

/// The history list as the window receives it, newest first.
fn list(core: &AppCore) -> Vec<Value> {
    as_json(&core.history_list(200, None).unwrap()).as_array().unwrap().clone()
}

/// The newest history entry as the window receives it from `history_get`.
fn newest_entry(core: &AppCore) -> Value {
    let id = core.history_list(1, None).unwrap()[0].id;
    as_json(&core.history_get(id).unwrap())
}

/// A server that answers `GET /x` with these bytes.
async fn server_with_body(body: Vec<u8>) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;
    server
}

/// Sends `GET <server>/x` once and returns the newest history entry.
async fn entry_for_body(body: Vec<u8>) -> Value {
    let server = server_with_body(body).await;
    let (core, _dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let result = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap();
    assert!(result.response.is_some(), "{:?}", result.error);
    newest_entry(&core)
}

#[tokio::test]
async fn successful_send_is_recorded() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/items"))
        .respond_with(ResponseTemplate::new(201).insert_header("x-synthetic", "yes").set_body_string(JSON_BODY))
        .mount(&server)
        .await;
    let (core, _dir) = core();
    assert!(core.history_list(50, None).unwrap().is_empty());
    let url = format!("{}/items", server.uri());
    import_items(
        &core,
        json!([{
            "name": "Create",
            "request": {
                "method": "POST",
                "header": [{ "key": "X-Mine", "value": "1" }],
                "body": { "mode": "raw", "raw": "{\"a\":1}", "options": { "raw": { "language": "json" } } },
                "url": { "raw": url }
            }
        }]),
    );
    let request_id = id_by_name(&core, "Create");

    let result = core.send_request("send-1", &request_id, None).await.unwrap();
    let response = result.response.expect("the history does not take the response away");

    let items = list(&core);
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(
        item.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>(),
        ["id", "sent_at", "node_id", "name", "method", "url", "status", "error_kind", "total_ms", "size"]
    );
    assert!(item["id"].is_i64());
    let sent_at = item["sent_at"].as_str().unwrap();
    assert!(sent_at.len() == 24 && sent_at.ends_with('Z') && sent_at.as_bytes()[10] == b'T', "{sent_at}");
    assert_eq!(item["node_id"], request_id.as_str());
    assert_eq!(item["name"], "Create");
    assert_eq!(item["method"], "POST");
    assert_eq!(item["url"], url.as_str());
    assert_eq!(item["status"], 201);
    assert!(item["error_kind"].is_null());
    assert!(item["total_ms"].as_f64().unwrap() > 0.0);
    assert_eq!(item["size"], JSON_BODY.len());

    let entry = as_json(&core.history_get(item["id"].as_i64().unwrap()).unwrap());
    for (key, value) in item.as_object().unwrap() {
        assert_eq!(&entry[key], value, "the entry repeats `{key}` of the list");
    }
    // the headers as they went to the network: the automatic ones are there too
    let request_headers: Vec<(String, String)> = serde_json::from_value(entry["request_headers"].clone()).unwrap();
    assert!(request_headers.contains(&("X-Mine".to_string(), "1".to_string())), "{request_headers:?}");
    assert!(request_headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("user-agent")), "{request_headers:?}");
    assert_eq!(entry["request_body"], "{\"a\":1}");
    assert_eq!(entry["request_body_truncated"], false);
    let response_headers: Vec<(String, String)> = serde_json::from_value(entry["response_headers"].clone()).unwrap();
    assert!(response_headers.contains(&("x-synthetic".to_string(), "yes".to_string())), "{response_headers:?}");
    assert_eq!(entry["response_body"], JSON_BODY);
    assert_eq!(entry["response_body_truncated"], false);
    assert_eq!(entry["response_body_binary"], false);
    assert!(entry["error_message"].is_null());
    assert_eq!(entry["warnings"], json!([]));
    assert_eq!(entry["node_exists"], true);

    // the body file is still there for the window
    assert_eq!(core.response_body(&response.body.id, false).unwrap(), JSON_BODY.as_bytes());
}

#[tokio::test]
async fn redirected_send_is_recorded_as_it_was_sent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/home"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/home"))
        .respond_with(ResponseTemplate::new(200).set_body_string("welcome"))
        .mount(&server)
        .await;
    let (core, _dir) = core();
    let url = format!("{}/login", server.uri());
    import_items(
        &core,
        json!([{
            "name": "Login",
            "request": {
                "method": "POST",
                "header": [{ "key": "X-Mine", "value": "1" }],
                "body": { "mode": "raw", "raw": "{\"user\":\"synthetic\"}", "options": { "raw": { "language": "json" } } },
                "url": { "raw": url }
            }
        }]),
    );

    let result = core.send_request("send-1", &id_by_name(&core, "Login"), None).await.unwrap();
    assert!(result.response.is_some(), "{:?}", result.error);

    // The entry is the request the user sent, not the last request of the chain (`GET /home`).
    let entry = newest_entry(&core);
    assert_eq!(entry["method"], "POST");
    assert_eq!(entry["url"], url.as_str());
    assert_eq!(entry["status"], 200);
    let request_headers = entry["request_headers"].as_array().unwrap();
    assert!(request_headers.contains(&json!(["X-Mine", "1"])), "{request_headers:?}");
    assert_eq!(entry["request_body"], "{\"user\":\"synthetic\"}");
    assert_eq!(entry["response_body"], "welcome");
}

#[tokio::test]
async fn connection_error_is_recorded_with_the_substituted_address() {
    let (core, _dir) = core();
    let mut item = get_item("Scripted", "{{base}}/x");
    item["event"] = json!([{ "listen": "prerequest", "script": { "type": "text/javascript", "exec": ["var a = 1;"] } }]);
    item["request"]["header"] = json!([{ "key": "X-Mine", "value": "{{base}}" }]);
    import_items(&core, json!([item]));
    let environment = core.create_environment("Local").unwrap();
    let base = serde_json::from_value(json!([{ "key": "base", "value": CLOSED_PORT }])).unwrap();
    core.save_environment(&environment, "Local", base).unwrap();
    core.set_active_environment(Some(&environment)).unwrap();

    let result = core.send_request("send-1", &id_by_name(&core, "Scripted"), None).await.unwrap();
    assert!(result.response.is_none());

    let entry = newest_entry(&core);
    assert_eq!(entry["error_kind"], "connect");
    assert_eq!(entry["method"], "GET");
    assert_eq!(entry["url"], format!("{CLOSED_PORT}/x"));
    assert!(entry["status"].is_null() && entry["total_ms"].is_null() && entry["size"].is_null());
    assert!(!entry["error_message"].as_str().unwrap().is_empty());
    // what was prepared: the headers of the request itself, variables substituted
    assert_eq!(entry["request_headers"], json!([["X-Mine", CLOSED_PORT]]));
    assert!(entry["request_body"].is_null());
    assert_eq!(entry["response_headers"], json!([]));
    assert!(entry["response_body"].is_null());
    assert_eq!(entry["response_body_binary"], false);
    assert_eq!(entry["warnings"], json!([{ "kind": "scripts_not_executed", "items": ["Scripted"] }]));
}

#[tokio::test]
async fn cancelled_send_is_recorded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let (core, _dir) = core();
    let url = format!("{}/slow", server.uri());
    import_items(&core, json!([get_item("Slow", &url)]));
    let id = id_by_name(&core, "Slow");

    let (result, ()) = tokio::join!(core.send_request("send-1", &id, None), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        core.cancel_send("send-1");
    });
    assert_eq!(as_json(&result.unwrap())["error"]["kind"], "cancelled");

    let entry = newest_entry(&core);
    assert_eq!(entry["error_kind"], "cancelled");
    assert_eq!(entry["url"], url.as_str());
    assert!(entry["status"].is_null());
    assert_eq!(list(&core).len(), 1);
}

#[tokio::test]
async fn request_that_could_not_be_prepared_is_recorded_as_typed() {
    let (core, _dir) = core();
    let collection = core.create_node(None, NodeKind::Collection, "C").unwrap();
    // a new request has an empty address
    let request = core.create_node(Some(&collection), NodeKind::Request, "Blank").unwrap();
    let result = core.send_request("send-1", &request, None).await.unwrap();
    assert_eq!(as_json(&result)["error"]["kind"], "invalid_url");
    let entry = newest_entry(&core);
    assert_eq!(entry["error_kind"], "invalid_url");
    assert_eq!(entry["node_id"], request.as_str());
    assert_eq!(entry["name"], "Blank");
    assert_eq!(entry["method"], "GET");
    assert_eq!(entry["url"], "");
    assert!(entry["status"].is_null() && entry["total_ms"].is_null() && entry["size"].is_null());
    assert_eq!(entry["request_headers"], json!([]));
    assert!(entry["request_body"].is_null());
    assert!(!entry["error_message"].as_str().unwrap().is_empty());

    // a draft that cannot be prepared is recorded as typed: nothing is substituted, no headers, no body
    let mut draft = core.get_node(&request).unwrap().content;
    draft["method"] = json!("POST");
    draft["url"] = json!("{{base}}/upload");
    draft["headers"] = json!([{ "key": "X-Mine", "value": "1" }]);
    draft["body"] = json!({ "mode": "binary", "data": { "file": { "path": "/synthetic/file.bin" } } });
    let globals = serde_json::from_value(json!([{ "key": "base", "value": CLOSED_PORT }])).unwrap();
    core.save_environment("globals", "", globals).unwrap();
    let result = core.send_request("send-2", &request, Some(draft)).await.unwrap();
    assert_eq!(as_json(&result)["error"]["kind"], "unconfirmed_files");
    let entry = newest_entry(&core);
    assert_eq!(entry["error_kind"], "unconfirmed_files");
    assert_eq!(entry["method"], "POST");
    assert_eq!(entry["url"], "{{base}}/upload");
    assert_eq!(entry["request_headers"], json!([]));
    assert!(entry["request_body"].is_null());
    assert_eq!(list(&core).len(), 2);
}

#[tokio::test]
async fn long_ascii_bodies_are_cut_to_one_mebibyte() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 2 * MIB]))
        .mount(&server)
        .await;
    let (core, _dir) = core();
    let collection = core.create_node(None, NodeKind::Collection, "C").unwrap();
    let request = core.create_node(Some(&collection), NodeKind::Request, "Big").unwrap();
    let mut draft = core.get_node(&request).unwrap().content;
    draft["method"] = json!("POST");
    draft["url"] = json!(format!("{}/big", server.uri()));
    draft["body"] = json!({ "mode": "raw", "data": { "text": "b".repeat(MIB + 10), "language": "text" } });

    let result = core.send_request("send-1", &request, Some(draft)).await.unwrap();
    let response = result.response.unwrap();
    assert_eq!(response.decoded_size, 2 * MIB as u64);

    let entry = newest_entry(&core);
    assert_eq!(entry["size"], 2 * MIB);
    let body = entry["response_body"].as_str().unwrap();
    assert_eq!(body.len(), MIB);
    assert!(body.bytes().all(|byte| byte == b'a'));
    assert_eq!(entry["response_body_truncated"], true);
    assert_eq!(entry["response_body_binary"], false);
    let sent = entry["request_body"].as_str().unwrap();
    assert_eq!(sent.len(), MIB);
    assert_eq!(entry["request_body_truncated"], true);
    // the response itself is whole
    assert_eq!(core.response_body(&response.body.id, false).unwrap().len(), 2 * MIB);
}

/// One ASCII byte, then two-byte characters: the cut at 1 MiB falls into the middle of a character.
#[tokio::test]
async fn cut_inside_a_character_keeps_the_body_text() {
    let text = format!("a{}", "я".repeat(600_000));
    assert!(text.len() > MIB && !text.is_char_boundary(MIB));
    let entry = entry_for_body(text.clone().into_bytes()).await;
    assert_eq!(entry["response_body_binary"], false);
    assert_eq!(entry["response_body_truncated"], true);
    let body = entry["response_body"].as_str().unwrap();
    assert_eq!(body.len(), MIB - 1);
    assert!(text.starts_with(body));
    assert!(body.ends_with('я'));
}

#[tokio::test]
async fn body_that_is_not_text_is_marked_binary() {
    let entry = entry_for_body(vec![b'o', b'k', 0xFF, 0xFE, 0x00, b'x']).await;
    assert_eq!(entry["response_body_binary"], true);
    assert!(entry["response_body"].is_null());
    assert_eq!(entry["response_body_truncated"], false);
    assert_eq!(entry["status"], 200);
    assert_eq!(entry["size"], 6);
}

#[tokio::test]
async fn empty_body_is_an_empty_text() {
    let entry = entry_for_body(Vec::new()).await;
    assert_eq!(entry["response_body"], "");
    assert_eq!(entry["response_body_binary"], false);
    assert_eq!(entry["response_body_truncated"], false);
    assert_eq!(entry["size"], 0);
}

#[tokio::test]
async fn history_keeps_only_the_newest_entries() {
    let server = server_with_body(b"ok".to_vec()).await;
    let dir = tempfile::tempdir().unwrap();
    let core = core_with(&dir, None, Some(3));
    let items: Vec<Value> = (1..=4).map(|n| get_item(&format!("R{n}"), &format!("{}/x", server.uri()))).collect();
    import_items(&core, json!(items));

    for n in 1..=3 {
        core.send_request(&format!("send-{n}"), &id_by_name(&core, &format!("R{n}")), None).await.unwrap();
    }
    let first_id = list(&core)[2]["id"].as_i64().unwrap();
    assert_eq!(as_json(&core.history_get(first_id).unwrap())["name"], "R1");

    core.send_request("send-4", &id_by_name(&core, "R4"), None).await.unwrap();
    let names: Vec<Value> = list(&core).into_iter().map(|item| item["name"].clone()).collect();
    assert_eq!(names, ["R4", "R3", "R2"], "the fourth send pushed the first one out");
    assert!(core.history_get(first_id).is_err());
}

#[tokio::test]
async fn list_pages_and_get_reports_a_deleted_request() {
    let server = server_with_body(b"ok".to_vec()).await;
    let (core, _dir) = core();
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri()))]));
    let request = id_by_name(&core, "Get");
    for n in 1..=3 {
        core.send_request(&format!("send-{n}"), &request, None).await.unwrap();
    }

    let all = core.history_list(200, None).unwrap();
    assert_eq!(all.len(), 3);
    assert!(all[0].id > all[1].id && all[1].id > all[2].id, "newest first");
    let page = core.history_list(2, None).unwrap();
    assert_eq!(page.iter().map(|item| item.id).collect::<Vec<_>>(), [all[0].id, all[1].id]);
    let rest = core.history_list(2, Some(page[1].id)).unwrap();
    assert_eq!(rest.iter().map(|item| item.id).collect::<Vec<_>>(), [all[2].id]);
    // the limit is kept within 1..=200
    assert_eq!(core.history_list(0, None).unwrap().len(), 1);
    assert_eq!(core.history_list(100_000, None).unwrap().len(), 3);

    assert_eq!(as_json(&core.history_get(all[0].id).unwrap())["node_exists"], true);
    core.delete_node(&request).unwrap();
    let entry = as_json(&core.history_get(all[0].id).unwrap());
    assert_eq!(entry["node_exists"], false);
    assert_eq!(entry["node_id"], request.as_str());
    assert_eq!(entry["name"], "Get");
    assert!(core.history_get(987_654).is_err());

    core.history_clear().unwrap();
    assert!(core.history_list(200, None).unwrap().is_empty());
    assert!(core.history_get(all[0].id).is_err());
    core.history_clear().unwrap();
}

/// A failure to write the history must not cost the user the response.
#[tokio::test]
async fn broken_history_does_not_take_the_response_away() {
    let server = server_with_body(JSON_BODY.as_bytes().to_vec()).await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("requestmaster.db");
    let core = core_with(&dir, Some(db_path.clone()), None);
    import_items(&core, json!([get_item("Get", &format!("{}/x", server.uri())), get_item("Closed", CLOSED_PORT)]));
    rusqlite::Connection::open(&db_path).unwrap().execute_batch("DROP TABLE history").unwrap();
    assert!(core.history_list(10, None).is_err(), "the history is broken");

    let result = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap();
    let response = result.response.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(core.response_body(&response.body.id, false).unwrap(), JSON_BODY.as_bytes());
    // nor the error of a failed send
    let result = core.send_request("send-2", &id_by_name(&core, "Closed"), None).await.unwrap();
    assert_eq!(as_json(&result)["error"]["kind"], "connect");
}

#[tokio::test]
async fn copy_response_body_in_both_views() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/x")).respond_with(ResponseTemplate::new(200).set_body_string(JSON_BODY)).mount(&server).await;
    Mock::given(method("GET"))
        .and(path("/bytes"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'o', b'k', 0xFF]))
        .mount(&server)
        .await;
    let (core, _dir) = core();
    import_items(
        &core,
        json!([get_item("Get", &format!("{}/x", server.uri())), get_item("Bytes", &format!("{}/bytes", server.uri()))]),
    );

    let response = core.send_request("send-1", &id_by_name(&core, "Get"), None).await.unwrap().response.unwrap();
    assert_eq!(core.copy_response_body(&response.body.id, true).unwrap(), JSON_BODY_PRETTY);
    assert_eq!(core.copy_response_body(&response.body.id, false).unwrap(), JSON_BODY);

    // bytes that are not UTF-8 are replaced, not refused
    let response = core.send_request("send-2", &id_by_name(&core, "Bytes"), None).await.unwrap().response.unwrap();
    assert_eq!(core.copy_response_body(&response.body.id, false).unwrap(), "ok\u{FFFD}");

    core.release_body(&response.body.id);
    assert_eq!(core.copy_response_body(&response.body.id, false), Err("response body is gone".to_string()));
}
