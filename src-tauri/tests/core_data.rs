//! `AppCore` without a window: tree, nodes, import, environments. The store lives in memory unless a test
//! needs to look into the database file itself; all Postman JSON is synthetic.

use std::collections::BTreeSet;

use requestmaster_lib::dto::{ImportFile, ImportResult, TimingSample, TreeRow};
use requestmaster_lib::{AppCore, CoreConfig, COMMANDS};
use rqm_model::{NodeContent, NodeKind};
use serde_json::{json, Value};
use tempfile::TempDir;

const SCHEMA_V21: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";
const SCHEMA_V20: &str = "https://schema.getpostman.com/json/collection/v2.0.0/collection.json";

/// A core over an in-memory store; the directory must outlive the core.
fn core() -> (AppCore, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let core = AppCore::new(CoreConfig { db_path: None, bodies_dir: dir.path().join("bodies"), history_keep: None, measure_file: None }).unwrap();
    (core, dir)
}

fn file(name: &str, json: &Value) -> ImportFile {
    ImportFile { name: name.to_string(), text: json.to_string() }
}

/// A collection with a folder holding one request and one more request next to the folder.
fn collection(name: &str) -> Value {
    json!({
        "info": { "name": name, "schema": SCHEMA_V21 },
        "item": [
            {
                "name": "Users",
                "item": [
                    { "name": "List users", "request": { "method": "GET", "url": { "raw": "http://127.0.0.1/users" } } }
                ]
            },
            { "name": "Create user", "request": { "method": "POST", "url": { "raw": "http://127.0.0.1/users" } } }
        ]
    })
}

fn environment(name: &str) -> Value {
    json!({
        "name": name,
        "values": [{ "key": "host", "value": "127.0.0.1", "enabled": true }],
        "_postman_variable_scope": "environment"
    })
}

fn globals(value: &str) -> Value {
    json!({
        "name": "Globals",
        "values": [{ "key": "token", "value": value, "enabled": true }],
        "_postman_variable_scope": "globals"
    })
}

/// Imports one file and returns its result.
fn import_one(core: &AppCore, name: &str, json: &Value) -> ImportResult {
    let mut results = core.import_files(vec![file(name, json)]);
    assert_eq!(results.len(), 1);
    results.remove(0)
}

/// The results as the window receives them.
fn as_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

/// Id of the tree row with this name.
fn id_by_name(core: &AppCore, name: &str) -> String {
    core.tree().unwrap().into_iter().find(|row| row.name == name).unwrap().id
}

#[test]
fn import_collection_then_tree_and_node() {
    let (core, _dir) = core();
    let result = import_one(&core, "demo.json", &collection("Demo"));
    let sent = as_json(&result);
    assert_eq!(sent["file_name"], "demo.json");
    assert_eq!(sent["kind"], "collection");
    assert_eq!(sent["name"], "Demo");
    assert_eq!(sent["error"], Value::Null);
    let root_id = result.root_id.clone().expect("a collection result carries the id of its root");

    let tree = core.tree().unwrap();
    assert_eq!(tree.len(), 4);
    // kind, parent and method of a row
    let facts = |row: &TreeRow| (row.kind, row.parent_id.clone(), row.method.clone());
    let root = tree.iter().find(|row| row.id == root_id).unwrap();
    assert_eq!(facts(root), (NodeKind::Collection, None, None));
    let folder = tree.iter().find(|row| row.name == "Users").unwrap();
    assert_eq!(facts(folder), (NodeKind::Folder, Some(root_id.clone()), None));
    let list = tree.iter().find(|row| row.name == "List users").unwrap();
    assert_eq!(facts(list), (NodeKind::Request, Some(folder.id.clone()), Some("GET".to_string())));
    let create = tree.iter().find(|row| row.name == "Create user").unwrap();
    assert_eq!(facts(create), (NodeKind::Request, Some(root_id.clone()), Some("POST".to_string())));

    // the window gets the tree with the contract's key names
    let sent_tree = as_json(&tree);
    let sent_create = sent_tree.as_array().unwrap().iter().find(|row| row["name"] == "Create user").unwrap();
    assert_eq!(sent_create["kind"], "request");
    assert_eq!(sent_create["method"], "POST");
    assert_eq!(sent_create["parent_id"], root_id.as_str());
    assert_eq!(sent_create["sort_order"], 1);

    let node = core.get_node(&create.id).unwrap();
    assert_eq!(node.id, create.id);
    assert_eq!(node.parent_id.as_deref(), Some(root_id.as_str()));
    assert_eq!(node.kind, NodeKind::Request);
    assert_eq!(node.name, "Create user");
    assert_eq!(node.content["method"], "POST");
    assert_eq!(node.content["url"], "http://127.0.0.1/users");
}

