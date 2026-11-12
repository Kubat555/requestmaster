//! Import of a Postman collection (v2.1) and of environment / globals files into the model.
//!
//! Where Postman allows two forms of a value (string or object, array or string) both are read.
//! A value of an unexpected type counts as a missing field; nothing unknown fails the import.

use rqm_model::{
    new_id, ApiKeyPlacement, Auth, Body, CollectionContent, EnvKind, Environment, Extra, FileRef,
    FolderContent, FormPart, FormValue, Node, NodeContent, NodeId, NodeKind, Param, RawLanguage,
    RequestContent, RequestSettings, Scripts, Variable, GLOBALS_ID,
};
use serde_json::{Map, Value};

use crate::detect::{detect, Detected, ImportError};
use crate::report::{ImportReport, Note, ReportItem, ReportKind};

type Obj = Map<String, Value>;

/// Result of importing one collection file.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedCollection {
    /// `nodes[0]` is the collection; every parent precedes its children; siblings keep file order
    /// with `sort_order` 0, 1, 2…; all ids are fresh.
    pub nodes: Vec<Node>,
    /// What was read but will not work as in Postman.
    pub report: ImportReport,
}

/// Imports a Postman collection of schema v2.1.0.
///
/// `nodes[0]` is the collection; every parent precedes its children; siblings keep file order
/// with `sort_order` 0, 1, 2…; all ids are fresh (`new_id()`).
pub fn import_collection(json: &Value) -> Result<ImportedCollection, ImportError> {
    if detect(json)? != Detected::Collection {
        return Err(ImportError::UnknownFormat);
    }
    let root = json.as_object().ok_or(ImportError::UnknownFormat)?;

    let name = json.pointer("/info/name").and_then(Value::as_str).unwrap_or_default().to_string();
    let mut notes = Vec::new();
    let auth = parse_auth(root.get("auth"), &mut notes);
    note_all_settings(root.get("protocolProfileBehavior"), &mut notes);
    let scripts = parse_scripts(root.get("event"));
    note_scripts(&scripts, &mut notes);
    let content = CollectionContent {
        auth,
        variables: parse_variables(root.get("variable"), VariableStyle::Collection),
        scripts,
        postman_extra: without(root, &["item"]),
    };

    let mut importer = Importer { nodes: Vec::new(), report: ImportReport::default() };
    let id = new_id();
    importer.push(
        Node {
            id: id.clone(),
            parent_id: None,
            kind: NodeKind::Collection,
            name: name.clone(),
            sort_order: 0,
            content: NodeContent::Collection(content),
        },
        &name,
        notes,
    );
    importer.items(root.get("item"), &id, &name);
    Ok(ImportedCollection { nodes: importer.nodes, report: importer.report })
}

/// Imports an environment or a globals file.
/// Globals get `id = GLOBALS_ID`, environments a fresh id.
pub fn import_environment(json: &Value) -> Result<Environment, ImportError> {
    let kind = match detect(json)? {
        Detected::Environment => EnvKind::Environment,
        Detected::Globals => EnvKind::Globals,
        Detected::Collection => return Err(ImportError::UnknownFormat),
    };
    let root = json.as_object().ok_or(ImportError::UnknownFormat)?;
    let (id, default_name) = match kind {
        EnvKind::Environment => (new_id(), ""),
        EnvKind::Globals => (GLOBALS_ID.to_string(), "Globals"),
    };
    Ok(Environment {
        id,
        kind,
        name: str_of(root, "name").unwrap_or(default_name).to_string(),
        variables: parse_variables(root.get("values"), VariableStyle::Environment),
        postman_extra: without(root, &["name"]),
    })
}

/// Collects the nodes and the report while the item tree is walked.
struct Importer {
    nodes: Vec<Node>,
    report: ImportReport,
}

