use rqm_model::*;
use rqm_store::*;

fn folder() -> NodeContent { NodeContent::Folder(FolderContent { auth: None, scripts: Scripts::default(), postman_extra: Extra::new() }) }
fn collection() -> NodeContent { NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() }) }
fn request(method: &str) -> NodeContent { NodeContent::Request(RequestContent { method: method.into(), url: "https://example.test".into(),
    query: vec![], path_variables: vec![], headers: vec![], body: Body::None, auth: None, scripts: Scripts::default(),
    settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }) }
/// C ─ F1 ─ F2 ─ R1 ; C ─ R2 ; C ─ R3
fn seeded() -> (Store, [NodeId; 6]) {
    let mut s = Store::open_in_memory().unwrap();
    let c = s.create_node(None, NodeKind::Collection, "C", &collection()).unwrap();
    let f1 = s.create_node(Some(&c), NodeKind::Folder, "F1", &folder()).unwrap();
    let f2 = s.create_node(Some(&f1), NodeKind::Folder, "F2", &folder()).unwrap();
    let r1 = s.create_node(Some(&f2), NodeKind::Request, "R1", &request("POST")).unwrap();
    let r2 = s.create_node(Some(&c), NodeKind::Request, "R2", &request("GET")).unwrap();
    let r3 = s.create_node(Some(&c), NodeKind::Request, "R3", &request("DELETE")).unwrap();
    (s, [c, f1, f2, r1, r2, r3])
}
fn children(s: &Store, parent: &str) -> Vec<(String, i64)> {
    s.tree().unwrap().into_iter().filter(|r| r.parent_id.as_deref() == Some(parent)).map(|r| (r.name, r.sort_order)).collect()
}

