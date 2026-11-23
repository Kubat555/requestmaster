//! The compatibility set: scripts as people write them for Postman, each through the real `send` against a
//! local server. The tree of every case: collection `Col` → folder `Folder` → request `Req`; the collection
//! has the consent and the variable `host` = the server; the environment `Dev` is chosen. After every send
//! the changes of the scripts are written to the store, and the next send reads its input anew.

use std::sync::Arc;
use std::time::Duration;

use rqm_http::{CookieJar, EngineConfig, ProxySetting, ReqwestEngine};
use rqm_model::*;
use rqm_runner::*;
use rqm_store::Store;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn var(key: &str, value: &str) -> Variable {
    Variable { key: key.into(), value: value.into(), secret: false, enabled: true, postman_extra: Extra::new() }
}
fn row(key: &str, value: &str) -> Param {
    Param { key: key.into(), value: value.into(), enabled: true, postman_extra: Extra::new() }
}
fn scripts(pre_request: &str, test: &str) -> Scripts {
    Scripts { pre_request: pre_request.into(), test: test.into() }
}
/// `console.log('<node> pre')` and `console.log('<node> test')`.
fn logging(node: &str) -> Scripts {
    scripts(&format!("console.log('{node} pre')"), &format!("console.log('{node} test')"))
}
fn request(method: &str, url: &str, pre_request: &str, test: &str) -> RequestContent {
    RequestContent { method: method.into(), url: url.into(), query: vec![], path_variables: vec![], headers: vec![],
        body: Body::None, auth: None, scripts: scripts(pre_request, test), settings: RequestSettings::default(),
        disabled_auto_headers: vec![], postman_extra: Extra::new() }
}
/// `GET {{host}}/data` with the scripts.
fn get(pre_request: &str, test: &str) -> RequestContent {
    request("GET", "{{host}}/data", pre_request, test)
}

/// A server that answers every request with `status` and the JSON `body`.
async fn json_server(status: u16, body: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(any()).respond_with(ResponseTemplate::new(status).set_body_raw(body, "application/json"))
        .mount(&server).await;
    server
}
/// A server that answers 200 with an empty JSON object.
async fn ok_server() -> MockServer {
    json_server(200, "{}").await
}

/// What a case starts from besides the request: the scripts of the outer levels and the variables of the scopes.
#[derive(Default)]
struct Case {
    collection: Scripts,
    folder: Scripts,
    /// The variable `host` is added to them.
    collection_variables: Vec<Variable>,
    environment: Vec<Variable>,
    globals: Vec<Variable>,
}

/// A case while it runs: the store, the ids of its tree and the one HTTP engine of all its sends.
struct World {
    store: Store,
    col: String,
    req: String,
    /// Id of the environment `Dev`.
    env: String,
    settings: AppSettings,
    engine: ReqwestEngine,
    _bodies: tempfile::TempDir,
}

impl Case {
    fn start(self, server: &MockServer, request: RequestContent) -> World {
        let mut store = Store::open_in_memory().unwrap();
        let mut variables = vec![var("host", &server.uri())];
        variables.extend(self.collection_variables);
        let content = CollectionContent { auth: None, variables, scripts: self.collection, postman_extra: Extra::new() };
        let col = store.create_node(None, NodeKind::Collection, "Col", &NodeContent::Collection(content)).unwrap();
        let folder = FolderContent { auth: None, scripts: self.folder, postman_extra: Extra::new() };
        let folder = store.create_node(Some(&col), NodeKind::Folder, "Folder", &NodeContent::Folder(folder)).unwrap();
        let req = store.create_node(Some(&folder), NodeKind::Request, "Req", &NodeContent::Request(request)).unwrap();
        set_scripts_allowed(&mut store, &col, true).unwrap();

        let dev = Environment { id: new_id(), kind: EnvKind::Environment, name: "Dev".into(),
            variables: self.environment, postman_extra: Extra::new() };
        store.save_environment(&dev).unwrap();
        let mut globals = store.globals().unwrap();
        globals.variables = self.globals;
        store.save_environment(&globals).unwrap();

        let bodies = tempfile::tempdir().unwrap();
        let config = EngineConfig { body_dir: bodies.path().to_path_buf(), user_agent: "RequestMaster/test".into() };
        let engine = ReqwestEngine::new(config, Arc::new(CookieJar::new())).unwrap();
        let settings = AppSettings { proxy: ProxySetting::Off, ..Default::default() };
        World { store, col, req, env: dev.id, settings, engine, _bodies: bodies }
    }
}

