//! What the scripts changed in the variables, written to the store over a fresh read.

use rqm_model::*;
use rqm_runner::*;
use rqm_store::Store;
use serde_json::{json, Value};

fn var(key: &str, value: &str) -> Variable {
    Variable { key: key.into(), value: value.into(), secret: false, enabled: true, postman_extra: Extra::new() }
}
fn off(key: &str, value: &str) -> Variable {
    Variable { enabled: false, ..var(key, value) }
}
fn set(scope: StoredScope, key: &str, value: &str) -> VariableChange {
    VariableChange { scope, op: VariableOp::Set { key: key.into(), value: value.into() } }
}
fn unset(scope: StoredScope, key: &str) -> VariableChange {
    VariableChange { scope, op: VariableOp::Unset { key: key.into() } }
}
fn extra(value: Value) -> Extra {
    value.as_object().unwrap().clone()
}
fn request() -> RequestContent {
    RequestContent { method: "GET".into(), url: "http://127.0.0.1:1/x".into(), query: vec![], path_variables: vec![],
        headers: vec![], body: Body::None, auth: None, scripts: Scripts::default(),
        settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }
}

struct Fixture {
    store: Store,
    col: String,
    req: String,
    /// Id of the environment `Dev`, which holds `x=1`.
    dev: String,
}

/// Collection `Col` (auth, scripts, `postman_extra`, the variable `token=old`) → request `Req`; environment `Dev`.
fn fixture() -> Fixture {
    let mut store = Store::open_in_memory().unwrap();
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(collection_content())).unwrap();
    let req = store.create_node(Some(&col), NodeKind::Request, "Req", &NodeContent::Request(request())).unwrap();
    let dev = environment(vec![var("x", "1")]);
    store.save_environment(&dev).unwrap();
    Fixture { store, col, req, dev: dev.id }
}
fn collection_content() -> CollectionContent {
    CollectionContent {
        auth: Some(Auth::Bearer { token: "{{token}}".into() }),
        variables: vec![var("token", "old")],
        scripts: Scripts { pre_request: "console.log(1)".into(), test: "console.log(2)".into() },
        postman_extra: extra(json!({"info": {"name": "Col", "_postman_id": "p-1"}})),
    }
}
fn environment(variables: Vec<Variable>) -> Environment {
    Environment { id: new_id(), kind: EnvKind::Environment, name: "Dev".into(), variables, postman_extra: Extra::new() }
}
fn stored_environment(store: &Store, id: &str) -> Environment {
    store.environments().unwrap().into_iter().find(|environment| environment.id == id).unwrap()
}
fn collection_of_store(store: &Store, id: &str) -> CollectionContent {
    let NodeContent::Collection(content) = store.node(id).unwrap().content else { panic!("not a collection") };
    content
}

#[test]
fn one_rule_for_set_unset_clear() {
    let secret_extra = extra(json!({"id": "x", "type": "secret"}));
    let b = Variable { secret: true, postman_extra: secret_extra, ..var("b", "3") };
    let initial = vec![var("a", "1"), off("a", "2"), b.clone(), var("a", "4")];
    let after = |op: VariableOp| {
        let mut variables = initial.clone();
        apply_to_variables(&mut variables, &op);
        variables
    };
    let set_op = |key: &str, value: &str| VariableOp::Set { key: key.into(), value: value.into() };

    assert_eq!(after(set_op("a", "9")), vec![var("a", "1"), off("a", "2"), b.clone(), var("a", "9")]);
    let changed_b = Variable { value: "t".into(), ..b.clone() };
    assert_eq!(after(set_op("b", "t")), vec![var("a", "1"), off("a", "2"), changed_b, var("a", "4")]);
    assert_eq!(
        after(set_op("n", "v")),
        vec![var("a", "1"), off("a", "2"), b.clone(), var("a", "4"), var("n", "v")]
    );
    let mut only_disabled = vec![off("d", "0")];
    apply_to_variables(&mut only_disabled, &set_op("d", "1"));
    assert_eq!(only_disabled, vec![off("d", "0"), var("d", "1")]);
    assert_eq!(after(VariableOp::Unset { key: "a".into() }), vec![off("a", "2"), b]);
    assert_eq!(after(VariableOp::Clear), vec![off("a", "2")]);
}

