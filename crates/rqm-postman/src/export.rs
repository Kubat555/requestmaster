//! Export of the model back to Postman files: a collection (schema v2.1) and environment / globals.
//!
//! Every object starts as a copy of its `postman_extra` (rule P1). A typed field is written anew
//! only when the model differs from what the import reads from the original value (rule P2), so
//! an untouched field keeps its original JSON word for word. List elements are rebuilt from their
//! own `postman_extra` (rule P3), and an edited script replaces only `script.exec` (rule P4).

use std::collections::{HashMap, HashSet};

use rqm_model::{
    new_id, Auth, Body, CollectionContent, EnvKind, Environment, Extra, FolderContent, FormPart, FormValue, Node,
    NodeContent, NodeId, Param, RawLanguage, RequestContent, RequestSettings, Scripts, Variable,
};
use serde_json::{json, Map, Value};

use crate::import::{
    flag, imported_files, parse_auth, parse_body, parse_headers, parse_request_settings,
    parse_scripts, parse_url, parse_variables, str_of, text_of, VariableStyle,
};

type Obj = Map<String, Value>;

/// `info.schema` written for a collection created in the application.
const SCHEMA_V21: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

/// Keys of a Postman `url` object that are generated from the model.
const URL_KEYS: [&str; 8] = ["raw", "protocol", "host", "port", "path", "query", "variable", "hash"];

/// Why a set of nodes cannot be exported as one collection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    /// There is no collection node (a node without a parent holding collection content).
    #[error("there is no collection among the nodes to export")]
    NoCollectionRoot,
    /// The node cannot be reached from the collection through folders: its parent is missing,
    /// is a request, or the node is a second collection. Carries the id of the first such node.
    #[error("node {0} does not belong to the exported collection")]
    OrphanNode(NodeId),
}

/// Exports one collection subtree as a Postman collection of schema v2.1.0.
///
/// `nodes` = one collection subtree in any order; children are ordered by `sort_order`
/// (equal values keep the order of `nodes`).
pub fn export_collection(nodes: &[Node]) -> Result<Value, ExportError> {
    let (root, collection) = nodes
        .iter()
        .find_map(|node| match &node.content {
            NodeContent::Collection(content) if node.parent_id.is_none() => Some((node, content)),
            _ => None,
        })
        .ok_or(ExportError::NoCollectionRoot)?;

    let mut children: HashMap<&str, Vec<&Node>> = HashMap::new();
    for node in nodes {
        if let Some(parent_id) = &node.parent_id {
            children.entry(parent_id.as_str()).or_default().push(node);
        }
    }
    for siblings in children.values_mut() {
        siblings.sort_by_key(|node| node.sort_order);
    }

    let mut tree = Tree { children, exported: HashSet::from([root.id.as_str()]) };
    let items = tree.items(root);
    match nodes.iter().find(|node| !tree.exported.contains(node.id.as_str())) {
        Some(orphan) => Err(ExportError::OrphanNode(orphan.id.clone())),
        None => Ok(collection_object(&root.name, collection, items)),
    }
}

/// Exports an environment or the globals set as a Postman file.
/// `_postman_variable_scope` follows `env.kind`.
pub fn export_environment(env: &Environment) -> Value {
    let mut out = Obj::new();
    out.insert("name".to_string(), Value::from(env.name.as_str()));
    copy_extra(&mut out, &env.postman_extra, &["name"]);
    // An environment created in the application has no `values` yet, so the key is always written for it.
    if env.postman_extra.is_empty() || parse_variables(out.get("values"), VariableStyle::Environment) != env.variables {
        let values = env.variables.iter().map(|v| variable_value(v, VariableStyle::Environment)).collect();
        out.insert("values".to_string(), Value::Array(values));
    }
    let scope = match env.kind {
        EnvKind::Environment => "environment",
        EnvKind::Globals => "globals",
    };
    if str_of(&out, "_postman_variable_scope") != Some(scope) {
        out.insert("_postman_variable_scope".to_string(), Value::from(scope));
    }
    Value::Object(out)
}

// ---- tree -------------------------------------------------------------------------------------

/// The nodes grouped by parent, and the ids already written to the output.
struct Tree<'a> {
    children: HashMap<&'a str, Vec<&'a Node>>,
    exported: HashSet<&'a str>,
}

impl<'a> Tree<'a> {
    /// The `item` array of a collection or a folder, depth first.
    /// A collection listed as a child is skipped and is reported later as an orphan.
    fn items(&mut self, parent: &'a Node) -> Value {
        let siblings = self.children.get(parent.id.as_str()).cloned().unwrap_or_default();
        let mut items = Vec::with_capacity(siblings.len());
        for node in siblings {
            let item = match &node.content {
                NodeContent::Collection(_) => continue,
                NodeContent::Folder(folder) => {
                    self.exported.insert(node.id.as_str());
                    let nested = self.items(node);
                    folder_item(&node.name, folder, nested)
                }
                NodeContent::Request(request) => {
                    self.exported.insert(node.id.as_str());
                    request_item(&node.name, request)
                }
            };
            items.push(item);
        }
        Value::Array(items)
    }
}

