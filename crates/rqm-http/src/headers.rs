//! Automatic request headers: the list shown to the user and the list actually sent.

use crate::types::{HttpBody, HttpRequest};

/// Encodings the engine can decode, advertised in `Accept-Encoding`.
const ACCEPT_ENCODING: &str = "gzip, deflate, br, zstd";

/// Automatic headers for display, in this order: Host, User-Agent, Accept, Accept-Encoding, Content-Type,
/// Content-Length. Skips a header the request sets itself (case-insensitive) or lists in
/// `disabled_auto_headers` (`host`, `accept` and `content-length` cannot be disabled and are ignored there).
/// `Content-Type` and `Content-Length` are listed only when the request has a body; for `Multipart` the value is
/// `multipart/form-data` without the boundary and `Content-Length` is not listed — the exact values are known
/// only after sending (`HttpResponse.sent.headers`).
///
/// `Host` has an empty value while the URL cannot be parsed; `Content-Length` of a file body is not listed
/// while the file cannot be read.
pub fn auto_headers(request: &HttpRequest, user_agent: &str) -> Vec<(String, String)> {
    let host = reqwest::Url::parse(&request.url).map(|url| host_header(&url)).unwrap_or_default();
    let (content_type, content_length) = match &request.body {
        HttpBody::None => (None, None),
        HttpBody::Bytes { data, default_content_type } => (default_content_type.clone(), Some(data.len() as u64)),
        HttpBody::Multipart(_) => (Some("multipart/form-data".to_string()), None),
        HttpBody::File { path, default_content_type } => {
            (default_content_type.clone(), std::fs::metadata(path).ok().map(|meta| meta.len()))
        }
    };
    compose(&request.headers, &request.options.disabled_auto_headers, user_agent, host, content_type, content_length)
}

/// Value of the `Host` header for a URL: the host, plus the port when it is not the scheme's default.
pub(crate) fn host_header(url: &reqwest::Url) -> String {
    let host = url.host_str().unwrap_or_default();
    match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    }
}

/// The automatic headers that apply: not overridden by `user_headers` and not switched off in `disabled`.
/// `content_type` and `content_length` are `None` when there is nothing to send for them.
pub(crate) fn compose(
    user_headers: &[(String, String)],
    disabled: &[String],
    user_agent: &str,
    host: String,
    content_type: Option<String>,
    content_length: Option<u64>,
) -> Vec<(String, String)> {
    // (name, value, whether the user may switch it off)
    let candidates = [
        ("Host", Some(host), false),
        ("User-Agent", Some(user_agent.to_string()), true),
        ("Accept", Some("*/*".to_string()), false),
        ("Accept-Encoding", Some(ACCEPT_ENCODING.to_string()), true),
        ("Content-Type", content_type, true),
        ("Content-Length", content_length.map(|len| len.to_string()), false),
    ];
    candidates
        .into_iter()
        .filter_map(|(name, value, can_disable)| {
            let overridden = user_headers.iter().any(|(user, _)| user.eq_ignore_ascii_case(name));
            let off = can_disable && disabled.iter().any(|d| d.eq_ignore_ascii_case(name));
            if overridden || off {
                return None;
            }
            value.map(|value| (name.to_string(), value))
        })
        .collect()
}
