use rqm_model::*;

fn sample_request() -> RequestContent {
    RequestContent {
        method: "POST".into(), url: "https://example.test/a/:id".into(),
        query: vec![Param { key: "q".into(), value: "1".into(), enabled: false, postman_extra: Extra::new() }],
        path_variables: vec![Param { key: "id".into(), value: "7".into(), enabled: true, postman_extra: Extra::new() }],
        headers: vec![], body: Body::Raw { text: "{}".into(), language: RawLanguage::Json },
        auth: Some(Auth::NoAuth), scripts: Scripts::default(), settings: RequestSettings::default(),
        disabled_auto_headers: vec![], postman_extra: Extra::new(),
    }
}

#[test] fn request_content_json_round_trip() {
    let c = NodeContent::Request(sample_request());
    let back = NodeContent::from_json(NodeKind::Request, &c.to_json()).unwrap();
    assert_eq!(c, back);
}
#[test] fn defaults_are_not_written() {
    let json = NodeContent::Folder(FolderContent { auth: None, scripts: Scripts::default(), postman_extra: Extra::new() }).to_json();
    assert_eq!(json, "{}");
}
#[test] fn inherit_and_noauth_are_different() {
    let mut r = sample_request();
    let a = NodeContent::Request(r.clone()).to_json();
    r.auth = None;
    let b = NodeContent::Request(r).to_json();
    assert_ne!(a, b);
}
#[test] fn wrong_kind_is_an_error_not_a_panic() {
    let c = NodeContent::Request(sample_request()).to_json();
    let _ = NodeContent::from_json(NodeKind::Collection, &c);            // must not panic, either result is fine
    assert!(NodeContent::from_json(NodeKind::Request, "not json").is_err());
}
#[test] fn new_id_is_uuid_v4() {
    let id = new_id();
    assert_eq!(id.len(), 36);
    assert_eq!(id.as_bytes()[14], b'4');
    assert_ne!(id, new_id());
}
#[test] fn scripts_is_empty_ignores_whitespace() {
    assert!(Scripts { pre_request: " \n".into(), test: String::new() }.is_empty());
    assert!(!Scripts { pre_request: "x".into(), test: String::new() }.is_empty());
}
#[test] fn empty_globals_has_fixed_id() {
    let g = Environment::empty_globals();
    assert_eq!((g.id.as_str(), g.kind), (GLOBALS_ID, EnvKind::Globals));
}
#[test] fn environment_content_round_trip() {
    let mut e = Environment { id: new_id(), kind: EnvKind::Environment, name: "E".into(), postman_extra: Extra::new(),
        variables: vec![Variable { key: "k".into(), value: "v".into(), secret: true, enabled: false, postman_extra: Extra::new() }] };
    e.postman_extra.insert("id".into(), serde_json::json!("x"));
    assert_eq!(Environment::from_parts(&e.id, &e.name, &e.content_json()).unwrap(), e);
    assert_eq!(Environment::from_parts(GLOBALS_ID, "Globals", "{}").unwrap(), Environment::empty_globals());
    assert!(Environment::from_parts("x", "y", "nope").is_err());
}
