fn main() {
    // App manifest: Tauri generates an `allow-<command>` permission for each of these commands, and only
    // the ones listed in `capabilities/default.json` are callable from the window. The same names are in
    // `COMMANDS` (src/lib.rs); a test compares that list with the capability file.
    let attributes = tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
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
    ]));
    // A build script reports a failure by panicking; the message is tauri-build's own.
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
