//! Authorization of a send: which `Auth` applies and what it adds to the request.

use base64::Engine as _;
use rqm_model::{ApiKeyPlacement, Auth, NodeContent};

use crate::SendInput;

/// What the effective authorization adds to the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuthEffect {
    /// Nothing is added.
    Nothing,
    /// A header `(name, value)`; replaces the request's own header with the same name.
    Header(String, String),
    /// A query parameter `(key, value)`; replaces the request's own parameter with the same key.
    Query(String, String),
    /// The type is not applied by the application; holds Postman's `auth.type`.
    Unsupported(String),
}

/// The request's own authorization, otherwise the nearest ancestor's (from the direct parent up to the
/// collection). `None` when no level sets one.
pub(crate) fn effective_auth(input: &SendInput) -> Option<&Auth> {
    input.request.auth.as_ref().or_else(|| {
        input.ancestors.iter().rev().find_map(|node| match &node.content {
            NodeContent::Collection(content) => content.auth.as_ref(),
            NodeContent::Folder(content) => content.auth.as_ref(),
            NodeContent::Request(_) => None,
        })
    })
}

/// Turns the authorization into a header or a query parameter. `substitute` resolves `{{variables}}` in
/// every field. Empty values are handled as in Postman: an empty token adds nothing, Basic is always added,
/// an API key is skipped only when its key is empty.
pub(crate) fn auth_effect(auth: &Auth, substitute: &mut dyn FnMut(&str) -> String) -> AuthEffect {
    const AUTHORIZATION: &str = "Authorization";
    match auth {
        Auth::NoAuth => AuthEffect::Nothing,
        Auth::Basic { username, password } => {
            let pair = format!("{}:{}", substitute(username), substitute(password));
            let encoded = base64::engine::general_purpose::STANDARD.encode(pair);
            AuthEffect::Header(AUTHORIZATION.to_string(), format!("Basic {encoded}"))
        }
        Auth::Bearer { token } => match substitute(token) {
            token if token.is_empty() => AuthEffect::Nothing,
            token => AuthEffect::Header(AUTHORIZATION.to_string(), format!("Bearer {token}")),
        },
        Auth::OAuth2Token { token, header_prefix, placement } => {
            let token = substitute(token);
            let prefix = substitute(header_prefix);
            match placement {
                _ if token.is_empty() => AuthEffect::Nothing,
                ApiKeyPlacement::Query => AuthEffect::Query("access_token".to_string(), token),
                ApiKeyPlacement::Header if prefix.is_empty() => AuthEffect::Header(AUTHORIZATION.to_string(), token),
                ApiKeyPlacement::Header => AuthEffect::Header(AUTHORIZATION.to_string(), format!("{prefix} {token}")),
            }
        }
        Auth::ApiKey { key, value, placement } => {
            let key = substitute(key);
            let value = substitute(value);
            match placement {
                _ if key.is_empty() => AuthEffect::Nothing,
                ApiKeyPlacement::Header => AuthEffect::Header(key, value),
                ApiKeyPlacement::Query => AuthEffect::Query(key, value),
            }
        }
        Auth::Unsupported { auth_type } => AuthEffect::Unsupported(auth_type.clone()),
    }
}