impl World {
    /// One send as the application does it: the input is read from the store, the scripts run for real, and
    /// what they changed in the variables is written back.
    async fn send(&mut self) -> SendOutcome {
        let input = load_send_input(&self.store, &self.req, None, Some(&self.env), self.settings.clone()).unwrap();
        assert_eq!(input.scripts, ScriptsMode::Run);
        let outcome = send(&self.engine, &input, CancellationToken::new()).await;
        apply_variable_changes(&mut self.store, &self.col, Some(&self.env), &outcome.scripts.changes).unwrap();
        outcome
    }
    /// The next send is of this request.
    fn replace_request(&mut self, request: RequestContent) {
        self.store.update_content(&self.req, &NodeContent::Request(request)).unwrap();
    }
    /// The stored variables of the environment `Dev`.
    fn environment(&self) -> Vec<Variable> {
        let environments = self.store.environments().unwrap();
        environments.into_iter().find(|environment| environment.id == self.env).unwrap().variables
    }
    /// The stored value of the enabled variable of `Dev`.
    fn environment_value(&self, key: &str) -> Option<String> {
        let variables = self.environment();
        variables.into_iter().rev().find(|variable| variable.enabled && variable.key == key).map(|variable| variable.value)
    }
}

/// One send of `request` in a tree without other scripts and variables.
async fn sent(server: &MockServer, request: RequestContent) -> (SendOutcome, World) {
    let mut world = Case::default().start(server, request);
    (world.send().await, world)
}

/// The console without the network lines.
fn console(outcome: &SendOutcome) -> Vec<&str> {
    let lines = outcome.scripts.console.iter().filter(|line| line.level != ConsoleLevel::Network);
    lines.map(|line| line.text.as_str()).collect()
}
/// `(name, passed)` of every test.
fn tests(outcome: &SendOutcome) -> Vec<(&str, bool)> {
    outcome.scripts.tests.iter().map(|test| (test.name.as_str(), test.passed)).collect()
}
fn status(outcome: &SendOutcome) -> u16 {
    outcome.result.as_ref().unwrap_or_else(|error| panic!("{error:?}")).status
}
fn set(scope: StoredScope, key: &str, value: &str) -> VariableChange {
    VariableChange { scope, op: VariableOp::Set { key: key.into(), value: value.into() } }
}
fn header_of<'a>(received: &'a Request, name: &str) -> &'a str {
    received.headers.get(name).unwrap_or_else(|| panic!("no header {name}")).to_str().unwrap()
}
async fn received(server: &MockServer) -> Vec<Request> {
    server.received_requests().await.unwrap()
}
/// The only request the server got.
async fn the_request(server: &MockServer) -> Request {
    let mut received = received(server).await;
    assert_eq!(received.len(), 1, "{received:?}");
    received.remove(0)
}

#[tokio::test]
async fn owner_token_from_body() {
    let server = json_server(200, "{\"token\":\"abc\"}").await;
    let test = "var jsonData = JSON.parse(responseBody);\npm.collectionVariables.set(\"token\", jsonData.token);";

    let (outcome, _world) = sent(&server, get("", test)).await;
    assert_eq!(outcome.scripts.changes, vec![set(StoredScope::Collection, "token", "abc")]);
    assert_eq!(outcome.scripts.errors, vec![]);
}

#[tokio::test]
async fn status_test() {
    let test = "pm.test(\"Status code is 200\", function () { pm.response.to.have.status(200); });";

    let (outcome, _world) = sent(&json_server(200, "{}").await, get("", test)).await;
    assert_eq!(tests(&outcome), [("Status code is 200", true)]);

    let (outcome, _world) = sent(&json_server(500, "{}").await, get("", test)).await;
    assert_eq!(status(&outcome), 500);
    assert_eq!(tests(&outcome), [("Status code is 200", false)]);
    assert_eq!(
        outcome.scripts.tests[0].error.as_deref(),
        Some("AssertionError: expected response to have status code 200 but got 500")
    );
}