#[test] fn schema_has_exactly_four_tables_and_version_2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("app.db");
    drop(Store::open(&path).unwrap());
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut names: Vec<String> = conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table','index') AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    names.sort();
    assert_eq!(names, vec!["app_state", "environment", "history", "node"]);
    assert_eq!(conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
    for table in ["node", "environment", "app_state", "history"] {
        let sql: String = conn.query_row("SELECT sql FROM sqlite_master WHERE name = ?1", [table], |r| r.get(0)).unwrap();
        assert!(sql.matches("--").count() >= 4, "{table}: table and column comments must be stored with the schema");
    }
    let cols: Vec<String> = conn.prepare("SELECT name FROM pragma_table_info('environment')").unwrap().query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
    assert_eq!(cols, vec!["id", "name", "content"]);
}
#[test] fn reopen_keeps_data_and_newer_schema_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    { let mut s = Store::open(&path).unwrap(); s.create_node(None, NodeKind::Collection, "Привет", &collection()).unwrap(); }
    assert_eq!(Store::open(&path).unwrap().tree().unwrap()[0].name, "Привет");
    rusqlite::Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 3").unwrap();
    assert!(matches!(Store::open(&path), Err(StoreError::NewerSchema(3))));
}
#[test] fn database_path_with_non_ascii_characters_works() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Профиль Ӯ 测试").join("app.db");
    { let mut s = Store::open(&path).unwrap(); s.state_set("k", "значение").unwrap(); }
    assert_eq!(Store::open(&path).unwrap().state_get("k").unwrap().as_deref(), Some("значение"));
}
#[test] fn default_path_is_under_local_app_data() {
    let p = Store::default_path().unwrap();
    assert!(p.ends_with("RequestMaster/app.db") || p.ends_with("RequestMaster\\app.db"));
    assert!(p.starts_with(std::env::var("LOCALAPPDATA").unwrap()));
}
#[test] fn tree_rows_carry_method_and_order() {
    let (s, [c, ..]) = seeded();
    assert_eq!(children(&s, &c), vec![("F1".into(), 0), ("R2".into(), 1), ("R3".into(), 2)]);
    let rows = s.tree().unwrap();
    assert_eq!(rows.iter().find(|r| r.name == "R1").unwrap().method.as_deref(), Some("POST"));
    assert_eq!(rows.iter().find(|r| r.name == "F1").unwrap().method, None);
}
#[test] fn subtree_and_ancestors() {
    let (s, [c, f1, f2, r1, ..]) = seeded();
    let names: Vec<_> = s.subtree(&f1).unwrap().into_iter().map(|n| n.name).collect();
    assert_eq!(names, vec!["F1", "F2", "R1"]);
    assert_eq!(s.subtree(&c).unwrap().len(), 6);
    let anc: Vec<_> = s.ancestors(&r1).unwrap().into_iter().map(|n| n.id).collect();
    assert_eq!(anc, vec![c.clone(), f1, f2]);
    assert!(s.ancestors(&c).unwrap().is_empty());
    assert!(matches!(s.node("missing"), Err(StoreError::NotFound(_))));
}
#[test] fn insert_nodes_is_atomic() {
    let mut s = Store::open_in_memory().unwrap();
    let root = Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: "C".into(), sort_order: 0, content: collection() };
    let orphan = Node { id: new_id(), parent_id: Some("missing".into()), kind: NodeKind::Request, name: "R".into(), sort_order: 0, content: request("GET") };
    assert!(s.insert_nodes(&[root, orphan]).is_err());
    assert!(s.tree().unwrap().is_empty());
}
#[test] fn imported_collections_are_appended_in_order() {
    let mut s = Store::open_in_memory().unwrap();
    for name in ["First", "Second"] {
        let root = Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: name.into(), sort_order: 0, content: collection() };
        let child = Node { id: new_id(), parent_id: Some(root.id.clone()), kind: NodeKind::Request, name: "R".into(), sort_order: 0, content: request("GET") };
        s.insert_nodes(&[root, child]).unwrap();
    }
    let roots: Vec<_> = s.tree().unwrap().into_iter().filter(|r| r.parent_id.is_none()).map(|r| (r.name, r.sort_order)).collect();
    assert_eq!(roots, vec![("First".into(), 0), ("Second".into(), 1)]);
}
#[test] fn rename_update_delete() {
    let (mut s, [c, f1, _, r1, ..]) = seeded();
    s.rename_node(&r1, "Новое имя").unwrap();
    s.update_content(&r1, &request("PATCH")).unwrap();
    let n = s.node(&r1).unwrap();
    assert_eq!(n.name, "Новое имя");
    assert!(matches!(n.content, NodeContent::Request(ref r) if r.method == "PATCH"));
    s.delete_node(&f1).unwrap();
    assert_eq!(s.subtree(&c).unwrap().len(), 3);                 // C, R2, R3
    assert_eq!(children(&s, &c), vec![("R2".into(), 0), ("R3".into(), 1)]);
}
#[test] fn delete_removes_every_descendant_and_keeps_the_foreign_keys_on() {
    let (mut s, [c, f1, f2, r1, r2, r3]) = seeded();
    s.delete_node(&f1).unwrap();
    let left: Vec<NodeId> = s.tree().unwrap().into_iter().map(|r| r.id).collect();
    assert!([&f1, &f2, &r1].iter().all(|id| !left.contains(id)));
    assert!([&c, &r2, &r3].iter().all(|id| left.contains(id)));
    assert!(matches!(s.delete_node(&f1), Err(StoreError::NotFound(_))));
    // After a deletion and after a refused one a node still cannot point at a parent that does not exist.
    let orphan = Node { id: new_id(), parent_id: Some(f1.clone()), kind: NodeKind::Request, name: "R".into(), sort_order: 0, content: request("GET") };
    assert!(s.insert_nodes(&[orphan]).is_err());
    s.delete_node(&c).unwrap();
    assert!(s.tree().unwrap().is_empty());
}
#[test] fn move_rules() {
    let (mut s, [c, f1, f2, r1, r2, r3]) = seeded();
    assert!(matches!(s.move_node(&f1, Some(&f2), 0), Err(StoreError::InvalidMove(_))));   // into own descendant
    assert!(matches!(s.move_node(&f1, Some(&f1), 0), Err(StoreError::InvalidMove(_))));   // into itself
    assert!(matches!(s.move_node(&r2, Some(&r3), 0), Err(StoreError::InvalidMove(_))));   // request as a parent
    assert!(matches!(s.move_node(&r2, None, 0), Err(StoreError::InvalidMove(_))));        // request as a root
    assert!(matches!(s.move_node(&c, Some(&f1), 0), Err(StoreError::InvalidMove(_))));    // collection under a folder
    assert_eq!(s.subtree(&c).unwrap().len(), 6, "failed moves change nothing");
    s.move_node(&r3, Some(&c), 0).unwrap();
    assert_eq!(children(&s, &c), vec![("R3".into(), 0), ("F1".into(), 1), ("R2".into(), 2)]);
    s.move_node(&r1, Some(&c), 99).unwrap();
    assert_eq!(children(&s, &c), vec![("R3".into(), 0), ("F1".into(), 1), ("R2".into(), 2), ("R1".into(), 3)]);
    assert!(children(&s, &f2).is_empty());
}
#[test] fn move_within_a_parent_and_between_collections() {
    let (mut s, [c, f1, f2, r1, r2, r3]) = seeded();
    // `index` counts the children of the new parent without the moved node
    s.move_node(&f1, Some(&c), 1).unwrap();
    assert_eq!(children(&s, &c), vec![("R2".into(), 0), ("F1".into(), 1), ("R3".into(), 2)]);
    s.move_node(&r3, Some(&c), 0).unwrap();
    assert_eq!(children(&s, &c), vec![("R3".into(), 0), ("R2".into(), 1), ("F1".into(), 2)]);
    s.move_node(&r3, Some(&c), 0).unwrap();
    assert_eq!(children(&s, &c), vec![("R3".into(), 0), ("R2".into(), 1), ("F1".into(), 2)], "moving to its own place changes nothing");
    assert_eq!(children(&s, &f1), vec![("F2".into(), 0)], "the descendants stay with the moved folder");

    let roots = |s: &Store| -> Vec<(String, i64)> { s.tree().unwrap().into_iter().filter(|r| r.parent_id.is_none()).map(|r| (r.name, r.sort_order)).collect() };
    let c2 = s.create_node(None, NodeKind::Collection, "C2", &collection()).unwrap();
    s.create_node(Some(&c2), NodeKind::Request, "R4", &request("GET")).unwrap();
    assert_eq!(roots(&s), vec![("C".into(), 0), ("C2".into(), 1)]);
    s.move_node(&c2, None, 0).unwrap();
    assert_eq!(roots(&s), vec![("C2".into(), 0), ("C".into(), 1)]);

    // a request into another collection: both lists of children are renumbered 0..n
    s.move_node(&r2, Some(&c2), 0).unwrap();
    assert_eq!(children(&s, &c2), vec![("R2".into(), 0), ("R4".into(), 1)]);
    assert_eq!(children(&s, &c), vec![("R3".into(), 0), ("F1".into(), 1)]);
    assert_eq!(s.node(&r2).unwrap().parent_id.as_deref(), Some(c2.as_str()));
    assert_eq!(s.ancestors(&r2).unwrap().into_iter().map(|n| n.id).collect::<Vec<_>>(), vec![c2.clone()]);
    // a folder with its descendants into another collection
    s.move_node(&f1, Some(&c2), 1).unwrap();
    assert_eq!(children(&s, &c2), vec![("R2".into(), 0), ("F1".into(), 1), ("R4".into(), 2)]);
    assert_eq!(children(&s, &c), vec![("R3".into(), 0)]);
    assert_eq!(s.ancestors(&r1).unwrap().into_iter().map(|n| n.id).collect::<Vec<_>>(), vec![c2.clone(), f1.clone(), f2.clone()]);
    assert_eq!(s.subtree(&c2).unwrap().len(), 6);
}
#[test] fn duplicate_is_deep_with_fresh_ids() {
    let (mut s, [c, f1, ..]) = seeded();
    let copy = s.duplicate_node(&f1).unwrap();
    assert_eq!(children(&s, &c), vec![("F1".into(), 0), ("F1 Copy".into(), 1), ("R2".into(), 2), ("R3".into(), 3)]);
    let ids: std::collections::HashSet<_> = s.subtree(&c).unwrap().into_iter().map(|n| n.id).collect();
    assert_eq!(ids.len(), 9);
    assert_eq!(s.subtree(&copy).unwrap().iter().map(|n| n.name.clone()).collect::<Vec<_>>(), vec!["F1 Copy", "F2", "R1"]);
}
#[test] fn environments_and_globals() {
    let mut s = Store::open_in_memory().unwrap();
    assert_eq!(s.globals().unwrap(), Environment::empty_globals());
    let var = Variable { key: "k".into(), value: "v".into(), secret: true, enabled: true, postman_extra: Extra::new() };
    let mut g = Environment::empty_globals(); g.variables.push(var.clone());
    s.save_environment(&g).unwrap();
    s.save_environment(&g).unwrap();
    assert_eq!(s.globals().unwrap().variables, vec![var.clone()]);
    for name in ["beta", "Яблоко", "Alpha", "арбуз"] {
        s.save_environment(&Environment { id: new_id(), kind: EnvKind::Environment, name: name.into(), variables: vec![var.clone()], postman_extra: Extra::new() }).unwrap();
    }
    let envs = s.environments().unwrap();
    assert_eq!(envs.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["Alpha", "beta", "арбуз", "Яблоко"]);
    s.delete_environment(&envs[0].id).unwrap();
    assert_eq!(s.environments().unwrap().len(), 3);
}
#[test] fn app_state_round_trip() {
    let mut s = Store::open_in_memory().unwrap();
    assert_eq!(s.state_get("cookie_jar").unwrap(), None);
    s.state_set("cookie_jar", "[]").unwrap();
    s.state_set("cookie_jar", "[1]").unwrap();
    assert_eq!(s.state_get("cookie_jar").unwrap().as_deref(), Some("[1]"));
}

