// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use requestmaster_lib::{commands, AppCore, CoreConfig};

/// Environment variable that turns the measuring on: the folder that holds everything of such a run. The
/// only environment variable the program itself reads.
const MEASURE_DIR_VAR: &str = "RQM_MEASURE_DIR";

/// The core over the application's database and the folder of response bodies in the temporary directory.
///
/// With `RQM_MEASURE_DIR=<folder>` everything lives in that folder instead: the database `app.db`, the
/// response bodies in `bodies` and the timings in `timings.tsv`. The working database is not opened and the
/// shared folder of response bodies is not emptied.
fn create_core() -> Result<AppCore, String> {
    let config = match std::env::var_os(MEASURE_DIR_VAR).filter(|dir| !dir.is_empty()).map(PathBuf::from) {
        Some(dir) => CoreConfig {
            db_path: Some(dir.join("app.db")),
            bodies_dir: dir.join("bodies"),
            history_keep: None,
            measure_file: Some(dir.join("timings.tsv")),
        },
        None => CoreConfig {
            db_path: Some(rqm_store::Store::default_path().map_err(|e| e.to_string())?),
            bodies_dir: std::env::temp_dir().join("RequestMaster").join("bodies"),
            history_keep: None,
            measure_file: None,
        },
    };
    AppCore::new(config)
}

fn main() {
    let finished = create_core().and_then(|core| {
        tauri::Builder::default()
            // The system windows "Save as" and "Open"; without the plugin the first command that asks for a
            // file panics.
            .plugin(tauri_plugin_dialog::init())
            .manage(core)
            .invoke_handler(tauri::generate_handler![
                commands::tree,
                commands::get_node,
                commands::save_node_content,
                commands::import_files,
                commands::import_globals,
                commands::environments,
                commands::active_environment,
                commands::set_active_environment,
                commands::send_request,
                commands::cancel_send,
                commands::response_body,
                commands::release_body,
                commands::format_json,
                commands::create_node,
                commands::rename_node,
                commands::duplicate_node,
                commands::delete_node,
                commands::get_environment,
                commands::save_environment,
                commands::create_environment,
                commands::delete_environment,
                commands::export_collection,
                commands::export_environment,
                commands::history_list,
                commands::history_get,
                commands::history_clear,
                commands::copy_response_body,
                commands::move_node,
                commands::variables_in_scope,
                commands::load_session,
                commands::save_window_state,
                commands::save_draft,
                commands::get_settings,
                commands::save_settings,
                commands::save_response_body,
                commands::pick_files,
                commands::report_timings,
            ])
            .run(tauri::generate_context!())
            .map_err(|e| e.to_string())
    });
    // No message window: the reason goes to stderr and the process ends with code 1.
    if let Err(error) = finished {
        eprintln!("RequestMaster cannot start: {error}");
        std::process::exit(1);
    }
}
