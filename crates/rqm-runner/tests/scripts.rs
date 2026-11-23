//! The send pipeline with scripts: pre-request → substitution → HTTP → test.
//! The tree of every test: collection `Col` → folder `A` → folder `B` → request `Req`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rqm_http::{CookieJar, EngineConfig, HttpError, ProxySetting, ReqwestEngine};
use rqm_model::*;
use rqm_runner::*;
use rqm_store::Store;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{body_string, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MIB: usize = 1024 * 1024;

fn var(key: &str, value: &str) -> Variable {
    Variable { key: key.into(), value: value.into(), secret: false, enabled: true, postman_extra: Extra::new() }
}
fn scripts(pre_request: &str, test: &str) -> Scripts {
    Scripts { pre_request: pre_request.into(), test: test.into() }
}
/// `console.log('<node> pre')` and `console.log('<node> test')`.
fn logging(node: &str) -> Scripts {
    scripts(&format!("console.log('{node} pre')"), &format!("console.log('{node} test')"))
}
fn get(url: &str, scripts: Scripts) -> RequestContent {
    RequestContent { method: "GET".into(), url: url.into(), query: vec![], path_variables: vec![], headers: vec![],
        body: Body::None, auth: None, scripts, settings: RequestSettings::default(), disabled_auto_headers: vec![],
        postman_extra: Extra::new() }
}

struct Tree {
    store: Store,
    col: String,
    a: String,
    b: String,
    req: String,
    /// Id of the environment `Dev`.
    dev: String,
}

/// The tree in a store in memory, with the environment `Dev` (not chosen until a test passes its id).
fn tree(variables: Vec<Variable>, col: Scripts, a: Scripts, b: Scripts, request: RequestContent) -> Tree {
    let mut store = Store::open_in_memory().unwrap();
    let content = CollectionContent { auth: None, variables, scripts: col, postman_extra: Extra::new() };
    let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(content)).unwrap();
    let folder = |scripts| NodeContent::Folder(FolderContent { auth: None, scripts, postman_extra: Extra::new() });
    let a = store.create_node(Some(&col), NodeKind::Folder, "A", &folder(a)).unwrap();
    let b = store.create_node(Some(&a), NodeKind::Folder, "B", &folder(b)).unwrap();
    let req = store.create_node(Some(&b), NodeKind::Request, "Req", &NodeContent::Request(request)).unwrap();
    let dev = Environment { id: new_id(), kind: EnvKind::Environment, name: "Dev".into(), variables: vec![],
        postman_extra: Extra::new() };
    store.save_environment(&dev).unwrap();
    Tree { store, col, a, b, req, dev: dev.id }
}

/// The input of a send of `Req`, scripts on; `Dev` is chosen when `with_dev`.
fn input(tree: &Tree, with_dev: bool) -> SendInput {
    let settings = AppSettings { proxy: ProxySetting::Off, ..Default::default() };
    let environment = with_dev.then_some(tree.dev.as_str());
    let mut input = load_send_input(&tree.store, &tree.req, None, environment, settings).unwrap();
    input.scripts = ScriptsMode::Run;
    input
}

/// Sends with a fresh engine; the folder of the response bodies lives as long as the second value.
async fn send_with(input: &SendInput, cancel: CancellationToken) -> (SendOutcome, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let config = EngineConfig { body_dir: dir.path().to_path_buf(), user_agent: "RequestMaster/test".into() };
    let engine = ReqwestEngine::new(config, Arc::new(CookieJar::new())).unwrap();
    (send(&engine, input, cancel).await, dir)
}
async fn sent(input: &SendInput) -> (SendOutcome, tempfile::TempDir) {
    send_with(input, CancellationToken::new()).await
}

/// The console lines the scripts wrote; the lines of the application itself (`source == None`) are left out.
fn script_lines(report: &ScriptsReport) -> Vec<&ConsoleEntry> {
    report.console.iter().filter(|line| line.source.is_some()).collect()
}
fn texts(report: &ScriptsReport) -> Vec<&str> {
    script_lines(report).into_iter().map(|line| line.text.as_str()).collect()
}
fn source(node_id: &str, node_name: &str, node_kind: NodeKind, phase: ScriptPhase) -> ScriptSource {
    ScriptSource { node_id: node_id.into(), node_name: node_name.into(), node_kind, phase }
}
fn set(scope: StoredScope, key: &str, value: &str) -> VariableChange {
    VariableChange { scope, op: VariableOp::Set { key: key.into(), value: value.into() } }
}
fn has_unresolved(outcome: &SendOutcome) -> bool {
    outcome.warnings.iter().any(|warning| matches!(warning, SendWarning::UnresolvedVariables { .. }))
}