#[tokio::test]
async fn legacy_tests_and_headers() {
    let test = "tests[\"Status code is 200\"] = responseCode.code === 200;\n\
        tests[\"Is json\"] = postman.getResponseHeader(\"Content-Type\").indexOf(\"json\") !== -1;";

    let (outcome, _world) = sent(&ok_server().await, get("", test)).await;
    assert_eq!(tests(&outcome), [("Status code is 200", true), ("Is json", true)]);
    assert_eq!(outcome.scripts.errors, vec![]);
}

#[tokio::test]
async fn json_value_checks() {
    let server = json_server(200, "{\"items\":[{\"id\":1},{\"id\":2}]}").await;
    let test = "const body = pm.response.json(); pm.test(\"has items\", () => { \
        pm.expect(body.items).to.be.an(\"array\").that.has.lengthOf(2); \
        pm.expect(body.items[0]).to.have.property(\"id\", 1); });";

    let (outcome, _world) = sent(&server, get("", test)).await;
    assert_eq!(tests(&outcome), [("has items", true)]);
    assert_eq!(outcome.scripts.errors, vec![]);
}

#[tokio::test]
async fn response_time_and_header() {
    let test = "pm.test(\"fast\", () => pm.expect(pm.response.responseTime).to.be.below(5000)); \
        pm.test(\"json\", () => pm.response.to.have.header(\"Content-Type\"));";

    let (outcome, _world) = sent(&ok_server().await, get("", test)).await;
    assert_eq!(tests(&outcome), [("fast", true), ("json", true)]);
    assert_eq!(outcome.scripts.errors, vec![]);
}

#[tokio::test]
async fn value_from_one_request_feeds_the_next() {
    let server = json_server(200, "{\"id\":7}").await;
    let first = request("GET", "{{host}}/first", "", "pm.environment.set(\"id\", pm.response.json().id)");
    let mut world = Case::default().start(&server, first);

    let outcome = world.send().await;
    assert_eq!(outcome.scripts.errors, vec![]);
    world.replace_request(request("GET", "{{host}}/items/{{id}}", "", ""));
    assert_eq!(status(&world.send().await), 200);

    let paths: Vec<String> = received(&server).await.iter().map(|request| request.url.path().to_string()).collect();
    assert_eq!(paths, ["/first", "/items/7"]);
}

#[tokio::test]
async fn number_is_text_in_the_next_send() {
    let server = ok_server().await;
    let mut world = Case::default().start(&server, get("", "pm.environment.set(\"count\", 1)"));

    let outcome = world.send().await;
    assert_eq!(outcome.scripts.errors, vec![]);
    let pre = "console.log(typeof pm.environment.get(\"count\"), pm.environment.get(\"count\") + 1)";
    world.replace_request(get(pre, ""));
    let outcome = world.send().await;
    // Unlike Postman: the store keeps one text value, so the number comes back as text.
    assert_eq!(console(&outcome), ["string 11"]);
}

#[tokio::test]
async fn timestamp_and_nonce_in_pre_request() {
    let server = ok_server().await;
    let pre = "pm.request.headers.upsert({ key: \"X-Time\", value: String(Date.now()) }); \
        pm.variables.set(\"nonce\", pm.variables.replaceIn(\"{{$guid}}\"));";
    let mut request = get(pre, "");
    request.headers = vec![row("X-Nonce", "{{nonce}}")];

    let (outcome, _world) = sent(&server, request).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    let received = the_request(&server).await;
    let time = header_of(&received, "X-Time");
    assert!(!time.is_empty() && time.bytes().all(|byte| byte.is_ascii_digit()), "X-Time: {time}");
    let nonce = header_of(&received, "X-Nonce");
    let is_uuid = nonce.len() == 36
        && nonce.bytes().enumerate().all(|(at, byte)| match at {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        });
    assert!(is_uuid, "X-Nonce: {nonce}");
}

