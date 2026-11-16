//! `AppCore` without a window: creating, renaming, duplicating and deleting nodes, the environment editor
//! and the export texts. The store lives in memory; all Postman JSON is synthetic.

use requestmaster_lib::dto::{ImportFile, VariableDto};
use requestmaster_lib::{AppCore, CoreConfig};
use rqm_model::NodeKind;
use serde_json::{json, Value};
use tempfile::TempDir;

const SCHEMA_V21: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

/// A core over an in-memory store; the directory must outlive the core.
fn core() -> (AppCore, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let config = CoreConfig { db_path: None, bodies_dir: dir.path().join("bodies"), history_keep: None };
    (AppCore::new(config).unwrap(), dir)
}

fn import(core: &AppCore, json: &Value) {
    let results = core.import_files(vec![ImportFile { name: "file.json".to_string(), text: json.to_string() }]);
    assert_eq!(results[0].error, None);
}

fn as_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

/// `(name, sort_order)` of the children of `parent` (`None` = the collections), in their order.
fn children(core: &AppCore, parent: Option<&str>) -> Vec<(String, i64)> {
    let rows = core.tree().unwrap();
    rows.into_iter().filter(|row| row.parent_id.as_deref() == parent).map(|row| (row.name, row.sort_order)).collect()
}

fn id_by_name(core: &AppCore, name: &str) -> String {
    core.tree().unwrap().into_iter().find(|row| row.name == name).unwrap().id
}

fn variable(key: &str, value: &str) -> VariableDto {
    VariableDto {
        key: key.to_string(),
        value: value.to_string(),
        secret: false,
        enabled: true,
        postman_extra: Default::default(),
    }
}

/// An environment file with keys the application does not know, on the file and on a variable.
fn environment_file(name: &str) -> Value {
    json!({
        "id": "synthetic-environment-id",
        "name": name,
        "values": [{ "key": "host", "value": "127.0.0.1", "enabled": true, "type": "default", "note": "kept" }],
        "_postman_variable_scope": "environment",
        "_postman_exported_using": "Synthetic/1.0"
    })
}

#[test]
fn create_collection_folder_and_request() {
    let (core, _dir) = core();
    let collection = core.create_node(None, NodeKind::Collection, "  My API  ").unwrap();
    let folder = core.create_node(Some(&collection), NodeKind::Folder, "Users").unwrap();
    let request = core.create_node(Some(&folder), NodeKind::Request, "List").unwrap();
    let second = core.create_node(Some(&collection), NodeKind::Request, "Ping").unwrap();

    assert_eq!(children(&core, None), vec![("My API".to_string(), 0)], "the name is trimmed");
    assert_eq!(children(&core, Some(&collection)), vec![("Users".to_string(), 0), ("Ping".to_string(), 1)]);
    assert_eq!(children(&core, Some(&folder)), vec![("List".to_string(), 0)]);
    let rows = core.tree().unwrap();
    assert_eq!(rows.iter().find(|row| row.id == request).unwrap().method.as_deref(), Some("GET"));
    assert_eq!(rows.iter().find(|row| row.id == second).unwrap().kind, NodeKind::Request);

    // a new request: GET, empty address, no body, authorization inherited
    let node = as_json(&core.get_node(&request).unwrap());
    assert_eq!(node["kind"], "request");
    assert_eq!(node["parent_id"], folder.as_str());
    assert_eq!(node["content"]["method"], "GET");
    assert_eq!(node["content"]["url"], "");
    assert!(node["content"].get("body").is_none(), "{}", node["content"]);
    assert!(node["content"].get("auth").is_none(), "{}", node["content"]);
    assert!(node["content"].get("headers").is_none(), "{}", node["content"]);
    // a new collection and a new folder are empty
    assert_eq!(as_json(&core.get_node(&collection).unwrap())["content"], json!({}));
    assert_eq!(as_json(&core.get_node(&folder).unwrap())["content"], json!({}));

    // a second collection goes after the first
    core.create_node(None, NodeKind::Collection, "Other").unwrap();
    assert_eq!(children(&core, None), vec![("My API".to_string(), 0), ("Other".to_string(), 1)]);
}

