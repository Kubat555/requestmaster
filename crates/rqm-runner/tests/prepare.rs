use rqm_http::{HttpBody, PartValue};
use rqm_model::*;
use rqm_runner::*;

fn p(k: &str, v: &str) -> Param { Param { key: k.into(), value: v.into(), enabled: true, postman_extra: Extra::new() } }
fn off(k: &str, v: &str) -> Param { Param { enabled: false, ..p(k, v) } }
fn var(k: &str, v: &str) -> Variable { Variable { key: k.into(), value: v.into(), secret: false, enabled: true, postman_extra: Extra::new() } }
fn base_request() -> RequestContent {
    RequestContent { method: "GET".into(), url: "https://h.test/x".into(), query: vec![], path_variables: vec![], headers: vec![],
        body: Body::None, auth: None, scripts: Scripts::default(), settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }
}
fn collection(auth: Option<Auth>, vars: Vec<Variable>, pre: &str) -> Node {
    Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: "Col".into(), sort_order: 0,
        content: NodeContent::Collection(CollectionContent { auth, variables: vars, scripts: Scripts { pre_request: pre.into(), test: String::new() }, postman_extra: Extra::new() }) }
}
fn folder(name: &str, auth: Option<Auth>) -> Node {
    Node { id: new_id(), parent_id: Some("x".into()), kind: NodeKind::Folder, name: name.into(), sort_order: 0,
        content: NodeContent::Folder(FolderContent { auth, scripts: Scripts::default(), postman_extra: Extra::new() }) }
}
fn input(request: RequestContent, ancestors: Vec<Node>) -> SendInput {
    SendInput { request_name: "Req".into(), request, ancestors, environment: None, globals: Environment::empty_globals(), settings: AppSettings::default() }
}
fn prep(i: &SendInput) -> Prepared { prepare(i, &mut |_| None).unwrap() }
fn header<'a>(h: &'a Prepared, name: &str) -> Option<&'a str> { h.http.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()) }
fn bytes(h: &Prepared) -> (String, Option<String>) { match &h.http.body { HttpBody::Bytes { data, default_content_type } => (String::from_utf8(data.clone()).unwrap(), default_content_type.clone()), _ => panic!("not bytes") } }