impl Importer {
    /// Adds a node and turns its notes into report records.
    fn push(&mut self, node: Node, path: &str, notes: Vec<Note>) {
        self.report.items.extend(notes.into_iter().map(|(kind, detail)| ReportItem {
            kind,
            node_id: node.id.clone(),
            node_path: path.to_string(),
            detail,
        }));
        self.nodes.push(node);
    }

    /// Imports the elements of an `item` array as children of `parent_id`, depth first.
    fn items(&mut self, items: Option<&Value>, parent_id: &NodeId, parent_path: &str) {
        for (sort_order, item) in (0_i64..).zip(objects(items)) {
            let name = str_of(item, "name").unwrap_or_default().to_string();
            let path = format!("{parent_path} / {name}");
            let id = new_id();
            let mut notes = Vec::new();
            // An element with an `item` array is a folder, anything else is a request.
            let children = item.get("item").filter(|v| v.is_array());
            let (kind, content) = if children.is_some() {
                (NodeKind::Folder, NodeContent::Folder(folder_content(item, &mut notes)))
            } else {
                (NodeKind::Request, NodeContent::Request(request_content(item, &mut notes)))
            };
            let node = Node { id: id.clone(), parent_id: Some(parent_id.clone()), kind, name, sort_order, content };
            self.push(node, &path, notes);
            if children.is_some() {
                self.items(children, &id, &path);
            }
        }
    }
}

/// Reads a folder element. Folder settings are not applied in the first version, so every
/// `protocolProfileBehavior` key is reported.
fn folder_content(item: &Obj, notes: &mut Vec<Note>) -> FolderContent {
    let auth = parse_auth(item.get("auth"), notes);
    note_all_settings(item.get("protocolProfileBehavior"), notes);
    let scripts = parse_scripts(item.get("event"));
    note_scripts(&scripts, notes);
    FolderContent { auth, scripts, postman_extra: without(item, &["item", "name"]) }
}

/// Reads a request element. `request` may be an object or a bare URL string (`GET` on that URL).
fn request_content(item: &Obj, notes: &mut Vec<Note>) -> RequestContent {
    let request = item.get("request");
    let fields = request.and_then(Value::as_object);
    let field = |key: &str| fields.and_then(|r| r.get(key));

    let url = match request {
        Some(Value::String(_)) => parse_url(request),
        _ => parse_url(field("url")),
    };
    let auth = parse_auth(field("auth"), notes);
    let (settings, disabled_auto_headers) = parse_request_settings(item.get("protocolProfileBehavior"), notes);
    let scripts = parse_scripts(item.get("event"));
    note_scripts(&scripts, notes);
    let body = parse_body(field("body"), notes);
    RequestContent {
        method: field("method").and_then(Value::as_str).unwrap_or("GET").to_string(),
        url: url.url,
        query: url.query,
        path_variables: url.path_variables,
        headers: parse_headers(field("header")),
        body,
        auth,
        scripts,
        settings,
        disabled_auto_headers,
        postman_extra: without(item, &["name"]),
    }
}

// ---- tolerant reading helpers -------------------------------------------------------------

/// String value of a key; `None` when the key is missing or is not a string.
pub(crate) fn str_of<'a>(obj: &'a Obj, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

/// `true` only when the key holds the boolean `true`.
pub(crate) fn flag(obj: &Obj, key: &str) -> bool {
    obj.get(key).and_then(Value::as_bool) == Some(true)
}

/// Text form of a value: a string as is, `null` or a missing value as `""`,
/// anything else (number, boolean, array, object) as compact JSON.
pub(crate) fn text_of(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// The objects of an array value; anything that is not an array gives nothing,
/// elements that are not objects are skipped.
pub(crate) fn objects(value: Option<&Value>) -> impl Iterator<Item = &Obj> {
    value.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object)
}