#[tokio::test]
async fn collection_script_fetches_a_token_once() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/auth"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("{\"token\":\"T-1\"}", "application/json"))
        .mount(&server).await;
    Mock::given(method("GET")).and(path("/data")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
    let collection = "if (!pm.environment.get(\"token\")) { \
        const r = await pm.sendRequest(pm.variables.get(\"host\") + \"/auth\"); \
        pm.environment.set(\"token\", r.json().token); }";
    let mut request = get("", "");
    request.headers = vec![row("Authorization", "Bearer {{token}}")];
    let case = Case { collection: scripts(collection, ""), ..Default::default() };
    let mut world = case.start(&server, request);

    for _ in 0..2 {
        let outcome = world.send().await;
        assert_eq!(outcome.scripts.errors, vec![]);
        assert_eq!(status(&outcome), 200);
    }
    let received = received(&server).await;
    let paths: Vec<&str> = received.iter().map(|request| request.url.path()).collect();
    assert_eq!(paths, ["/auth", "/data", "/data"]);
    for data in received.iter().filter(|request| request.url.path() == "/data") {
        assert_eq!(header_of(data, "Authorization"), "Bearer T-1");
    }
}

#[tokio::test]
async fn callback_request_with_a_test_inside() {
    let test = "pm.sendRequest(pm.variables.get(\"host\") + \"/check\", function (err, res) { \
        pm.test(\"check ok\", function () { pm.expect(err).to.equal(null); pm.expect(res).to.have.status(200); }); });";

    let (outcome, _world) = sent(&ok_server().await, get("", test)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("check ok", true)]);
    assert_eq!(outcome.scripts.tests[0].error, None);
}

#[tokio::test]
async fn folder_scripts_between_collection_and_request() {
    let server = ok_server().await;
    let case = Case { collection: logging("Col"), folder: logging("Folder"), ..Default::default() };
    let mut request = get("", "");
    request.scripts = logging("Req");
    let mut world = case.start(&server, request);

    let outcome = world.send().await;
    assert_eq!(console(&outcome), ["Col pre", "Folder pre", "Req pre", "Col test", "Folder test", "Req test"]);
}

#[tokio::test]
async fn local_variable_does_not_outlive_the_send() {
    let server = ok_server().await;
    let mut world = Case::default().start(&server, get("pm.variables.set(\"tmp\", \"1\")", ""));

    let outcome = world.send().await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(outcome.scripts.changes, vec![]);
    world.replace_request(get("console.log(pm.variables.get(\"tmp\"))", ""));
    let outcome = world.send().await;
    assert_eq!(console(&outcome), ["undefined"]);
}

#[tokio::test]
async fn unset_and_clear() {
    let server = ok_server().await;
    let disabled = Variable { enabled: false, ..var("kept", "2") };
    let case = Case {
        environment: vec![var("a", "1"), var("b", "2")],
        globals: vec![var("g", "1"), disabled.clone()],
        ..Default::default()
    };
    let mut world = case.start(&server, get("", "pm.environment.unset(\"a\"); pm.globals.clear();"));

    let outcome = world.send().await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(world.environment(), vec![var("b", "2")]);
    // A script does not see a disabled row and does not erase it.
    assert_eq!(world.store.globals().unwrap().variables, vec![disabled]);
}

#[tokio::test]
async fn console_objects() {
    let (outcome, _world) = sent(&ok_server().await, get("", "console.log(\"user\", { id: 1 }, [1, 2])")).await;
    assert_eq!(console(&outcome), ["user {\"id\":1} [1,2]"]);
}

#[tokio::test]
async fn error_in_a_test_script_has_its_line() {
    let (outcome, world) = sent(&ok_server().await, get("", "const a = pm.response.json();\na.missing.deep;")).await;
    assert_eq!(status(&outcome), 200);
    let [error] = &outcome.scripts.errors[..] else { panic!("{:?}", outcome.scripts.errors) };
    assert_eq!(error.kind, ScriptErrorKind::Exception);
    assert_eq!(
        error.source,
        ScriptSource { node_id: world.req.clone(), node_name: "Req".into(), node_kind: NodeKind::Request,
            phase: ScriptPhase::Test }
    );
    assert_eq!(error.line, Some(2));
    assert!(error.message.starts_with("TypeError"), "{}", error.message);
}