#[test] fn scope_priority_and_substitution_everywhere() {
    let mut r = base_request();
    r.method = "POST".into();
    r.url = "{{base}}/u/:id".into();
    r.path_variables = vec![p("id", "{{who}}")];
    r.query = vec![p("{{k}}", "{{who}} x&y"), off("skip", "1")];
    r.headers = vec![p("X-{{k}}", "{{who}}"), off("X-Off", "1"), p("", "ignored")];
    r.body = Body::Raw { text: "{\"a\":\"{{who}}\",\"b\":\"{{nope}}\"}".into(), language: RawLanguage::Json };
    let mut i = input(r, vec![collection(None, vec![var("who", "collection"), var("k", "key")], "")]);
    i.globals.variables = vec![var("who", "global"), var("base", "h.test")];
    i.environment = Some(Environment { id: new_id(), kind: EnvKind::Environment, name: "E".into(), variables: vec![var("who", "env"), Variable { enabled: false, ..var("k", "disabled") }], postman_extra: Extra::new() });
    let out = prep(&i);
    assert_eq!(out.http.url, "http://h.test/u/env?key=env%20x%26y");
    assert_eq!(header(&out, "X-key"), Some("env"));
    assert_eq!(out.http.headers.len(), 1);
    assert_eq!(bytes(&out), ("{\"a\":\"env\",\"b\":\"{{nope}}\"}".into(), Some("application/json".into())));
    assert!(matches!(&out.warnings[..], [SendWarning::UnresolvedVariables { names }] if names == &vec!["nope".to_string()]));
}
#[test] fn auth_is_inherited_from_the_nearest_ancestor() {
    let col = collection(Some(Auth::Bearer { token: "{{t}}".into() }), vec![var("t", "tok")], "");
    assert_eq!(header(&prep(&input(base_request(), vec![col.clone(), folder("F", None)])), "authorization"), Some("Bearer tok"));
    let basic = folder("F", Some(Auth::Basic { username: "u".into(), password: "p".into() }));
    assert_eq!(header(&prep(&input(base_request(), vec![col.clone(), basic])), "authorization"), Some("Basic dTpw"));
    assert_eq!(header(&prep(&input(base_request(), vec![col.clone(), folder("F", Some(Auth::NoAuth))])), "authorization"), None);
    let mut r = base_request(); r.auth = Some(Auth::NoAuth);
    assert_eq!(header(&prep(&input(r, vec![col.clone()])), "authorization"), None);
    assert_eq!(header(&prep(&input(base_request(), vec![])), "authorization"), None);
}
#[test] fn auth_kinds() {
    let with = |a: Auth| { let mut r = base_request(); r.auth = Some(a); r.headers = vec![p("Authorization", "manual")]; prep(&input(r, vec![])) };
    let oauth = |prefix: &str, placement| Auth::OAuth2Token { token: "t".into(), header_prefix: prefix.into(), placement };
    assert_eq!(header(&with(oauth("Token", ApiKeyPlacement::Header)), "authorization"), Some("Token t"));
    assert_eq!(header(&with(oauth("", ApiKeyPlacement::Header)), "authorization"), Some("t"));
    let oq = with(oauth("Bearer", ApiKeyPlacement::Query));
    assert_eq!((oq.http.url.as_str(), header(&oq, "authorization")), ("https://h.test/x?access_token=t", Some("manual")));
    assert_eq!(header(&with(Auth::Basic { username: String::new(), password: String::new() }), "authorization"), Some("Basic Og=="));
    assert_eq!(header(&with(Auth::ApiKey { key: String::new(), value: "v".into(), placement: ApiKeyPlacement::Header }), "authorization"), Some("manual"));
    assert_eq!(header(&with(Auth::Bearer { token: "t".into() }), "authorization"), Some("Bearer t"));
    assert_eq!(header(&with(Auth::Bearer { token: String::new() }), "authorization"), Some("manual"));
    let k = with(Auth::ApiKey { key: "X-Api".into(), value: "v".into(), placement: ApiKeyPlacement::Header });
    assert_eq!(header(&k, "x-api"), Some("v"));
    let q = with(Auth::ApiKey { key: "api_key".into(), value: "v".into(), placement: ApiKeyPlacement::Query });
    assert_eq!(q.http.url, "https://h.test/x?api_key=v");
    let u = with(Auth::Unsupported { auth_type: "awsv4".into() });
    assert_eq!(header(&u, "authorization"), Some("manual"));
    assert!(u.warnings.iter().any(|w| matches!(w, SendWarning::UnsupportedAuth { auth_type } if auth_type == "awsv4")));
}
#[test] fn bodies() {
    let with = |b: Body| { let mut r = base_request(); r.method = "POST".into(); r.body = b; prep(&input(r, vec![])) };
    for (lang, ct) in [(RawLanguage::Text, "text/plain"), (RawLanguage::Xml, "application/xml"), (RawLanguage::Html, "text/html"), (RawLanguage::JavaScript, "application/javascript")] {
        assert_eq!(bytes(&with(Body::Raw { text: "t".into(), language: lang })).1.as_deref(), Some(ct));
    }
    assert_eq!(bytes(&with(Body::UrlEncoded(vec![p("a b", "1&2"), off("x", "y"), p("я", "ю")]))),
        ("a+b=1%262&%D1%8F=%D1%8E".into(), Some("application/x-www-form-urlencoded".into())));
    assert_eq!(bytes(&with(Body::GraphQl { query: "{ a }".into(), variables: "{\"n\": 1}".into() })),
        ("{\"query\":\"{ a }\",\"variables\":{\"n\":1}}".into(), Some("application/json".into())));
    assert_eq!(bytes(&with(Body::GraphQl { query: "{ a }".into(), variables: "  ".into() })).0, "{\"query\":\"{ a }\"}");
    assert_eq!(bytes(&with(Body::GraphQl { query: "q".into(), variables: "{oops".into() })).0, "{\"query\":\"q\",\"variables\":{oops}");
    assert!(matches!(with(Body::Binary { file: None }).http.body, HttpBody::None));
    assert!(matches!(with(Body::Binary { file: Some(FileRef { path: "C:/f.bin".into(), confirmed: true }) }).http.body, HttpBody::File { default_content_type: None, .. }));
    let form = with(Body::FormData(vec![
        FormPart { key: "t".into(), value: FormValue::Text("v".into()), enabled: true, content_type: None, postman_extra: Extra::new() },
        FormPart { key: "f".into(), value: FormValue::Files(vec![FileRef { path: "C:/1".into(), confirmed: true }, FileRef { path: "C:/2".into(), confirmed: true }]), enabled: true, content_type: Some("x/y".into()), postman_extra: Extra::new() },
        FormPart { key: "skipped".into(), value: FormValue::Files(vec![FileRef { path: "C:/3".into(), confirmed: false }]), enabled: false, content_type: None, postman_extra: Extra::new() }]));
    let HttpBody::Multipart(parts) = &form.http.body else { panic!() };
    assert_eq!(parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["t", "f", "f"]);
    assert!(matches!(parts[1].value, PartValue::File(_)) && parts[1].content_type.as_deref() == Some("x/y"));
}
#[test] fn unconfirmed_files_block_the_send() {
    let mut r = base_request(); r.method = "POST".into();
    r.body = Body::FormData(vec![FormPart { key: "doc".into(), value: FormValue::Files(vec![FileRef { path: "C:/secret.txt".into(), confirmed: false }]), enabled: true, content_type: None, postman_extra: Extra::new() }]);
    assert!(matches!(prepare(&input(r, vec![]), &mut |_| None), Err(PrepareError::UnconfirmedFiles { fields }) if fields == vec!["doc".to_string()]));
    let mut r = base_request(); r.method = "POST".into();
    r.body = Body::Binary { file: Some(FileRef { path: "C:/f".into(), confirmed: false }) };
    assert!(matches!(prepare(&input(r, vec![]), &mut |_| None), Err(PrepareError::UnconfirmedFiles { fields }) if fields == vec!["binary".to_string()]));
}
#[test] fn body_pruning() {
    for (m, pruned) in [("GET", true), ("get", true), ("HEAD", true), ("COPY", true), ("PURGE", true), ("UNLOCK", true), ("POST", false), ("DELETE", false)] {
        let mut r = base_request(); r.method = m.into(); r.body = Body::Raw { text: "x".into(), language: RawLanguage::Text };
        let out = prep(&input(r, vec![]));
        assert_eq!(matches!(out.http.body, HttpBody::None), pruned, "{m}");
        assert_eq!(out.warnings.iter().any(|w| matches!(w, SendWarning::BodyPruned { .. })), pruned, "{m}");
    }
    let mut r = base_request(); r.body = Body::Raw { text: "x".into(), language: RawLanguage::Text };
    r.settings.disable_body_pruning = Some(true);
    let out = prep(&input(r, vec![]));
    assert!(matches!(out.http.body, HttpBody::Bytes { .. }) && out.warnings.is_empty());
    assert!(prep(&input(base_request(), vec![])).warnings.is_empty());          // GET without a body: no warning
}
#[test] fn settings_resolution() {
    let mut i = input(base_request(), vec![]);
    i.settings.verify_tls = false;
    i.settings.timeout = Some(std::time::Duration::from_secs(3));
    i.request.disabled_auto_headers = vec!["accept-encoding".into()];
    let o = prep(&i).http.options;
    assert_eq!((o.follow_redirects, o.max_redirects, o.verify_tls, o.timeout), (true, 10, false, Some(std::time::Duration::from_secs(3))));
    assert_eq!(o.disabled_auto_headers, vec!["accept-encoding".to_string()]);
    i.request.settings = RequestSettings { follow_redirects: Some(false), max_redirects: Some(2), strict_ssl: Some(true), disable_body_pruning: None };
    let o = prep(&i).http.options;
    assert_eq!((o.follow_redirects, o.max_redirects, o.verify_tls), (false, 2, true));
}
#[test] fn scripts_produce_a_warning_naming_the_nodes() {
    let mut r = base_request(); r.scripts.test = "pm.test('x', () => {})".into();
    let out = prep(&input(r, vec![collection(None, vec![], "console.log(1)"), folder("F", None)]));
    assert!(matches!(&out.warnings[..], [SendWarning::ScriptsNotExecuted { nodes }] if nodes == &vec!["Col".to_string(), "Req".to_string()]));
}
#[test] fn url_rules() {
    let mut r = base_request(); r.url = "h.test/a b#frag".into(); r.query = vec![p("q", "")];
    assert_eq!(prep(&input(r, vec![])).http.url, "http://h.test/a b?q=");
    let mut r = base_request(); r.url = "  ".into();
    assert!(matches!(prepare(&input(r, vec![]), &mut |_| None), Err(PrepareError::InvalidUrl(_))));
    let mut r = base_request(); r.url = "https://h.test/{{$guid}}/{{$guid}}".into();
    let mut n = 0;
    let out = prepare(&input(r, vec![]), &mut |name| { n += 1; (name == "guid").then(|| n.to_string()) }).unwrap();
    assert_eq!(out.http.url, "https://h.test/1/2");
}
