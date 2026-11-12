//! Redirect rules of the engine: which response is a redirect and what the next request looks like.

use reqwest::{Method, Url};

/// Headers that describe the body; dropped together with the body when the method becomes `GET`.
const BODY_HEADERS: [&str; 4] = ["content-type", "content-length", "content-encoding", "transfer-encoding"];
/// Headers that carry credentials; not sent to another origin.
const CREDENTIAL_HEADERS: [&str; 3] = ["authorization", "cookie", "proxy-authorization"];

/// The request that follows a redirect response.
#[derive(Debug)]
pub(crate) struct Next {
    /// `Location` resolved against the URL of the redirected request.
    pub url: Url,
    /// Method of the next request.
    pub method: Method,
    /// The method became `GET`: the body and its headers are not sent any more.
    pub drops_body: bool,
    /// Scheme, host or port differ from the redirected request: credentials are not sent any more.
    pub cross_origin: bool,
}

/// Decides whether a response to `method url` is a redirect to follow.
///
/// Only 301, 302, 303, 307 and 308 with a `Location` that resolves to an `http`/`https` URL count; anything
/// else is an ordinary response (`None`). 303 turns every method except `HEAD` into `GET`; 301 and 302 turn
/// every method except `GET` and `HEAD` into `GET`; 307 and 308 keep the method and the body.
pub(crate) fn next(status: u16, method: &Method, url: &Url, location: Option<&str>) -> Option<Next> {
    if !matches!(status, 301 | 302 | 303 | 307 | 308) {
        return None;
    }
    let target = url.join(location?).ok()?;
    if !matches!(target.scheme(), "http" | "https") || !target.has_host() {
        return None;
    }
    let becomes_get = matches!(status, 301..=303) && *method != Method::GET && *method != Method::HEAD;
    let origin = |url: &Url| (url.scheme().to_string(), url.host_str().map(str::to_string), url.port_or_known_default());
    Some(Next {
        cross_origin: origin(url) != origin(&target),
        method: if becomes_get { Method::GET } else { method.clone() },
        drops_body: becomes_get,
        url: target,
    })
}

/// Removes from the user's headers what must not travel to the next request: the body headers when the
/// body is dropped, the credential headers when the origin changes.
pub(crate) fn strip_headers(headers: &mut Vec<(String, String)>, next: &Next) {
    let listed = |name: &str, list: &[&str]| list.iter().any(|listed| name.eq_ignore_ascii_case(listed));
    headers.retain(|(name, _)| {
        !(next.drops_body && listed(name, &BODY_HEADERS) || next.cross_origin && listed(name, &CREDENTIAL_HEADERS))
    });
}