/// DDL of schema version 1: `schema.sql` has not changed since that version.
const SCHEMA_V1: &str = include_str!("../src/schema.sql");

fn user_version(conn: &rusqlite::Connection) -> i64 { conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap() }
fn table_names(conn: &rusqlite::Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

#[test] fn database_of_version_1_gets_history_and_keeps_its_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(&format!("BEGIN;\n{SCHEMA_V1}\nPRAGMA user_version = 1;\nCOMMIT;")).unwrap();
        conn.execute("INSERT INTO node (id, parent_id, kind, name, sort_order, content) VALUES ('c1', NULL, 'collection', 'Старая', 0, '{}')", []).unwrap();
        conn.execute("INSERT INTO app_state (key, value) VALUES ('k', 'v')", []).unwrap();
        assert_eq!(table_names(&conn), vec!["app_state", "environment", "node"]);
    }
    {
        let mut s = Store::open(&path).unwrap();
        let rows = s.tree().unwrap();
        assert_eq!(rows.iter().map(|r| (r.id.as_str(), r.name.as_str())).collect::<Vec<_>>(), vec![("c1", "Старая")]);
        assert_eq!(s.state_get("k").unwrap().as_deref(), Some("v"));
        assert!(s.history_list(10, None).unwrap().is_empty());
        s.history_add("2026-11-13T10:42:07.123Z", "{}", "{}", 500).unwrap();
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(user_version(&conn), 2);
    assert_eq!(table_names(&conn), vec!["app_state", "environment", "history", "node"]);
    drop(conn);
    // the upgraded file opens again like any database of version 2
    let s = Store::open(&path).unwrap();
    assert_eq!(s.tree().unwrap().len(), 1);
    assert_eq!(s.history_list(10, None).unwrap().len(), 1);
}
#[test] fn file_with_tables_but_version_0_is_still_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("other.db");
    rusqlite::Connection::open(&path).unwrap().execute_batch("CREATE TABLE something (x INTEGER)").unwrap();
    assert!(matches!(Store::open(&path), Err(StoreError::Sqlite(_))));
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert_eq!((user_version(&conn), table_names(&conn)), (0, vec!["something".to_string()]), "the foreign file is left as it was");
}
#[test] fn history_add_keeps_only_the_newest() {
    let mut s = Store::open_in_memory().unwrap();
    let ids: Vec<i64> = (1..=5).map(|n| s.history_add(&format!("t{n}"), &format!("s{n}"), &format!("d{n}"), 3).unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids grow: {ids:?}");
    let rows = s.history_list(10, None).unwrap();
    assert_eq!(rows.iter().map(|r| r.summary.as_str()).collect::<Vec<_>>(), vec!["s5", "s4", "s3"]);
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![ids[4], ids[3], ids[2]]);
    assert!(matches!(s.history_detail(ids[0]), Err(StoreError::NotFound(_))), "the oldest entries are gone");
    assert_eq!(s.history_detail(ids[4]).unwrap(), "d5");
}
#[test] fn history_list_detail_and_clear() {
    let mut s = Store::open_in_memory().unwrap();
    let ids: Vec<i64> = (1..=4).map(|n| s.history_add(&format!("t{n}"), &format!("s{n}"), &format!("d{n}"), 500).unwrap()).collect();
    let all = s.history_list(10, None).unwrap();
    assert_eq!(all[0], HistoryRow { id: ids[3], sent_at: "t4".into(), summary: "s4".into() });
    assert_eq!(all.iter().map(|r| r.id).collect::<Vec<_>>(), vec![ids[3], ids[2], ids[1], ids[0]], "newest first");
    assert_eq!(s.history_list(2, None).unwrap().iter().map(|r| r.id).collect::<Vec<_>>(), vec![ids[3], ids[2]]);
    assert_eq!(s.history_list(2, Some(ids[2])).unwrap().iter().map(|r| r.id).collect::<Vec<_>>(), vec![ids[1], ids[0]]);
    assert!(s.history_list(10, Some(ids[0])).unwrap().is_empty());
    assert_eq!(s.history_detail(ids[1]).unwrap(), "d2");
    assert!(matches!(s.history_detail(987_654), Err(StoreError::NotFound(_))));
    s.history_clear().unwrap();
    assert!(s.history_list(10, None).unwrap().is_empty());
    assert!(matches!(s.history_detail(ids[1]), Err(StoreError::NotFound(_))));
    s.history_clear().unwrap();
}
#[test] fn state_delete_and_entries() {
    let mut s = Store::open_in_memory().unwrap();
    s.state_delete("never-written").unwrap();
    assert!(s.state_entries("draft:").unwrap().is_empty());
    for (key, value) in [("draft:b", "2"), ("drafts", "x"), ("draft:a", "1"), ("window_state", "{}"), ("abc", "p"), ("a%x", "q"), ("a_c", "r")] {
        s.state_set(key, value).unwrap();
    }
    let pairs = |list: &[(&str, &str)]| list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
    assert_eq!(s.state_entries("draft:").unwrap(), pairs(&[("draft:a", "1"), ("draft:b", "2")]), "ordered by key, without `drafts`");
    // the prefix is compared literally: `%` and `_` are not wildcards
    assert_eq!(s.state_entries("a%").unwrap(), pairs(&[("a%x", "q")]));
    assert_eq!(s.state_entries("a_").unwrap(), pairs(&[("a_c", "r")]));
    assert_eq!(s.state_entries("").unwrap().len(), 7, "an empty prefix gives every key");
    s.state_delete("draft:a").unwrap();
    assert_eq!(s.state_get("draft:a").unwrap(), None);
    assert_eq!(s.state_entries("draft:").unwrap(), pairs(&[("draft:b", "2")]));
    s.state_delete("draft:a").unwrap();
    assert_eq!(s.state_get("drafts").unwrap().as_deref(), Some("x"), "the other keys stay");
}