/// Copy of an object without the given keys, key order kept (rule P1).
fn without(obj: &Obj, keys: &[&str]) -> Extra {
    obj.iter().filter(|(k, _)| !keys.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// A `Param` created from text, without a Postman source object.
fn plain_param(key: &str, value: &str) -> Param {
    Param { key: key.to_string(), value: value.to_string(), enabled: true, postman_extra: Extra::new() }
}

// ---- typed fields ---------------------------------------------------------------------------

/// Reads a list of `{key, value, disabled}` rows: query, path variables, headers, url-encoded fields.
pub(crate) fn parse_params(value: Option<&Value>) -> Vec<Param> {
    objects(value)
        .map(|row| Param {
            key: str_of(row, "key").unwrap_or_default().to_string(),
            value: text_of(row.get("value")),
            enabled: !flag(row, "disabled"),
            postman_extra: row.clone(),
        })
        .collect()
}

/// The parts of a Postman `url` as the model keeps them.
pub(crate) struct UrlParts {
    /// Address without the query; a `#fragment` stays at the end.
    pub(crate) url: String,
    /// Query parameters, including the disabled ones.
    pub(crate) query: Vec<Param>,
    /// Values of the `:name` path segments (`url.variable`).
    pub(crate) path_variables: Vec<Param>,
}

/// Reads `url` given as a string or as an object.
///
/// The address is `raw`; without a string `raw` it is assembled from `protocol`, `host`, `port`
/// and `path`. The query comes from `url.query`, and when that is missing, from the address text.
pub(crate) fn parse_url(value: Option<&Value>) -> UrlParts {
    let obj = value.and_then(Value::as_object);
    let address = match (value, obj) {
        (Some(Value::String(s)), _) => s.clone(),
        (_, Some(o)) => str_of(o, "raw").map(str::to_string).unwrap_or_else(|| assemble_address(o)),
        _ => String::new(),
    };
    let (url, query_text) = split_query(&address);
    let listed_query = obj.and_then(|o| o.get("query")).filter(|q| q.is_array());
    let query = match listed_query {
        Some(_) => parse_params(listed_query),
        None => query_from_text(query_text),
    };
    UrlParts { url, query, path_variables: parse_params(obj.and_then(|o| o.get("variable"))) }
}

/// `protocol://` + `host` + `:port` + `/` + `path`; every part is optional.
fn assemble_address(url: &Obj) -> String {
    let mut address = String::new();
    if let Some(protocol) = str_of(url, "protocol").filter(|p| !p.is_empty()) {
        address.push_str(protocol);
        address.push_str("://");
    }
    address.push_str(&joined(url.get("host"), "."));
    // The port is a string in Postman files; a number is accepted too.
    let port = match url.get("port") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    if !port.is_empty() {
        address.push(':');
        address.push_str(&port);
    }
    let path = joined(url.get("path"), "/");
    if !path.is_empty() {
        if !path.starts_with('/') {
            address.push('/');
        }
        address.push_str(&path);
    }
    address
}

/// `host` / `path` as text: a string as is, an array joined with the separator.
/// An array element is a string or an object with a string `value`.
fn joined(value: Option<&Value>, separator: &str) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| part.as_str().or_else(|| part.get("value").and_then(Value::as_str)))
            .collect::<Vec<_>>()
            .join(separator),
        _ => String::new(),
    }
}

/// Cuts the query out of an address: the part from `?` up to `#` (or to the end).
/// Returns the address without it (the fragment stays) and the query text without `?`.
fn split_query(address: &str) -> (String, Option<&str>) {
    let fragment_at = address.find('#').unwrap_or(address.len());
    let (before, fragment) = address.split_at(fragment_at);
    match before.split_once('?') {
        Some((base, query)) => (format!("{base}{fragment}"), Some(query)),
        None => (address.to_string(), None),
    }
}

/// Splits query text by `&` and the first `=`; nothing is percent-decoded.
fn query_from_text(text: Option<&str>) -> Vec<Param> {
    text.unwrap_or_default()
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            plain_param(key, value)
        })
        .collect()
}

