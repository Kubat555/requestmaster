//! Tauri commands of the window: one per [`AppCore`] method, nothing but the call.
//!
//! The commands are `async` so that parsing a large collection or reading a response body does not freeze
//! the window; the store lock is taken and released inside the `AppCore` method and is never held across
//! an `.await`. `cancel_send` alone is synchronous: it only fires a token.

use rqm_model::NodeKind;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::dto::{
    EnvSummary, EnvironmentDto, HistoryEntry, HistoryItem, ImportFile, ImportResult, NodeDto, ScopeVariable,
    SendResult, TreeRow, VariableDto,
};
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

/// Re-indents the JSON text of a request body; text that is not valid JSON is an error.
#[tauri::command(rename_all = "snake_case")]
pub async fn format_json(core: State<'_, AppCore>, text: String) -> Result<String, String> {
    core.format_json(&text)
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

/// Creates a node under `parent_id` (`null` = a new collection) and returns its id.
#[tauri::command(rename_all = "snake_case")]
pub async fn create_node(
    core: State<'_, AppCore>,
    parent_id: Option<String>,
    kind: NodeKind,
    name: String,
) -> Result<String, String> {
    core.create_node(parent_id.as_deref(), kind, &name)
}

/// Renames the node.
#[tauri::command(rename_all = "snake_case")]
pub async fn rename_node(core: State<'_, AppCore>, id: String, name: String) -> Result<(), String> {
    core.rename_node(&id, &name)
}

/// Copies the node with its descendants; returns the id of the copy.
#[tauri::command(rename_all = "snake_case")]
pub async fn duplicate_node(core: State<'_, AppCore>, id: String) -> Result<String, String> {
    core.duplicate_node(&id)
}

/// Deletes the node with its descendants.
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_node(core: State<'_, AppCore>, id: String) -> Result<(), String> {
    core.delete_node(&id)
}

/// Moves the node under `parent_id` (`null` = among the collections) to position `index`, counted without
/// the moved node.
#[tauri::command(rename_all = "snake_case")]
pub async fn move_node(
    core: State<'_, AppCore>,
    id: String,
    parent_id: Option<String>,
    index: usize,
) -> Result<(), String> {
    core.move_node(&id, parent_id.as_deref(), index)
}

/// The variables `{{name}}` resolves at the node, with the active environment.
#[tauri::command(rename_all = "snake_case")]
pub async fn variables_in_scope(core: State<'_, AppCore>, node_id: String) -> Result<Vec<ScopeVariable>, String> {
    core.variables_in_scope(&node_id)
}

/// The environment with its variables; the id `globals` gives the globals set.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_environment(core: State<'_, AppCore>, id: String) -> Result<EnvironmentDto, String> {
    core.get_environment(&id)
}

/// Replaces the name and the variables of the environment; the id `globals` saves the globals set.
#[tauri::command(rename_all = "snake_case")]
pub async fn save_environment(
    core: State<'_, AppCore>,
    id: String,
    name: String,
    variables: Vec<VariableDto>,
) -> Result<(), String> {
    core.save_environment(&id, &name, variables)
}

/// Creates an environment without variables; returns its id.
#[tauri::command(rename_all = "snake_case")]
pub async fn create_environment(core: State<'_, AppCore>, name: String) -> Result<String, String> {
    core.create_environment(&name)
}

/// Deletes the environment.
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_environment(core: State<'_, AppCore>, id: String) -> Result<(), String> {
    core.delete_environment(&id)
}

/// Saves the collection as a Postman file the user chooses; returns its path, or `null` when the user
/// closed the "Save as" window.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_collection(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
    id: String,
) -> Result<Option<String>, String> {
    let (file_name, text) = core.export_collection_text(&id)?;
    save_as(&app, &file_name, &text)
}

/// Saves the environment (the id `globals` = the globals set) as a Postman file the user chooses; returns
/// its path, or `null` when the user closed the "Save as" window.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_environment(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
    id: String,
) -> Result<Option<String>, String> {
    let (file_name, text) = core.export_environment_text(&id)?;
    save_as(&app, &file_name, &text)
}

/// History entries, newest first; `before_id` continues the list below that entry.
#[tauri::command(rename_all = "snake_case")]
pub async fn history_list(
    core: State<'_, AppCore>,
    limit: usize,
    before_id: Option<i64>,
) -> Result<Vec<HistoryItem>, String> {
    core.history_list(limit, before_id)
}

/// One history entry with the request and the response as they were kept.
#[tauri::command(rename_all = "snake_case")]
pub async fn history_get(core: State<'_, AppCore>, id: i64) -> Result<HistoryEntry, String> {
    core.history_get(id)
}

/// Deletes the whole history.
#[tauri::command(rename_all = "snake_case")]
pub async fn history_clear(core: State<'_, AppCore>) -> Result<(), String> {
    core.history_clear()
}

/// The text of a response body for the clipboard, re-indented when `pretty` and the body is JSON.
#[tauri::command(rename_all = "snake_case")]
pub async fn copy_response_body(core: State<'_, AppCore>, body_id: String, pretty: bool) -> Result<String, String> {
    core.copy_response_body(&body_id, pretty)
}

/// Asks the user where to save a file named `file_name` and writes `text` there. `None` when the user
/// closed the window without choosing. The blocking dialog is allowed here: the export commands are `async`
/// and do not run on the main thread.
fn save_as(app: &tauri::AppHandle, file_name: &str, text: &str) -> Result<Option<String>, String> {
    let chosen = app.dialog().file().set_file_name(file_name).add_filter("JSON", &["json"]).blocking_save_file();
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|error| format!("cannot use the chosen place: {error}"))?;
    std::fs::write(&path, text).map_err(|error| format!("cannot write the file: {error}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}