#[tokio::test]
async fn milestone_pre_request_sets_a_variable_and_a_test_checks_the_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/login")).and(query_param("u", "kubat"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"access_token\":\"T-1\"}"))
        .mount(&server).await;
    let test = "pm.test('status is 200', function () { if (pm.response.code !== 200) throw new Error('code ' + pm.response.code); });\n\
                var data = JSON.parse(responseBody);\n\
                pm.collectionVariables.set('token', data.access_token);";
    let request = get(
        "{{host}}/login?u={{user}}",
        scripts("pm.variables.set('user', 'kubat'); pm.environment.set('stage', 'pre');", test),
    );
    let tree = tree(vec![var("host", &server.uri())], Scripts::default(), Scripts::default(), Scripts::default(), request);

    let (outcome, _dir) = sent(&input(&tree, true)).await;
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
    assert_eq!(outcome.scripts.tests, vec![TestOutcome {
        source: source(&tree.req, "Req", NodeKind::Request, ScriptPhase::Test),
        name: "status is 200".into(), passed: true, skipped: false, error: None,
    }]);
    assert_eq!(outcome.scripts.changes, vec![
        set(StoredScope::Environment, "stage", "pre"),
        set(StoredScope::Collection, "token", "T-1"),
    ]);
    assert_eq!(outcome.scripts.errors, vec![]);
    // `user` is local: it is known to the substitution only because the pre-request script ran first.
    assert!(!has_unresolved(&outcome), "{:?}", outcome.warnings);
}

#[tokio::test]
async fn scripts_run_in_postman_order() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let tree = tree(vec![], logging("Col"), logging("A"), logging("B"), get(&server.uri(), logging("Req")));

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
    assert_eq!(
        texts(&outcome.scripts),
        ["Col pre", "A pre", "B pre", "Req pre", "Col test", "A test", "B test", "Req test"]
    );
    let nodes = [
        (&tree.col, "Col", NodeKind::Collection),
        (&tree.a, "A", NodeKind::Folder),
        (&tree.b, "B", NodeKind::Folder),
        (&tree.req, "Req", NodeKind::Request),
    ];
    let expected: Vec<ScriptSource> = [ScriptPhase::PreRequest, ScriptPhase::Test]
        .into_iter()
        .flat_map(|phase| nodes.iter().map(move |(id, name, kind)| source(id, name, *kind, phase)))
        .collect();
    let lines = script_lines(&outcome.scripts);
    let sources: Vec<ScriptSource> = lines.iter().map(|line| line.source.clone().unwrap()).collect();
    assert_eq!(sources, expected);
    assert!(lines.windows(2).all(|pair| pair[0].at_ms <= pair[1].at_ms), "at_ms goes back");
}

#[tokio::test]
async fn variables_flow_from_script_to_script_and_into_the_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/p7")).and(header("X-Id", "7"))
        .respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let mut request = get(
        "{{host}}/{{path}}",
        scripts(
            "pm.variables.set('path', 'p' + pm.environment.get('id'))",
            "console.log(pm.variables.get('path'), typeof pm.environment.get('id'))",
        ),
    );
    request.headers = vec![Param { key: "X-Id".into(), value: "{{id}}".into(), enabled: true, postman_extra: Extra::new() }];
    let col = scripts("pm.environment.set('id', 7)", "");
    let tree = tree(vec![var("host", &server.uri())], col, Scripts::default(), Scripts::default(), request);

    let (outcome, _dir) = sent(&input(&tree, true)).await;
    assert_eq!(outcome.result.as_ref().unwrap().status, 200, "the server takes only GET /p7 with X-Id: 7");
    assert_eq!(texts(&outcome.scripts), ["p7 number"]);
    assert_eq!(outcome.scripts.changes, vec![set(StoredScope::Environment, "id", "7")]);
}

#[tokio::test]
async fn pre_request_error_stops_everything() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let col = scripts("pm.globals.set('before', '1');\nthrow new Error('nope');", "console.log('Col test')");
    let tree = tree(vec![], col, logging("A"), logging("B"), get(&server.uri(), logging("Req")));

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    let Err(SendError::Script(failure)) = &outcome.result else { panic!("{:?}", outcome.result) };
    assert_eq!(failure.source, source(&tree.col, "Col", NodeKind::Collection, ScriptPhase::PreRequest));
    assert_eq!((failure.line, failure.message.as_str()), (Some(2), "Error: nope"));
    assert!(outcome.request.is_none());
    assert!(outcome.warnings.is_empty());
    assert_eq!(texts(&outcome.scripts), Vec::<&str>::new());
    assert!(outcome.scripts.changes.contains(&set(StoredScope::Globals, "before", "1")));
    assert!(server.received_requests().await.unwrap().is_empty(), "nothing may be sent");
}