/// The collection object: `info`, `item`, then everything else from the source.
fn collection_object(name: &str, collection: &CollectionContent, items: Value) -> Value {
    let extra = &collection.postman_extra;
    let mut info = match extra.get("info") {
        Some(Value::Object(info)) => info.clone(),
        // Created in the application: a new Postman id and the v2.1 schema.
        _ => {
            let mut info = Obj::new();
            info.insert("_postman_id".to_string(), Value::from(new_id()));
            info.insert("name".to_string(), Value::from(name));
            info.insert("schema".to_string(), Value::from(SCHEMA_V21));
            info
        }
    };
    if str_of(&info, "name").unwrap_or_default() != name {
        info.insert("name".to_string(), Value::from(name));
    }

    let mut out = Obj::new();
    out.insert("info".to_string(), Value::Object(info));
    out.insert("item".to_string(), items);
    copy_extra(&mut out, extra, &["info", "item"]);
    write_auth(&mut out, &collection.auth);
    write_scripts(&mut out, &collection.scripts);
    if parse_variables(out.get("variable"), VariableStyle::Collection) != collection.variables {
        let variables = collection.variables.iter().map(|v| variable_value(v, VariableStyle::Collection)).collect();
        out.insert("variable".to_string(), Value::Array(variables));
    }
    Value::Object(out)
}

/// A folder element: `name`, `item`, then everything else from the source.
fn folder_item(name: &str, folder: &FolderContent, items: Value) -> Value {
    let mut out = Obj::new();
    out.insert("name".to_string(), Value::from(name));
    out.insert("item".to_string(), items);
    copy_extra(&mut out, &folder.postman_extra, &["name", "item"]);
    write_auth(&mut out, &folder.auth);
    write_scripts(&mut out, &folder.scripts);
    Value::Object(out)
}

/// A request element: `name`, then the source keys in their order with `request` updated in place.
fn request_item(name: &str, request: &RequestContent) -> Value {
    let mut out = Obj::new();
    out.insert("name".to_string(), Value::from(name));
    copy_extra(&mut out, &request.postman_extra, &["name"]);

    let written = match out.get("request") {
        Some(Value::Object(source)) => {
            let mut fields = source.clone();
            if str_of(&fields, "method").unwrap_or("GET") != request.method {
                fields.insert("method".to_string(), Value::from(request.method.as_str()));
            }
            if parse_headers(fields.get("header")) != request.headers {
                fields.insert("header".to_string(), params_value(&request.headers));
            }
            write_body(&mut fields, &request.body);
            write_url(&mut fields, request);
            write_auth(&mut fields, &request.auth);
            Some(fields)
        }
        // `request` given as a bare URL string (or another non-object value) stays as it is
        // while the model still says what the import read from it.
        Some(source) if bare_request_unchanged(source, request) => None,
        // Created in the application, or a non-object `request` that was edited: a full object.
        _ => {
            let mut fields = Obj::new();
            write_auth(&mut fields, &request.auth);
            fields.insert("method".to_string(), Value::from(request.method.as_str()));
            fields.insert("header".to_string(), params_value(&request.headers));
            write_body(&mut fields, &request.body);
            fields.insert("url".to_string(), url_value(None, request));
            Some(fields)
        }
    };
    if let Some(fields) = written {
        out.insert("request".to_string(), Value::Object(fields));
    }
    write_scripts(&mut out, &request.scripts);
    write_settings(&mut out, &request.settings, &request.disabled_auto_headers);
    Value::Object(out)
}

/// `true` when the model equals what the import reads from a `request` that is not an object:
/// `GET` on the URL of a string, nothing else set.
fn bare_request_unchanged(source: &Value, request: &RequestContent) -> bool {
    let url = parse_url(source.as_str().map(|_| source));
    request.method == "GET"
        && url.url == request.url
        && url.query == request.query
        && url.path_variables == request.path_variables
        && request.headers.is_empty()
        && request.body == Body::None
        && request.auth.is_none()
}

/// Copies the source keys in their order, except the ones the export writes itself.
fn copy_extra(out: &mut Obj, extra: &Extra, skip: &[&str]) {
    for (key, value) in extra {
        if !skip.contains(&key.as_str()) {
            out.insert(key.clone(), value.clone());
        }
    }
}

// ---- list elements (rule P3) ------------------------------------------------------------------

/// Writes `key` when the element is new or the name differs from the source.
fn set_key(row: &mut Obj, key: &str, fresh: bool) {
    if fresh || str_of(row, "key").unwrap_or_default() != key {
        row.insert("key".to_string(), Value::from(key));
    }
}

/// Writes `value` when the element is new or the text form of the source value differs,
/// so numbers, booleans and `null` of an untouched element survive.
fn set_value(row: &mut Obj, value: &str, fresh: bool) {
    if fresh || text_of(row.get("value")) != value {
        row.insert("value".to_string(), Value::from(value));
    }
}

