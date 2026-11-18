//! Building the HTTP request from a request of the model: variables, URL, headers, authorization, body.

use std::path::PathBuf;

use rqm_http::{HttpBody, HttpRequest, MultipartPart, PartValue, SendOptions};
use rqm_model::{Body, FormValue, Param, RawLanguage};
use rqm_vars::{apply_path_variables, Resolver};

use crate::auth::{auth_effect, effective_auth, AuthEffect};
use crate::{ScopeSources, ScriptsOff, SendInput};

/// Methods whose body is not sent unless the request disables body pruning (the list of postman-runtime).
const BODYLESS_METHODS: [&str; 5] = ["GET", "HEAD", "COPY", "PURGE", "UNLOCK"];

/// Something the user should know about a send; never blocks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendWarning {
    /// Scripts exist but did not run; names of the nodes that have one, from the collection down.
    /// Given by [`crate::send`], never by [`prepare`].
    ScriptsNotRun { reason: ScriptsOff, nodes: Vec<String> },
    /// The effective authorization has a type the application does not apply; holds Postman's `auth.type`.
    UnsupportedAuth { auth_type: String },
    /// The body was not sent because of the method; holds the method as typed.
    BodyPruned { method: String },
    /// `{{names}}` that were not found (without braces), deduplicated, in order of first appearance.
    UnresolvedVariables { names: Vec<String> },
}

/// Why the HTTP request could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrepareError {
    /// Files imported from elsewhere and not picked again on this computer; holds the names of the form
    /// fields (`"binary"` for a binary body).
    #[error("files are not confirmed on this computer: {}", .fields.join(", "))]
    UnconfirmedFiles { fields: Vec<String> },
    /// The address is empty after substitution; holds the address as typed.
    #[error("invalid URL: {0:?}")]
    InvalidUrl(String),
}

/// The request to send and the warnings found while building it.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// Ready for [`rqm_http::HttpEngine::send`].
    pub http: HttpRequest,
    /// In the order: authorization, body pruning, unresolved variables.
    pub warnings: Vec<SendWarning>,
}

/// Variable substitution of one `prepare` call; remembers every name that was not found.
struct Vars<'a> {
    resolver: Resolver,
    dynamic: &'a mut dyn FnMut(&str) -> Option<String>,
    /// Deduplicated, in order of first appearance.
    unresolved: Vec<String>,
}

impl Vars<'_> {
    /// `text` with the `{{tokens}}` replaced.
    fn substitute(&mut self, text: &str) -> String {
        let result = self.resolver.substitute_with(text, self.dynamic);
        for name in result.unresolved {
            if !self.unresolved.contains(&name) {
                self.unresolved.push(name);
            }
        }
        result.text
    }

    /// Enabled rows as substituted `(key, value)` pairs.
    fn pairs(&mut self, params: &[Param]) -> Vec<(String, String)> {
        let enabled = params.iter().filter(|param| param.enabled);
        enabled.map(|param| (self.substitute(&param.key), self.substitute(&param.value))).collect()
    }
}