#[tokio::test]
async fn test_error_keeps_the_response_and_the_next_scripts_run() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let col = scripts("", "throw new Error('boom')");
    let request = get(&server.uri(), scripts("", "pm.test('still', () => {})"));
    let tree = tree(vec![], col, Scripts::default(), Scripts::default(), request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
    let sources: Vec<&ScriptSource> = outcome.scripts.errors.iter().map(|error| &error.source).collect();
    assert_eq!(sources, [&source(&tree.col, "Col", NodeKind::Collection, ScriptPhase::Test)]);
    let tests: Vec<(&str, bool)> = outcome.scripts.tests.iter().map(|test| (test.name.as_str(), test.passed)).collect();
    assert_eq!(tests, [("still", true)]);
}

#[tokio::test]
async fn no_response_no_tests() {
    let tree = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(),
        get("http://127.0.0.1:1/x", logging("Req")));

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert!(matches!(outcome.result, Err(SendError::Http(HttpError::Connect(_)))), "{:?}", outcome.result);
    assert_eq!(texts(&outcome.scripts), ["Req pre"]);
}

#[tokio::test]
async fn scripts_off_give_a_warning_and_run_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/as-typed")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let url = format!("{}/as-typed", server.uri());
    // A script that would break the send if a single line of it ran.
    let col = scripts("throw new Error('must not run')", "");
    let blank = || scripts("  \n", "\n\t ");
    let blank_tree = tree(vec![], blank(), blank(), blank(), get(&url, blank()));
    let tree = tree(vec![], col, Scripts::default(), Scripts::default(), get(&url, logging("Req")));

    for reason in [ScriptsOff::Disabled, ScriptsOff::NotAllowed { collection: "Col".into() }] {
        let mut input = input(&tree, false);
        input.scripts = ScriptsMode::Off(reason.clone());
        let (outcome, _dir) = sent(&input).await;
        assert_eq!(outcome.result.as_ref().unwrap().status, 200, "{reason:?}");
        assert_eq!(
            outcome.warnings.first(),
            Some(&SendWarning::ScriptsNotRun { reason, nodes: vec!["Col".into(), "Req".into()] })
        );
        assert_eq!(outcome.scripts, ScriptsReport::default());
    }

    // Scripts of blanks and line breaks are no scripts: there is nothing to warn about.
    let mut input = input(&blank_tree, false);
    input.scripts = ScriptsMode::Off(ScriptsOff::Disabled);
    let (outcome, _dir) = sent(&input).await;
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
    assert_eq!(outcome.warnings, vec![]);
}

#[tokio::test]
async fn unconfirmed_files_stop_before_any_script() {
    let mut request = get("http://127.0.0.1:1/x", scripts("console.log('ran')", ""));
    request.method = "POST".into();
    request.body = Body::Binary { file: Some(FileRef { path: "C:/synthetic/a.bin".into(), confirmed: false }) };
    let tree = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(), request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert!(
        matches!(outcome.result, Err(SendError::Prepare(PrepareError::UnconfirmedFiles { .. }))),
        "{:?}",
        outcome.result
    );
    assert_eq!(outcome.scripts, ScriptsReport::default());
}

#[tokio::test]
async fn cancel_during_pre_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let tree = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(),
        get(&server.uri(), scripts("while (true) {}", "console.log('Req test')")));

    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        canceller.cancel();
    });
    let started = Instant::now();
    let (outcome, _dir) = send_with(&input(&tree, false), cancel).await;
    assert!(matches!(outcome.result, Err(SendError::Cancelled)), "{:?}", outcome.result);
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
    assert!(server.received_requests().await.unwrap().is_empty(), "nothing may be sent");
}

#[tokio::test]
async fn body_over_the_limit_fails_the_test_not_the_send() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 11 * MIB]))
        .mount(&server).await;
    let request = get(&server.uri(), scripts("", "pm.test('json', () => pm.response.json())"));
    let tree = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(), request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.result.as_ref().unwrap().decoded_size, 11 * MIB as u64);
    let [test] = &outcome.scripts.tests[..] else { panic!("{:?}", outcome.scripts.tests) };
    assert!(!test.passed);
    assert!(test.error.as_deref().unwrap_or_default().contains("10 MB"), "{:?}", test.error);
}

