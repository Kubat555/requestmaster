//! Tauri commands of the window: one per [`AppCore`] method, nothing but the call.
//!
//! The commands are `async` so that parsing a large collection does not freeze the window; the store lock
//! is taken and released inside the `AppCore` method and is never held across an `.await`.

use tauri::State;

use crate::dto::{EnvSummary, ImportFile, ImportResult, NodeDto, TreeRow};
use crate::AppCore;

/// All nodes of the tree without their content.
#[tauri::command(rename_all = "snake_case")]
pub async fn tree(core: State<'_, AppCore>) -> Result<Vec<TreeRow>, String> {
    core.tree()
}

/// The node with its content.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_node(core: State<'_, AppCore>, id: String) -> Result<NodeDto, String> {
    core.get_node(&id)
}

/// Replaces the content of the node.
#[tauri::command(rename_all = "snake_case")]
pub async fn save_node_content(core: State<'_, AppCore>, id: String, content: serde_json::Value) -> Result<(), String> {
    core.save_node_content(&id, content)
}

/// Imports Postman files; every file gets its own result.
#[tauri::command(rename_all = "snake_case")]
pub async fn import_files(core: State<'_, AppCore>, files: Vec<ImportFile>) -> Result<Vec<ImportResult>, String> {
    Ok(core.import_files(files))
}

/// Merges a globals file with the policy the user chose for the conflicts.
#[tauri::command(rename_all = "snake_case")]
pub async fn import_globals(core: State<'_, AppCore>, text: String, policy: String) -> Result<(), String> {
    core.import_globals(&text, &policy)
}

/// Environments of the switcher.
#[tauri::command(rename_all = "snake_case")]
pub async fn environments(core: State<'_, AppCore>) -> Result<Vec<EnvSummary>, String> {
    core.environments()
}

/// Id of the active environment.
#[tauri::command(rename_all = "snake_case")]
pub async fn active_environment(core: State<'_, AppCore>) -> Result<Option<String>, String> {
    core.active_environment()
}

/// Chooses the active environment; `null` clears the choice.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_active_environment(core: State<'_, AppCore>, id: Option<String>) -> Result<(), String> {
    core.set_active_environment(id.as_deref())
}
