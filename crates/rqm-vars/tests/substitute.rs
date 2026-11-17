use rqm_vars::*;

fn r(pairs: &[(ScopeKind, &str, &str)]) -> Resolver {
    let mut scopes: Vec<Scope> = [ScopeKind::Global, ScopeKind::Collection, ScopeKind::Environment, ScopeKind::Local]
        .into_iter().map(|kind| Scope { kind, values: vec![] }).collect();
    for (k, n, v) in pairs { scopes.iter_mut().find(|s| s.kind == *k).unwrap().values.push((n.to_string(), v.to_string())); }
    Resolver::new(scopes)
}
fn no_dyn(_: &str) -> Option<String> { None }
fn sub(res: &Resolver, s: &str) -> (String, Vec<String>) { let o = res.substitute_with(s, &mut no_dyn); (o.text, o.unresolved) }

#[test] fn table_of_cases() {
    use ScopeKind::*;
    let res = r(&[(Global, "host", "g.test"), (Collection, "host", "c.test"), (Environment, "host", "e.test"),
                  (Global, "only_g", "G"), (Environment, "b", "host"), (Environment, "tpl", "{{host}}/x"),
                  (Environment, "empty", ""), (Environment, "sp ace", "S"), (Local, "loc", "L")]);
    let cases: &[(&str, &str, &[&str])] = &[
        ("{{host}}", "e.test", &[]),
        ("{{only_g}}-{{loc}}", "G-L", &[]),
        ("a{{host}}b{{host}}c", "ae.testbe.testc", &[]),
        ("{{{{b}}}}", "e.test", &[]),                 // inner token first
        ("{{tpl}}", "e.test/x", &[]),                 // value containing a token
        ("{{missing}}", "{{missing}}", &["missing"]),
        ("{{missing}} {{missing}}", "{{missing}} {{missing}}", &["missing"]),
        ("{{empty}}|", "|", &[]),
        ("{{sp ace}}", "S", &[]),
        ("{{ host }}", "{{ host }}", &[" host "]),
        ("{host} {{host}", "{host} {{host}", &[]),
        ("{{}}", "{{}}", &[""]),
        ("", "", &[]),
        ("плюс {{host}} ✓", "плюс e.test ✓", &[]),
    ];
    for (input, text, unresolved) in cases {
        let (t, u) = sub(&res, input);
        assert_eq!(&t, text, "input {input:?}");
        assert_eq!(u, unresolved.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "input {input:?}");
    }
}
#[test] fn narrower_scope_wins_and_lookup_reports_scope() {
    use ScopeKind::*;
    let res = r(&[(Global, "x", "1"), (Collection, "x", "2")]);
    assert_eq!(res.lookup("x"), Some(("2", Collection)));
    assert_eq!(res.lookup("nope"), None);
    let res = r(&[(Global, "x", "1"), (Local, "x", "4"), (Environment, "x", "3")]);
    assert_eq!(res.lookup("x"), Some(("4", Local)));
}
#[test] fn cycles_terminate() {
    use ScopeKind::*;
    let res = r(&[(Global, "a", "{{a}}"), (Global, "p", "{{q}}"), (Global, "q", "{{p}}"), (Global, "grow", "{{grow}}{{grow}}")]);
    assert_eq!(sub(&res, "{{a}}").0, "{{a}}");
    let (t, _) = sub(&res, "{{p}}");
    assert!(t == "{{p}}" || t == "{{q}}");
    let started = std::time::Instant::now();
    let (t, _) = sub(&res, "{{grow}}");
    assert!(t.len() <= 8 * (1 << 19));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}
