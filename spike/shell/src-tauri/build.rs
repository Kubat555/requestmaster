fn main() {
    // App manifest: Tauri generates `allow-<command>` permissions for these commands, and only the
    // ones listed in `capabilities/default.json` are callable from the window.
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["gen_json", "gen_bytes", "db_roundtrip", "mark_ready"]),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