#[test]
fn create_and_rename_refuse_an_empty_name_and_a_wrong_place() {
    let (core, _dir) = core();
    let collection = core.create_node(None, NodeKind::Collection, "C").unwrap();
    let request = core.create_node(Some(&collection), NodeKind::Request, "R").unwrap();

    assert_eq!(core.create_node(None, NodeKind::Collection, ""), Err("name is empty".to_string()));
    assert_eq!(core.create_node(Some(&collection), NodeKind::Folder, " \t "), Err("name is empty".to_string()));
    assert_eq!(core.rename_node(&request, "  "), Err("name is empty".to_string()));

    // the tree rules of the store
    let in_root = core.create_node(None, NodeKind::Request, "Root request").unwrap_err();
    assert!(in_root.contains("only a collection can be a root"), "{in_root}");
    assert!(core.create_node(None, NodeKind::Folder, "Root folder").is_err());
    let under_request = core.create_node(Some(&request), NodeKind::Request, "Child").unwrap_err();
    assert!(under_request.contains("a request cannot have children"), "{under_request}");
    assert!(core.create_node(Some(&collection), NodeKind::Collection, "Nested").is_err());
    assert!(core.create_node(Some("no-such-node"), NodeKind::Folder, "F").is_err());

    assert_eq!(core.tree().unwrap().len(), 2, "nothing was created by the refused calls");
    assert_eq!(id_by_name(&core, "R"), request);
}

#[test]
fn rename_duplicate_and_delete() {
    let (core, _dir) = core();
    let collection = core.create_node(None, NodeKind::Collection, "C").unwrap();
    let folder = core.create_node(Some(&collection), NodeKind::Folder, "F").unwrap();
    let inner = core.create_node(Some(&folder), NodeKind::Folder, "Inner").unwrap();
    core.create_node(Some(&inner), NodeKind::Request, "Deep").unwrap();
    core.create_node(Some(&collection), NodeKind::Request, "Last").unwrap();

    core.rename_node(&folder, "  Папка  ").unwrap();
    assert_eq!(core.get_node(&folder).unwrap().name, "Папка");
    assert!(core.rename_node("no-such-node", "X").is_err());

    // the copy stands right after the original, with its descendants under fresh ids
    let before: Vec<String> = core.tree().unwrap().into_iter().map(|row| row.id).collect();
    let copy = core.duplicate_node(&folder).unwrap();
    assert_eq!(
        children(&core, Some(&collection)),
        vec![("Папка".to_string(), 0), ("Папка Copy".to_string(), 1), ("Last".to_string(), 2)]
    );
    assert!(!before.contains(&copy));
    let rows = core.tree().unwrap();
    assert_eq!(rows.len(), before.len() + 3);
    let copied_inner = rows.iter().find(|row| row.parent_id.as_deref() == Some(copy.as_str())).unwrap();
    assert_eq!(copied_inner.name, "Inner");
    assert_ne!(copied_inner.id, inner);
    let copied_deep = rows.iter().find(|row| row.parent_id.as_deref() == Some(copied_inner.id.as_str())).unwrap();
    assert_eq!(copied_deep.name, "Deep");
    assert!(!before.contains(&copied_deep.id));
    assert!(core.duplicate_node("no-such-node").is_err());

    // deleting a folder takes its descendants along
    core.delete_node(&folder).unwrap();
    assert_eq!(children(&core, Some(&collection)), vec![("Папка Copy".to_string(), 0), ("Last".to_string(), 1)]);
    assert_eq!(core.tree().unwrap().len(), before.len());
    assert!(core.get_node(&inner).is_err());
    assert!(core.delete_node(&folder).is_err(), "it is gone already");

    core.delete_node(&collection).unwrap();
    assert!(core.tree().unwrap().is_empty());
}

/// C ─ F ─ Inner ─ Deep ; C ─ R2 ; C ─ R3
fn seeded(core: &AppCore) -> [String; 6] {
    let collection = core.create_node(None, NodeKind::Collection, "C").unwrap();
    let folder = core.create_node(Some(&collection), NodeKind::Folder, "F").unwrap();
    let inner = core.create_node(Some(&folder), NodeKind::Folder, "Inner").unwrap();
    let deep = core.create_node(Some(&inner), NodeKind::Request, "Deep").unwrap();
    let second = core.create_node(Some(&collection), NodeKind::Request, "R2").unwrap();
    let third = core.create_node(Some(&collection), NodeKind::Request, "R3").unwrap();
    [collection, folder, inner, deep, second, third]
}

/// `names` with the positions 0..n, as [`children`] returns them.
fn ordered(names: &[&str]) -> Vec<(String, i64)> {
    names.iter().zip(0..).map(|(name, position)| (name.to_string(), position)).collect()
}

