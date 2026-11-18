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
    assert_eq!(input.scripts, ScriptsMode::Off(ScriptsOff::Disabled), "scripts are off until Task 10");
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
