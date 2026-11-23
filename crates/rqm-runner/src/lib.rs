//! RequestMaster "Send" scenario: reads a request with its surroundings from the store, runs the pre-request
//! scripts, turns the request into an [`rqm_http::HttpRequest`] (variables, authorization, body), sends it and
//! runs the test scripts.

mod auth;
mod body_view;
mod cookies_state;
mod prepare;
mod scope;
mod script_host;
mod scripts;
mod send;
mod trust;
mod variable_changes;

use std::time::Duration;

use rqm_http::ProxySetting;
use rqm_model::{Environment, Node, NodeContent, RequestContent};
use rqm_store::{Store, StoreError};

pub use body_view::{body_view, format_json, BodyView, INLINE_BODY_LIMIT, TRUNCATED_PREVIEW};
pub use cookies_state::{load_cookies, save_cookies, COOKIE_JAR_KEY};
pub use prepare::{prepare, PrepareError, Prepared, SendWarning};
pub use rqm_script::unsupported_api;
pub use scope::{variables_in_scope, ScopeSources, ScopeVariable, VariableOrigin};
pub use scripts::{
    script_steps, ConsoleEntry, ConsoleLevel, ScriptErrorKind, ScriptFailure, ScriptPhase, ScriptSource, ScriptStep,
    ScriptsReport, StoredScope, TestOutcome, VariableChange, VariableOp,
};
pub use send::{send, PreparedSummary, SendError, SendOutcome};
pub use trust::{collection_of, forget_deleted_collections, scripts_allowed, set_scripts_allowed, SCRIPTS_ALLOWED_KEY};
pub use variable_changes::{apply_to_variables, apply_variable_changes, AppliedChange};

/// Application-wide settings of a send; a request may override `verify_tls` with its own `strict_ssl`.
#[derive(Debug, Clone, PartialEq)]
pub struct AppSettings {
    /// Limit for the whole exchange; `None` — no limit.
    pub timeout: Option<Duration>,
    /// Whether the server's TLS certificate is verified.
    pub verify_tls: bool,
    /// Which proxy to use.
    pub proxy: ProxySetting,
    /// Whether scripts run at all.
    pub scripts_enabled: bool,
    /// JavaScript execution time of one script.
    pub script_timeout: Duration,
}

impl Default for AppSettings {
    /// No timeout, verify TLS, system proxy; scripts run, each for 10 s at most.
    fn default() -> Self {
        AppSettings {
            timeout: None,
            verify_tls: true,
            proxy: ProxySetting::System,
            scripts_enabled: true,
            script_timeout: Duration::from_secs(10),
        }
    }
}

/// Whether the scripts of a send run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptsMode {
    /// They run.
    Run,
    /// They do not, and why.
    Off(ScriptsOff),
}

/// Why the scripts of a send do not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptsOff {
    /// Switched off in the settings.
    Disabled,
    /// The collection has no consent; holds its name.
    NotAllowed { collection: String },
}

/// Everything [`send`] needs to run the scripts and build the HTTP request; gathered by [`load_send_input`].
#[derive(Debug, Clone)]
pub struct SendInput {
    /// Id of the request node: `pm.info.requestId`, the source of its scripts.
    pub request_id: String,
    /// Name of the request node: `pm.info.requestName`, and the name of the node in what its scripts report.
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
    /// Whether the scripts of this send run.
    pub scripts: ScriptsMode,
}

/// The environment `id`; an unknown id is `StoreError::NotFound`.
fn environment_by_id(store: &Store, id: &str) -> Result<Environment, StoreError> {
    let found = store.environments()?.into_iter().find(|environment| environment.id == id);
    found.ok_or_else(|| StoreError::NotFound(id.to_string()))
}

/// Reads the request (or uses `draft` instead of the stored content), its ancestors, globals and the environment.
/// A node that is not a request, or an unknown environment id, yields `StoreError::NotFound`.
/// The scripts run when the settings switch them on and the collection of the request has the consent
/// ([`scripts_allowed`]).
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
    let ancestors = store.ancestors(request_id)?;
    let scripts = match ancestors.first() {
        _ if !settings.scripts_enabled => ScriptsMode::Off(ScriptsOff::Disabled),
        Some(collection) if scripts_allowed(store, &collection.id)? => ScriptsMode::Run,
        Some(collection) => ScriptsMode::Off(ScriptsOff::NotAllowed { collection: collection.name.clone() }),
        // The tree rules give every request a collection; without one nobody gave a consent.
        None => ScriptsMode::Off(ScriptsOff::NotAllowed { collection: String::new() }),
    };
    Ok(SendInput {
        request_id: request_id.to_string(),
        request_name: node.name,
        request: draft.unwrap_or(stored),
        ancestors,
        environment,
        globals: store.globals()?,
        settings,
        scripts,
    })
}