/// Brings `disabled` in line with the model: `disabled: true` for a switched-off row,
/// no key for a row that was switched back on.
fn set_disabled(row: &mut Obj, enabled: bool) {
    if flag(row, "disabled") == enabled {
        if enabled {
            row.shift_remove("disabled");
        } else {
            row.insert("disabled".to_string(), Value::Bool(true));
        }
    }
}

/// A query parameter, path variable, header or url-encoded field.
fn param_value(param: &Param) -> Value {
    let fresh = param.postman_extra.is_empty();
    let mut row = param.postman_extra.clone();
    set_key(&mut row, &param.key, fresh);
    set_value(&mut row, &param.value, fresh);
    set_disabled(&mut row, param.enabled);
    Value::Object(row)
}

/// An array of [`param_value`] rows.
fn params_value(params: &[Param]) -> Value {
    Value::Array(params.iter().map(param_value).collect())
}

/// A variable of a collection (`disabled`) or of an environment / globals (`enabled`).
fn variable_value(variable: &Variable, style: VariableStyle) -> Value {
    let fresh = variable.postman_extra.is_empty();
    let mut row = variable.postman_extra.clone();
    set_key(&mut row, &variable.key, fresh);
    set_value(&mut row, &variable.value, fresh);

    let was_secret = str_of(&row, "type") == Some("secret");
    let new_type = match (variable.secret, was_secret, fresh) {
        (true, false, _) => Some("secret"),
        (false, true, _) => Some("default"),
        (false, false, true) => Some(match style {
            VariableStyle::Environment => "default",
            VariableStyle::Collection => "string",
        }),
        // Unchanged: the source `type` stays, and a missing one stays missing.
        _ => None,
    };
    if let Some(new_type) = new_type {
        row.insert("type".to_string(), Value::from(new_type));
    }

    match style {
        VariableStyle::Collection => set_disabled(&mut row, variable.enabled),
        VariableStyle::Environment => {
            let was_enabled = row.get("enabled").and_then(Value::as_bool) != Some(false);
            if fresh || was_enabled != variable.enabled {
                row.insert("enabled".to_string(), Value::Bool(variable.enabled));
            }
        }
    }
    Value::Object(row)
}

/// One `formdata` part. `FileRef::confirmed` is not part of the Postman format and is not written.
fn form_part_value(part: &FormPart) -> Value {
    let fresh = part.postman_extra.is_empty();
    let mut row = part.postman_extra.clone();
    set_key(&mut row, &part.key, fresh);
    let was_file = str_of(&row, "type") == Some("file");
    match &part.value {
        FormValue::Text(text) => {
            set_value(&mut row, text, fresh || was_file);
            if fresh || was_file {
                row.shift_remove("src");
                row.insert("type".to_string(), Value::from("text"));
            }
        }
        FormValue::Files(files) => {
            if !was_file {
                row.shift_remove("value");
                row.insert("type".to_string(), Value::from("file"));
            }
            if !was_file || imported_files(row.get("src")).iter().map(|f| &f.path).ne(files.iter().map(|f| &f.path)) {
                let src = match files.as_slice() {
                    [] => Value::Null,
                    [file] => Value::from(file.path.as_str()),
                    many => many.iter().map(|f| Value::from(f.path.as_str())).collect(),
                };
                row.insert("src".to_string(), src);
            }
        }
    }
    if str_of(&row, "contentType") != part.content_type.as_deref() {
        match &part.content_type {
            Some(content_type) => row.insert("contentType".to_string(), Value::from(content_type.as_str())),
            None => row.shift_remove("contentType"),
        };
    }
    set_disabled(&mut row, part.enabled);
    Value::Object(row)
}

// ---- typed fields (rule P2) -------------------------------------------------------------------

/// Writes `url` when the address, the query or the path variables differ from the source.
fn write_url(fields: &mut Obj, request: &RequestContent) {
    let source = parse_url(fields.get("url"));
    if source.url != request.url || source.query != request.query || source.path_variables != request.path_variables {
        let url = url_value(fields.get("url"), request);
        fields.insert("url".to_string(), url);
    }
}