#[tokio::test]
async fn draft_script_of_the_request_runs() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let tree = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(),
        get(&server.uri(), Scripts::default()));
    let draft = get(&server.uri(), scripts("", "console.log('from the draft')"));
    let settings = AppSettings { proxy: ProxySetting::Off, ..Default::default() };
    let mut input = load_send_input(&tree.store, &tree.req, Some(draft), None, settings).unwrap();
    input.scripts = ScriptsMode::Run;

    let (outcome, _dir) = sent(&input).await;
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
    assert_eq!(texts(&outcome.scripts), ["from the draft"]);
}

// ---- pm.sendRequest: the requests of a script go through the same HTTP engine ----

/// The tree with the collection variable `host` = the server and a pre-request script of the request, which
/// is `GET {{host}}/data`.
fn tree_with_pre_request(server: &MockServer, pre_request: &str) -> Tree {
    let request = get("{{host}}/data", scripts(pre_request, ""));
    tree(vec![var("host", &server.uri())], Scripts::default(), Scripts::default(), Scripts::default(), request)
}
/// `GET /data` answers 200.
async fn mount_data(server: &MockServer) {
    Mock::given(method("GET")).and(path("/data")).respond_with(ResponseTemplate::new(200)).mount(server).await;
}
/// What the scripts wrote with `console.log`.
fn logged(report: &ScriptsReport) -> Vec<&str> {
    let lines = report.console.iter().filter(|line| line.level == ConsoleLevel::Log);
    lines.map(|line| line.text.as_str()).collect()
}

const FETCH_TOKEN: &str = "const res = await pm.sendRequest({ url: pm.collectionVariables.get('host') + '/token', method: 'POST',\n\
    header: { 'Content-Type': 'application/json' }, body: { mode: 'raw', raw: JSON.stringify({ user: 'u' }) } });\n\
    pm.environment.set('token', res.json().access_token);";
const LOGIN: &str = "await pm.sendRequest(pm.collectionVariables.get('host') + '/login')";

