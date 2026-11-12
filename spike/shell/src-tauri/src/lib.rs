//! Window probe (stage 0 spike): commands for the measuring panel. Throwaway code.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serde_json::{json, Value};
use tauri::Manager;

/// Reference point for `mark_ready`, captured as the first statement of `main`.
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Generated bodies by (MB, minified). Generation happens once per cell, so repeated runs measure
/// only the transfer, not the generation.
static BODIES: OnceLock<Mutex<HashMap<(u32, bool), Arc<String>>>> = OnceLock::new();

const READY_FILE: &str = "requestmaster-spike-ready.txt";
const CYRILLIC_SAMPLE: &str = "Привет, мир — проверка";

fn item(i: usize) -> Value {
    json!({
        "id": i,
        "name": format!("item-{i}"),
        "active": i % 2 == 0,
        "score": i as f64 * 0.37,
        "tags": ["alpha", "beta", "gamma"],
        "address": { "city": "Bishkek", "street": format!("Street {}", i % 500), "zip": 720000 + i % 1000 },
        "note": "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt."
    })
}

fn serialize(items: &[Value], minified: bool) -> String {
    if minified {
        serde_json::to_string(items).unwrap()
    } else {
        serde_json::to_string_pretty(items).unwrap()
    }
}

/// A JSON array of objects whose serialized size is at least `mb` MiB (a few percent over at most).
fn build_body(mb: u32, minified: bool) -> String {
    let target = mb as usize * 1024 * 1024;
    let sample: Vec<Value> = (0..100).map(item).collect();
    let per_item = serialize(&sample, minified).len() / sample.len();
    let mut count = target / per_item + 1;
    loop {
        let items: Vec<Value> = (0..count).map(item).collect();
        let body = serialize(&items, minified);
        if body.len() >= target {
            return body;
        }
        count += (target - body.len()) / per_item + 1;
    }
}

fn body(mb: u32, minified: bool) -> Arc<String> {
    let cache = BODIES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(found) = cache.lock().unwrap().get(&(mb, minified)) {
        return found.clone();
    }
    let built = Arc::new(build_body(mb, minified));
    cache.lock().unwrap().insert((mb, minified), built.clone());
    built
}

/// The body as a string: Tauri JSON-encodes it on the way to the webview.
#[tauri::command]
async fn gen_json(mb: u32, minified: bool) -> String {
    body(mb, minified).as_str().to_owned()
}

/// The same body as raw bytes: `ipc::Response` skips JSON encoding, JS receives an `ArrayBuffer`.
#[tauri::command]
async fn gen_bytes(mb: u32, minified: bool) -> tauri::ipc::Response {
    tauri::ipc::Response::new(body(mb, minified).as_bytes().to_vec())
}

/// Writes and reads back a Cyrillic string through bundled SQLite in the app's local data dir.
/// Returns two lines: the value read back, then the database file path.
#[tauri::command]
async fn db_roundtrip(app: tauri::AppHandle) -> Result<String, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("spike.db");
    let conn = rusqlite::Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch("DROP TABLE IF EXISTS roundtrip; CREATE TABLE roundtrip (value TEXT NOT NULL);")
        .map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO roundtrip (value) VALUES (?1)", [CYRILLIC_SAMPLE])
        .map_err(|e| e.to_string())?;
    let read: String = conn
        .query_row("SELECT value FROM roundtrip", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    conn.execute_batch("DROP TABLE roundtrip;").map_err(|e| e.to_string())?;
    Ok(format!("{read}\n{}", path.display()))
}

/// Writes milliseconds since process start to `%TEMP%\requestmaster-spike-ready.txt`.
#[tauri::command]
fn mark_ready() {
    let elapsed = STARTED.get().map(|s| s.elapsed().as_millis()).unwrap_or(0);
    let _ = std::fs::write(std::env::temp_dir().join(READY_FILE), elapsed.to_string());
}

pub fn run(started: Instant) {
    let _ = STARTED.set(started);
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![gen_json, gen_bytes, db_roundtrip, mark_ready])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_reaches_requested_size_and_is_valid_json() {
        for minified in [true, false] {
            let body = build_body(1, minified);
            assert!(body.len() >= 1024 * 1024);
            assert!(body.len() < 1024 * 1024 * 105 / 100);
            let parsed: Value = serde_json::from_str(&body).unwrap();
            assert!(parsed.as_array().unwrap().len() > 1000);
        }
    }

    #[test]
    fn sqlite_roundtrip_keeps_cyrillic() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE roundtrip (value TEXT NOT NULL);").unwrap();
        conn.execute("INSERT INTO roundtrip (value) VALUES (?1)", [CYRILLIC_SAMPLE]).unwrap();
        let read: String = conn.query_row("SELECT value FROM roundtrip", [], |r| r.get(0)).unwrap();
        assert_eq!(read, CYRILLIC_SAMPLE);
    }
}