#[test]
fn changes_are_applied_over_a_fresh_read() {
    let mut f = fixture();
    // The send has read its input: the snapshot holds `x=1` only.
    let input = load_send_input(&f.store, &f.req, None, Some(&f.dev), AppSettings::default()).unwrap();
    assert_eq!(input.environment.as_ref().unwrap().variables, vec![var("x", "1")]);
    // Another writer saves the environment while the request is on its way.
    let mut edited = stored_environment(&f.store, &f.dev);
    edited.variables.push(var("y", "user"));
    f.store.save_environment(&edited).unwrap();

    let changes = [set(StoredScope::Environment, "x", "2")];
    apply_variable_changes(&mut f.store, &f.col, Some(&f.dev), &changes).unwrap();
    assert_eq!(stored_environment(&f.store, &f.dev).variables, vec![var("x", "2"), var("y", "user")]);
}

#[test]
fn three_scopes_are_written_and_reported() {
    let mut f = fixture();
    let changes = [
        set(StoredScope::Globals, "g", "1"),
        set(StoredScope::Collection, "token", "T"),
        unset(StoredScope::Environment, "x"),
        set(StoredScope::Environment, "z", "3"),
    ];
    let applied = apply_variable_changes(&mut f.store, &f.col, Some(&f.dev), &changes).unwrap();

    assert_eq!(f.store.globals().unwrap().variables, vec![var("g", "1")]);
    assert_eq!(stored_environment(&f.store, &f.dev).variables, vec![var("z", "3")]);
    // Of the collection node only the variables change.
    let expected = CollectionContent { variables: vec![var("token", "T")], ..collection_content() };
    assert_eq!(collection_of_store(&f.store, &f.col), expected);

    let targets = ["globals", f.col.as_str(), f.dev.as_str(), f.dev.as_str()];
    let expected: Vec<AppliedChange> = changes
        .iter()
        .zip(targets)
        .map(|(change, target)| AppliedChange { scope: change.scope, target_id: target.into(), op: change.op.clone() })
        .collect();
    assert_eq!(applied, expected);
}

#[test]
fn globals_row_is_created_when_missing() {
    let mut f = fixture();
    assert_eq!(f.store.globals().unwrap(), Environment::empty_globals(), "nothing is stored yet");
    apply_variable_changes(&mut f.store, &f.col, None, &[set(StoredScope::Globals, "g", "1")]).unwrap();
    let globals = f.store.globals().unwrap();
    assert_eq!((globals.id.as_str(), globals.kind, globals.variables), ("globals", EnvKind::Globals, vec![var("g", "1")]));
}

#[test]
fn missing_targets_are_skipped() {
    let globals_change = set(StoredScope::Globals, "g", "1");
    let changes =
        [set(StoredScope::Environment, "x", "2"), globals_change.clone(), set(StoredScope::Collection, "token", "T")];
    let applied_globals =
        AppliedChange { scope: StoredScope::Globals, target_id: "globals".into(), op: globals_change.op.clone() };

    // No environment is chosen: its changes go nowhere and are not reported.
    let mut f = fixture();
    let applied = apply_variable_changes(&mut f.store, &f.col, None, &changes).unwrap();
    assert_eq!(applied.iter().map(|change| change.scope).collect::<Vec<_>>(), [StoredScope::Globals, StoredScope::Collection]);
    assert_eq!(stored_environment(&f.store, &f.dev).variables, vec![var("x", "1")]);

    // The environment and the collection were deleted while the request was on its way.
    let mut f = fixture();
    f.store.delete_environment(&f.dev).unwrap();
    f.store.delete_node(&f.col).unwrap();
    let applied = apply_variable_changes(&mut f.store, &f.col, Some(&f.dev), &changes).unwrap();
    assert_eq!(applied, vec![applied_globals]);
    assert_eq!(f.store.environments().unwrap(), vec![], "a deleted environment must not come back");
    assert!(f.store.node(&f.col).is_err());
    assert_eq!(f.store.globals().unwrap().variables, vec![var("g", "1")]);
}

