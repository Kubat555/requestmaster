// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use requestmaster_lib::{commands, AppCore, CoreConfig};

/// The core over the application's database and the folder of response bodies in the temporary directory.
fn create_core() -> Result<AppCore, String> {
    let db_path = rqm_store::Store::default_path().map_err(|e| e.to_string())?;
    let bodies_dir = std::env::temp_dir().join("RequestMaster").join("bodies");
    AppCore::new(CoreConfig { db_path: Some(db_path), bodies_dir })
}

fn main() {
    let finished = create_core().and_then(|core| {
        tauri::Builder::default()
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
