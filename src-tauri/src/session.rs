//! What [`AppCore`] remembers between two runs of the window: its layout with the open tabs, and the unsaved
//! drafts. Whatever is stored, a start must not fail on it: a value that cannot be used gives the defaults.

use std::collections::HashSet;

use rqm_model::GLOBALS_ID;
use rqm_store::StoreError;

use crate::core::{message, AppCore};
use crate::dto::{DraftDto, SavedTab, SavedTabKind, SessionDto, WindowState};

/// Key of `app_state` holding the layout of the window and its open tabs: the JSON of [`WindowState`].
/// Written by `save_window_state`, read by `load_session`.
pub const WINDOW_STATE_KEY: &str = "window_state";

/// Prefix of the `app_state` keys holding the unsaved drafts, one key `draft:<node id>` per node: the content
/// of the node as the editor of the window holds it, a JSON object. Written and deleted by `save_draft`;
/// `load_session` reads them and deletes the ones nobody can use.
pub const DRAFT_KEY_PREFIX: &str = "draft:";

impl AppCore {
    /// The layout of the window with its open tabs, and the unsaved drafts.
    ///
    /// A missing or unreadable layout gives the defaults. Tabs of nodes and environments that no longer
    /// exist are left out, and so is the active tab when it was one of them. A draft of a node that no
    /// longer exists, or one that is not a JSON object, is deleted from the store. `measure` tells whether
    /// the core was started with a file for the timings. The only error is a failure of the store itself.
    pub fn load_session(&self) -> Result<SessionDto, String> {
        let mut store = self.store()?;
        let nodes: HashSet<String> = store.tree().map_err(message)?.into_iter().map(|row| row.id).collect();
        let environments: HashSet<String> =
            store.environments().map_err(message)?.into_iter().map(|environment| environment.id).collect();

        let mut window = store
            .state_get(WINDOW_STATE_KEY)
            .map_err(message)?
            .and_then(|value| serde_json::from_str::<WindowState>(&value).ok())
            .unwrap_or_else(default_window);
        let (tabs, gone): (Vec<SavedTab>, Vec<SavedTab>) = window.tabs.into_iter().partition(|tab| match tab.kind {
            SavedTabKind::Node => nodes.contains(&tab.id),
            SavedTabKind::Environment => tab.id == GLOBALS_ID || environments.contains(&tab.id),
        });
        window.tabs = tabs;
        if window.active.as_ref().is_some_and(|active| gone.iter().any(|tab| tab_key(tab) == *active)) {
            window.active = None;
        }

        let mut drafts = Vec::new();
        for (key, value) in store.state_entries(DRAFT_KEY_PREFIX).map_err(message)? {
            let node_id = &key[DRAFT_KEY_PREFIX.len()..];
            match serde_json::from_str::<serde_json::Value>(&value) {
                Ok(content) if content.is_object() && nodes.contains(node_id) => {
                    drafts.push(DraftDto { node_id: node_id.to_string(), content });
                }
                _ => store.state_delete(&key).map_err(message)?,
            }
        }
        Ok(SessionDto { window, drafts, measure: self.measure_file.is_some() })
    }

    /// Remembers the layout of the window and its open tabs, as given.
    pub fn save_window_state(&self, state: WindowState) -> Result<(), String> {
        let value = serde_json::to_string(&state).map_err(message)?;
        self.store()?.state_set(WINDOW_STATE_KEY, &value).map_err(message)
    }

    /// Remembers the unsaved draft of the node `node_id`; `None` forgets it. The fields of the draft are not
    /// checked. A draft of a node that does not exist is not written, and that is not an error.
    pub fn save_draft(&self, node_id: &str, content: Option<serde_json::Value>) -> Result<(), String> {
        let key = format!("{DRAFT_KEY_PREFIX}{node_id}");
        let mut store = self.store()?;
        let Some(content) = content else {
            return store.state_delete(&key).map_err(message);
        };
        match store.node(node_id) {
            Ok(_) => store.state_set(&key, &content.to_string()).map_err(message),
            Err(StoreError::NotFound(_)) => Ok(()),
            Err(error) => Err(message(error)),
        }
    }
}

/// The window before anything was remembered: the side panel 280 px wide, the response placed by the
/// width of the window, the split in the middle, no tabs.
fn default_window() -> WindowState {
    WindowState { sidebar_width: 280.0, placement: "auto".to_string(), split: 50.0, tabs: Vec::new(), active: None }
}

/// The key of a tab as [`WindowState::active`] names it: the id of the node, or `env:<id>` for an environment.
fn tab_key(tab: &SavedTab) -> String {
    match tab.kind {
        SavedTabKind::Node => tab.id.clone(),
        SavedTabKind::Environment => format!("env:{}", tab.id),
    }
}
