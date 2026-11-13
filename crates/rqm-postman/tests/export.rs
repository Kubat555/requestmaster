use rqm_model::*;
use rqm_postman::*;

fn load(name: &str) -> serde_json::Value {
    parse_text(&std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()).unwrap()
}
fn req_mut<'a>(nodes: &'a mut [Node], name: &str) -> &'a mut RequestContent {
    match &mut nodes.iter_mut().find(|n| n.name == name).unwrap().content { NodeContent::Request(r) => r, _ => panic!() }
}
fn item<'a>(v: &'a serde_json::Value, path: &[&str]) -> &'a serde_json::Value {
    path.iter().fold(v, |cur, name| cur["item"].as_array().unwrap().iter().find(|i| i["name"] == *name).unwrap())
}

#[test] fn untouched_collection_round_trips_exactly() {
    let src = load("collection_small.json");
    let out = export_collection(&import_collection(&src).unwrap().nodes).unwrap();
    assert_eq!(semantic_diff(&src, &out), Vec::<String>::new());
}
#[test] fn untouched_environment_and_globals_round_trip() {
    for f in ["environment.json", "globals.json"] {
        let src = load(f);
        assert_eq!(semantic_diff(&src, &export_environment(&import_environment(&src).unwrap())), Vec::<String>::new());
    }
}
#[test] fn editing_one_header_keeps_everything_else() {
    let src = load("collection_small.json");
    let mut nodes = import_collection(&src).unwrap().nodes;
    let idx = req_mut(&mut nodes, "R1").headers.iter().position(|h| !h.postman_extra.contains_key("description")).unwrap();
    req_mut(&mut nodes, "R1").headers[idx].value = "changed".into();
    let out = export_collection(&nodes).unwrap();
    let diff = semantic_diff(&src, &out);
    assert_eq!(diff.len(), 1, "only the edited header value may differ");
    assert!(diff[0].ends_with(&format!(".request.header[{idx}].value")));
    let r1 = item(&out, &["F1", "F2", "R1"]);
    assert!(r1["request"]["header"].as_array().unwrap().iter().any(|h| h.get("description").is_some()));
    assert_eq!(r1["request"]["url"], item(&src, &["F1", "F2", "R1"])["request"]["url"]);
}
#[test] fn explicit_null_auth_survives() {
    let src = load("collection_small.json");
    let out = export_collection(&import_collection(&src).unwrap().nodes).unwrap();
    assert!(item(&out, &["R3"])["request"].get("auth").is_some_and(|a| a.is_null()));
}
#[test] fn unsupported_auth_is_never_rewritten() {
    let src = load("collection_small.json");
    let mut nodes = import_collection(&src).unwrap().nodes;
    req_mut(&mut nodes, "R4").method = "PUT".into();
    let out = export_collection(&nodes).unwrap();
    assert_eq!(item(&out, &["R4"])["request"]["auth"], item(&src, &["R4"])["request"]["auth"]);
    assert_eq!(item(&out, &["R4"])["request"]["method"], "PUT");
}
#[test] fn editing_a_script_replaces_only_exec() {
    let src = load("collection_small.json");
    let mut nodes = import_collection(&src).unwrap().nodes;
    req_mut(&mut nodes, "R1").scripts.test = "a\nb".into();
    let out = export_collection(&nodes).unwrap();
    let ev = &item(&out, &["F1", "F2", "R1"])["event"][0];
    assert_eq!(ev["script"]["exec"], serde_json::json!(["a", "b"]));
    assert_eq!(ev["script"]["type"], item(&src, &["F1", "F2", "R1"])["event"][0]["script"]["type"]);
}
#[test] fn rename_and_reorder_are_exported() {
    let mut nodes = import_collection(&load("collection_small.json")).unwrap().nodes;
    nodes.iter_mut().find(|n| n.name == "R3").unwrap().sort_order = 99;
    nodes.iter_mut().find(|n| n.name == "R5").unwrap().name = "R5 renamed".into();
    let out = export_collection(&nodes).unwrap();
    let names: Vec<_> = out["item"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, vec!["F1", "R4", "R5 renamed", "R6", "R3"]);
}
#[test] fn node_created_in_app_exports_valid_v21() {
    let root = Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: "New".into(), sort_order: 0,
        content: NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() }) };
    let r = Node { id: new_id(), parent_id: Some(root.id.clone()), kind: NodeKind::Request, name: "Get".into(), sort_order: 0,
        content: NodeContent::Request(RequestContent { method: "GET".into(), url: "https://example.test/x".into(),
            query: vec![Param { key: "a".into(), value: "1".into(), enabled: true, postman_extra: Extra::new() },
                        Param { key: "b".into(), value: "2".into(), enabled: false, postman_extra: Extra::new() }],
            path_variables: vec![], headers: vec![], body: Body::None, auth: Some(Auth::Bearer { token: "t".into() }),
            scripts: Scripts::default(), settings: RequestSettings::default(), disabled_auto_headers: vec!["accept-encoding".into()],
            postman_extra: Extra::new() }) };
    let out = export_collection(&[r, root]).unwrap();
    assert_eq!(out["item"][0]["protocolProfileBehavior"]["disabledSystemHeaders"], serde_json::json!({"accept-encoding": true}));
    assert!(matches!(detect(&out), Ok(Detected::Collection)));
    assert_eq!(out["info"]["name"], "New");
    let url = &out["item"][0]["request"]["url"];
    assert_eq!(url["raw"], "https://example.test/x?a=1");
    assert_eq!((&url["protocol"], &url["host"], &url["path"]), (&serde_json::json!("https"), &serde_json::json!(["example", "test"]), &serde_json::json!(["x"])));
    assert!(url.get("port").is_none() && url.get("hash").is_none());
    assert_eq!(out["item"][0]["request"]["url"]["query"].as_array().unwrap().len(), 2);
    assert_eq!(out["item"][0]["request"]["auth"]["type"], "bearer");
    // and it imports back to the same typed content
    let back = import_collection(&out).unwrap();
    assert_eq!(back.nodes.len(), 2);
    let NodeContent::Request(b) = &back.nodes[1].content else { panic!() };
    assert_eq!((b.url.as_str(), b.query.len(), b.disabled_auto_headers.len()), ("https://example.test/x", 2, 1));
}
#[test] fn generated_url_parts() {
    let mut nodes = import_collection(&load("collection_small.json")).unwrap().nodes;
    req_mut(&mut nodes, "R5").url = "{{base}}:8080/a/b#frag".into();
    let out = export_collection(&nodes).unwrap();
    let url = &item(&out, &["R5"])["request"]["url"];
    assert!(url.get("protocol").is_none());
    assert_eq!((&url["host"], &url["port"], &url["path"], &url["hash"]),
        (&serde_json::json!(["{{base}}"]), &serde_json::json!("8080"), &serde_json::json!(["a", "b"]), &serde_json::json!("frag")));
}
#[test] fn query_typed_into_the_address_is_exported_as_query() {
    for (typed, raw, hash) in [("https://h.test/api?id=5", "https://h.test/api?id=5&b=2", None),
                               ("https://h.test/api?id=5#frag", "https://h.test/api?id=5&b=2#frag", Some("frag"))] {
        let mut nodes = import_collection(&load("collection_small.json")).unwrap().nodes;
        let r = req_mut(&mut nodes, "R5");
        r.url = typed.into();
        r.query = vec![Param { key: "b".into(), value: "2".into(), enabled: true, postman_extra: Extra::new() }];
        let out = export_collection(&nodes).unwrap();
        let url = &item(&out, &["R5"])["request"]["url"];
        assert_eq!(url["raw"], raw);
        assert_eq!(url["path"], serde_json::json!(["api"]));
        let pairs: Vec<_> = url["query"].as_array().unwrap().iter().map(|q| (q["key"].as_str().unwrap(), q["value"].as_str().unwrap())).collect();
        assert_eq!(pairs, vec![("id", "5"), ("b", "2")]);
        assert_eq!(url.get("hash").and_then(|h| h.as_str()), hash);
        // and it imports back with the query out of the address
        let mut back = import_collection(&out).unwrap().nodes;
        let b = req_mut(&mut back, "R5");
        assert_eq!(b.url, typed.replace("?id=5", ""));
        let pairs: Vec<_> = b.query.iter().map(|q| (q.key.as_str(), q.value.as_str(), q.enabled)).collect();
        assert_eq!(pairs, vec![("id", "5", true), ("b", "2", true)]);
    }
}
#[test] fn editing_one_environment_variable_keeps_the_others() {
    let src = load("environment.json");
    let mut e = import_environment(&src).unwrap();
    let idx = e.variables.iter().position(|v| v.secret).unwrap();
    e.variables[idx].value = "changed".into();
    let diff = semantic_diff(&src, &export_environment(&e));
    assert_eq!(diff, vec![format!("$.values[{idx}].value")]);     // the number, the null and the missing `type` survive
}
#[test] fn export_errors() {
    assert!(matches!(export_collection(&[]), Err(ExportError::NoCollectionRoot)));
}
#[test] fn globals_merge_rules() {
    let v = |k: &str, val: &str| Variable { key: k.into(), value: val.into(), secret: false, enabled: true, postman_extra: Extra::new() };
    let cur = vec![v("a", "1"), v("b", "2")];
    let inc = vec![v("b", "20"), v("a", "1"), v("c", "3")];
    assert_eq!(globals_conflicts(&cur, &inc), vec!["b".to_string()]);
    let take: Vec<_> = merge_globals(&cur, &inc, MergePolicy::TakeIncoming).into_iter().map(|x| (x.key, x.value)).collect();
    assert_eq!(take, vec![("a".into(), "1".into()), ("b".into(), "20".into()), ("c".into(), "3".into())]);
    let keep: Vec<_> = merge_globals(&cur, &inc, MergePolicy::KeepCurrent).into_iter().map(|x| (x.key, x.value)).collect();
    assert_eq!(keep, vec![("a".into(), "1".into()), ("b".into(), "2".into()), ("c".into(), "3".into())]);
}
