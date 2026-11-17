//! Tauri commands of the window: one per [`AppCore`] method, nothing but the call.
//!
//! The commands are `async` so that parsing a large collection or reading a response body does not freeze
//! the window; the store lock is taken and released inside the `AppCore` method and is never held across
//! an `.await`. `cancel_send` alone is synchronous: it only fires a token.

use std::path::{Path, PathBuf};

use rqm_model::NodeKind;
use tauri::State;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::dto::{
    EnvSummary, EnvironmentDto, HistoryEntry, HistoryItem, ImportFile, ImportResult, NodeDto, ScopeVariable,
    SendResult, SessionDto, SettingsDto, TimingSample, TreeRow, VariableDto, WindowState,
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
    window: tauri::WebviewWindow,
    core: State<'_, AppCore>,
    id: String,
) -> Result<Option<String>, String> {
    let (file_name, text) = core.export_collection_text(&id)?;
    save_text_as(&window, &file_name, &text)
}

/// Saves the environment (the id `globals` = the globals set) as a Postman file the user chooses; returns
/// its path, or `null` when the user closed the "Save as" window.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_environment(
    window: tauri::WebviewWindow,
    core: State<'_, AppCore>,
    id: String,
) -> Result<Option<String>, String> {
    let (file_name, text) = core.export_environment_text(&id)?;
    save_text_as(&window, &file_name, &text)
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

/// What the window restores at start: its layout with the open tabs, and the unsaved drafts.
#[tauri::command(rename_all = "snake_case")]
pub async fn load_session(core: State<'_, AppCore>) -> Result<SessionDto, String> {
    core.load_session()
}

/// Remembers the layout of the window and its open tabs.
#[tauri::command(rename_all = "snake_case")]
pub async fn save_window_state(core: State<'_, AppCore>, state: WindowState) -> Result<(), String> {
    core.save_window_state(state)
}

/// Remembers the unsaved draft of the node; `null` forgets it.
#[tauri::command(rename_all = "snake_case")]
pub async fn save_draft(
    core: State<'_, AppCore>,
    node_id: String,
    content: Option<serde_json::Value>,
) -> Result<(), String> {
    core.save_draft(&node_id, content)
}

/// The application settings.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_settings(core: State<'_, AppCore>) -> Result<SettingsDto, String> {
    core.get_settings()
}

/// Saves the application settings; they are used from the next send on.
#[tauri::command(rename_all = "snake_case")]
pub async fn save_settings(core: State<'_, AppCore>, settings: SettingsDto) -> Result<(), String> {
    core.save_settings(settings)
}

/// Saves a response body, as received and decoded, to a file the user chooses; returns its path, or `null`
/// when the user closed the "Save as" window.
#[tauri::command(rename_all = "snake_case")]
pub async fn save_response_body(
    window: tauri::WebviewWindow,
    core: State<'_, AppCore>,
    body_id: String,
) -> Result<Option<String>, String> {
    let file_name = core.response_file_name(&body_id)?;
    let extension = Path::new(&file_name).extension().and_then(|extension| extension.to_str()).unwrap_or_default();
    let Some(path) = ask_save_path(&window, &file_name, extension)? else {
        return Ok(None);
    };
    core.save_response_body(&body_id, &path)?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Asks the user for a file to attach to a request, or for several when `multiple`; returns the chosen
/// paths, none when the user closed the window.
#[tauri::command(rename_all = "snake_case")]
pub async fn pick_files(window: tauri::WebviewWindow, multiple: bool) -> Result<Vec<String>, String> {
    let dialog = window.dialog().file().set_parent(&window);
    let chosen = if multiple {
        dialog.blocking_pick_files().unwrap_or_default()
    } else {
        dialog.blocking_pick_file().into_iter().collect()
    };
    chosen.into_iter().map(|file| Ok(chosen_path(file)?.to_string_lossy().into_owned())).collect()
}

/// Appends the timings the window measured to the measure file; without one does nothing.
#[tauri::command(rename_all = "snake_case")]
pub async fn report_timings(core: State<'_, AppCore>, samples: Vec<TimingSample>) -> Result<(), String> {
    core.report_timings(samples)
}

/// Asks the user where to save a file named `file_name`; the system window belongs to `window` and stays
/// above it. `None` when the user closed it without choosing. The blocking dialog is allowed here: the
/// commands that ask are `async` and do not run on the main thread.
fn ask_save_path(window: &tauri::WebviewWindow, file_name: &str, extension: &str) -> Result<Option<PathBuf>, String> {
    let chosen = window
        .dialog()
        .file()
        .set_parent(window)
        .set_file_name(file_name)
        .add_filter(extension.to_ascii_uppercase(), &[extension])
        .blocking_save_file();
    chosen.map(chosen_path).transpose()
}

/// The path of a file the user chose in a system window.
fn chosen_path(file: FilePath) -> Result<PathBuf, String> {
    file.into_path().map_err(|error| format!("cannot use the chosen place: {error}"))
}

/// Asks the user where to save the JSON file named `file_name` and writes `text` there; returns its path,
/// `None` when the user closed the window without choosing.
fn save_text_as(window: &tauri::WebviewWindow, file_name: &str, text: &str) -> Result<Option<String>, String> {
    let Some(path) = ask_save_path(window, file_name, "json")? else {
        return Ok(None);
    };
    std::fs::write(&path, text).map_err(|error| format!("cannot write the file: {error}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}