/// Pure: no network, no disk. Uses `dynamic` for `{{$...}}`. The variables are the stored ones: no script runs.
pub fn prepare(
    input: &SendInput,
    dynamic: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<Prepared, PrepareError> {
    prepare_with(input, ScopeSources::of_input(input).resolver(), dynamic)
}

/// [`prepare`] with the resolver given by the caller.
pub(crate) fn prepare_with(
    input: &SendInput,
    resolver: Resolver,
    dynamic: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<Prepared, PrepareError> {
    let request = &input.request;
    let unconfirmed = unconfirmed_file_fields(&request.body);
    if !unconfirmed.is_empty() {
        return Err(PrepareError::UnconfirmedFiles { fields: unconfirmed });
    }

    let mut warnings = Vec::new();
    let mut vars = Vars { resolver, dynamic, unresolved: Vec::new() };

    let substituted_url = vars.substitute(&request.url);
    let mut url = substituted_url.trim().to_string();
    if url.is_empty() {
        return Err(PrepareError::InvalidUrl(request.url.clone()));
    }
    let mut query = vars.pairs(&request.query);
    // Only the values are substituted: the key of a path variable is the `:name` written in the address.
    let path_variables: Vec<(String, String)> = request
        .path_variables
        .iter()
        .filter(|param| param.enabled)
        .map(|param| (param.key.clone(), vars.substitute(&param.value)))
        .collect();
    url = apply_path_variables(&url, &path_variables);

    let mut headers = vars.pairs(&request.headers);
    headers.retain(|(name, _)| !name.is_empty());

    let mut body = build_body(&request.body, &mut vars);

    if let Some(auth) = effective_auth(input) {
        match auth_effect(auth, &mut |text| vars.substitute(text)) {
            AuthEffect::Nothing => {}
            AuthEffect::Header(name, value) => {
                headers.retain(|(existing, _)| !existing.eq_ignore_ascii_case(&name));
                headers.push((name, value));
            }
            AuthEffect::Query(key, value) => {
                query.retain(|(existing, _)| *existing != key);
                query.push((key, value));
            }
            AuthEffect::Unsupported(auth_type) => warnings.push(SendWarning::UnsupportedAuth { auth_type }),
        }
    }

    let prune = BODYLESS_METHODS.iter().any(|method| request.method.eq_ignore_ascii_case(method))
        && !matches!(body, HttpBody::None)
        && request.settings.disable_body_pruning != Some(true);
    if prune {
        body = HttpBody::None;
        warnings.push(SendWarning::BodyPruned { method: request.method.clone() });
    }

    if !vars.unresolved.is_empty() {
        warnings.push(SendWarning::UnresolvedVariables { names: vars.unresolved });
    }

    let options = SendOptions {
        timeout: input.settings.timeout,
        follow_redirects: request.settings.follow_redirects.unwrap_or(true),
        max_redirects: request.settings.max_redirects.unwrap_or(10),
        verify_tls: request.settings.strict_ssl.unwrap_or(input.settings.verify_tls),
        proxy: input.settings.proxy.clone(),
        disabled_auto_headers: request.disabled_auto_headers.clone(),
    };
    let http = HttpRequest { method: request.method.clone(), url: finish_url(&url, &query), headers, body, options };
    Ok(Prepared { http, warnings })
}

/// Names of the enabled form fields (or `"binary"`) that refer to a file not confirmed on this computer.
pub(crate) fn unconfirmed_file_fields(body: &Body) -> Vec<String> {
    match body {
        Body::FormData(parts) => parts
            .iter()
            .filter(|part| part.enabled)
            .filter(|part| matches!(&part.value, FormValue::Files(files) if files.iter().any(|file| !file.confirmed)))
            .map(|part| part.key.clone())
            .collect(),
        Body::Binary { file: Some(file) } if !file.confirmed => vec!["binary".to_string()],
        _ => Vec::new(),
    }
}

/// The body to send, before body pruning. File paths and file contents are not substituted.
fn build_body(body: &Body, vars: &mut Vars) -> HttpBody {
    let bytes = |data: Vec<u8>, content_type: &str| HttpBody::Bytes {
        data,
        default_content_type: Some(content_type.to_string()),
    };
    match body {
        Body::None | Body::Binary { file: None } => HttpBody::None,
        Body::Raw { text, language } => {
            let content_type = match language {
                RawLanguage::Text => "text/plain",
                RawLanguage::Json => "application/json",
                RawLanguage::Xml => "application/xml",
                RawLanguage::Html => "text/html",
                RawLanguage::JavaScript => "application/javascript",
            };
            bytes(vars.substitute(text).into_bytes(), content_type)
        }
        Body::UrlEncoded(params) => {
            let pairs = vars.pairs(params);
            let encoded: Vec<String> =
                pairs.iter().map(|(key, value)| format!("{}={}", form_encode(key), form_encode(value))).collect();
            bytes(encoded.join("&").into_bytes(), "application/x-www-form-urlencoded")
        }
        Body::FormData(parts) => {
            let mut out = Vec::new();
            for part in parts.iter().filter(|part| part.enabled) {
                let name = vars.substitute(&part.key);
                let content_type = part.content_type.clone();
                match &part.value {
                    FormValue::Text(text) => {
                        let value = PartValue::Text(vars.substitute(text));
                        out.push(MultipartPart { name, value, content_type });
                    }
                    // Several files in one field are several parts with the same name.
                    FormValue::Files(files) => out.extend(files.iter().map(|file| MultipartPart {
                        name: name.clone(),
                        value: PartValue::File(PathBuf::from(&file.path)),
                        content_type: content_type.clone(),
                    })),
                }
            }
            HttpBody::Multipart(out)
        }
        Body::Binary { file: Some(file) } => {
            HttpBody::File { path: PathBuf::from(&file.path), default_content_type: None }
        }
        Body::GraphQl { query, variables } => {
            // `Display` of a JSON string value is the quoted and escaped string.
            let mut json = format!("{{\"query\":{}", serde_json::Value::String(vars.substitute(query)));
            let variables = vars.substitute(variables);
            if !variables.trim().is_empty() {
                json.push_str(",\"variables\":");
                // As in Postman, invalid JSON goes into the body as typed (the body is then invalid too).
                json.push_str(&compact_json(&variables).unwrap_or(variables));
            }
            json.push('}');
            bytes(json.into_bytes(), "application/json")
        }
    }
}

/// `text` without the whitespace between JSON tokens, or `None` when it is not valid JSON.
/// Works on the text, so numbers and key order stay exactly as typed.
fn compact_json(text: &str) -> Option<String> {
    serde_json::from_str::<serde::de::IgnoredAny>(text).ok()?;
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in text.chars() {
        if in_string {
            out.push(ch);
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if !matches!(ch, ' ' | '\t' | '\n' | '\r') {
            in_string = ch == '"';
            out.push(ch);
        }
    }
    Some(out)
}

/// `application/x-www-form-urlencoded` form of a key or a value: letters, digits and `*-._` stay, a space
/// becomes `+`, every other byte of the UTF-8 text is percent-encoded.
fn form_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => out.push(char::from(byte)),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Percent-encodes only what cannot stand in a query: space, `"`, `#`, `<`, `>`, `&`, and `=` in a key.
/// Anything else, `%` included, is kept: the user may have typed an already encoded value.
fn query_encode(text: &str, is_key: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '"' => out.push_str("%22"),
            '#' => out.push_str("%23"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '&' => out.push_str("%26"),
            '=' if is_key => out.push_str("%3D"),
            _ => out.push(ch),
        }
    }
    out
}

/// Final address: the `#fragment` is dropped, `http://` is put in front of an address without a scheme,
/// the query parameters are appended.
fn finish_url(url: &str, query: &[(String, String)]) -> String {
    let without_fragment = url.split('#').next().unwrap_or(url);
    // A `://` inside a query typed into the address is not a scheme.
    let before_query = without_fragment.split('?').next().unwrap_or(without_fragment);
    let mut out = String::new();
    if !before_query.contains("://") {
        out.push_str("http://");
    }
    out.push_str(without_fragment);
    let mut separator = if without_fragment.contains('?') { '&' } else { '?' };
    for (key, value) in query {
        out.push(separator);
        out.push_str(&query_encode(key, true));
        out.push('=');
        out.push_str(&query_encode(value, false));
        separator = '&';
    }
    out
}
