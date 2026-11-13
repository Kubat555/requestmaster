//! Tauri commands of the window: one per [`AppCore`] method, nothing but the call.
//!
//! The commands are `async` so that parsing a large collection or reading a response body does not freeze
//! the window; the store lock is taken and released inside the `AppCore` method and is never held across
//! an `.await`. `cancel_send` alone is synchronous: it only fires a token.

use tauri::State;

use crate::dto::{EnvSummary, ImportFile, ImportResult, NodeDto, SendResult, TreeRow};
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

/// Sends the request, or its unsaved `draft`, and waits for the response.
#[tauri::command(rename_all = "snake_case")]
pub async fn send_request(
    core: State<'_, AppCore>,
    send_id: String,
    request_id: String,
    draft: Option<serde_json::Value>,
) -> Result<SendResult, String> {
    core.send_request(&send_id, &request_id, draft).await
}

/// Stops the send with this id; an unknown id is not an error.
#[tauri::command(rename_all = "snake_case")]
pub fn cancel_send(core: State<'_, AppCore>, send_id: String) {
    core.cancel_send(&send_id);
}

/// Raw bytes of a response body: whole or its beginning, re-indented when `pretty` and the body is JSON.
#[tauri::command(rename_all = "snake_case")]
pub async fn response_body(
    core: State<'_, AppCore>,
    body_id: String,
    pretty: bool,
) -> Result<tauri::ipc::Response, String> {
    core.response_body(&body_id, pretty).map(tauri::ipc::Response::new)
}

/// Deletes the file of a response body; an unknown id is not an error.
#[tauri::command(rename_all = "snake_case")]
pub async fn release_body(core: State<'_, AppCore>, body_id: String) -> Result<(), String> {
    core.release_body(&body_id);
    Ok(())
}