/// `POST /token` with the body `{"user":"u"}` gives the token `T-9`; `GET /data` wants it.
async fn token_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/token")).and(body_string("{\"user\":\"u\"}"))
        .and(header("Content-Type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"access_token\":\"T-9\"}"))
        .mount(&server).await;
    Mock::given(method("GET")).and(path("/data")).and(header("Authorization", "Bearer T-9"))
        .respond_with(ResponseTemplate::new(200)).mount(&server).await;
    server
}
fn token_tree(server: &MockServer) -> Tree {
    let mut request = get("{{host}}/data", scripts(FETCH_TOKEN, ""));
    request.headers = vec![Param { key: "Authorization".into(), value: "Bearer {{token}}".into(), enabled: true,
        postman_extra: Extra::new() }];
    tree(vec![var("host", &server.uri())], Scripts::default(), Scripts::default(), Scripts::default(), request)
}
/// `GET /login` sets the cookie `sid=1`; `GET /data` wants it.
async fn login_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/login"))
        .respond_with(ResponseTemplate::new(200).insert_header("set-cookie", "sid=1; Path=/").set_body_string("in"))
        .mount(&server).await;
    Mock::given(method("GET")).and(path("/data")).and(header("Cookie", "sid=1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("data")).mount(&server).await;
    server
}

#[tokio::test]
async fn token_is_fetched_in_pre_request() {
    let server = token_server().await;
    let tree = token_tree(&server);

    let (outcome, _dir) = sent(&input(&tree, true)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(outcome.result.as_ref().unwrap().status, 200, "the server takes only GET /data with the token");
    assert_eq!(outcome.scripts.changes, vec![set(StoredScope::Environment, "token", "T-9")]);
}

#[tokio::test]
async fn send_request_shares_the_cookies() {
    let server = login_server().await;
    let tree = tree_with_pre_request(&server, LOGIN);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(outcome.result.as_ref().unwrap().status, 200, "the server takes only GET /data with the cookie");
}

#[tokio::test]
async fn bodies_of_script_requests_are_removed() {
    let server = login_server().await;
    let tree = tree_with_pre_request(&server, LOGIN);

    let (outcome, dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    let response = outcome.result.as_ref().unwrap();
    let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(files, vec![response.body_path.clone()], "only the body of the main response stays");
}

#[tokio::test]
async fn urlencoded_and_formdata_bodies() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    mount_data(&server).await;
    let pre = "const host = pm.collectionVariables.get('host');\n\
        await pm.sendRequest({ url: host + '/form', method: 'POST', body: { mode: 'urlencoded',\n\
          urlencoded: [{ key: 'a', value: '1' }, { key: 'b', value: 'x y' }] } });\n\
        await pm.sendRequest({ url: host + '/multi', method: 'POST', body: { mode: 'formdata',\n\
          formdata: [{ key: 't', value: 'v' }] } });";
    let tree = tree_with_pre_request(&server, pre);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    let received = server.received_requests().await.unwrap();
    let content_type = |request: &wiremock::Request| {
        request.headers.get("content-type").unwrap().to_str().unwrap().to_string()
    };
    let form = received.iter().find(|request| request.url.path() == "/form").unwrap();
    assert_eq!(String::from_utf8_lossy(&form.body), "a=1&b=x+y");
    assert_eq!(content_type(form), "application/x-www-form-urlencoded");
    let multi = received.iter().find(|request| request.url.path() == "/multi").unwrap();
    assert!(content_type(multi).starts_with("multipart/form-data"), "{}", content_type(multi));
    assert!(String::from_utf8_lossy(&multi.body).contains("name=\"t\""), "{:?}", multi.body);
}

#[tokio::test]
async fn network_error_in_a_script_request_does_not_stop_the_send() {
    let server = MockServer::start().await;
    mount_data(&server).await;
    let pre = "pm.sendRequest('http://127.0.0.1:1/x', (err) => console.log('E: ' + err.message))";
    let tree = tree_with_pre_request(&server, pre);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    let [line] = logged(&outcome.scripts)[..] else { panic!("{:?}", outcome.scripts.console) };
    assert!(line.starts_with("E: connection failed"), "{line}");
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
}

#[tokio::test]
async fn console_has_network_lines_in_order() {
    let server = token_server().await;
    let tree = token_tree(&server);

    let (outcome, _dir) = sent(&input(&tree, true)).await;
    let network = outcome.scripts.console.iter().filter(|line| line.level == ConsoleLevel::Network);
    let [script, app] = &network.collect::<Vec<_>>()[..] else { panic!("{:?}", outcome.scripts.console) };
    assert_eq!(script.source, Some(source(&tree.req, "Req", NodeKind::Request, ScriptPhase::PreRequest)));
    assert!(script.text.starts_with(&format!("POST {}/token → 200 OK (", server.uri())), "{}", script.text);
    assert_eq!(app.source, None);
    assert!(app.text.starts_with(&format!("GET {}/data → 200 OK (", server.uri())), "{}", app.text);
    assert!(script.text.ends_with(" ms)") && app.text.ends_with(" ms)"), "{} | {}", script.text, app.text);
    assert!(script.at_ms <= app.at_ms);
}

#[tokio::test]
async fn cancel_during_a_script_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30))).mount(&server).await;
    mount_data(&server).await;
    let tree = token_tree(&server);

    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        canceller.cancel();
    });
    let started = Instant::now();
    let (outcome, _dir) = send_with(&input(&tree, true), cancel).await;
    assert!(matches!(outcome.result, Err(SendError::Cancelled)), "{:?}", outcome.result);
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

#[tokio::test]
async fn script_request_uses_the_application_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2))).mount(&server).await;
    mount_data(&server).await;
    let pre = "try { await pm.sendRequest(pm.collectionVariables.get('host') + '/slow') } catch (e) { console.log('failed') }";
    let tree = tree_with_pre_request(&server, pre);
    let mut input = input(&tree, false);
    input.settings.timeout = Some(Duration::from_millis(200));

    let (outcome, _dir) = sent(&input).await;
    assert_eq!(logged(&outcome.scripts), ["failed"]);
    let slow = outcome.scripts.console.iter().find(|line| line.text.contains("/slow")).expect("no network line");
    assert!(slow.text.ends_with("/slow → error: request timed out"), "{}", slow.text);
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
}

#[tokio::test]
async fn big_answer_of_a_script_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 11 * MIB])).mount(&server).await;
    mount_data(&server).await;
    let pre = "const res = await pm.sendRequest(pm.collectionVariables.get('host') + '/big');\n\
        try { res.text() } catch (e) { console.log(e.message) }";
    let tree = tree_with_pre_request(&server, pre);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(logged(&outcome.scripts), ["response body is larger than 10 MB and is not available to scripts"]);
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
}