/// The `url` object `{raw, protocol, host, port, path, query, variable, hash}`.
///
/// Postman libraries do not read `raw`, they assemble the address from the parts, so the parts
/// are always written. The address is split the way `Url.parse` of postman-collection does it:
/// `hash` after `#`, `protocol` before `://`, the host up to the first `/` (digits after its last
/// `:` are the port, the rest is cut by `.`), the remainder cut by `/` into `path`.
/// Unknown keys of the source `url` object are kept.
fn url_value(source: Option<&Value>, request: &RequestContent) -> Value {
    let (address, hash) = match request.url.split_once('#') {
        Some((address, hash)) => (address, Some(hash)),
        None => (request.url.as_str(), None),
    };
    let mut raw = address.to_string();
    let enabled: Vec<String> =
        request.query.iter().filter(|p| p.enabled).map(|p| format!("{}={}", p.key, p.value)).collect();
    if !enabled.is_empty() {
        raw.push('?');
        raw.push_str(&enabled.join("&"));
    }
    if let Some(hash) = hash {
        raw.push('#');
        raw.push_str(hash);
    }

    // `://` after a `/` belongs to the path, not to the scheme.
    let (protocol, rest) = match address.split_once("://") {
        Some((protocol, rest)) if !protocol.is_empty() && !protocol.contains('/') => (Some(protocol), rest),
        _ => (None, address),
    };
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),
        None => (rest, None),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => (host, Some(port)),
        _ => (authority, None),
    };
    let cut = |text: &str, separator: char| -> Value { text.split(separator).map(Value::from).collect() };

    let mut url = Obj::new();
    url.insert("raw".to_string(), Value::from(raw));
    if let Some(protocol) = protocol {
        url.insert("protocol".to_string(), Value::from(protocol));
    }
    url.insert("host".to_string(), if host.is_empty() { json!([]) } else { cut(host, '.') });
    if let Some(port) = port {
        url.insert("port".to_string(), Value::from(port));
    }
    if let Some(path) = path {
        url.insert("path".to_string(), cut(path, '/'));
    }
    if !request.query.is_empty() {
        url.insert("query".to_string(), params_value(&request.query));
    }
    if !request.path_variables.is_empty() {
        url.insert("variable".to_string(), params_value(&request.path_variables));
    }
    if let Some(hash) = hash.filter(|hash| !hash.is_empty()) {
        url.insert("hash".to_string(), Value::from(hash));
    }
    for (key, value) in source.and_then(Value::as_object).into_iter().flatten() {
        if !URL_KEYS.contains(&key.as_str()) {
            url.insert(key.clone(), value.clone());
        }
    }
    Value::Object(url)
}

/// Writes `body` when it differs from the source. `Body::None` removes the key.
/// The source object is the base while the mode stays the same, so its unknown keys survive;
/// after a change of mode the body is built from scratch.
fn write_body(fields: &mut Obj, body: &Body) {
    if parse_body(fields.get("body"), &mut Vec::new()) == *body {
        return;
    }
    let mode = match body {
        Body::None => {
            fields.shift_remove("body");
            return;
        }
        Body::Raw { .. } => "raw",
        Body::UrlEncoded(_) => "urlencoded",
        Body::FormData(_) => "formdata",
        Body::Binary { .. } => "file",
        Body::GraphQl { .. } => "graphql",
    };
    let mut out = match fields.get("body") {
        Some(Value::Object(source)) if str_of(source, "mode") == Some(mode) => source.clone(),
        _ => Obj::new(),
    };
    out.insert("mode".to_string(), Value::from(mode));
    match body {
        Body::None => {}
        Body::Raw { text, language } => {
            if str_of(&out, "raw") != Some(text.as_str()) {
                out.insert("raw".to_string(), Value::from(text.as_str()));
            }
            let source_language = match out.get("options").and_then(|o| o.pointer("/raw/language")).and_then(Value::as_str) {
                Some("json") => RawLanguage::Json,
                Some("xml") => RawLanguage::Xml,
                Some("html") => RawLanguage::Html,
                Some("javascript") => RawLanguage::JavaScript,
                _ => RawLanguage::Text,
            };
            if source_language != *language {
                let name = match language {
                    RawLanguage::Text => "text",
                    RawLanguage::Json => "json",
                    RawLanguage::Xml => "xml",
                    RawLanguage::Html => "html",
                    RawLanguage::JavaScript => "javascript",
                };
                let raw_options = object_at(object_at(&mut out, "options"), "raw");
                raw_options.insert("language".to_string(), Value::from(name));
            }
        }
        Body::UrlEncoded(params) => {
            out.insert("urlencoded".to_string(), params_value(params));
        }
        Body::FormData(parts) => {
            out.insert("formdata".to_string(), Value::Array(parts.iter().map(form_part_value).collect()));
        }
        Body::Binary { file } => {
            let holder = object_at(&mut out, "file");
            let source_path = imported_files(holder.get("src")).pop().map(|f| f.path);
            let path = file.as_ref().map(|f| f.path.as_str());
            if source_path.as_deref() != path || !holder.contains_key("src") {
                holder.insert("src".to_string(), path.map_or(Value::Null, Value::from));
            }
        }
        Body::GraphQl { query, variables } => {
            let holder = object_at(&mut out, "graphql");
            if str_of(holder, "query") != Some(query.as_str()) {
                holder.insert("query".to_string(), Value::from(query.as_str()));
            }
            // Variables given as a JSON value in the source stay a value while their text is the same.
            let source_variables = match holder.get("variables") {
                None => None,
                Some(Value::String(text)) => Some(text.clone()),
                Some(other) => Some(other.to_string()),
            };
            if source_variables.as_deref() != Some(variables.as_str()) {
                holder.insert("variables".to_string(), Value::from(variables.as_str()));
            }
        }
    }
    fields.insert("body".to_string(), Value::Object(out));
}

/// The object under `key`, created (or put in place of a value of another type) when needed.
fn object_at<'a>(holder: &'a mut Obj, key: &str) -> &'a mut Obj {
    let slot = holder.entry(key).or_insert_with(|| Value::Object(Obj::new()));
    if !slot.is_object() {
        *slot = Value::Object(Obj::new());
    }
    // Cannot fail: the slot was made an object just above.
    slot.as_object_mut().expect("the slot holds an object")
}

