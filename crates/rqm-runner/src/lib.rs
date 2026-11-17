//! RequestMaster "Send" scenario without scripts: reads a request with its surroundings from the store,
//! turns it into an [`rqm_http::HttpRequest`] (variables, authorization, body) and sends it.

mod auth;
mod body_view;
mod cookies_state;
mod prepare;
mod scope;
mod send;

use std::time::Duration;

use rqm_http::ProxySetting;
use rqm_model::{Environment, Node, NodeContent, RequestContent};
use rqm_store::{Store, StoreError};

pub use body_view::{body_view, format_json, BodyView, INLINE_BODY_LIMIT, TRUNCATED_PREVIEW};
pub use cookies_state::{load_cookies, save_cookies, COOKIE_JAR_KEY};
pub use prepare::{prepare, PrepareError, Prepared, SendWarning};
pub use scope::{variables_in_scope, ScopeSources, ScopeVariable, VariableOrigin};
pub use send::{send, PreparedSummary, SendError, SendOutcome};

/// Application-wide settings of a send; a request may override `verify_tls` with its own `strict_ssl`.
#[derive(Debug, Clone, PartialEq)]
pub struct AppSettings {
    /// Limit for the whole exchange; `None` — no limit.
    pub timeout: Option<Duration>,
    /// Whether the server's TLS certificate is verified.
    pub verify_tls: bool,
    /// Which proxy to use.
    pub proxy: ProxySetting,
}

impl Default for AppSettings {
    /// No timeout, verify TLS, system proxy.
    fn default() -> Self {
        AppSettings { timeout: None, verify_tls: true, proxy: ProxySetting::System }
    }
}

/// Everything [`prepare`] needs to build the HTTP request; gathered by [`load_send_input`].
#[derive(Debug, Clone)]
pub struct SendInput {
    /// Name of the request node; used to name the node in [`SendWarning::ScriptsNotExecuted`].
    pub request_name: String,
    /// Content to send: the stored one or the unsaved draft from the editor.
    pub request: RequestContent,
    /// From the collection down to the direct parent folder.
    pub ancestors: Vec<Node>,
    /// The environment chosen for the send; `None` — no environment.
    pub environment: Option<Environment>,
    /// The globals set of the store.
    pub globals: Environment,
    /// Application-wide settings at the moment of the send.
    pub settings: AppSettings,
}

/// The environment `id`; an unknown id is `StoreError::NotFound`.
fn environment_by_id(store: &Store, id: &str) -> Result<Environment, StoreError> {
    let found = store.environments()?.into_iter().find(|environment| environment.id == id);
    found.ok_or_else(|| StoreError::NotFound(id.to_string()))
}

/// Reads the request (or uses `draft` instead of the stored content), its ancestors, globals and the environment.
/// A node that is not a request, or an unknown environment id, yields `StoreError::NotFound`.
pub fn load_send_input(
    store: &Store,
    request_id: &str,
    draft: Option<RequestContent>,
    environment_id: Option<&str>,
    settings: AppSettings,
) -> Result<SendInput, StoreError> {
    let node = store.node(request_id)?;
    let NodeContent::Request(stored) = node.content else {
        return Err(StoreError::NotFound(request_id.to_string()));
    };
    let environment = environment_id.map(|id| environment_by_id(store, id)).transpose()?;
    Ok(SendInput {
        request_name: node.name,
        request: draft.unwrap_or(stored),
        ancestors: store.ancestors(request_id)?,
        environment,
        globals: store.globals()?,
        settings,
    })
}