#[test] fn pass_limit_is_19() {
    use ScopeKind::*;
    // v0 -> {{v1}} -> ... -> {{v25}} -> "end": 26 levels, more than 19 passes
    let pairs: Vec<(String, String)> = (0..26).map(|i| (format!("v{i}"), if i == 25 { "end".into() } else { format!("{{{{v{}}}}}", i + 1) })).collect();
    let res = Resolver::new(vec![Scope { kind: Global, values: pairs }]);
    assert_eq!(sub(&res, "{{v7}}").0, "end");          // 19 passes are enough from v7
    assert_eq!(sub(&res, "{{v0}}").0, "{{v19}}");      // stops after 19 passes
}
#[test] fn dynamic_is_called_per_occurrence_and_user_variable_wins() {
    use ScopeKind::*;
    let mut n = 0;
    let mut dynamic = |name: &str| { n += 1; (name == "guid").then(|| format!("G{n}")) };
    let res = r(&[(Global, "$mine", "user")]);
    let out = res.substitute_with("{{$guid}}/{{$guid}}/{{$mine}}/{{$unknown}}", &mut dynamic);
    assert_eq!(out.text, "G1/G2/user/{{$unknown}}");
    assert_eq!(out.unresolved, vec!["$unknown".to_string()]);
}
#[test] fn system_dynamic_values_have_the_right_shape() {
    for name in ["guid", "randomUUID"] {
        let a = system_dynamic(name).unwrap();
        assert_eq!(a.len(), 36);
        assert_ne!(a, system_dynamic(name).unwrap());
    }
    assert!(system_dynamic("timestamp").unwrap().parse::<u64>().unwrap() > 1_700_000_000);
    let iso = system_dynamic("isoTimestamp").unwrap();
    assert_eq!(iso.len(), 24);
    assert!(iso.ends_with('Z') && iso.as_bytes()[10] == b'T');
    for _ in 0..200 { assert!(system_dynamic("randomInt").unwrap().parse::<u32>().unwrap() <= 1000); }
    assert_eq!(system_dynamic("nope"), None);
}
#[test] fn path_variables() {
    let v = vec![("id".to_string(), "7".to_string()), ("name".to_string(), "a b".to_string())];
    assert_eq!(apply_path_variables("https://h.test/u/:id/x/:name?q=:id#:id", &v), "https://h.test/u/7/x/a b?q=:id#:id");
    assert_eq!(apply_path_variables("https://h.test:8080/:missing/:id", &v), "https://h.test:8080/:missing/7");
    assert_eq!(apply_path_variables("{{base}}/:id", &v), "{{base}}/7");
    assert_eq!(apply_path_variables("https://h.test/:idx", &v), "https://h.test/:idx");
    assert_eq!(apply_path_variables("https://h.test/:id.json", &v), "https://h.test/7.json");
    assert_eq!(apply_path_variables("https://h.test/:e/x", &[("e".to_string(), String::new())]), "https://h.test/:e/x");
}
#[test] fn dynamic_names_all_resolve() {
    for name in DYNAMIC_NAMES { assert!(system_dynamic(name).is_some(), "{name}"); }
    assert!(system_dynamic("nope").is_none());
}
#[test] fn entries_are_sorted_by_name() {
    use ScopeKind::*;
    let res = r(&[(Global, "zeta", "1"), (Environment, "alpha", "{{zeta}}"), (Collection, "Mid", "3"), (Global, "alpha", "g"), (Global, "$guid", "u")]);
    // the raw value (not substituted) and the scope that won; byte order of the names
    assert_eq!(res.entries(), vec![("$guid", "u", Global), ("Mid", "3", Collection), ("alpha", "{{zeta}}", Environment), ("zeta", "1", Global)]);
    assert!(r(&[]).entries().is_empty());
}

#[test] fn growth_is_bounded_by_one_mebibyte() {
    // Doubles on every pass: without a bound 19 passes would give 2^19 copies of the token.
    let res = r(&[(ScopeKind::Global, "a", "{{a}}{{a}}"), (ScopeKind::Global, "big", &"x".repeat(700_000))]);
    let (text, _) = sub(&res, "{{a}}");
    assert!(text.len() <= 5 + 1_048_576, "{}", text.len());
    assert!(text.len() > 500_000, "{}", text.len());
    assert!(text.split("{{a}}").all(str::is_empty), "only whole tokens are left");
    // One pass is bounded too: the second value would cross the bound, so its token stays as written.
    let (text, unresolved) = sub(&res, "{{big}}-{{big}}");
    assert_eq!(text.len(), 700_000 + "-{{big}}".len());
    assert!(text.ends_with("x-{{big}}"));
    assert!(unresolved.is_empty());
}

#[test] fn a_large_input_is_still_substituted_in_full() {
    let res = r(&[(ScopeKind::Global, "x", "{{y}}"), (ScopeKind::Global, "y", "v")]);
    let input = format!("{}{{{{x}}}}", "t".repeat(2 * 1_048_576));
    let (text, unresolved) = sub(&res, &input);
    assert_eq!(text.len(), 2 * 1_048_576 + 1);
    assert!(text.ends_with("tv"));
    assert!(unresolved.is_empty());
}