#[test]
fn move_node_reorders_and_moves() {
    let (core, _dir) = core();
    let [collection, folder, inner, deep, second, third] = seeded(&core);

    // among the siblings: the index counts the children without the moved node
    core.move_node(&folder, Some(&collection), 1).unwrap();
    assert_eq!(children(&core, Some(&collection)), ordered(&["R2", "F", "R3"]));
    core.move_node(&third, Some(&collection), 0).unwrap();
    assert_eq!(children(&core, Some(&collection)), ordered(&["R3", "R2", "F"]));
    // an index past the end means "the last one"
    core.move_node(&third, Some(&collection), 99).unwrap();
    assert_eq!(children(&core, Some(&collection)), ordered(&["R2", "F", "R3"]));

    // into a folder and out of it
    core.move_node(&second, Some(&inner), 0).unwrap();
    assert_eq!(children(&core, Some(&inner)), ordered(&["R2", "Deep"]));
    assert_eq!(children(&core, Some(&collection)), ordered(&["F", "R3"]));
    assert_eq!(core.get_node(&second).unwrap().parent_id.as_deref(), Some(inner.as_str()));
    core.move_node(&deep, Some(&collection), 1).unwrap();
    assert_eq!(children(&core, Some(&collection)), ordered(&["F", "Deep", "R3"]));
    assert_eq!(children(&core, Some(&inner)), ordered(&["R2"]));

    // the collections themselves, and a folder with its descendants into another collection
    let other = core.create_node(None, NodeKind::Collection, "Other").unwrap();
    core.move_node(&other, None, 0).unwrap();
    assert_eq!(children(&core, None), ordered(&["Other", "C"]));
    core.move_node(&folder, Some(&other), 0).unwrap();
    assert_eq!(children(&core, Some(&other)), ordered(&["F"]));
    assert_eq!(children(&core, Some(&folder)), ordered(&["Inner"]));
    assert_eq!(children(&core, Some(&inner)), ordered(&["R2"]));
    assert_eq!(children(&core, Some(&collection)), ordered(&["Deep", "R3"]));
    assert_eq!(core.tree().unwrap().len(), 7, "nothing is lost or copied");
}

#[test]
fn move_node_refuses_forbidden_places_and_changes_nothing() {
    let (core, _dir) = core();
    let [collection, folder, inner, deep, second, third] = seeded(&core);
    let before = as_json(&core.tree().unwrap());

    // the error is the broken rule itself, without the `invalid move:` prefix of the store
    let into_itself_or_descendant = "a node cannot be moved into itself or its descendant".to_string();
    assert_eq!(core.move_node(&folder, Some(&folder), 0), Err(into_itself_or_descendant.clone()));
    assert_eq!(core.move_node(&folder, Some(&inner), 0), Err(into_itself_or_descendant));
    assert_eq!(core.move_node(&second, None, 0), Err("only a collection can be a root".to_string()));
    assert_eq!(core.move_node(&folder, None, 0), Err("only a collection can be a root".to_string()));
    assert_eq!(core.move_node(&collection, Some(&folder), 0), Err("a collection can only be a root".to_string()));
    assert_eq!(core.move_node(&second, Some(&third), 0), Err("a request cannot have children".to_string()));
    assert_eq!(core.move_node(&folder, Some(&deep), 0), Err("a request cannot have children".to_string()));
    // a node or a parent that does not exist is an error too, in the words of the store
    assert!(core.move_node("no-such-node", Some(&collection), 0).is_err());
    assert!(core.move_node(&second, Some("no-such-node"), 0).is_err());

    assert_eq!(as_json(&core.tree().unwrap()), before, "the tree is as it was");
}

/// A collection named `name` whose variable `host` is `host`, with one request `<name> request`.
fn collection_with_host(core: &AppCore, name: &str, host: &str) -> (String, String) {
    let collection = core.create_node(None, NodeKind::Collection, name).unwrap();
    core.save_node_content(&collection, json!({ "variables": [{ "key": "host", "value": host }] })).unwrap();
    let request = core.create_node(Some(&collection), NodeKind::Request, &format!("{name} request")).unwrap();
    (collection, request)
}

/// The entry `name` of `variables_in_scope(node_id)` as the window gets it.
fn scope_entry(core: &AppCore, node_id: &str, name: &str) -> Value {
    let all = core.variables_in_scope(node_id).unwrap();
    as_json(&all.into_iter().find(|variable| variable.name == name).unwrap())
}