#[test]
fn import_reports_each_file_independently() {
    let (core, _dir) = core();
    let old_collection = json!({ "info": { "name": "Old", "schema": SCHEMA_V20 }, "item": [] });
    let results = core.import_files(vec![
        ImportFile { name: "picture.png".to_string(), text: "\u{fffd}PNG\r\n\u{1a}\n\0\0\0\rIHDR".to_string() },
        file("old.json", &old_collection),
        file("demo.json", &collection("Demo")),
        file("dev.json", &environment("Dev")),
    ]);
    let sent = as_json(&results);
    let sent = sent.as_array().unwrap();
    assert_eq!(sent.len(), 4);
    let names: Vec<&str> = sent.iter().map(|r| r["file_name"].as_str().unwrap()).collect();
    assert_eq!(names, ["picture.png", "old.json", "demo.json", "dev.json"]);
    let kinds: Vec<&str> = sent.iter().map(|r| r["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["error", "error", "collection", "environment"]);
    assert_eq!(sent[0]["error"], "not a JSON file");
    assert_eq!(sent[1]["error"], "Collection v2.0 is not supported yet");
    assert_eq!(sent[2]["error"], Value::Null);
    assert_eq!(sent[3]["error"], Value::Null);
    assert_eq!(sent[3]["name"], "Dev");
    assert_eq!(sent[3]["root_id"], Value::Null);

    // JSON that is not a Postman export
    let unknown = import_one(&core, "package.json", &json!({ "name": "x", "version": "1.0.0" }));
    assert_eq!(as_json(&unknown)["kind"], "error");
    assert_eq!(unknown.error.as_deref(), Some("unknown format"));

    let tree = core.tree().unwrap();
    assert_eq!(tree.iter().filter(|row| row.kind == NodeKind::Collection).count(), 1);
    assert_eq!(tree.len(), 4);
    let environments = core.environments().unwrap();
    assert_eq!(environments.len(), 1);
    assert_eq!(environments[0].name, "Dev");
    assert_eq!(as_json(&environments), json!([{ "id": environments[0].id, "name": "Dev" }]));
}

#[test]
fn second_import_appends_collection() {
    let (core, _dir) = core();
    let first = import_one(&core, "a.json", &collection("First")).root_id.unwrap();
    let second = import_one(&core, "b.json", &collection("Second")).root_id.unwrap();
    let roots: Vec<(String, String, i64)> = core
        .tree()
        .unwrap()
        .into_iter()
        .filter(|row| row.parent_id.is_none())
        .map(|row| (row.id, row.name, row.sort_order))
        .collect();
    assert_eq!(roots, [(first, "First".to_string(), 0), (second, "Second".to_string(), 1)]);
}

#[test]
fn save_node_content_round_trip_and_rejects_garbage() {
    let (core, _dir) = core();
    import_one(&core, "demo.json", &collection("Demo"));
    let id = id_by_name(&core, "Create user");

    let mut content = core.get_node(&id).unwrap().content;
    content["url"] = json!("http://127.0.0.1/people");
    core.save_node_content(&id, content.clone()).unwrap();
    let saved = core.get_node(&id).unwrap().content;
    assert_eq!(saved["url"], "http://127.0.0.1/people");
    assert_eq!(saved, content);

    assert!(core.save_node_content(&id, json!("x")).is_err());
    assert_eq!(core.get_node(&id).unwrap().content, content);

    assert!(core.save_node_content("no-such-node", content).is_err());
    assert!(core.get_node("no-such-node").is_err());
}

#[test]
fn nulls_and_empty_scripts_from_the_window_are_defaults() {
    let (core, _dir) = core();
    import_one(&core, "demo.json", &collection("Demo"));
    let id = id_by_name(&core, "Create user");

    // what the window sends for a request whose Scripts and Settings were left alone
    let mut content = core.get_node(&id).unwrap().content;
    content["scripts"] = json!({ "pre_request": "", "test": "" });
    content["settings"] =
        json!({ "follow_redirects": null, "max_redirects": null, "strict_ssl": null, "disable_body_pruning": null });
    core.save_node_content(&id, content).unwrap();

    let saved = core.get_node(&id).unwrap().content;
    let saved = saved.as_object().unwrap();
    assert!(!saved.contains_key("settings"));
    assert!(!saved.contains_key("scripts"));
    assert_eq!(saved["method"], "POST");

    // a setting and a script that are set come back, the ones left alone still do not
    let mut content = Value::Object(saved.clone());
    content["scripts"] = json!({ "pre_request": "", "test": "check();" });
    content["settings"] =
        json!({ "follow_redirects": false, "max_redirects": 3, "strict_ssl": null, "disable_body_pruning": null });
    core.save_node_content(&id, content).unwrap();
    let saved = core.get_node(&id).unwrap().content;
    assert_eq!(saved["scripts"], json!({ "test": "check();" }));
    assert_eq!(saved["settings"], json!({ "follow_redirects": false, "max_redirects": 3 }));
}

#[test]
fn postman_extra_never_reaches_the_window_and_survives_save() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.db");
    let core =
        AppCore::new(CoreConfig { db_path: Some(db_path.clone()), bodies_dir: dir.path().join("bodies"), history_keep: None, measure_file: None }).unwrap();
    let saved_example = json!([{ "name": "Example", "code": 200, "body": "{\"id\":12345678901234567890}" }]);
    let source = json!({
        "info": { "name": "Demo", "schema": SCHEMA_V21 },
        "item": [{
            "name": "Get user",
            "x_unknown": { "kept": true },
            "request": {
                "method": "GET",
                "header": [{ "key": "Accept", "value": "application/json", "description": "what we want back" }],
                "url": { "raw": "http://127.0.0.1/user" }
            },
            "response": saved_example
        }]
    });
    import_one(&core, "demo.json", &source);
    let id = id_by_name(&core, "Get user");

    let stored_extra = |id: &str| {
        let store = rqm_store::Store::open(&db_path).unwrap();
        match store.node(id).unwrap().content {
            NodeContent::Request(request) => (request.url, request.postman_extra),
            other => panic!("expected a request, got {other:?}"),
        }
    };
    let (_, extra_before) = stored_extra(&id);
    assert_eq!(extra_before["response"], saved_example);
    assert_eq!(extra_before["x_unknown"], json!({ "kept": true }));

    let mut content = core.get_node(&id).unwrap().content;
    assert!(content.get("postman_extra").is_none());
    // the row's own source object stays: the window returns it untouched
    assert_eq!(content["headers"][0]["postman_extra"]["description"], "what we want back");

    content["url"] = json!("http://127.0.0.1/account");
    core.save_node_content(&id, content).unwrap();

    let (url_after, extra_after) = stored_extra(&id);
    assert_eq!(url_after, "http://127.0.0.1/account");
    assert_eq!(extra_after, extra_before);
    assert!(core.get_node(&id).unwrap().content.get("postman_extra").is_none());
}

#[test]
fn commands_are_listed_everywhere() {
    let expected = [
        "tree",
        "get_node",
        "save_node_content",
        "import_files",
        "import_globals",
        "environments",
        "active_environment",
        "set_active_environment",
        "send_request",
        "cancel_send",
        "response_body",
        "release_body",
        "format_json",
        "create_node",
        "rename_node",
        "duplicate_node",
        "delete_node",
        "get_environment",
        "save_environment",
        "create_environment",
        "delete_environment",
        "export_collection",
        "export_environment",
        "history_list",
        "history_get",
        "history_clear",
        "copy_response_body",
        "move_node",
        "variables_in_scope",
        "load_session",
        "save_window_state",
        "save_draft",
        "get_settings",
        "save_settings",
        "save_response_body",
        "pick_files",
        "report_timings",
    ];
    assert_eq!(COMMANDS.iter().copied().collect::<BTreeSet<_>>(), expected.into_iter().collect::<BTreeSet<_>>());
    assert_eq!(COMMANDS.len(), 37);

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities").join("default.json");
    let capability: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let permissions: Vec<&str> =
        capability["permissions"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    let allowed: BTreeSet<String> = permissions.iter().map(|p| p.to_string()).collect();
    assert_eq!(allowed.len(), permissions.len(), "a permission is listed twice");
    let mut wanted: BTreeSet<String> = COMMANDS.iter().map(|name| format!("allow-{}", name.replace('_', "-"))).collect();
    // What `onCloseRequested` of the JavaScript API calls: it listens to the close request of the window and,
    // once the window may close, destroys it. Nothing else of the Tauri core is open to the window.
    wanted.extend(["core:event:allow-listen", "core:event:allow-unlisten", "core:window:allow-destroy"].map(String::from));
    assert_eq!(allowed, wanted);
}

#[test]
fn format_json_reindents_and_rejects_what_is_not_json() {
    let (core, _dir) = core();
    assert_eq!(core.format_json("{\"b\":1.50,\"a\":[]}"), Ok("{
  \"b\": 1.50,
  \"a\": []
}".to_string()));
    assert_eq!(core.format_json("{\"id\": {{id}}}"), Err("not valid JSON".to_string()));
}

#[test]
fn globals_without_conflicts_merge_and_conflicts_wait_for_choice() {
    let (core, _dir) = core();
    let (first, second) = (globals("one"), globals("two"));
    let (text1, text2) = (first.to_string(), second.to_string());
    let no_conflicts: Vec<String> = Vec::new();
    let conflict = vec!["token".to_string()];

    let applied = import_one(&core, "g1.json", &first);
    assert_eq!(as_json(&applied)["kind"], "globals");
    assert_eq!(applied.conflicts, no_conflicts);
    assert_eq!(applied.error, None);

    // a different value of the same variable: nothing is applied, the window has to ask
    let waiting = import_one(&core, "g2.json", &second);
    assert_eq!(as_json(&waiting)["kind"], "globals");
    assert_eq!(waiting.conflicts, conflict);
    assert_eq!(waiting.error, None);
    assert_eq!(import_one(&core, "g2.json", &second).conflicts, conflict);

    core.import_globals(&text2, "take_incoming").unwrap();
    assert_eq!(import_one(&core, "g2.json", &second).conflicts, no_conflicts);
    assert_eq!(import_one(&core, "g1.json", &first).conflicts, conflict);

    core.import_globals(&text1, "keep_current").unwrap();
    assert_eq!(import_one(&core, "g2.json", &second).conflicts, no_conflicts);

    assert!(core.import_globals(&text1, "nope").is_err());
    assert_eq!(import_one(&core, "g2.json", &second).conflicts, no_conflicts);

    // globals are not an environment of the switcher
    assert!(core.environments().unwrap().is_empty());
}

#[test]
fn active_environment_rules() {
    let (core, _dir) = core();
    assert_eq!(core.active_environment().unwrap(), None);

    import_one(&core, "dev.json", &environment("Dev"));
    let id = core.environments().unwrap()[0].id.clone();
    core.set_active_environment(Some(&id)).unwrap();
    assert_eq!(core.active_environment().unwrap(), Some(id.clone()));

    assert!(core.set_active_environment(Some("no-such-environment")).is_err());
    assert_eq!(core.active_environment().unwrap(), Some(id));

    core.set_active_environment(None).unwrap();
    assert_eq!(core.active_environment().unwrap(), None);
}

#[test]
fn report_items_use_contract_names() {
    let (core, _dir) = core();
    let source = json!({
        "info": { "name": "Signed", "schema": SCHEMA_V21 },
        "auth": { "type": "awsv4", "awsv4": [{ "key": "region", "value": "eu-west-1", "type": "string" }] },
        "event": [{ "listen": "test", "script": { "type": "text/javascript", "exec": ["pm.test('ok', () => {});"] } }],
        "item": []
    });
    let result = import_one(&core, "signed.json", &source);
    let root_id = result.root_id.clone().unwrap();
    let sent = as_json(&result);
    let report = sent["report"].as_array().unwrap();
    let kinds: Vec<&str> = report.iter().map(|item| item["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["unsupported_auth", "has_scripts"]);
    assert_eq!(report[0]["detail"], "awsv4");
    for item in report {
        assert_eq!(item["node_id"], root_id.as_str());
        assert_eq!(item["node_path"], "Signed");
        assert!(item["detail"].is_string());
    }
}

#[test]
fn csp_is_strict() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let security = config["app"]["security"].as_object().unwrap();
    let csp = security["csp"].as_str().unwrap();
    assert!(!csp.contains("unsafe-inline"), "{csp}");
    assert!(!csp.contains("unsafe-eval"), "{csp}");
    // Each directive is checked whole: `style-src 'self'` followed by anything else would not be strict.
    let directives: Vec<&str> = csp.split(';').map(str::trim).collect();
    for wanted in ["style-src 'self'", "frame-src 'none'", "form-action 'none'"] {
        assert!(directives.contains(&wanted), "{wanted} is not in {csp}");
    }
    // Without the key Tauri adds the nonce of the style tag of index.html to `style-src` itself.
    assert!(!security.contains_key("dangerousDisableAssetCspModification"));
}

/// A core over an in-memory store that appends its timings to `<dir>/timings.tsv`.
fn measuring_core(dir: &TempDir) -> AppCore {
    AppCore::new(CoreConfig {
        db_path: None,
        bodies_dir: dir.path().join("bodies"),
        history_keep: None,
        measure_file: Some(dir.path().join("timings.tsv")),
    })
    .unwrap()
}

fn sample(name: &str, ms: f64) -> TimingSample {
    TimingSample { name: name.to_string(), ms }
}

#[test]
fn report_timings_without_a_measure_file_does_nothing() {
    let (core, dir) = core();
    assert_eq!(core.report_timings(vec![sample("ready", 812.5)]), Ok(()));
    let left: Vec<String> =
        std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left, ["bodies"], "nothing but the bodies folder is created");
}

#[test]
fn report_timings_appends_lines() {
    let dir = tempfile::tempdir().unwrap();
    let core = measuring_core(&dir);
    core.report_timings(vec![sample("ready", 812.5)]).unwrap();
    core.report_timings(vec![sample("click:tab", 64.0), sample("click:tab:proc", 30.0), sample("keydown:url", 24.0)])
        .unwrap();
    let text = std::fs::read_to_string(dir.path().join("timings.tsv")).unwrap();
    assert_eq!(text, "ready	812.5
click:tab	64
click:tab:proc	30
keydown:url	24
");
    assert_eq!(text.lines().count(), 4);
}

#[test]
fn load_session_tells_whether_the_timings_are_measured() {
    let (plain, _plain_dir) = core();
    assert!(!plain.load_session().unwrap().measure);
    assert_eq!(as_json(&plain.load_session().unwrap())["measure"], json!(false));

    let dir = tempfile::tempdir().unwrap();
    let measuring = measuring_core(&dir);
    assert!(measuring.load_session().unwrap().measure);
    assert_eq!(as_json(&measuring.load_session().unwrap())["measure"], json!(true));
}