// ---- pm.request: what a pre-request script changes is sent; a test script sees what was sent ----

fn row(key: &str, value: &str, enabled: bool) -> Param {
    Param { key: key.into(), value: value.into(), enabled, postman_extra: Extra::new() }
}
/// A server that answers 200 to everything.
async fn any_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any()).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    server
}
/// The only request the server got.
async fn the_request(server: &MockServer) -> wiremock::Request {
    let mut received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1, "{received:?}");
    received.remove(0)
}
/// The tree with the collection variables `host` = the server, `id` = `7`, `token` = `T-1` and no scripts
/// but those of the request.
fn tree_of(server: &MockServer, request: RequestContent) -> Tree {
    let variables = vec![var("host", &server.uri()), var("id", "7"), var("token", "T-1")];
    tree(variables, Scripts::default(), Scripts::default(), Scripts::default(), request)
}
fn set_collection_auth(tree: &mut Tree, auth: Auth) {
    let NodeContent::Collection(mut content) = tree.store.node(&tree.col).unwrap().content else { panic!() };
    content.auth = Some(auth);
    tree.store.update_content(&tree.col, &NodeContent::Collection(content)).unwrap();
}
fn json_body(text: &str) -> Body {
    Body::Raw { text: text.into(), language: RawLanguage::Json }
}

#[tokio::test]
async fn pre_request_changes_the_request_that_is_sent() {
    let server = any_server().await;
    let pre = "pm.request.headers.add({ key: 'X-Sig', value: 'abc' }); pm.variables.set('stamp', '42');\n\
        pm.request.url.query.add({ key: 'ts', value: '{{stamp}}' });\n\
        pm.request.body.raw = JSON.stringify({ n: 1 }); pm.request.method = 'PUT';";
    let stored = get("{{host}}/x", scripts(pre, ""));
    let tree = tree_of(&server, stored.clone());

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
    let received = the_request(&server).await;
    assert_eq!((received.method.as_str(), received.url.path(), received.url.query()), ("PUT", "/x", Some("ts=42")));
    assert_eq!(received.headers.get("X-Sig").unwrap(), "abc");
    assert_eq!(String::from_utf8_lossy(&received.body), "{\"n\":1}");
    // The history is told the same.
    let summary = outcome.request.as_ref().unwrap();
    assert_eq!((summary.method.as_str(), summary.url.as_str()), ("PUT", format!("{}/x?ts=42", server.uri()).as_str()));
    assert!(summary.headers.contains(&("X-Sig".to_string(), "abc".to_string())), "{:?}", summary.headers);
    assert_eq!(summary.body.as_deref(), Some("{\"n\":1}"));
    // Only this send was changed: the stored request is as it was.
    assert_eq!(tree.store.node(&tree.req).unwrap().content, NodeContent::Request(stored));
}