#[test]
fn moved_request_sees_the_variables_of_its_new_collection() {
    let (core, _dir) = core();
    let (_first, request) = collection_with_host(&core, "First", "one");
    let (second, _) = collection_with_host(&core, "Second", "two");
    let folder = core.create_node(Some(&second), NodeKind::Folder, "F").unwrap();

    let expected = |value: &str| json!({ "name": "host", "scope": "collection", "value": value, "secret": false });
    assert_eq!(scope_entry(&core, &request, "host"), expected("one"));
    core.move_node(&request, Some(&folder), 0).unwrap();
    assert_eq!(scope_entry(&core, &request, "host"), expected("two"));
    // a folder and the collection itself see the same collection scope
    assert_eq!(scope_entry(&core, &folder, "host"), expected("two"));
    assert_eq!(scope_entry(&core, &second, "host"), expected("two"));
}

#[test]
fn variables_in_scope_uses_the_active_environment() {
    let (core, _dir) = core();
    let (collection, request) = collection_with_host(&core, "C", "c");
    core.save_environment("globals", "", vec![variable("host", "g"), variable("only_global", "1")]).unwrap();
    let environment = core.create_environment("E").unwrap();
    let secret = VariableDto { secret: true, ..variable("token", "s3cr3t") };
    core.save_environment(&environment, "E", vec![variable("host", "e"), secret]).unwrap();
    core.save_node_content(
        &collection,
        json!({ "variables": [{ "key": "host", "value": "c" }, { "key": "auth", "value": "Bearer {{token}}" }] }),
    )
    .unwrap();
    let other = core.create_environment("Other").unwrap();
    core.save_environment(&other, "Other", vec![variable("host", "o")]).unwrap();

    // no environment is chosen: the collection wins over the globals, the token is unknown
    assert_eq!(
        scope_entry(&core, &request, "host"),
        json!({ "name": "host", "scope": "collection", "value": "c", "secret": false })
    );
    assert_eq!(
        scope_entry(&core, &request, "auth"),
        json!({ "name": "auth", "scope": "collection", "value": "Bearer {{token}}", "secret": false })
    );

    core.set_active_environment(Some(&environment)).unwrap();
    assert_eq!(
        scope_entry(&core, &request, "host"),
        json!({ "name": "host", "scope": "environment", "value": "e", "secret": false })
    );
    assert_eq!(
        scope_entry(&core, &request, "only_global"),
        json!({ "name": "only_global", "scope": "global", "value": "1", "secret": false })
    );
    // the secret and the open variable that refers to it come without a value
    assert_eq!(
        scope_entry(&core, &request, "token"),
        json!({ "name": "token", "scope": "environment", "value": null, "secret": true })
    );
    assert_eq!(
        scope_entry(&core, &request, "auth"),
        json!({ "name": "auth", "scope": "collection", "value": null, "secret": true })
    );
    assert_eq!(
        scope_entry(&core, &request, "$guid"),
        json!({ "name": "$guid", "scope": "dynamic", "value": null, "secret": false })
    );
    let all = as_json(&core.variables_in_scope(&request).unwrap());
    assert!(!all.to_string().contains("s3cr3t"), "{all}");
    let names: Vec<&str> = all.as_array().unwrap().iter().map(|entry| entry["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["auth", "host", "only_global", "token", "$guid", "$randomUUID", "$timestamp", "$isoTimestamp", "$randomInt"]
    );

    core.set_active_environment(Some(&other)).unwrap();
    assert_eq!(scope_entry(&core, &request, "host")["value"], "o");
    // the active environment was deleted: the choice is gone, not an error
    core.delete_environment(&other).unwrap();
    assert_eq!(scope_entry(&core, &request, "host")["scope"], "collection");
}

#[test]
fn variables_in_scope_of_an_unknown_node_is_an_error() {
    let (core, _dir) = core();
    let error = core.variables_in_scope("no-such-node").unwrap_err();
    assert!(error.contains("no-such-node"), "{error}");
}

#[test]
fn environment_is_created_saved_and_read() {
    let (core, _dir) = core();
    assert_eq!(core.create_environment("  "), Err("name is empty".to_string()));
    let id = core.create_environment("  Local  ").unwrap();
    let summaries = as_json(&core.environments().unwrap());
    assert_eq!(summaries, json!([{ "id": id, "name": "Local" }]));
    assert_eq!(as_json(&core.get_environment(&id).unwrap()), json!({ "id": id, "name": "Local", "variables": [] }));

    let variables = vec![
        variable("host", "127.0.0.1"),
        VariableDto { secret: true, ..variable("token", "s3cret") },
        VariableDto { enabled: false, ..variable("unused", "x") },
    ];
    core.save_environment(&id, "  Stage  ", variables).unwrap();
    let read = as_json(&core.get_environment(&id).unwrap());
    assert_eq!(read["name"], "Stage");
    // every variable has all five keys, whatever its values are
    assert_eq!(
        read["variables"],
        json!([
            { "key": "host", "value": "127.0.0.1", "secret": false, "enabled": true, "postman_extra": {} },
            { "key": "token", "value": "s3cret", "secret": true, "enabled": true, "postman_extra": {} },
            { "key": "unused", "value": "x", "secret": false, "enabled": false, "postman_extra": {} }
        ])
    );
    assert_eq!(core.environments().unwrap()[0].name, "Stage");

    assert_eq!(core.save_environment(&id, "   ", vec![]), Err("name is empty".to_string()));
    assert_eq!(core.get_environment(&id).unwrap().variables.len(), 3, "a refused save changes nothing");
}

/// What the window sends: `secret`, `enabled` and `postman_extra` may be missing.
#[test]
fn variable_from_the_window_has_defaults() {
    let sent: VariableDto = serde_json::from_value(json!({ "key": "k", "value": "v" })).unwrap();
    assert_eq!(sent, variable("k", "v"));
    assert_eq!(
        as_json(&sent),
        json!({ "key": "k", "value": "v", "secret": false, "enabled": true, "postman_extra": {} })
    );
}

#[test]
fn postman_extra_of_an_imported_environment_survives_a_save_from_the_window() {
    let (core, _dir) = core();
    let source = environment_file("Imported");
    import(&core, &source);
    let id = core.environments().unwrap()[0].id.clone();

    // the window gets the variables, changes a value and sends them back; it knows nothing of the keys of the file
    let read = core.get_environment(&id).unwrap();
    assert_eq!(as_json(&read).as_object().unwrap().keys().collect::<Vec<_>>(), ["id", "name", "variables"]);
    let mut variables = read.variables;
    assert!(!variables[0].postman_extra.is_empty(), "the variable carries its Postman object");
    variables[0].value = "10.0.0.1".to_string();
    variables.push(variable("added", "1"));
    core.save_environment(&id, "Imported", variables).unwrap();

    let (file_name, text) = core.export_environment_text(&id).unwrap();
    assert_eq!(file_name, "Imported.postman_environment.json");
    let exported: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(exported["id"], "synthetic-environment-id");
    assert_eq!(exported["_postman_exported_using"], "Synthetic/1.0");
    assert_eq!(exported["_postman_variable_scope"], "environment");
    assert_eq!(exported["values"][0]["value"], "10.0.0.1");
    assert_eq!(exported["values"][0]["note"], "kept");
    assert_eq!(exported["values"][1]["key"], "added");
    assert!(text.contains("\n  \"name\": \"Imported\""), "indented with two spaces: {text}");
}

#[test]
fn untouched_environment_exports_without_differences() {
    let (core, _dir) = core();
    let source = environment_file("Dev / \"stage\"?");
    import(&core, &source);
    let id = core.environments().unwrap()[0].id.clone();
    let (file_name, text) = core.export_environment_text(&id).unwrap();
    assert_eq!(file_name, "Dev _ _stage__.postman_environment.json");
    let exported: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(rqm_postman::semantic_diff(&source, &exported), Vec::<String>::new());
    assert!(core.export_environment_text("no-such-environment").is_err());
}

#[test]
fn deleting_the_active_environment_clears_the_choice() {
    let (core, _dir) = core();
    let first = core.create_environment("First").unwrap();
    let second = core.create_environment("Second").unwrap();
    core.set_active_environment(Some(&first)).unwrap();

    // deleting another environment leaves the choice alone
    core.delete_environment(&second).unwrap();
    assert_eq!(core.active_environment().unwrap(), Some(first.clone()));

    core.delete_environment(&first).unwrap();
    assert_eq!(core.active_environment().unwrap(), None);
    assert!(core.environments().unwrap().is_empty());
    assert!(core.delete_environment(&first).is_err(), "it is gone already");
    assert!(core.get_environment(&first).is_err());
}

#[test]
fn globals_are_read_and_saved_but_never_deleted() {
    let (core, _dir) = core();
    // nothing stored yet: an empty set
    assert_eq!(
        as_json(&core.get_environment("globals").unwrap()),
        json!({ "id": "globals", "name": "Globals", "variables": [] })
    );

    // the name the window sends is ignored, an empty one included
    core.save_environment("globals", "", vec![variable("token", "abc")]).unwrap();
    core.save_environment("globals", "Renamed", vec![variable("token", "abc"), variable("n", "1")]).unwrap();
    let read = core.get_environment("globals").unwrap();
    assert_eq!(read.name, "Globals");
    assert_eq!(read.variables, vec![variable("token", "abc"), variable("n", "1")]);

    assert!(core.delete_environment("globals").is_err());
    assert_eq!(core.get_environment("globals").unwrap().variables.len(), 2);
    assert!(core.environments().unwrap().is_empty(), "globals are not an environment of the switcher");

    let (file_name, text) = core.export_environment_text("globals").unwrap();
    assert_eq!(file_name, "globals.postman_globals.json");
    let exported: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(exported["_postman_variable_scope"], "globals");
    assert_eq!(exported["values"][1]["key"], "n");
}

/// The store would insert an environment under any id it is given; the core must not let that happen.
#[test]
fn unknown_environment_id_is_an_error_and_creates_nothing() {
    let (core, _dir) = core();
    let known = core.create_environment("Known").unwrap();

    let read = core.get_environment("no-such-environment").unwrap_err();
    assert!(read.contains("environment not found"), "{read}");
    let saved = core.save_environment("no-such-environment", "Ghost", vec![variable("k", "v")]).unwrap_err();
    assert!(saved.contains("environment not found"), "{saved}");

    assert_eq!(as_json(&core.environments().unwrap()), json!([{ "id": known, "name": "Known" }]));
    assert!(core.get_environment("globals").unwrap().variables.is_empty());
}

#[test]
fn imported_collection_exports_without_differences() {
    let (core, _dir) = core();
    let source = json!({
        "info": { "_postman_id": "synthetic-collection-id", "name": "Demo: v1/v2 <draft>|*", "schema": SCHEMA_V21 },
        "item": [
            {
                "name": "Users",
                "item": [{
                    "name": "List users",
                    "request": {
                        "method": "GET",
                        "header": [{ "key": "Accept", "value": "application/json" }],
                        "url": {
                            "raw": "http://127.0.0.1/users?page=1",
                            "protocol": "http",
                            "host": ["127", "0", "0", "1"],
                            "path": ["users"],
                            "query": [{ "key": "page", "value": "1" }]
                        }
                    },
                    "response": []
                }]
            },
            {
                "name": "Create user",
                "request": {
                    "method": "POST",
                    "header": [],
                    "body": {
                        "mode": "raw",
                        "raw": "{\"name\":\"{{name}}\"}",
                        "options": { "raw": { "language": "json" } }
                    },
                    "url": {
                        "raw": "http://127.0.0.1/users",
                        "protocol": "http",
                        "host": ["127", "0", "0", "1"],
                        "path": ["users"]
                    }
                },
                "response": []
            }
        ],
        "variable": [{ "key": "name", "value": "synthetic" }]
    });
    import(&core, &source);
    let collection = core.tree().unwrap().into_iter().find(|row| row.parent_id.is_none()).unwrap().id;

    let (file_name, text) = core.export_collection_text(&collection).unwrap();
    assert_eq!(file_name, "Demo_ v1_v2 _draft___.postman_collection.json");
    let exported: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(rqm_postman::semantic_diff(&source, &exported), Vec::<String>::new());
    assert!(text.starts_with("{\n  \"info\": {\n    "), "indented with two spaces");

    // only a collection can be exported
    assert!(core.export_collection_text(&id_by_name(&core, "Users")).is_err());
    assert!(core.export_collection_text(&id_by_name(&core, "Create user")).is_err());
    assert!(core.export_collection_text("no-such-node").is_err());
}

#[test]
fn collection_created_here_exports_as_a_postman_collection() {
    let (core, _dir) = core();
    let collection = core.create_node(None, NodeKind::Collection, "Fresh").unwrap();
    core.create_node(Some(&collection), NodeKind::Request, "Ping").unwrap();
    let (file_name, text) = core.export_collection_text(&collection).unwrap();
    assert_eq!(file_name, "Fresh.postman_collection.json");
    let exported: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(exported["info"]["name"], "Fresh");
    assert_eq!(exported["info"]["schema"], SCHEMA_V21);
    assert_eq!(exported["item"][0]["name"], "Ping");
    assert_eq!(exported["item"][0]["request"]["method"], "GET");
}
