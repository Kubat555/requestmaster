// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use requestmaster_lib::window_place::{fits_on_monitors, inner_size, MonitorArea, PlaceTracker, WindowSnapshot};
use requestmaster_lib::{commands, AppCore, CoreConfig};
use tauri::{Manager, PhysicalPosition, PhysicalSize, Window, WindowEvent};

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

/// Label of the only window, as `tauri.conf.json` names it.
const MAIN_WINDOW: &str = "main";

/// The tracker of the place of the main window, kept in the state of the application from the setup on.
struct TrackedPlace(Mutex<PlaceTracker>);

/// What the window says about itself now; `None` when it cannot be asked.
fn snapshot(window: &Window) -> Option<WindowSnapshot> {
    let position = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    Some(WindowSnapshot {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized: window.is_maximized().ok()?,
        minimized: window.is_minimized().ok()?,
    })
}

/// Puts the still hidden window where it was closed, when that place is stored and is still on some monitor;
/// otherwise the window stays where the configuration created it. Nothing here can fail the start: whatever
/// cannot be read or applied is skipped. Returns the tracker that knows the resulting place.
fn restore_place(window: &Window, core: &AppCore) -> PlaceTracker {
    let monitors: Vec<MonitorArea> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|monitor| MonitorArea {
            x: monitor.position().x,
            y: monitor.position().y,
            width: monitor.size().width,
            height: monitor.size().height,
        })
        .collect();
    let stored = core.window_place().filter(|place| fits_on_monitors(place, &monitors));
    if let Some(place) = stored {
        let _ = window.set_position(PhysicalPosition::new(place.x, place.y));
        // The size is set after the move: the frame is measured on the monitor the window is on now, whose
        // scale may differ from the one it was created on.
        if let (Ok(outer), Ok(inner)) = (window.outer_size(), window.inner_size()) {
            let (width, height) =
                inner_size((place.width, place.height), (outer.width, outer.height), (inner.width, inner.height));
            let _ = window.set_size(PhysicalSize::new(width, height));
        }
        if place.maximized {
            let _ = window.maximize();
        }
    }
    let mut tracker = PlaceTracker::new(stored);
    if let Some(now) = snapshot(window) {
        tracker.observe(now);
    }
    tracker
}

/// Follows the main window: its place while it is moved and resized, and the saving of that place when the
/// window is asked to close and when it is destroyed. A failure to save is ignored.
fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW {
        return;
    }
    let Some(tracked) = window.try_state::<TrackedPlace>() else {
        return;
    };
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            if let (Some(now), Ok(mut tracker)) = (snapshot(window), tracked.0.lock()) {
                tracker.observe(now);
            }
        }
        WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
            let place = tracked.0.lock().ok().and_then(|tracker| tracker.place());
            if let Some(place) = place {
                let _ = window.state::<AppCore>().save_window_place(&place);
            }
        }
        _ => {}
    }
}

fn main() {
    let finished = create_core().and_then(|core| {
        tauri::Builder::default()
            // The system windows "Save as" and "Open"; without the plugin the first command that asks for a
            // file panics.
            .plugin(tauri_plugin_dialog::init())
            .manage(core)
            // The window is created hidden (`visible: false`) and shown here, already at its place, so that
            // it does not jump.
            .setup(|app| {
                let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
                    return Ok(());
                };
                // The window itself, without its webview: the type the window events come with.
                let window = window.as_ref().window();
                let tracker = restore_place(&window, &app.state::<AppCore>());
                app.manage(TrackedPlace(Mutex::new(tracker)));
                window.show()?;
                Ok(())
            })
            .on_window_event(on_window_event)
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