#[tokio::test]
async fn query_rows_of_the_model_reach_the_script() {
    let server = any_server().await;
    let mut request = get("{{host}}/x", scripts("console.log(pm.request.url.getQueryString())", ""));
    request.query = vec![row("a", "1", true), row("b", "2", false)];
    let tree = tree_of(&server, request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(texts(&outcome.scripts), ["a=1"]);
    assert_eq!(the_request(&server).await.url.query(), Some("a=1"));
}

#[tokio::test]
async fn query_stands_before_the_fragment_and_stays_encoded() {
    let server = any_server().await;
    let pre = "console.log(pm.request.url.getQueryString(), pm.request.url.toString().endsWith('#frag'));\n\
        pm.request.url.query.add({ key: 'x', value: '1' })";
    let mut request = get("{{host}}/x#frag", scripts(pre, ""));
    request.query = vec![row("q", "{{v}}", true)];
    let mut tree = tree_of(&server, request);
    let NodeContent::Collection(mut content) = tree.store.node(&tree.col).unwrap().content else { panic!() };
    content.variables.push(var("v", "a&b c"));
    tree.store.update_content(&tree.col, &NodeContent::Collection(content)).unwrap();

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    // The script sees the variable as typed; its value is encoded when the request is built.
    assert_eq!(texts(&outcome.scripts), ["q={{v}} true"]);
    let received = the_request(&server).await;
    assert_eq!((received.url.path(), received.url.query()), ("/x", Some("q=a%26b%20c&x=1")));
}

#[tokio::test]
async fn literal_query_characters_survive_a_script_edit() {
    let server = any_server().await;
    let pre = "console.log(pm.request.url.query.count()); pm.request.url.query.add({ key: 'ts', value: '1' })";
    let mut request = get("{{host}}/x", scripts(pre, ""));
    request.query = vec![row("k", "a&b#c", true), row("m=n", "1", true)];
    let tree = tree_of(&server, request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(texts(&outcome.scripts), ["2"]);
    // What goes out without the script, and `ts` after it.
    assert_eq!(the_request(&server).await.url.query(), Some("k=a%26b%23c&m%3Dn=1&ts=1"));
}

#[tokio::test]
async fn query_characters_a_script_adds_are_sent_encoded() {
    let server = any_server().await;
    let pre = "pm.request.url.query.add({ key: 'f', value: 'a&b#c' }); pm.request.url.query.add({ key: 'k=1', value: 'v' })";
    let tree = tree_of(&server, get("{{host}}/x", scripts(pre, "")));

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(the_request(&server).await.url.query(), Some("f=a%26b%23c&k%3D1=v"));
}

#[tokio::test]
async fn api_key_in_the_query_still_replaces_the_row() {
    let server = any_server().await;
    let mut request = get("{{host}}/x", scripts("pm.request.url.query.add({ key: 'ts', value: '1' })", ""));
    request.query = vec![row("key", "old", true)];
    let mut tree = tree_of(&server, request);
    let auth = Auth::ApiKey { key: "key".into(), value: "secret".into(), placement: ApiKeyPlacement::Query };
    set_collection_auth(&mut tree, auth);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    // One parameter `key`, and it is the one of the authorization.
    assert_eq!(the_request(&server).await.url.query(), Some("ts=1&key=secret"));
}

#[tokio::test]
async fn test_script_sees_the_sent_request_and_cannot_change_it() {
    let server = any_server().await;
    let test = "console.log(pm.request.headers.get('X-P'), pm.request.url.toString().startsWith('http://127.0.0.1'),\n\
        JSON.parse(pm.request.body.raw).n, pm.request.headers.get('Authorization'), pm.request.body.mode,\n\
        pm.request.headers.has('X-Off'), request.data, request.headers.authorization);\n\
        pm.request.method = 'DELETE'";
    let mut request = get("{{host}}/x", scripts("pm.request.headers.add({ key: 'X-P', value: '1' })", test));
    request.method = "POST".into();
    request.body = json_body("{\"n\":{{id}}}");
    request.headers = vec![row("X-Off", "1", false)];
    let mut tree = tree_of(&server, request);
    set_collection_auth(&mut tree, Auth::Bearer { token: "{{token}}".into() });

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(texts(&outcome.scripts), ["1 true 7 Bearer T-1 raw false {\"n\":7} Bearer T-1"]);
    assert_eq!(the_request(&server).await.method.as_str(), "POST");
}

#[tokio::test]
async fn test_script_sees_the_sent_urlencoded_body() {
    let server = any_server().await;
    let test = "console.log(pm.request.body.mode, pm.request.body.urlencoded.get('a'), pm.request.body.urlencoded.get('stamp'))";
    let mut request = get("{{host}}/x", scripts("", test));
    request.method = "POST".into();
    request.body = Body::UrlEncoded(vec![row("a", "{{id}}", true), row("stamp", "{{$timestamp}}", true)]);
    let tree = tree_of(&server, request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    let received = the_request(&server).await;
    let body = String::from_utf8_lossy(&received.body).into_owned();
    let stamp = body.strip_prefix("a=7&stamp=").unwrap_or_else(|| panic!("{body}"));
    assert!(!stamp.is_empty() && stamp.bytes().all(|byte| byte.is_ascii_digit()), "{body}");
    // The very value that went out: nothing is substituted a second time for the script.
    assert_eq!(texts(&outcome.scripts), [format!("urlencoded 7 {stamp}")]);
}

#[tokio::test]
async fn pruned_body_is_still_visible_to_the_test() {
    let server = any_server().await;
    let mut request = get("{{host}}/x", scripts("", "console.log(pm.request.body.raw)"));
    request.body = json_body("{\"n\":{{id}}}");
    let tree = tree_of(&server, request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(texts(&outcome.scripts), ["{\"n\":7}"]);
    assert!(the_request(&server).await.body.is_empty(), "GET sends no body");
}

#[tokio::test]
async fn skip_request_sends_nothing() {
    let server = any_server().await;
    let col = scripts("console.log('before'); pm.execution.skipRequest(); console.log('after')", "");
    let tree = tree(vec![], col, Scripts::default(), Scripts::default(), get(&server.uri(), logging("Req")));

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert!(matches!(outcome.result, Err(SendError::Skipped)), "{:?}", outcome.result);
    assert!(outcome.request.is_none());
    // Neither the pre-request script of the request nor any test script ran.
    assert_eq!(texts(&outcome.scripts), ["before"]);
    assert_eq!(outcome.scripts.errors, vec![]);
    assert!(server.received_requests().await.unwrap().is_empty(), "nothing may be sent");
}

#[tokio::test]
async fn script_cannot_turn_the_body_into_a_file() {
    let server = any_server().await;
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "the contents of a file").unwrap();
    let path = file.path().to_string_lossy().replace('\\', "/");
    let pre = format!("pm.request.body.update({{ mode: 'file', file: {{ src: '{path}' }} }})");
    let mut request = get("{{host}}/x", scripts(&pre, ""));
    request.method = "POST".into();
    request.body = Body::Raw { text: "as typed".into(), language: RawLanguage::Text };
    let tree = tree_of(&server, request);

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(String::from_utf8_lossy(&the_request(&server).await.body), "as typed");
}

// ---- the limits of a script as a send meets them ----

/// The tree with a pre-request script of the request and the input of its send with a script time limit
/// of 200 ms.
fn short_timeout(server: &MockServer, pre_request: &str) -> (Tree, SendInput) {
    let tree = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(),
        get(&server.uri(), scripts(pre_request, "")));
    let mut input = input(&tree, false);
    input.settings.script_timeout = Duration::from_millis(200);
    (tree, input)
}
fn script_failure(outcome: &SendOutcome) -> &ScriptFailure {
    let Err(SendError::Script(failure)) = &outcome.result else { panic!("{:?}", outcome.result) };
    failure
}

#[tokio::test]
async fn script_timeout_comes_from_the_settings() {
    let server = any_server().await;
    let (_tree, input) = short_timeout(&server, "while (true) {}");

    let started = Instant::now();
    let (outcome, _dir) = sent(&input).await;
    assert_eq!(script_failure(&outcome).kind, ScriptErrorKind::Timeout);
    // Far below the default limit of 10 s.
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

/// The only test of this file that leaves an abandoned thread behind until the test process ends.
#[tokio::test]
async fn native_loop_in_pre_request_ends_the_send() {
    let server = any_server().await;
    let (_tree, input) =
        short_timeout(&server, "pm.globals.set('a', '1'); [].reverse.call({ length: 2 ** 53 - 1 })");

    let started = Instant::now();
    let (outcome, _dir) = sent(&input).await;
    let limit = Duration::from_millis(200) + rqm_script::WATCHDOG_GRACE + Duration::from_secs(1);
    assert!(started.elapsed() <= limit, "{:?}", started.elapsed());
    assert_eq!(script_failure(&outcome).kind, ScriptErrorKind::Timeout);
    assert!(outcome.scripts.changes.contains(&set(StoredScope::Globals, "a", "1")), "{:?}", outcome.scripts.changes);
    assert!(server.received_requests().await.unwrap().is_empty(), "nothing may be sent");

    // The abandoned thread does not stand in the way of the next send.
    let plain = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(),
        get(&server.uri(), Scripts::default()));
    let (outcome, _dir) = sent(&self::input(&plain, false)).await;
    assert_eq!(outcome.result.as_ref().unwrap().status, 200);
}

#[tokio::test]
async fn host_memory_limit_ends_the_send_and_keeps_the_store_small() {
    let server = any_server().await;
    let pre = "try { while (true) pm.globals.set('k', 'x'.repeat(1 << 19)) } catch (e) {}";
    let mut tree = tree(vec![], Scripts::default(), Scripts::default(), Scripts::default(),
        get(&server.uri(), scripts(pre, "")));

    let (outcome, _dir) = sent(&input(&tree, false)).await;
    assert_eq!(script_failure(&outcome).kind, ScriptErrorKind::Memory);

    let col = tree.col.clone();
    apply_variable_changes(&mut tree.store, &col, None, &outcome.scripts.changes).unwrap();
    let globals = tree.store.globals().unwrap();
    let [k] = &globals.variables[..] else { panic!("{} variables", globals.variables.len()) };
    assert!(k.key == "k" && k.value.len() <= rqm_script::MAX_VALUE_BYTES, "{} bytes", k.value.len());
    assert!(globals.content_json().len() < 2 * MIB, "{} bytes", globals.content_json().len());
}