/// Writes `auth` when it differs from the source.
///
/// Unchanged: the source value stays, so an explicit `null` survives and a missing key stays
/// missing. `None` in the model after a type in the source removes the key.
/// `Auth::Unsupported` is never generated: the source `auth` stays as it is.
fn write_auth(holder: &mut Obj, auth: &Option<Auth>) {
    if parse_auth(holder.get("auth"), &mut Vec::new()) == *auth {
        return;
    }
    match auth {
        None => {
            holder.shift_remove("auth");
        }
        Some(Auth::Unsupported { .. }) => {}
        Some(auth) => {
            let value = auth_value(holder.get("auth"), auth);
            holder.insert("auth".to_string(), value);
        }
    }
}

/// The `auth` object of a supported type. The source object of the same type is the base, so
/// parameters the application does not know (OAuth 2.0 has many) survive an edit of the token;
/// when the result would still be read differently from the model, it is built from scratch.
fn auth_value(source: Option<&Value>, auth: &Auth) -> Value {
    let type_name = match auth {
        Auth::NoAuth => "noauth",
        Auth::Basic { .. } => "basic",
        Auth::Bearer { .. } => "bearer",
        Auth::ApiKey { .. } => "apikey",
        Auth::OAuth2Token { .. } => "oauth2",
        // Not reachable from `write_auth`; the type name keeps the function total.
        Auth::Unsupported { auth_type } => auth_type.as_str(),
    };
    let placement = |placement: &rqm_model::ApiKeyPlacement, query: &str| match placement {
        rqm_model::ApiKeyPlacement::Query => query.to_string(),
        rqm_model::ApiKeyPlacement::Header => "header".to_string(),
    };
    let params: Vec<(&str, String)> = match auth {
        Auth::NoAuth | Auth::Unsupported { .. } => Vec::new(),
        Auth::Basic { username, password } => vec![("username", username.clone()), ("password", password.clone())],
        Auth::Bearer { token } => vec![("token", token.clone())],
        Auth::ApiKey { key, value, placement: at } => {
            vec![("key", key.clone()), ("value", value.clone()), ("in", placement(at, "query"))]
        }
        Auth::OAuth2Token { token, header_prefix, placement: at } => vec![
            ("accessToken", token.clone()),
            ("headerPrefix", header_prefix.clone()),
            ("addTokenTo", placement(at, "queryParams")),
        ],
    };
    let fill = |mut out: Obj| -> Value {
        out.insert("type".to_string(), Value::from(type_name));
        for (key, value) in &params {
            set_auth_param(&mut out, type_name, key, value);
        }
        Value::Object(out)
    };

    let based = source
        .and_then(Value::as_object)
        .filter(|source| str_of(source, "type") == Some(type_name))
        .map(|source| fill(source.clone()))
        .filter(|value| parse_auth(Some(value), &mut Vec::new()).as_ref() == Some(auth));
    based.unwrap_or_else(|| fill(Obj::new()))
}

/// Writes one auth parameter when the import would read something else from the current value.
/// Parameters are kept in the form of the source: an array of `{key, value, type}` rows
/// (also the form of a new object) or an object.
fn set_auth_param(auth: &mut Obj, type_name: &str, key: &str, value: &str) {
    let current = match auth.get(type_name) {
        Some(Value::Array(rows)) => rows
            .iter()
            .filter_map(Value::as_object)
            .find(|row| str_of(row, "key") == Some(key))
            .and_then(|row| row.get("value")),
        Some(Value::Object(fields)) => fields.get(key),
        _ => None,
    };
    // The same reading as in `parse_auth`, including its defaults.
    let read = match key {
        "in" => (if current.and_then(Value::as_str) == Some("query") { "query" } else { "header" }).to_string(),
        "addTokenTo" => {
            (if current.and_then(Value::as_str) == Some("queryParams") { "queryParams" } else { "header" }).to_string()
        }
        "headerPrefix" => match current {
            None | Some(Value::Null) => "Bearer".to_string(),
            prefix => text_of(prefix).trim().to_string(),
        },
        _ => text_of(current),
    };
    if read == value {
        return;
    }
    let slot = auth.entry(type_name).or_insert_with(|| Value::Array(Vec::new()));
    match slot {
        Value::Object(fields) => {
            fields.insert(key.to_string(), Value::from(value));
        }
        Value::Array(rows) => {
            match rows.iter_mut().filter_map(Value::as_object_mut).find(|row| str_of(row, "key") == Some(key)) {
                Some(row) => {
                    row.insert("value".to_string(), Value::from(value));
                }
                None => rows.push(json!({"key": key, "value": value, "type": "string"})),
            }
        }
        other => *other = json!([{"key": key, "value": value, "type": "string"}]),
    }
}

