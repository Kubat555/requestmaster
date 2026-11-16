use rqm_model::*;
use rqm_runner::*;
use rqm_store::{Store, StoreError};

fn var(k: &str, v: &str) -> Variable { Variable { key: k.into(), value: v.into(), secret: false, enabled: true, postman_extra: Extra::new() } }
fn secret(k: &str, v: &str) -> Variable { Variable { secret: true, ..var(k, v) } }
fn off(k: &str, v: &str) -> Variable { Variable { enabled: false, ..var(k, v) } }
fn sources(globals: Vec<Variable>, collection: Vec<Variable>, environment: Vec<Variable>) -> ScopeSources { ScopeSources { globals, collection, environment } }
fn find<'a>(all: &'a [ScopeVariable], name: &str) -> &'a ScopeVariable { all.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("no {name} in {all:?}")) }
fn names(all: &[ScopeVariable]) -> Vec<&str> { all.iter().map(|v| v.name.as_str()).collect() }
fn dynamic_names(all: &[ScopeVariable]) -> Vec<&str> { all.iter().filter(|v| v.origin == VariableOrigin::Dynamic).map(|v| v.name.as_str()).collect() }
fn collection_content(vars: Vec<Variable>) -> NodeContent { NodeContent::Collection(CollectionContent { auth: None, variables: vars, scripts: Scripts::default(), postman_extra: Extra::new() }) }
fn folder_content() -> NodeContent { NodeContent::Folder(FolderContent { auth: None, scripts: Scripts::default(), postman_extra: Extra::new() }) }
fn request_content(url: &str) -> RequestContent {
    RequestContent { method: "GET".into(), url: url.into(), query: vec![], path_variables: vec![], headers: vec![],
        body: Body::None, auth: None, scripts: Scripts::default(), settings: RequestSettings::default(), disabled_auto_headers: vec![], postman_extra: Extra::new() }
}
fn environment(name: &str, vars: Vec<Variable>) -> Environment { Environment { id: new_id(), kind: EnvKind::Environment, name: name.into(), variables: vars, postman_extra: Extra::new() } }