/// Reads `header` given as an array of rows or as text (`"A: 1\nB: 2"`).
pub(crate) fn parse_headers(value: Option<&Value>) -> Vec<Param> {
    match value {
        Some(Value::String(text)) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| {
                let (key, value) = line.split_once(':').unwrap_or((line, ""));
                plain_param(key.trim(), value.trim())
            })
            .collect(),
        other => parse_params(other),
    }
}

/// Reads `body`. Notes: a file reference (detail = form field name or `binary`),
/// an unknown `mode`, a GraphQL `operationName`.
pub(crate) fn parse_body(value: Option<&Value>, notes: &mut Vec<Note>) -> Body {
    let Some(body) = value.and_then(Value::as_object) else { return Body::None };
    let Some(mode) = str_of(body, "mode") else { return Body::None };
    // A switched-off body without content is the same as no body.
    if flag(body, "disabled") && matches!(body.get(mode), None | Some(Value::Null)) {
        return Body::None;
    }
    match mode {
        "raw" => {
            let language = body.get("options").and_then(|o| o.pointer("/raw/language")).and_then(Value::as_str);
            Body::Raw {
                text: str_of(body, "raw").unwrap_or_default().to_string(),
                language: match language {
                    Some("json") => RawLanguage::Json,
                    Some("xml") => RawLanguage::Xml,
                    Some("html") => RawLanguage::Html,
                    Some("javascript") => RawLanguage::JavaScript,
                    _ => RawLanguage::Text,
                },
            }
        }
        "urlencoded" => Body::UrlEncoded(parse_params(body.get("urlencoded"))),
        "formdata" => Body::FormData(objects(body.get("formdata")).map(|part| form_part(part, notes)).collect()),
        "file" => {
            let file = imported_files(body.get("file").and_then(|f| f.get("src"))).pop();
            if file.is_some() {
                notes.push((ReportKind::FileReference, "binary".to_string()));
            }
            Body::Binary { file }
        }
        "graphql" => {
            let graphql = body.get("graphql").and_then(Value::as_object);
            if graphql.is_some_and(|g| g.contains_key("operationName")) {
                notes.push((ReportKind::SettingNotApplied, "body.graphql.operationName".to_string()));
            }
            Body::GraphQl {
                query: graphql.and_then(|g| str_of(g, "query")).unwrap_or_default().to_string(),
                // A string as is; any other JSON value as compact JSON text.
                variables: match graphql.and_then(|g| g.get("variables")) {
                    None => String::new(),
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                },
            }
        }
        unknown => {
            notes.push((ReportKind::SettingNotApplied, format!("body.mode: {unknown}")));
            Body::None
        }
    }
}

/// Reads one `formdata` part; `type: "file"` makes it a file part.
fn form_part(part: &Obj, notes: &mut Vec<Note>) -> FormPart {
    let key = str_of(part, "key").unwrap_or_default().to_string();
    let value = if str_of(part, "type") == Some("file") {
        let files = imported_files(part.get("src"));
        if !files.is_empty() {
            notes.push((ReportKind::FileReference, key.clone()));
        }
        FormValue::Files(files)
    } else {
        FormValue::Text(text_of(part.get("value")))
    };
    FormPart {
        key,
        value,
        enabled: !flag(part, "disabled"),
        content_type: str_of(part, "contentType").map(str::to_string),
        postman_extra: part.clone(),
    }
}