/// Writes the edited scripts into `event` (rule P4): in the source event only `script.exec` is
/// replaced (the script cut by `\n`); a script without a source event gets a new event.
fn write_scripts(holder: &mut Obj, scripts: &Scripts) {
    let source = parse_scripts(holder.get("event"));
    for (listen, old, new) in
        [("prerequest", &source.pre_request, &scripts.pre_request), ("test", &source.test, &scripts.test)]
    {
        if old == new {
            continue;
        }
        let exec: Vec<Value> = if new.is_empty() { Vec::new() } else { new.split('\n').map(Value::from).collect() };
        let slot = holder.entry("event").or_insert_with(|| Value::Array(Vec::new()));
        if !slot.is_array() {
            *slot = Value::Array(Vec::new());
        }
        let Value::Array(events) = slot else { continue };
        // The import reads the first event of a kind, so that is the one to update.
        match events.iter_mut().filter_map(Value::as_object_mut).find(|event| str_of(event, "listen") == Some(listen)) {
            Some(event) => {
                object_at(event, "script").insert("exec".to_string(), Value::Array(exec));
            }
            None => events.push(json!({"listen": listen, "script": {"type": "text/javascript", "exec": exec}})),
        }
    }
}

/// Writes `protocolProfileBehavior` when the settings or the switched-off automatic headers
/// differ from the source: only the changed keys are touched, `disabledSystemHeaders` becomes
/// `{name: true}` (no key for an empty list), and an object left empty is not written.
fn write_settings(item: &mut Obj, settings: &RequestSettings, disabled_auto_headers: &[String]) {
    let (source, source_disabled) = parse_request_settings(item.get("protocolProfileBehavior"), &mut Vec::new());
    if source == *settings && source_disabled == disabled_auto_headers {
        return;
    }
    let mut out = match item.get("protocolProfileBehavior") {
        Some(Value::Object(behavior)) => behavior.clone(),
        _ => Obj::new(),
    };
    set_setting(&mut out, "followRedirects", source.follow_redirects, settings.follow_redirects);
    set_setting(&mut out, "maxRedirects", source.max_redirects, settings.max_redirects);
    set_setting(&mut out, "strictSSL", source.strict_ssl, settings.strict_ssl);
    set_setting(&mut out, "disableBodyPruning", source.disable_body_pruning, settings.disable_body_pruning);
    if source_disabled != disabled_auto_headers {
        if disabled_auto_headers.is_empty() {
            out.shift_remove("disabledSystemHeaders");
        } else {
            let headers: Obj = disabled_auto_headers.iter().map(|name| (name.clone(), Value::Bool(true))).collect();
            out.insert("disabledSystemHeaders".to_string(), Value::Object(headers));
        }
    }
    if out.is_empty() {
        item.shift_remove("protocolProfileBehavior");
    } else {
        item.insert("protocolProfileBehavior".to_string(), Value::Object(out));
    }
}

