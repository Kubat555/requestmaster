//! Spike: "typed core + bag of unknown fields" model for Postman Collection v2.1.
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

type Extra = Map<String, Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub info: Info,
    pub item: Vec<Item>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<Vec<Event>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<Vec<Variable>>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Folder or request, told apart by the presence of the `item` / `request` key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Item {
    Folder(Folder),
    Request(RequestItem),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub item: Vec<Item>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<Vec<Event>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<Vec<Variable>>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub request: Request,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<Vec<Event>>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Request is either a bare URL string or an object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Request {
    Str(String),
    Obj(RequestObj),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestObj {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<Url>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<Vec<Header>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Url is either a string or an object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Url {
    Str(String),
    Obj(UrlObj),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlObj {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub listen: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<Script>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exec: Option<Exec>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `exec` is either one string or an array of lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Exec {
    Str(String),
    Lines(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(flatten)]
    pub extra: Extra,
}

pub fn parse_collection(json: &str) -> Result<Collection, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn to_json(c: &Collection) -> String {
    serde_json::to_string_pretty(c).unwrap()
}

/// Paths where `a` and `b` differ. Object key order is ignored, array order is not.
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
                    None => out.push(format!("{path}.{k} (missing in right)")),
                }
            }
            for k in y.keys().filter(|k| !x.contains_key(*k)) {
                out.push(format!("{path}.{k} (missing in left)"));
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!("{path} (array length {} vs {})", x.len(), y.len()));
            }
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                diff_at(&format!("{path}[{i}]"), v, w, out);
            }
        }
        _ => {
            if a != b {
                out.push(format!("{path} (value differs)"));
            }
        }
    }
}

pub fn walk_items<'a>(items: &'a [Item], f: &mut dyn FnMut(&'a Item)) {
    for it in items {
        f(it);
        if let Item::Folder(fo) = it {
            walk_items(&fo.item, f);
        }
    }
}

pub fn count_folders(c: &Collection) -> usize {
    let mut n = 0;
    walk_items(&c.item, &mut |i| n += matches!(i, Item::Folder(_)) as usize);
    n
}

pub fn count_requests(c: &Collection) -> usize {
    let mut n = 0;
    walk_items(&c.item, &mut |i| n += matches!(i, Item::Request(_)) as usize);
    n
}

fn exec_text(e: &Event) -> Option<String> {
    match e.script.as_ref()?.exec.as_ref()? {
        Exec::Str(s) => Some(s.clone()),
        Exec::Lines(l) => Some(l.join("\n")),
    }
}

/// Counts of `pm.<a>.<b>`, `postman.<a>`, `require('<x>')`, and legacy markers in all scripts.
pub fn script_api_usage(c: &Collection) -> BTreeMap<String, usize> {
    let mut texts: Vec<String> = Vec::new();
    let mut take = |ev: &Option<Vec<Event>>| {
        for e in ev.iter().flatten() {
            texts.extend(exec_text(e));
        }
    };
    take(&c.event);
    walk_items(&c.item, &mut |i| match i {
        Item::Folder(f) => take(&f.event),
        Item::Request(r) => take(&r.event),
    });

    let pm = Regex::new(r"\bpm\.(\w+)(?:\.(\w+))?").unwrap();
    let postman = Regex::new(r"\bpostman\.(\w+)").unwrap();
    let req = Regex::new(r#"\brequire\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();
    let mut m = BTreeMap::new();
    let mut bump = |k: String| *m.entry(k).or_insert(0usize) += 1;
    for t in &texts {
        for c in pm.captures_iter(t) {
            bump(match c.get(2) {
                Some(b) => format!("pm.{}.{}", &c[1], b.as_str()),
                None => format!("pm.{}", &c[1]),
            });
        }
        for c in postman.captures_iter(t) {
            bump(format!("postman.{}", &c[1]));
        }
        for c in req.captures_iter(t) {
            bump(format!("require('{}')", &c[1]));
        }
        for lit in ["tests[", "responseBody", "responseCode"] {
            for _ in t.matches(lit) {
                bump(lit.to_string());
            }
        }
    }
    m
}
