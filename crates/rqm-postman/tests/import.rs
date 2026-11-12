use rqm_model::*;
use rqm_postman::*;

fn load(name: &str) -> serde_json::Value {
    parse_text(&std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()).unwrap()
}
fn find<'a>(nodes: &'a [Node], name: &str) -> &'a Node { nodes.iter().find(|n| n.name == name).unwrap() }
fn req(n: &Node) -> &RequestContent { match &n.content { NodeContent::Request(r) => r, _ => panic!("not a request") } }

#[test] fn detects_formats() {
    assert!(matches!(detect(&load("collection_small.json")), Ok(Detected::Collection)));
    assert!(matches!(detect(&load("environment.json")), Ok(Detected::Environment)));
    assert!(matches!(detect(&load("globals.json")), Ok(Detected::Globals)));
    assert!(matches!(detect(&serde_json::json!({"info": {"schema": "https://schema.getpostman.com/json/collection/v2.0.0/collection.json"}})), Err(ImportError::UnsupportedSchema(_))));
    assert!(matches!(detect(&serde_json::json!([1, 2])), Err(ImportError::UnknownFormat)));
}
#[test] fn bom_is_stripped() { assert!(parse_text("\u{feff}{\"a\":1}").is_ok()); }
#[test] fn not_json_is_an_error() { assert!(matches!(parse_text("<html>"), Err(ImportError::NotJson(_)))); }