/// File references of a `src` value: a string, an array of strings or `null`.
/// Empty paths are dropped; an imported path is never confirmed.
pub(crate) fn imported_files(src: Option<&Value>) -> Vec<FileRef> {
    let paths: Vec<&str> = match src {
        Some(Value::String(path)) => vec![path.as_str()],
        Some(Value::Array(paths)) => paths.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    paths
        .into_iter()
        .filter(|path| !path.is_empty())
        .map(|path| FileRef { path: path.to_string(), confirmed: false })
        .collect()
}

/// Reads `auth`. A missing key or `null` is `None` (inherit from the parent).
/// A type the application does not apply becomes `Auth::Unsupported` with an `UnsupportedAuth` note.
pub(crate) fn parse_auth(value: Option<&Value>, notes: &mut Vec<Note>) -> Option<Auth> {
    let auth = value?.as_object()?;
    // A missing or non-string type is an unknown type with an empty name.
    let auth_type = str_of(auth, "type").unwrap_or_default();
    // Parameters come as an array of `{key, value, type}` or as an object.
    let param = |key: &str| -> Option<&Value> {
        match auth.get(auth_type)? {
            Value::Array(rows) => rows
                .iter()
                .filter_map(Value::as_object)
                .find(|row| str_of(row, "key") == Some(key))
                .and_then(|row| row.get("value")),
            Value::Object(fields) => fields.get(key),
            _ => None,
        }
    };
    let text = |key: &str| text_of(param(key));
    let is = |key: &str, expected: &str| param(key).and_then(Value::as_str) == Some(expected);
    let is_mac_token = || param("tokenType").and_then(Value::as_str).is_some_and(|t| t.eq_ignore_ascii_case("mac"));

    Some(match auth_type {
        "noauth" => Auth::NoAuth,
        "basic" => Auth::Basic { username: text("username"), password: text("password") },
        "bearer" => Auth::Bearer { token: text("token") },
        "apikey" => Auth::ApiKey {
            key: text("key"),
            value: text("value"),
            placement: if is("in", "query") { ApiKeyPlacement::Query } else { ApiKeyPlacement::Header },
        },
        "oauth2" if !is_mac_token() => Auth::OAuth2Token {
            token: text("accessToken"),
            header_prefix: match param("headerPrefix") {
                None | Some(Value::Null) => "Bearer".to_string(),
                prefix => text_of(prefix).trim().to_string(),
            },
            placement: if is("addTokenTo", "queryParams") { ApiKeyPlacement::Query } else { ApiKeyPlacement::Header },
        },
        other => {
            notes.push((ReportKind::UnsupportedAuth, other.to_string()));
            Auth::Unsupported { auth_type: other.to_string() }
        }
    })
}

/// Reads `event`: `prerequest` and `test` scripts; `exec` lines are joined with `\n` (rule P4).
/// When an event is listed twice, the first one is taken.
pub(crate) fn parse_scripts(value: Option<&Value>) -> Scripts {
    let mut pre_request = None;
    let mut test = None;
    for event in objects(value) {
        let slot = match str_of(event, "listen") {
            Some("prerequest") => &mut pre_request,
            Some("test") => &mut test,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(match event.get("script").and_then(|s| s.get("exec")) {
                Some(Value::String(code)) => code.clone(),
                Some(Value::Array(lines)) => lines.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"),
                _ => String::new(),
            });
        }
    }
    Scripts { pre_request: pre_request.unwrap_or_default(), test: test.unwrap_or_default() }
}

/// Adds one `HasScripts` note when the node has a non-empty script; detail = the event names.
fn note_scripts(scripts: &Scripts, notes: &mut Vec<Note>) {
    let events: Vec<&str> = [("prerequest", &scripts.pre_request), ("test", &scripts.test)]
        .into_iter()
        .filter(|(_, code)| !code.trim().is_empty())
        .map(|(event, _)| event)
        .collect();
    if !events.is_empty() {
        notes.push((ReportKind::HasScripts, events.join(", ")));
    }
}

/// Which key of a variable holds the on/off state.
#[derive(Clone, Copy)]
pub(crate) enum VariableStyle {
    /// Collection `variable`: `disabled: true` switches the variable off.
    Collection,
    /// Environment / globals `values`: `enabled: false` switches the variable off.
    Environment,
}

/// Reads a list of variables. A non-string value is kept in its text form (`null` → `""`).
pub(crate) fn parse_variables(value: Option<&Value>, style: VariableStyle) -> Vec<Variable> {
    objects(value)
        .map(|row| Variable {
            key: str_of(row, "key").unwrap_or_default().to_string(),
            value: text_of(row.get("value")),
            secret: str_of(row, "type") == Some("secret"),
            enabled: match style {
                VariableStyle::Collection => !flag(row, "disabled"),
                VariableStyle::Environment => row.get("enabled").and_then(Value::as_bool) != Some(false),
            },
            postman_extra: row.clone(),
        })
        .collect()
}

/// Reads `protocolProfileBehavior` of a request: the settings the application applies and the
/// lower-case names of the switched-off automatic headers (`disabledSystemHeaders`).
/// Every other key, and a known key with a value of the wrong type, gives a `SettingNotApplied` note.
pub(crate) fn parse_request_settings(value: Option<&Value>, notes: &mut Vec<Note>) -> (RequestSettings, Vec<String>) {
    let mut settings = RequestSettings::default();
    let mut disabled_auto_headers = Vec::new();
    for (key, setting) in value.and_then(Value::as_object).into_iter().flatten() {
        let applied = match key.as_str() {
            "followRedirects" => store(&mut settings.follow_redirects, setting.as_bool()),
            "maxRedirects" => store(&mut settings.max_redirects, setting.as_u64().and_then(|n| u32::try_from(n).ok())),
            "strictSSL" => store(&mut settings.strict_ssl, setting.as_bool()),
            "disableBodyPruning" => store(&mut settings.disable_body_pruning, setting.as_bool()),
            "disabledSystemHeaders" => match setting.as_object() {
                Some(headers) => {
                    let off = headers.iter().filter(|(_, v)| v.as_bool() == Some(true));
                    disabled_auto_headers.extend(off.map(|(name, _)| name.to_lowercase()));
                    true
                }
                None => false,
            },
            _ => false,
        };
        if !applied {
            notes.push((ReportKind::SettingNotApplied, key.clone()));
        }
    }
    (settings, disabled_auto_headers)
}

/// Writes a setting when its value has the expected type; tells whether it was written.
fn store<T>(slot: &mut Option<T>, value: Option<T>) -> bool {
    let applied = value.is_some();
    *slot = value;
    applied
}

/// `protocolProfileBehavior` of a collection or a folder: every key is reported as not applied.
fn note_all_settings(value: Option<&Value>, notes: &mut Vec<Note>) {
    for key in value.and_then(Value::as_object).into_iter().flat_map(Map::keys) {
        notes.push((ReportKind::SettingNotApplied, key.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pairs(params: &[Param]) -> Vec<(&str, &str)> {
        params.iter().map(|p| (p.key.as_str(), p.value.as_str())).collect()
    }

    #[test]
    fn url_string_is_split_into_address_and_query() {
        let u = parse_url(Some(&json!("https://example.test/a?x=1&y&z=a=b#frag")));
        assert_eq!(u.url, "https://example.test/a#frag");
        assert_eq!(pairs(&u.query), vec![("x", "1"), ("y", ""), ("z", "a=b")]);
        assert!(u.query.iter().all(|p| p.enabled && p.postman_extra.is_empty()));
        // `?` after `#` belongs to the fragment
        assert_eq!(parse_url(Some(&json!("https://example.test/a#f?x=1"))).url, "https://example.test/a#f?x=1");
    }

    #[test]
    fn url_without_raw_is_assembled_from_parts() {
        let u = parse_url(Some(&json!({"protocol": "https", "host": ["example", "test"], "port": "8443",
            "path": ["a", ":id"], "query": [{"key": "q", "value": null}], "variable": [{"key": "id", "value": 7}]})));
        assert_eq!(u.url, "https://example.test:8443/a/:id");
        assert_eq!(pairs(&u.query), vec![("q", "")]);
        assert_eq!(pairs(&u.path_variables), vec![("id", "7")]);
        let s = parse_url(Some(&json!({"raw": 7, "host": "example.test", "path": "a/b"})));
        assert_eq!(s.url, "example.test/a/b");
    }

    #[test]
    fn url_object_query_wins_over_raw_text() {
        let u = parse_url(Some(&json!({"raw": "https://example.test/a?x=1", "query": []})));
        assert_eq!((u.url.as_str(), u.query.len()), ("https://example.test/a", 0));
        let v = parse_url(Some(&json!({"raw": "https://example.test/a?x=1", "query": null})));
        assert_eq!(pairs(&v.query), vec![("x", "1")]);
    }

    #[test]
    fn auth_forms_and_oauth2_rules() {
        let mut notes = Vec::new();
        assert_eq!(parse_auth(None, &mut notes), None);
        assert_eq!(parse_auth(Some(&json!(null)), &mut notes), None);
        assert_eq!(
            parse_auth(Some(&json!({"type": "basic", "basic": {"username": "u", "password": "p"}})), &mut notes),
            Some(Auth::Basic { username: "u".into(), password: "p".into() })
        );
        assert_eq!(
            parse_auth(Some(&json!({"type": "apikey", "apikey": [{"key": "key", "value": "k"}, {"key": "value", "value": "v"}]})), &mut notes),
            Some(Auth::ApiKey { key: "k".into(), value: "v".into(), placement: ApiKeyPlacement::Header })
        );
        assert_eq!(
            parse_auth(Some(&json!({"type": "oauth2", "oauth2": [{"key": "accessToken", "value": "t"}]})), &mut notes),
            Some(Auth::OAuth2Token { token: "t".into(), header_prefix: "Bearer".into(), placement: ApiKeyPlacement::Header })
        );
        assert_eq!(
            parse_auth(Some(&json!({"type": "oauth2", "oauth2": {"accessToken": "t", "headerPrefix": "  ", "addTokenTo": "queryParams"}})), &mut notes),
            Some(Auth::OAuth2Token { token: "t".into(), header_prefix: String::new(), placement: ApiKeyPlacement::Query })
        );
        assert_eq!(
            parse_auth(Some(&json!({"type": "oauth2", "oauth2": {"accessToken": "t", "headerPrefix": null}})), &mut notes),
            Some(Auth::OAuth2Token { token: "t".into(), header_prefix: "Bearer".into(), placement: ApiKeyPlacement::Header })
        );
        assert!(notes.is_empty());
        assert_eq!(
            parse_auth(Some(&json!({"type": "oauth2", "oauth2": {"accessToken": "t", "tokenType": "MAC"}})), &mut notes),
            Some(Auth::Unsupported { auth_type: "oauth2".into() })
        );
        assert_eq!(notes, vec![(ReportKind::UnsupportedAuth, "oauth2".to_string())]);
    }

    #[test]
    fn body_rules() {
        let mut notes = Vec::new();
        assert_eq!(parse_body(None, &mut notes), Body::None);
        assert_eq!(parse_body(Some(&json!({"mode": "raw", "disabled": true})), &mut notes), Body::None);
        assert_eq!(
            parse_body(Some(&json!({"mode": "raw", "raw": "<a/>", "options": {"raw": {"language": "xml"}}})), &mut notes),
            Body::Raw { text: "<a/>".into(), language: RawLanguage::Xml }
        );
        assert_eq!(
            parse_body(Some(&json!({"mode": "raw", "raw": "x", "options": {"raw": {"language": "text"}}})), &mut notes),
            Body::Raw { text: "x".into(), language: RawLanguage::Text }
        );
        assert_eq!(parse_body(Some(&json!({"mode": "file", "file": {"src": null}})), &mut notes), Body::Binary { file: None });
        let form = parse_body(
            Some(&json!({"mode": "formdata", "formdata": [
                {"key": "a", "type": "file", "src": ["/synthetic/1", "/synthetic/2"], "contentType": "text/plain"},
                {"key": "b", "type": "file", "src": null, "disabled": true}]})),
            &mut notes,
        );
        let Body::FormData(parts) = form else { panic!("expected form-data") };
        assert!(matches!(&parts[0].value, FormValue::Files(f) if f.len() == 2));
        assert_eq!(parts[0].content_type.as_deref(), Some("text/plain"));
        assert!(matches!(&parts[1].value, FormValue::Files(f) if f.is_empty()) && !parts[1].enabled);
        assert_eq!(notes, vec![(ReportKind::FileReference, "a".to_string())]);

        notes.clear();
        assert_eq!(
            parse_body(Some(&json!({"mode": "graphql", "graphql": {"query": "{ a }", "variables": {"id": 1}, "operationName": "Q"}})), &mut notes),
            Body::GraphQl { query: "{ a }".into(), variables: "{\"id\":1}".into() }
        );
        assert_eq!(parse_body(Some(&json!({"mode": "weird"})), &mut notes), Body::None);
        assert_eq!(
            notes,
            vec![
                (ReportKind::SettingNotApplied, "body.graphql.operationName".to_string()),
                (ReportKind::SettingNotApplied, "body.mode: weird".to_string())
            ]
        );
    }

    #[test]
    fn scripts_accept_string_and_lines() {
        let s = parse_scripts(Some(&json!([
            {"listen": "prerequest", "script": {"exec": "one"}},
            {"listen": "test", "script": {"exec": ["a", "b"]}},
            {"listen": "other", "script": {"exec": "x"}}])));
        assert_eq!(s, Scripts { pre_request: "one".into(), test: "a\nb".into() });
    }

    #[test]
    fn settings_of_requests_and_folders() {
        let mut notes = Vec::new();
        let (settings, off) = parse_request_settings(
            Some(&json!({"maxRedirects": 3, "strictSSL": false, "followRedirects": "yes",
                "disabledSystemHeaders": {"User-Agent": true, "Accept": false}})),
            &mut notes,
        );
        assert_eq!(settings, RequestSettings { follow_redirects: None, max_redirects: Some(3), strict_ssl: Some(false), disable_body_pruning: None });
        assert_eq!(off, vec!["user-agent".to_string()]);
        assert_eq!(notes, vec![(ReportKind::SettingNotApplied, "followRedirects".to_string())]);

        let j = json!({"info": {"name": "C", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
            "item": [{"name": "F", "item": [], "protocolProfileBehavior": {"followRedirects": false},
                "event": [{"listen": "test", "script": {"exec": ["  "]}}]}]});
        let c = import_collection(&j).unwrap();
        assert_eq!(c.report.items.len(), 1);
        assert_eq!((c.report.items[0].kind, c.report.items[0].detail.as_str()), (ReportKind::SettingNotApplied, "followRedirects"));
        assert_eq!(c.report.items[0].node_path, "C / F");
        assert_eq!(c.report.items[0].node_id, c.nodes[1].id);
    }

    #[test]
    fn collection_variables_keep_text_form() {
        let v = parse_variables(
            Some(&json!([{"key": "n", "value": 1.5}, {"key": "b", "value": true, "type": "secret"}, {"key": "z", "value": null, "disabled": true}])),
            VariableStyle::Collection,
        );
        assert_eq!(v.iter().map(|x| x.value.as_str()).collect::<Vec<_>>(), vec!["1.5", "true", ""]);
        assert!(v[1].secret && !v[2].enabled && v[0].postman_extra.contains_key("value"));
    }

    #[test]
    fn wrong_file_kind_is_rejected() {
        let env = json!({"name": "e", "values": [], "_postman_variable_scope": "environment"});
        assert_eq!(import_collection(&env).unwrap_err(), ImportError::UnknownFormat);
        let col = json!({"info": {"schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"}, "item": []});
        assert_eq!(import_environment(&col).unwrap_err(), ImportError::UnknownFormat);
    }
}