/// Writes one setting when it changed; `None` removes the key.
fn set_setting<T: PartialEq + Into<Value>>(out: &mut Obj, key: &str, source: Option<T>, new: Option<T>) {
    if source != new {
        match new {
            Some(value) => out.insert(key.to_string(), value.into()),
            None => out.shift_remove(key),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::import_collection;
    use rqm_model::{ApiKeyPlacement, FileRef, NodeKind};

    const SCHEMA: &str = SCHEMA_V21;

    /// Imports a one-request collection, lets the test edit the request and exports the item back.
    fn edited(request: Value, edit: impl FnOnce(&mut RequestContent)) -> Value {
        let source = json!({"info": {"name": "C", "schema": SCHEMA}, "item": [{"name": "R", "request": request}]});
        let mut nodes = import_collection(&source).unwrap().nodes;
        let NodeContent::Request(content) = &mut nodes[1].content else { panic!("not a request") };
        edit(content);
        export_collection(&nodes).unwrap()["item"][0].clone()
    }

    #[test]
    fn nodes_outside_the_collection_are_orphans() {
        let source = json!({"info": {"name": "C", "schema": SCHEMA}, "item": [{"name": "R", "request": "https://example.test"}]});
        let mut nodes = import_collection(&source).unwrap().nodes;
        let request_id = nodes[1].id.clone();
        let mut stray = nodes[1].clone();
        stray.id = new_id();
        // a child of a request
        stray.parent_id = Some(request_id);
        nodes.push(stray.clone());
        assert_eq!(export_collection(&nodes), Err(ExportError::OrphanNode(stray.id.clone())));
        // a parent that is not among the nodes
        nodes[2].parent_id = Some(new_id());
        assert_eq!(export_collection(&nodes), Err(ExportError::OrphanNode(stray.id.clone())));
        // a second collection
        nodes.truncate(2);
        let mut second = nodes[0].clone();
        second.id = new_id();
        nodes.push(second.clone());
        assert_eq!(export_collection(&nodes), Err(ExportError::OrphanNode(second.id)));
    }

    #[test]
    fn empty_folder_keeps_its_item_array() {
        let root = Node {
            id: new_id(),
            parent_id: None,
            kind: NodeKind::Collection,
            name: "C".into(),
            sort_order: 0,
            content: NodeContent::Collection(CollectionContent {
                auth: None,
                variables: vec![Variable { key: "v".into(), value: "1".into(), secret: false, enabled: false, postman_extra: Extra::new() }],
                scripts: Scripts { pre_request: "a\nb".into(), test: String::new() },
                postman_extra: Extra::new(),
            }),
        };
        let folder = Node {
            id: new_id(),
            parent_id: Some(root.id.clone()),
            kind: NodeKind::Folder,
            name: "F".into(),
            sort_order: 0,
            content: NodeContent::Folder(FolderContent { auth: Some(Auth::NoAuth), scripts: Scripts::default(), postman_extra: Extra::new() }),
        };
        let out = export_collection(&[folder, root]).unwrap();
        assert_eq!(out["item"], json!([{"name": "F", "item": [], "auth": {"type": "noauth"}}]));
        assert_eq!(out["variable"], json!([{"key": "v", "value": "1", "type": "string", "disabled": true}]));
        assert_eq!(out["event"], json!([{"listen": "prerequest", "script": {"type": "text/javascript", "exec": ["a", "b"]}}]));
        assert!(out["info"]["_postman_id"].is_string());
        assert_eq!(import_collection(&out).unwrap().nodes[1].kind, NodeKind::Folder);
    }

    #[test]
    fn edited_auth_keeps_unknown_parameters() {
        let auth = json!({"type": "oauth2", "oauth2": [
            {"key": "clientId", "value": "id", "type": "string"},
            {"key": "accessToken", "value": "old", "type": "string"}]});
        let out = edited(json!({"method": "GET", "auth": auth, "url": "https://example.test"}), |r| {
            r.auth = Some(Auth::OAuth2Token { token: "new".into(), header_prefix: "Bearer".into(), placement: ApiKeyPlacement::Header });
        });
        assert_eq!(out["request"]["auth"], json!({"type": "oauth2", "oauth2": [
            {"key": "clientId", "value": "id", "type": "string"},
            {"key": "accessToken", "value": "new", "type": "string"}]}));

        // another type: built from scratch; the object form of the parameters is not kept
        let out = edited(json!({"method": "GET", "auth": {"type": "basic", "basic": {"username": "u", "password": "p"}}, "url": "https://example.test"}), |r| {
            r.auth = Some(Auth::ApiKey { key: "k".into(), value: "v".into(), placement: ApiKeyPlacement::Query });
        });
        assert_eq!(out["request"]["auth"], json!({"type": "apikey", "apikey": [
            {"key": "key", "value": "k", "type": "string"},
            {"key": "value", "value": "v", "type": "string"},
            {"key": "in", "value": "query", "type": "string"}]}));

        // the same type in the object form: edited in place
        let out = edited(json!({"method": "GET", "auth": {"type": "basic", "basic": {"username": "u", "password": "p"}}, "url": "https://example.test"}), |r| {
            r.auth = Some(Auth::Basic { username: "u".into(), password: "changed".into() });
        });
        assert_eq!(out["request"]["auth"], json!({"type": "basic", "basic": {"username": "u", "password": "changed"}}));
    }

    #[test]
    fn auth_set_to_inherit_removes_the_key() {
        let out = edited(json!({"method": "GET", "auth": {"type": "noauth"}, "url": "https://example.test"}), |r| r.auth = None);
        assert!(out["request"].get("auth").is_none());
    }

    #[test]
    fn bare_request_string_is_kept_until_edited() {
        assert_eq!(edited(json!("https://example.test/a?x=1"), |_| {})["request"], json!("https://example.test/a?x=1"));
        let out = edited(json!("https://example.test/a?x=1"), |r| r.method = "POST".into());
        assert_eq!(out["request"], json!({"method": "POST", "header": [], "url": {
            "raw": "https://example.test/a?x=1", "protocol": "https", "host": ["example", "test"], "path": ["a"],
            "query": [{"key": "x", "value": "1"}]}}));
    }

    #[test]
    fn edited_url_keeps_row_details_and_unknown_keys() {
        let url = json!({"raw": "https://example.test/a?x=1", "protocol": "https", "host": ["example", "test"], "path": ["a"],
            "query": [{"key": "x", "value": 1, "description": "d"}], "custom": true});
        let out = edited(json!({"method": "GET", "url": url}), |r| r.url = "example.test/b/".into());
        assert_eq!(out["request"]["url"], json!({"raw": "example.test/b/?x=1", "host": ["example", "test"], "path": ["b", ""],
            "query": [{"key": "x", "value": 1, "description": "d"}], "custom": true}));
    }

    #[test]
    fn edited_bodies() {
        // raw: the language changes, unknown keys of the body stay
        let body = json!({"mode": "raw", "raw": "{}", "options": {"raw": {"language": "json"}}, "custom": 1});
        let out = edited(json!({"method": "POST", "body": body, "url": "https://example.test"}), |r| {
            r.body = Body::Raw { text: "<a/>".into(), language: RawLanguage::Xml };
        });
        assert_eq!(out["request"]["body"], json!({"mode": "raw", "raw": "<a/>", "options": {"raw": {"language": "xml"}}, "custom": 1}));

        // another mode: nothing of the old body is left; `confirmed` is not written
        let out = edited(json!({"method": "POST", "body": body, "url": "https://example.test"}), |r| {
            r.body = Body::FormData(vec![
                FormPart { key: "t".into(), value: FormValue::Text("1".into()), enabled: false, content_type: None, postman_extra: Extra::new() },
                FormPart {
                    key: "f".into(),
                    value: FormValue::Files(vec![FileRef { path: "/synthetic/a".into(), confirmed: true }]),
                    enabled: true,
                    content_type: Some("text/plain".into()),
                    postman_extra: Extra::new(),
                },
            ]);
        });
        assert_eq!(out["request"]["body"], json!({"mode": "formdata", "formdata": [
            {"key": "t", "value": "1", "type": "text", "disabled": true},
            {"key": "f", "type": "file", "src": "/synthetic/a", "contentType": "text/plain"}]}));

        // one edited form part: the other one and the unknown keys of both stay
        let form = json!({"mode": "formdata", "formdata": [
            {"key": "a", "value": 5, "type": "text", "description": "d"},
            {"key": "b", "type": "file", "src": ["/synthetic/1", "/synthetic/2"], "uuid": "u"}]});
        let out = edited(json!({"method": "POST", "body": form, "url": "https://example.test"}), |r| {
            let Body::FormData(parts) = &mut r.body else { panic!("not form-data") };
            parts[1].value = FormValue::Files(vec![]);
        });
        assert_eq!(out["request"]["body"]["formdata"], json!([
            {"key": "a", "value": 5, "type": "text", "description": "d"},
            {"key": "b", "type": "file", "src": null, "uuid": "u"}]));

        // binary and GraphQL from scratch, then no body at all
        let out = edited(json!({"method": "POST", "url": "https://example.test"}), |r| r.body = Body::Binary { file: None });
        assert_eq!(out["request"]["body"], json!({"mode": "file", "file": {"src": null}}));
        let out = edited(json!({"method": "POST", "url": "https://example.test"}), |r| {
            r.body = Body::GraphQl { query: "{ a }".into(), variables: String::new() };
        });
        assert_eq!(out["request"]["body"], json!({"mode": "graphql", "graphql": {"query": "{ a }", "variables": ""}}));
        let out = edited(json!({"method": "POST", "body": body, "url": "https://example.test"}), |r| r.body = Body::None);
        assert!(out["request"].get("body").is_none());
    }

    #[test]
    fn edited_settings_touch_only_changed_keys() {
        let source = json!({"info": {"name": "C", "schema": SCHEMA}, "item": [{"name": "R", "request": "https://example.test",
            "protocolProfileBehavior": {"followRedirects": false, "insecureHTTPParser": true, "disabledSystemHeaders": {"Accept": true}}}]});
        let mut nodes = import_collection(&source).unwrap().nodes;
        let NodeContent::Request(content) = &mut nodes[1].content else { panic!("not a request") };
        content.settings.follow_redirects = None;
        content.settings.max_redirects = Some(3);
        content.disabled_auto_headers.clear();
        let out = export_collection(&nodes).unwrap();
        assert_eq!(out["item"][0]["protocolProfileBehavior"], json!({"insecureHTTPParser": true, "maxRedirects": 3}));

        let NodeContent::Request(content) = &mut nodes[1].content else { panic!("not a request") };
        content.postman_extra.shift_remove("protocolProfileBehavior");
        content.settings = RequestSettings::default();
        assert!(export_collection(&nodes).unwrap()["item"][0].get("protocolProfileBehavior").is_none());
    }

    #[test]
    fn variable_type_and_state_rules() {
        let source = json!({"name": "E", "_postman_variable_scope": "environment", "values": [
            {"key": "a", "value": "1", "type": "secret", "enabled": true},
            {"key": "b", "value": "2", "enabled": true},
            {"key": "c", "value": "3", "type": "any", "enabled": true}]});
        let mut env = crate::import::import_environment(&source).unwrap();
        env.variables[0].secret = false;
        env.variables[1].secret = true;
        env.variables[2].enabled = false;
        env.variables.push(Variable { key: "d".into(), value: "4".into(), secret: false, enabled: true, postman_extra: Extra::new() });
        env.name = "Renamed".into();
        env.kind = EnvKind::Globals;
        assert_eq!(export_environment(&env), json!({"name": "Renamed", "_postman_variable_scope": "globals", "values": [
            {"key": "a", "value": "1", "type": "default", "enabled": true},
            {"key": "b", "value": "2", "enabled": true, "type": "secret"},
            {"key": "c", "value": "3", "type": "any", "enabled": false},
            {"key": "d", "value": "4", "type": "default", "enabled": true}]}));

        let fresh = Environment { id: new_id(), kind: EnvKind::Environment, name: "N".into(), variables: vec![], postman_extra: Extra::new() };
        assert_eq!(export_environment(&fresh), json!({"name": "N", "values": [], "_postman_variable_scope": "environment"}));
    }
}
