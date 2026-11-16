//! What `{{name}}` means at a node of [`AppCore`]: the data behind the highlighting and the autocompletion
//! of variables in the window.

use rqm_runner::ScopeSources;

use crate::core::{active_environment_in, message, AppCore};
use crate::dto::ScopeVariable;

impl AppCore {
    /// The variables `{{name}}` resolves at the node `node_id` (a request, a folder or a collection) with
    /// the active environment: one entry per name, sorted by name, then the dynamic names. Values of secret
    /// variables, and values that refer to them, are not returned. An unknown node is an error.
    pub fn variables_in_scope(&self, node_id: &str) -> Result<Vec<ScopeVariable>, String> {
        let store = self.store()?;
        let environment_id = active_environment_in(&store)?;
        let sources = ScopeSources::load(&store, node_id, environment_id.as_deref()).map_err(message)?;
        Ok(rqm_runner::variables_in_scope(&sources).into_iter().map(ScopeVariable::from).collect())
    }
}
