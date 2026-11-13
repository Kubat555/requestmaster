use rqm_http::{CookieJar, EngineConfig, ReqwestEngine};
use rqm_model::*;
use rqm_runner::*;
use rqm_store::Store;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

fn engine(jar: Arc<CookieJar>) -> (ReqwestEngine, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    (ReqwestEngine::new(EngineConfig { body_dir: dir.path().to_path_buf(), user_agent: "RequestMaster/test".into() }, jar).unwrap(), dir)
}
fn request(url: &str) -> RequestContent {
    RequestContent { method: "POST".into(), url: url.into(), query: vec![], path_variables: vec![],
        headers: vec![], body: Body::Raw { text: "{\"n\":{{n}}}".into(), language: RawLanguage::Json }, auth: None,
        scripts: Scripts::default(), settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }
}
fn settings() -> AppSettings { AppSettings { proxy: rqm_http::ProxySetting::Off, ..Default::default() } }   // independent of the machine's proxy

#[tokio::test] async fn stored_request_is_sent_with_inherited_auth_and_variables() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/items")).and(header("authorization", "Bearer tok")).and(body_string("{\"n\":5}"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "sid=1; Path=/").set_body_string("{\"ok\":true,\"list\":[1,2]}")).mount(&server).await;
    let mut store = Store::open_in_memory().unwrap();
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(CollectionContent {
        auth: Some(Auth::Bearer { token: "{{token}}".into() }),
        variables: vec![Variable { key: "token".into(), value: "tok".into(), secret: true, enabled: true, postman_extra: Extra::new() }],
        scripts: Scripts::default(), postman_extra: Extra::new() })).unwrap();
    let id = store.create_node(Some(&col), NodeKind::Request, "Create", &NodeContent::Request(request("{{base}}/items"))).unwrap();
    let env = Environment { id: new_id(), kind: EnvKind::Environment, name: "Local".into(), postman_extra: Extra::new(),
        variables: vec![Variable { key: "base".into(), value: server.uri(), secret: false, enabled: true, postman_extra: Extra::new() },
                        Variable { key: "n".into(), value: "5".into(), secret: false, enabled: true, postman_extra: Extra::new() }] };
    store.save_environment(&env).unwrap();

    let jar = Arc::new(load_cookies(&store).unwrap());
    let (e, _dir) = engine(jar.clone());
    let input = load_send_input(&store, &id, None, Some(&env.id), settings()).unwrap();
    let out = send(&e, &input, CancellationToken::new()).await;
    let resp = out.result.unwrap();
    assert_eq!(resp.status, 200);
    assert!(out.warnings.is_empty());

    let view = body_view(&resp.body_path, true).unwrap();
    assert_eq!(String::from_utf8(view.bytes).unwrap(), "{\n  \"ok\": true,\n  \"list\": [\n    1,\n    2\n  ]\n}");
    assert!(view.pretty && !view.truncated);
    assert_eq!(body_view(&resp.body_path, false).unwrap().bytes, b"{\"ok\":true,\"list\":[1,2]}");

    save_cookies(&mut store, &jar).unwrap();
    assert_eq!(load_cookies(&store).unwrap().list().len(), 1);
}
#[tokio::test] async fn draft_is_sent_instead_of_the_stored_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/draft")).respond_with(ResponseTemplate::new(204)).mount(&server).await;
    let mut store = Store::open_in_memory().unwrap();
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() })).unwrap();
    let id = store.create_node(Some(&col), NodeKind::Request, "R", &NodeContent::Request(request("http://127.0.0.1:1/stored"))).unwrap();
    let mut draft = request(&format!("{}/draft", server.uri()));
    draft.body = Body::None;
    let (e, _dir) = engine(Arc::new(CookieJar::new()));
    let input = load_send_input(&store, &id, Some(draft), None, settings()).unwrap();
    assert_eq!(send(&e, &input, CancellationToken::new()).await.result.unwrap().status, 204);
}
#[tokio::test] async fn warnings_survive_a_network_error_and_prepare_errors_are_reported() {
    let (e, _dir) = engine(Arc::new(CookieJar::new()));
    let mut r = request("http://127.0.0.1:1/x"); r.scripts.pre_request = "x".into(); r.body = Body::None;
    let input = SendInput { request_name: "R".into(), request: r, ancestors: vec![], environment: None, globals: Environment::empty_globals(), settings: settings() };
    let out = send(&e, &input, CancellationToken::new()).await;
    assert!(matches!(out.result, Err(SendError::Http(rqm_http::HttpError::Connect(_)))));
    assert!(matches!(&out.warnings[..], [SendWarning::ScriptsNotExecuted { .. }]));
    let mut r = request(""); r.body = Body::None;
    let input = SendInput { request_name: "R".into(), request: r, ancestors: vec![], environment: None, globals: Environment::empty_globals(), settings: settings() };
    assert!(matches!(send(&e, &input, CancellationToken::new()).await.result, Err(SendError::Prepare(PrepareError::InvalidUrl(_)))));
}
#[test] fn load_send_input_errors_and_cookie_fallback() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(load_send_input(&store, "missing", None, None, settings()).is_err());
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(CollectionContent { auth: None, variables: vec![], scripts: Scripts::default(), postman_extra: Extra::new() })).unwrap();
    assert!(load_send_input(&store, &col, None, None, settings()).is_err(), "a collection is not a request");
    store.state_set(COOKIE_JAR_KEY, "garbage").unwrap();
    assert!(load_cookies(&store).unwrap().list().is_empty());
}
#[test] fn body_view_limits() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("s"); std::fs::write(&small, b"not json").unwrap();
    let v = body_view(&small, true).unwrap();
    assert_eq!((v.bytes.as_slice(), v.pretty, v.truncated, v.total), (&b"not json"[..], false, false, 8));
    let big = dir.path().join("b");
    let f = std::fs::File::create(&big).unwrap(); f.set_len(INLINE_BODY_LIMIT + 1).unwrap();
    let v = body_view(&big, true).unwrap();
    assert_eq!((v.bytes.len() as u64, v.truncated, v.pretty, v.total), (TRUNCATED_PREVIEW, true, false, INLINE_BODY_LIMIT + 1));
    let edge = dir.path().join("e");
    let f = std::fs::File::create(&edge).unwrap(); f.set_len(INLINE_BODY_LIMIT).unwrap();
    assert!(!body_view(&edge, false).unwrap().truncated);
}

/// A server may put a UTF-8 byte order mark before the JSON; Pretty must still work.
#[test] fn body_view_skips_a_leading_bom() {
    let dir = tempfile::tempdir().unwrap();
    let json = dir.path().join("j"); std::fs::write(&json, b"\xEF\xBB\xBF{\"a\":1}").unwrap();
    let v = body_view(&json, true).unwrap();
    assert!(v.pretty && !v.truncated);
    assert_eq!(v.bytes, b"{\n  \"a\": 1\n}", "the re-indented text has no BOM");
    assert_eq!(v.total, 10);
    let raw = body_view(&json, false).unwrap();
    assert!(!raw.pretty);
    assert_eq!(raw.bytes, b"\xEF\xBB\xBF{\"a\":1}", "the raw view is the body as stored");

    let garbage = dir.path().join("g"); std::fs::write(&garbage, b"\xEF\xBB\xBFnot json").unwrap();
    let v = body_view(&garbage, true).unwrap();
    assert_eq!((v.bytes.as_slice(), v.pretty), (&b"\xEF\xBB\xBFnot json"[..], false));
}