#[test]
fn unchanged_list_is_not_written() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("rqm.db");
    let mut store = Store::open(&db).unwrap();
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(collection_content())).unwrap();
    let dev = environment(vec![var("x", "1")]);
    store.save_environment(&dev).unwrap();
    // The same content in a spelling the application never writes: any write would normalise it.
    let spelled = "{ \"variables\": [ { \"key\": \"x\", \"value\": \"1\" } ] }";
    let raw = rusqlite::Connection::open(&db).unwrap();
    raw.execute("UPDATE environment SET content = ?1 WHERE id = ?2", [spelled, dev.id.as_str()]).unwrap();
    let content = || -> String {
        raw.query_row("SELECT content FROM environment WHERE id = ?1", [dev.id.as_str()], |row| row.get(0)).unwrap()
    };

    let applied =
        apply_variable_changes(&mut store, &col, Some(&dev.id), &[unset(StoredScope::Environment, "nope")]).unwrap();
    assert_eq!(applied.len(), 1, "the target exists, so the operation is reported");
    assert_eq!(content(), spelled);
    // And a real change does reach the row.
    apply_variable_changes(&mut store, &col, Some(&dev.id), &[set(StoredScope::Environment, "x", "2")]).unwrap();
    assert_ne!(content(), spelled);
}

#[test]
fn export_keeps_untouched_rows() {
    let source = json!({
        "id": "e-1", "name": "Dev", "_postman_variable_scope": "environment",
        "values": [{"key": "n", "value": 5, "description": "a number", "enabled": true}]
    });
    let environment = rqm_postman::import_environment(&source).unwrap();
    let mut f = fixture();
    f.store.save_environment(&environment).unwrap();

    apply_variable_changes(&mut f.store, &f.col, Some(&environment.id), &[set(StoredScope::Environment, "other", "v")])
        .unwrap();
    let exported = rqm_postman::export_environment(&stored_environment(&f.store, &environment.id));
    let values = exported["values"].as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], source["values"][0], "the untouched row is exported as it came");
    assert_eq!((&values[1]["key"], &values[1]["value"]), (&json!("other"), &json!("v")));
}

#[test]
fn export_of_a_collection_keeps_untouched_variable_rows() {
    let source = json!({
        "info": {"_postman_id": "c-1", "name": "Col",
                 "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
        "item": [{"name": "Req", "request": {"method": "GET", "header": [], "url": {"raw": "http://127.0.0.1:1/x",
                  "protocol": "http", "host": ["127", "0", "0", "1"], "port": "1", "path": ["x"]}}, "response": []}],
        "event": [{"listen": "prerequest", "script": {"type": "text/javascript", "exec": ["console.log('pre')"]}}],
        "auth": {"type": "bearer", "bearer": [{"key": "token", "value": "{{token}}", "type": "string"}]},
        "variable": [{"key": "n", "value": 5, "description": "a number", "id": "v-1"},
                     {"key": "token", "value": "old", "type": "string"}]
    });
    let imported = rqm_postman::import_collection(&source).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    store.insert_nodes(&imported.nodes).unwrap();
    let col = imported.nodes[0].id.clone();
    let before = rqm_postman::export_collection(&store.subtree(&col).unwrap()).unwrap();

    apply_variable_changes(&mut store, &col, None, &[set(StoredScope::Collection, "token", "T")]).unwrap();
    let after = rqm_postman::export_collection(&store.subtree(&col).unwrap()).unwrap();

    assert_eq!(after["variable"][0], source["variable"][0], "the untouched row is exported as it came");
    assert_eq!(after["variable"][1], json!({"key": "token", "value": "T", "type": "string"}));
    // Exactly one path differs between the two exports.
    let mut restored = after.clone();
    restored["variable"][1]["value"] = json!("old");
    assert_eq!(restored, before);
    for key in ["info", "event", "auth", "item"] {
        assert_eq!(after[key], source[key], "{key}");
    }
}
