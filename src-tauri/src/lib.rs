//! RequestMaster window shell: a thin Tauri layer over the `rqm-*` core.
//!
//! All logic lives in [`AppCore`], plain Rust without Tauri types; the commands in [`commands`] only call it.

pub mod commands;
pub mod core;
pub mod dto;
mod export;
mod history;
mod send;

pub use crate::core::{AppCore, CoreConfig, ACTIVE_ENVIRONMENT_KEY};

/// Names of all Tauri commands of the window. The same names are passed to `tauri_build::AppManifest::commands`
/// in `build.rs`, and `capabilities/default.json` allows each of them as `allow-<name-with-hyphens>`: a command
/// missing there silently does nothing in the window, so a test compares the lists.
pub const COMMANDS: &[&str] = &[
    "tree",
    "get_node",
    "save_node_content",
    "import_files",
    "import_globals",
    "environments",
    "active_environment",
    "set_active_environment",
    "send_request",
    "cancel_send",
    "response_body",
    "release_body",
    "format_json",
    "create_node",
    "rename_node",
    "duplicate_node",
    "delete_node",
    "get_environment",
    "save_environment",
    "create_environment",
    "delete_environment",
    "export_collection",
    "export_environment",
    "history_list",
    "history_get",
    "history_clear",
    "copy_response_body",
];
