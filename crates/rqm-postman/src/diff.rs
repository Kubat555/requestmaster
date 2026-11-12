//! Semantic comparison of two JSON values, used to check that export loses nothing.

use serde_json::Value;

/// JSON paths (`$.a`, `$.b[0]`) where `a` and `b` differ; values are never part of the result.
/// Empty result = semantically equal. Object key order is ignored, array order is not.
/// Arrays of different length give the path of the array itself.
pub fn semantic_diff(a: &Value, b: &Value) -> Vec<String> {
    let mut out = Vec::new();
    diff_at("$", a, b, &mut out);
    out
}

fn diff_at(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => diff_at(&format!("{path}.{k}"), v, w, out),
                    None => out.push(format!("{path}.{k}")),
                }
            }
            for k in y.keys().filter(|k| !x.contains_key(*k)) {
                out.push(format!("{path}.{k}"));
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(path.to_string());
            }
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                diff_at(&format!("{path}[{i}]"), v, w, out);
            }
        }
        _ => {
            if a != b {
                out.push(path.to_string());
            }
        }
    }
}