#[test] fn narrowest_scope_wins_and_value_is_fully_substituted() {
    let all = variables_in_scope(&sources(vec![var("host", "g")], vec![var("host", "c"), var("url", "{{host}}/api")], vec![var("host", "e")]));
    let host = find(&all, "host");
    assert_eq!((host.origin, host.value.as_deref(), host.secret), (VariableOrigin::Environment, Some("e"), false));
    let url = find(&all, "url");
    assert_eq!((url.origin, url.value.as_deref(), url.secret), (VariableOrigin::Collection, Some("e/api"), false));
    assert_eq!(all.iter().filter(|v| v.name == "host").count(), 1, "one entry per name");
    // the variables by name, then the dynamic ones in the order of DYNAMIC_NAMES
    assert_eq!(names(&all), vec!["host", "url", "$guid", "$randomUUID", "$timestamp", "$isoTimestamp", "$randomInt"]);
    let only_global = variables_in_scope(&sources(vec![var("host", "g")], vec![], vec![]));
    assert_eq!((find(&only_global, "host").origin, find(&only_global, "host").value.as_deref()), (VariableOrigin::Global, Some("g")));
}
#[test] fn disabled_variables_are_left_out() {
    let all = variables_in_scope(&sources(vec![off("gone", "1"), var("host", "g")], vec![off("host", "c")], vec![off("host", "e"), off("also_gone", "2")]));
    assert_eq!(names(&all)[..1], ["host"]);
    assert_eq!(all.len(), 1 + 5);
    // a disabled narrower variable does not shadow the enabled wider one
    assert_eq!((find(&all, "host").origin, find(&all, "host").value.as_deref()), (VariableOrigin::Global, Some("g")));
    // a disabled secret does not make its name secret
    let all = variables_in_scope(&sources(vec![var("token", "open")], vec![], vec![Variable { enabled: false, ..secret("token", "s3cr3t") }]));
    assert_eq!((find(&all, "token").value.as_deref(), find(&all, "token").secret), (Some("open"), false));
}
#[test] fn secret_is_never_returned_even_through_a_reference() {
    let all = variables_in_scope(&sources(vec![var("plain", "p")], vec![var("auth", "Bearer {{token}}")], vec![secret("token", "s3cr3t")]));
    for name in ["token", "auth"] {
        let found = find(&all, name);
        assert!(found.secret, "{name}");
        assert_eq!(found.value, None, "{name}");
    }
    assert_eq!((find(&all, "token").origin, find(&all, "auth").origin), (VariableOrigin::Environment, VariableOrigin::Collection));
    assert!(!format!("{all:?}").contains("s3cr3t"));
    assert_eq!((find(&all, "plain").value.as_deref(), find(&all, "plain").secret), (Some("p"), false));
    // a secret shadowed by an open variable of a narrower scope is not what the name resolves to
    let all = variables_in_scope(&sources(vec![secret("token", "s3cr3t")], vec![], vec![var("token", "open")]));
    assert_eq!((find(&all, "token").value.as_deref(), find(&all, "token").secret), (Some("open"), false));
    // an empty secret is still a secret; a reference through two steps is one too
    let all = variables_in_scope(&sources(vec![secret("empty", ""), secret("deep", "s3cr3t"), var("mid", "{{deep}}"), var("top", "<{{mid}}>")], vec![], vec![]));
    for name in ["empty", "deep", "mid", "top"] {
        assert_eq!((find(&all, name).value.as_deref(), find(&all, name).secret), (None, true), "{name}");
    }
    assert!(!format!("{all:?}").contains("s3cr3t"));
}
#[test] fn dynamic_names_are_listed_unless_shadowed() {
    let all = variables_in_scope(&sources(vec![], vec![], vec![]));
    assert_eq!(names(&all), vec!["$guid", "$randomUUID", "$timestamp", "$isoTimestamp", "$randomInt"]);
    for entry in &all {
        assert_eq!((entry.origin, entry.value.as_deref(), entry.secret), (VariableOrigin::Dynamic, None, false), "{}", entry.name);
    }
    let all = variables_in_scope(&sources(vec![var("$guid", "mine")], vec![], vec![]));
    let guid = find(&all, "$guid");
    assert_eq!((guid.origin, guid.value.as_deref()), (VariableOrigin::Global, Some("mine")));
    assert_eq!(all.iter().filter(|v| v.name == "$guid").count(), 1);
    assert_eq!(dynamic_names(&all), vec!["$randomUUID", "$timestamp", "$isoTimestamp", "$randomInt"]);
    // a value that refers to a dynamic name keeps the token: the value is created at the send
    let all = variables_in_scope(&sources(vec![var("id", "{{$guid}}")], vec![], vec![]));
    assert_eq!(find(&all, "id").value.as_deref(), Some("{{$guid}}"));
}
#[test] fn request_folder_and_collection_share_one_collection_scope() {
    let mut s = Store::open_in_memory().unwrap();
    let c = s.create_node(None, NodeKind::Collection, "C", &collection_content(vec![var("host", "c"), off("skipped", "x")])).unwrap();
    let f = s.create_node(Some(&c), NodeKind::Folder, "F", &folder_content()).unwrap();
    let r = s.create_node(Some(&f), NodeKind::Request, "R", &NodeContent::Request(request_content("{{host}}"))).unwrap();
    let other = s.create_node(None, NodeKind::Collection, "Other", &collection_content(vec![var("host", "other")])).unwrap();
    let mut g = Environment::empty_globals(); g.variables.push(var("g", "1"));
    s.save_environment(&g).unwrap();
    let env = environment("E", vec![var("e", "2")]);
    s.save_environment(&env).unwrap();

    for id in [&c, &f, &r] {
        let loaded = ScopeSources::load(&s, id, Some(&env.id)).unwrap();
        // the stored variables as they are, the disabled one included
        assert_eq!(loaded.collection, vec![var("host", "c"), off("skipped", "x")], "{id}");
        assert_eq!((loaded.globals.clone(), loaded.environment.clone()), (vec![var("g", "1")], vec![var("e", "2")]));
        let all = variables_in_scope(&loaded);
        assert_eq!(names(&all)[..3], ["e", "g", "host"]);
        assert_eq!((find(&all, "host").origin, find(&all, "host").value.as_deref()), (VariableOrigin::Collection, Some("c")));
    }
    assert_eq!(ScopeSources::load(&s, &other, None).unwrap().collection, vec![var("host", "other")]);
    assert!(ScopeSources::load(&s, &r, None).unwrap().environment.is_empty());
    assert!(matches!(ScopeSources::load(&s, "no-such-node", None), Err(StoreError::NotFound(_))));
    assert!(matches!(ScopeSources::load(&s, &r, Some("no-such-environment")), Err(StoreError::NotFound(_))));
    // the globals set is not an environment
    assert!(matches!(ScopeSources::load(&s, &r, Some(GLOBALS_ID)), Err(StoreError::NotFound(_))));
}
#[test] fn cycle_terminates_and_keeps_the_token() {
    let started = std::time::Instant::now();
    let all = variables_in_scope(&sources(vec![var("a", "{{b}}"), var("b", "{{a}}"), var("self", "{{self}}")], vec![], vec![]));
    for name in ["a", "b", "self"] {
        let found = find(&all, name);
        assert!(found.value.as_deref().unwrap().contains("{{"), "{name}: {:?}", found.value);
        assert!(!found.secret, "{name}");
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}
#[test] fn scope_and_send_agree() {
    let collection = Node { id: new_id(), parent_id: None, kind: NodeKind::Collection, name: "Col".into(), sort_order: 0, content: collection_content(vec![var("a", "c"), off("b", "disabled")]) };
    let mut globals = Environment::empty_globals(); globals.variables.push(var("a", "h.test"));
    let input = SendInput { request_name: "Req".into(), request: request_content("{{a}}/{{b}}"), ancestors: vec![collection],
        environment: Some(environment("E", vec![var("a", "env.test")])), globals, settings: AppSettings::default() };
    let out = prepare(&input, &mut |_| None).unwrap();
    assert_eq!(out.http.url, "http://env.test/{{b}}");
    assert!(matches!(&out.warnings[..], [SendWarning::UnresolvedVariables { names }] if names == &vec!["b".to_string()]));
    let all = variables_in_scope(&ScopeSources::of_input(&input));
    assert_eq!((find(&all, "a").origin, find(&all, "a").value.as_deref()), (VariableOrigin::Environment, Some("env.test")));
    assert!(all.iter().all(|v| v.name != "b"), "{all:?}");
}