#[test] fn tree_shape_and_order() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    assert_eq!(c.nodes[0].kind, NodeKind::Collection);
    assert_eq!(c.nodes.iter().filter(|n| n.kind == NodeKind::Folder).count(), 2);
    assert_eq!(c.nodes.iter().filter(|n| n.kind == NodeKind::Request).count(), 6);
    let root = &c.nodes[0].id;
    let top: Vec<_> = c.nodes.iter().filter(|n| n.parent_id.as_ref() == Some(root)).map(|n| (n.name.as_str(), n.sort_order)).collect();
    assert_eq!(top, vec![("F1", 0), ("R3", 1), ("R4", 2), ("R5", 3), ("R6", 4)]);
    assert_eq!(find(&c.nodes, "F2").parent_id.as_ref(), Some(&find(&c.nodes, "F1").id));
    // every parent precedes its children
    for (i, n) in c.nodes.iter().enumerate().skip(1) {
        let p = n.parent_id.as_ref().unwrap();
        assert!(c.nodes[..i].iter().any(|x| &x.id == p));
    }
}
#[test] fn request_r1_fields() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    let r = req(find(&c.nodes, "R1"));
    assert_eq!(r.method, "POST");
    assert!(!r.url.contains('?'));
    assert_eq!(r.query.iter().filter(|p| !p.enabled).count(), 1);
    assert_eq!(r.path_variables.len(), 1);
    assert_eq!(r.headers.iter().filter(|p| !p.enabled).count(), 1);
    assert!(r.headers.iter().any(|p| p.postman_extra.contains_key("description")));
    assert!(matches!(&r.body, Body::Raw { language: RawLanguage::Json, .. }));
    assert!(r.scripts.test.contains('\n'));
    assert_eq!(r.settings, RequestSettings { follow_redirects: Some(false), max_redirects: None, strict_ssl: None, disable_body_pruning: Some(true) });
    assert_eq!(r.auth, None);
}
#[test] fn auth_variants() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    assert!(matches!(&c.nodes[0].content, NodeContent::Collection(x) if matches!(x.auth, Some(Auth::Bearer { .. }))));
    assert_eq!(req(find(&c.nodes, "R3")).auth, None);
    assert!(matches!(req(find(&c.nodes, "R4")).auth, Some(Auth::Unsupported { ref auth_type }) if auth_type == "awsv4"));
    assert_eq!(req(find(&c.nodes, "R5")).auth, Some(Auth::NoAuth));
    assert!(matches!(req(find(&c.nodes, "R6")).auth, Some(Auth::ApiKey { placement: ApiKeyPlacement::Query, .. })));
}
#[test] fn bodies_and_files() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    assert_eq!(req(find(&c.nodes, "R2")).method, "GET");
    match &req(find(&c.nodes, "R3")).body {
        Body::FormData(parts) => {
            assert!(parts.iter().any(|p| matches!(p.value, FormValue::Text(_))));
            assert!(parts.iter().any(|p| matches!(&p.value, FormValue::Files(f) if f.len() == 1 && !f[0].confirmed)));
        }
        _ => panic!("expected form-data"),
    }
    assert_eq!(req(find(&c.nodes, "R3")).headers.len(), 2);
    assert!(matches!(&req(find(&c.nodes, "R4")).body, Body::Binary { file: Some(f) } if !f.confirmed));
    assert!(matches!(&req(find(&c.nodes, "R5")).body, Body::UrlEncoded(_)));
    assert!(matches!(&req(find(&c.nodes, "R6")).body, Body::GraphQl { .. }));
}
#[test] fn collection_variables_and_extra() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    let NodeContent::Collection(col) = &c.nodes[0].content else { panic!() };
    assert_eq!(col.variables.len(), 2);
    assert_eq!(col.variables.iter().filter(|v| !v.enabled).count(), 1);
    assert!(!col.scripts.pre_request.is_empty());
    assert!(col.postman_extra.contains_key("_exporter_id"));
    assert!(col.postman_extra.contains_key("info"));
    assert!(!col.postman_extra.contains_key("item"));
}
#[test] fn report_lists_everything_unsupported() {
    let c = import_collection(&load("collection_small.json")).unwrap();
    let count = |k: ReportKind| c.report.items.iter().filter(|i| std::mem::discriminant(&i.kind) == std::mem::discriminant(&k)).count();
    assert_eq!(count(ReportKind::UnsupportedAuth), 1);
    assert_eq!(count(ReportKind::SettingNotApplied), 1);          // insecureHTTPParser
    assert_eq!(count(ReportKind::HasScripts), 2);                 // collection + R1
    assert_eq!(c.report.items.len(), 6);
    assert_eq!(count(ReportKind::FileReference), 2);              // R3 file part + R4 binary
    let item = c.report.items.iter().find(|i| matches!(i.kind, ReportKind::SettingNotApplied)).unwrap();
    assert_eq!(item.detail, "insecureHTTPParser");
    assert!(item.node_path.ends_with("F1 / F2 / R1"));
}
#[test] fn environment_and_globals() {
    let e = import_environment(&load("environment.json")).unwrap();
    assert_eq!(e.kind, EnvKind::Environment);
    assert!(e.variables.iter().any(|v| v.secret));
    assert!(e.variables.iter().any(|v| !v.enabled));
    assert!(e.postman_extra.contains_key("values") && !e.postman_extra.contains_key("name"));
    assert_eq!(e.variables.iter().find(|v| v.key == "num").unwrap().value, "5");
    assert_eq!(e.variables.iter().find(|v| v.key == "nil").unwrap().value, "");
    let g = import_environment(&load("globals.json")).unwrap();
    assert_eq!((g.id.as_str(), g.kind), (GLOBALS_ID, EnvKind::Globals));
}
#[test] fn unknown_shapes_do_not_break_import() {
    let j = serde_json::json!({"info": {"name": "x", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
        "item": [{"name": "odd", "request": {"method": 5, "url": {"raw": 7}, "header": null, "body": {"mode": "weird"}, "auth": {"type": 3}}, "event": "nope"}]});
    let c = import_collection(&j).unwrap();
    assert_eq!(c.nodes.len(), 2);
}
#[test] fn semantic_diff_ignores_key_order_only() {
    let a = serde_json::json!({"a": 1, "b": [1, 2]});
    assert!(semantic_diff(&a, &serde_json::json!({"b": [1, 2], "a": 1})).is_empty());
    assert_eq!(semantic_diff(&a, &serde_json::json!({"a": 1, "b": [2, 1]})).len(), 2);
    assert_eq!(semantic_diff(&a, &serde_json::json!({"a": null, "b": [1, 2]})), vec!["$.a".to_string()]);
}
