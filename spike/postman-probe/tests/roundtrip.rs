use postman_probe::*;

fn fixture() -> String {
    std::fs::read_to_string("fixtures/synthetic.postman_collection.json").unwrap()
}

#[test]
fn synthetic_round_trip_has_empty_diff() {
    let src = fixture();
    let out = to_json(&parse_collection(&src).unwrap());
    let d = semantic_diff(&serde_json::from_str(&src).unwrap(), &serde_json::from_str(&out).unwrap());
    assert!(d.is_empty(), "{d:#?}");
}

#[test]
fn unknown_fields_and_auth_survive_round_trip() {
    let out = to_json(&parse_collection(&fixture()).unwrap());
    for needle in ["_exporter_id", "x-unknown", "x-future", "protocolProfileBehavior"] {
        assert!(out.contains(needle), "{needle} lost");
    }
}

#[test]
fn cyrillic_survives_round_trip() {
    let out = to_json(&parse_collection(&fixture()).unwrap());
    assert!(out.contains("Получить токен"));
}

#[test]
fn folder_and_request_are_distinguished() {
    let c = parse_collection(&fixture()).unwrap();
    assert_eq!(count_folders(&c), 2);
    assert_eq!(count_requests(&c), 4);
}

#[test]
fn script_api_usage_counts_calls() {
    let u = script_api_usage(&parse_collection(&fixture()).unwrap());
    assert_eq!(u.get("pm.environment.set"), Some(&1));
    assert_eq!(u.get("require('lodash')"), Some(&1));
}

#[test]
fn semantic_diff_detects_changes_and_ignores_key_order() {
    let a: serde_json::Value = serde_json::from_str(r#"{"a":1,"b":[1,2]}"#).unwrap();
    let same: serde_json::Value = serde_json::from_str(r#"{"b":[1,2],"a":1}"#).unwrap();
    let diff: serde_json::Value = serde_json::from_str(r#"{"a":1,"b":[2,1]}"#).unwrap();
    assert!(semantic_diff(&a, &same).is_empty());
    assert_eq!(semantic_diff(&a, &diff).len(), 2);
}

#[test]
fn all_owner_samples_round_trip() {
    let dir = std::path::Path::new("../samples");
    if !dir.exists() {
        eprintln!("SKIPPED: ../samples not found");
        return;
    }
    let mut checked = 0;
    for e in walkdir::WalkDir::new(dir).into_iter().flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let src = std::fs::read_to_string(p).unwrap();
        let v: serde_json::Value = match serde_json::from_str(&src) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let is_v21 = v["info"]["schema"].as_str().map_or(false, |s| s.contains("v2.1"));
        if !is_v21 {
            continue;
        }
        let out = to_json(&parse_collection(&src).unwrap_or_else(|e| panic!("{}: parse error {e}", p.display())));
        let d = semantic_diff(&v, &serde_json::from_str(&out).unwrap());
        assert!(d.is_empty(), "{}: {d:#?}", p.display());
        checked += 1;
    }
    eprintln!("owner samples checked: {checked}");
}