#[tokio::test]
async fn syntax_error_in_pre_request() {
    let server = ok_server().await;

    let (outcome, _world) = sent(&server, get("pm.variables.set(\"a\", \"1\");\nif (", "")).await;
    let Err(SendError::Script(failure)) = &outcome.result else { panic!("{:?}", outcome.result) };
    assert_eq!(failure.line, Some(2));
    assert!(received(&server).await.is_empty(), "nothing may be sent");
}

#[tokio::test]
async fn await_and_timer() {
    let test = "await new Promise((r) => setTimeout(r, 20)); pm.test(\"after wait\", () => {});";

    let (outcome, _world) = sent(&ok_server().await, get("", test)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("after wait", true)]);
}

#[tokio::test]
async fn set_next_request_is_ignored() {
    let test = "postman.setNextRequest(\"Other\"); pm.execution.setNextRequest(null); pm.test(\"ran\", () => {});";

    let (outcome, _world) = sent(&ok_server().await, get("", test)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("ran", true)]);
}

#[tokio::test]
async fn skip_request_by_condition() {
    let server = ok_server().await;
    let pre = "if (!pm.environment.get(\"enabled\")) pm.execution.skipRequest();";

    let (outcome, _world) = sent(&server, get(pre, "")).await;
    assert!(matches!(outcome.result, Err(SendError::Skipped)), "{:?}", outcome.result);
    assert!(received(&server).await.is_empty(), "nothing may be sent");
}

#[tokio::test]
async fn lodash_and_btoa() {
    let server = json_server(200, "{\"items\":[{\"name\":\"a\"},{\"name\":\"b\"}]}").await;
    let test = "const _ = require(\"lodash\"); \
        pm.environment.set(\"names\", _.map(pm.response.json().items, \"name\").join(\",\")); \
        pm.environment.set(\"b64\", btoa(\"user:pass\"));";

    let (outcome, world) = sent(&server, get("", test)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(world.environment_value("names").as_deref(), Some("a,b"));
    assert_eq!(world.environment_value("b64").as_deref(), Some("dXNlcjpwYXNz"));
}

#[tokio::test]
async fn unsupported_module_is_a_clear_error() {
    let (outcome, _world) = sent(&ok_server().await, get("", "const moment = require(\"moment\");")).await;
    assert_eq!(status(&outcome), 200);
    let [error] = &outcome.scripts.errors[..] else { panic!("{:?}", outcome.scripts.errors) };
    assert_eq!(error.message, "Error: Cannot find module 'moment' (not supported in RequestMaster)");
}

#[tokio::test]
async fn urlencoded_body_built_in_pre_request() {
    let server = ok_server().await;
    let pre = "pm.request.body.urlencoded.add({ key: \"sig\", value: \"x\" })";
    let mut request = request("POST", "{{host}}/form", pre, "");
    request.body = Body::UrlEncoded(vec![row("a", "1")]);

    let (outcome, _world) = sent(&server, request).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(String::from_utf8_lossy(&the_request(&server).await.body), "a=1&sig=x");
}

#[tokio::test]
async fn secret_variable_is_readable() {
    let server = ok_server().await;
    let secret = Variable { secret: true, ..var("password", "s3cret") };
    let pre = "pm.request.headers.add({ key: \"X-P\", value: pm.environment.get(\"password\") })";
    let mut world = Case { environment: vec![secret], ..Default::default() }.start(&server, get(pre, ""));

    let outcome = world.send().await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(header_of(&the_request(&server).await, "X-P"), "s3cret");
}

#[tokio::test]
async fn non_json_body_fails_only_the_test() {
    let server = MockServer::start().await;
    Mock::given(any()).respond_with(ResponseTemplate::new(200).set_body_raw("<html></html>", "text/html"))
        .mount(&server).await;
    let test = "pm.test(\"json\", () => pm.response.json()); tests[\"has html\"] = responseBody.indexOf(\"<html>\") === 0;";

    let (outcome, _world) = sent(&server, get("", test)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("json", false), ("has html", true)]);
    let error = outcome.scripts.tests[0].error.as_deref().unwrap_or_default();
    assert!(error.starts_with("JSONError"), "{error}");
}

#[tokio::test]
async fn disabled_variable_is_invisible() {
    let server = ok_server().await;
    let disabled = Variable { enabled: false, ..var("off", "1") };
    let test = "console.log(pm.environment.has(\"off\"), JSON.stringify(pm.environment.toObject()));\n\
        pm.environment.set(\"off\", \"2\")";
    let mut world = Case { environment: vec![disabled.clone()], ..Default::default() }.start(&server, get("", test));

    let outcome = world.send().await;
    assert_eq!(console(&outcome), ["false {}"]);
    assert_eq!(world.environment(), vec![disabled, var("off", "2")]);
}

#[tokio::test]
async fn endless_script_does_not_hang_the_send() {
    let server = ok_server().await;
    let mut world = Case::default().start(&server, get("", "while (true) {}"));
    world.settings.script_timeout = Duration::from_millis(300);

    let outcome = world.send().await;
    assert_eq!(status(&outcome), 200);
    let [error] = &outcome.scripts.errors[..] else { panic!("{:?}", outcome.scripts.errors) };
    assert_eq!(error.kind, ScriptErrorKind::Timeout);

    world.replace_request(get("", "pm.test(\"next\", () => {});"));
    let outcome = world.send().await;
    assert_eq!(status(&outcome), 200);
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("next", true)]);
}

#[tokio::test]
async fn test_reads_the_body_that_was_sent() {
    let server = MockServer::start().await;
    // The server answers with the body it was sent.
    let echo = |received: &Request| ResponseTemplate::new(200).set_body_raw(received.body.clone(), "application/json");
    Mock::given(any()).respond_with(echo).mount(&server).await;
    let test = "const sent = JSON.parse(pm.request.body.raw); \
        pm.test(\"echo\", () => pm.expect(pm.response.json().name).to.eql(sent.name));";
    let mut request = request("POST", "{{host}}/echo", "", test);
    request.body = Body::Raw { text: "{\"name\":\"{{userName}}\",\"age\":{{age}}}".into(), language: RawLanguage::Json };
    let case = Case { collection_variables: vec![var("userName", "bob"), var("age", "5")], ..Default::default() };
    let mut world = case.start(&server, request);

    let outcome = world.send().await;
    // `pm.request` of a test script is the request that was sent: with `{{age}}` in it the text is no JSON.
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("echo", true)]);
    assert_eq!(outcome.scripts.tests[0].error, None);
}

#[tokio::test]
async fn send_request_keeps_braces() {
    let server = ok_server().await;
    let pre = "await pm.sendRequest({ url: pm.variables.get(\"host\") + \"/render\", method: \"POST\", \
        header: { \"Content-Type\": \"application/json\" }, \
        body: { mode: \"raw\", raw: JSON.stringify({ template: \"Hello {{name}}\" }) } });";
    let case = Case { collection_variables: vec![var("name", "X")], ..Default::default() };
    let mut world = case.start(&server, get(pre, ""));

    let outcome = world.send().await;
    assert_eq!(outcome.scripts.errors, vec![]);
    let received = received(&server).await;
    let render = received.iter().find(|request| request.url.path() == "/render").expect("no request to /render");
    // As in Postman: the request of a script goes out as the script gave it.
    assert_eq!(String::from_utf8_lossy(&render.body), "{\"template\":\"Hello {{name}}\"}");
}

#[tokio::test]
async fn legacy_set_is_seen_in_the_environment_object() {
    let server = json_server(200, "{\"token\":\"abc\"}").await;
    let test = "var data = JSON.parse(responseBody); postman.setEnvironmentVariable(\"token\", data.token); \
        tests[\"token saved\"] = environment.token === data.token;";

    let (outcome, _world) = sent(&server, get("", test)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("token saved", true)]);
    assert_eq!(outcome.scripts.changes, vec![set(StoredScope::Environment, "token", "abc")]);
}

#[tokio::test]
async fn assignment_without_var_to_a_legacy_name() {
    let server = json_server(200, "{\"token\":\"abc\"}").await;
    let test = "data = JSON.parse(responseBody); tests[\"has token\"] = data.token === \"abc\";";

    let (outcome, _world) = sent(&server, get("", test)).await;
    assert_eq!(outcome.scripts.errors, vec![]);
    assert_eq!(tests(&outcome), [("has token", true)]);
}
