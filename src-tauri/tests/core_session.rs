//! `AppCore` without a window: what is remembered between two runs — the layout of the window with its open
//! tabs, the unsaved drafts — and the application settings. The database lives in a file, so that a test can
//! put a broken value there through a second connection; all Postman JSON is synthetic.

use std::path::{Path, PathBuf};
use std::time::Duration;

use requestmaster_lib::dto::{DraftDto, ImportFile, MotionLevel, ProxyDto, SavedTab, SavedTabKind, SettingsDto, WindowState};
use requestmaster_lib::{
    AppCore, CoreConfig, DEFAULT_SCRIPT_TIMEOUT_MS, DRAFT_KEY_PREFIX, MAX_SCRIPT_TIMEOUT_MS, MAX_TIMEOUT_MS,
    MIN_SCRIPT_TIMEOUT_MS, SETTINGS_KEY, WINDOW_STATE_KEY,
};
use rqm_http::ProxySetting;
use rqm_model::NodeKind;
use rqm_runner::AppSettings;
use serde_json::{json, Value};
use tempfile::TempDir;

const SCHEMA_V21: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

fn db_path(dir: &TempDir) -> PathBuf {
    dir.path().join("requestmaster.db")
}

/// A core over the database file of `dir`.
fn open(dir: &TempDir) -> AppCore {
    AppCore::new(CoreConfig { db_path: Some(db_path(dir)), bodies_dir: dir.path().join("bodies"), history_keep: None, measure_file: None })
        .unwrap()
}

/// A core over a new database with the collection `Synthetic` holding the requests `A`…`E`, and the
/// environments `First` and `Second`. The directory must outlive the core.
fn core() -> (AppCore, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    let items: Vec<Value> = ["A", "B", "C", "D", "E"]
        .iter()
        .map(|name| json!({ "name": name, "request": { "method": "GET", "url": { "raw": "http://127.0.0.1/x" } } }))
        .collect();
    import(&core, &json!({ "info": { "name": "Synthetic", "schema": SCHEMA_V21 }, "item": items }));
    for name in ["First", "Second"] {
        import(&core, &json!({ "name": name, "values": [], "_postman_variable_scope": "environment" }));
    }
    (core, dir)
}

fn import(core: &AppCore, json: &Value) {
    let results = core.import_files(vec![ImportFile { name: "file.json".to_string(), text: json.to_string() }]);
    assert_eq!(results[0].error, None);
}

fn node_id(core: &AppCore, name: &str) -> String {
    core.tree().unwrap().into_iter().find(|row| row.name == name).unwrap().id
}

fn environment_id(core: &AppCore, name: &str) -> String {
    core.environments().unwrap().into_iter().find(|environment| environment.name == name).unwrap().id
}

fn as_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

/// Writes a value of `app_state` behind the back of the core.
fn raw_set(db: &Path, key: &str, value: &str) {
    rusqlite::Connection::open(db)
        .unwrap()
        .execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )
        .unwrap();
}

/// Reads a value of `app_state` behind the back of the core; `None` — there is no such key.
fn raw_get(db: &Path, key: &str) -> Option<String> {
    let connection = rusqlite::Connection::open(db).unwrap();
    let mut statement = connection.prepare("SELECT value FROM app_state WHERE key = ?1").unwrap();
    let mut rows = statement.query_map([key], |row| row.get(0)).unwrap();
    rows.next().map(|value| value.unwrap())
}

fn draft_key(node_id: &str) -> String {
    format!("{DRAFT_KEY_PREFIX}{node_id}")
}

fn node_tab(id: &str) -> SavedTab {
    SavedTab { kind: SavedTabKind::Node, id: id.to_string() }
}

fn environment_tab(id: &str) -> SavedTab {
    SavedTab { kind: SavedTabKind::Environment, id: id.to_string() }
}

fn default_window() -> WindowState {
    WindowState { sidebar_width: 280.0, placement: "auto".to_string(), split: 50.0, tabs: Vec::new(), active: None }
}

fn window_with(tabs: Vec<SavedTab>, active: Option<&str>) -> WindowState {
    WindowState { tabs, active: active.map(str::to_string), ..default_window() }
}

fn default_settings_value() -> SettingsDto {
    SettingsDto {
        timeout_ms: 0,
        verify_tls: true,
        proxy: ProxyDto::System,
        motion: MotionLevel::Normal,
        scripts_enabled: true,
        script_timeout_ms: 10_000,
    }
}

#[test]
fn empty_database_gives_the_default_session() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    let session = core.load_session().unwrap();
    assert_eq!(session.window, default_window());
    assert!(session.drafts.is_empty());
    assert_eq!(
        as_json(&session),
        json!({
            "window": { "sidebar_width": 280.0, "placement": "auto", "split": 50.0, "tabs": [], "active": null },
            "drafts": [],
            "measure": false
        })
    );
    assert_eq!(raw_get(&db_path(&dir), WINDOW_STATE_KEY), None, "reading writes nothing");
}

#[test]
fn window_state_round_trips() {
    let (core, dir) = core();
    let id = node_id(&core, "A");
    // the shape of the plan's section about the database, with the id of a real node
    let text = format!(
        r#"{{ "sidebar_width": 280, "placement": "auto", "split": 50,
  "tabs": [ {{ "kind": "node", "id": "{id}" }}, {{ "kind": "environment", "id": "globals" }} ],
  "active": "env:globals" }}"#
    );
    let state: WindowState = serde_json::from_str(&text).unwrap();
    assert_eq!(state, window_with(vec![node_tab(&id), environment_tab("globals")], Some("env:globals")));

    core.save_window_state(state.clone()).unwrap();
    assert_eq!(core.load_session().unwrap().window, state);
    let stored: Value = serde_json::from_str(&raw_get(&db_path(&dir), WINDOW_STATE_KEY).unwrap()).unwrap();
    assert_eq!(stored, as_json(&state));
    assert_eq!(stored["tabs"], json!([{ "kind": "node", "id": id }, { "kind": "environment", "id": "globals" }]));

    // the next save replaces the value
    let other = WindowState { sidebar_width: 412.5, placement: "right".to_string(), split: 33.0, ..default_window() };
    core.save_window_state(other.clone()).unwrap();
    assert_eq!(core.load_session().unwrap().window, other);
}

#[test]
fn broken_window_state_gives_defaults() {
    let (core, dir) = core();
    let id = node_id(&core, "A");
    core.save_draft(&id, Some(json!({ "method": "POST" }))).unwrap();
    for broken in ["{broken", "", "[]", "null", "5", r#"{ "sidebar_width": "wide" }"#, r#"{ "sidebar_width": 300 }"#] {
        raw_set(&db_path(&dir), WINDOW_STATE_KEY, broken);
        let session = core.load_session().unwrap();
        assert_eq!(session.window, default_window(), "value: {broken}");
        assert_eq!(session.drafts.len(), 1, "the drafts do not depend on the window state");
    }
}

#[test]
fn tabs_of_deleted_nodes_and_environments_are_left_out_and_active_is_cleared() {
    let (core, _dir) = core();
    let (a, b) = (node_id(&core, "A"), node_id(&core, "B"));
    let (first, second) = (environment_id(&core, "First"), environment_id(&core, "Second"));
    let tabs = vec![
        node_tab(&a),
        node_tab(&b),
        environment_tab(&first),
        environment_tab(&second),
        environment_tab("globals"),
        node_tab("no-such-node"),
        // an id is looked up by the kind of its tab: a node is not an environment and the other way round
        environment_tab(&a),
        node_tab(&first),
    ];
    let kept = vec![node_tab(&a), environment_tab(&first), environment_tab("globals")];

    core.save_window_state(window_with(tabs.clone(), Some(&b))).unwrap();
    assert_eq!(core.load_session().unwrap().window.tabs.len(), 5, "everything that exists is kept, in order");
    assert_eq!(core.load_session().unwrap().window.active.as_deref(), Some(b.as_str()));

    core.delete_node(&b).unwrap();
    core.delete_environment(&second).unwrap();
    let window = core.load_session().unwrap().window;
    assert_eq!(window.tabs, kept);
    assert_eq!(window.active, None, "the active tab was a deleted node");
    assert_eq!((window.sidebar_width, window.placement.as_str(), window.split), (280.0, "auto", 50.0));

    core.save_window_state(window_with(tabs.clone(), Some(&format!("env:{second}")))).unwrap();
    let window = core.load_session().unwrap().window;
    assert_eq!(window.tabs, kept);
    assert_eq!(window.active, None, "the active tab was a deleted environment");

    // a tab that stays keeps being the active one
    core.save_window_state(window_with(tabs.clone(), Some(&a))).unwrap();
    assert_eq!(core.load_session().unwrap().window.active.as_deref(), Some(a.as_str()));
    core.save_window_state(window_with(tabs.clone(), Some(&format!("env:{first}")))).unwrap();
    assert_eq!(core.load_session().unwrap().window.active, Some(format!("env:{first}")));
    core.save_window_state(window_with(tabs, Some("env:globals"))).unwrap();
    assert_eq!(core.load_session().unwrap().window.active.as_deref(), Some("env:globals"));
}

#[test]
fn draft_round_trips_and_null_deletes_it() {
    let (core, dir) = core();
    let (a, collection) = (node_id(&core, "A"), node_id(&core, "Synthetic"));
    let request_draft = json!({
        "method": "POST", "url": "{{host}}/login",
        "headers": [{ "key": "X-Token", "value": "t", "enabled": true }],
        "body": { "mode": "raw", "data": { "text": "{\"a\": 1.50}", "language": "json" } }
    });
    // the core does not check the fields of a draft: any object is kept as it came
    let collection_draft = json!({ "anything": [1, 2, 3] });
    core.save_draft(&a, Some(request_draft.clone())).unwrap();
    core.save_draft(&collection, Some(collection_draft.clone())).unwrap();

    let mut expected = vec![
        DraftDto { node_id: a.clone(), content: request_draft },
        DraftDto { node_id: collection.clone(), content: collection_draft },
    ];
    expected.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let session = core.load_session().unwrap();
    assert_eq!(session.drafts, expected, "ordered by the id of the node");
    assert_eq!(as_json(&session)["drafts"][0]["node_id"], expected[0].node_id.as_str());

    // a newer draft replaces the older one
    core.save_draft(&a, Some(json!({ "method": "PUT" }))).unwrap();
    let drafts = core.load_session().unwrap().drafts;
    assert_eq!(drafts.iter().find(|draft| draft.node_id == a).unwrap().content, json!({ "method": "PUT" }));

    core.save_draft(&a, None).unwrap();
    let drafts = core.load_session().unwrap().drafts;
    assert_eq!(drafts.iter().map(|draft| draft.node_id.as_str()).collect::<Vec<_>>(), vec![collection.as_str()]);
    assert_eq!(raw_get(&db_path(&dir), &draft_key(&a)), None, "the key is deleted");
    // deleting a draft that is not there is not an error
    core.save_draft(&a, None).unwrap();
    core.save_draft("no-such-node", None).unwrap();
}

#[test]
fn draft_of_a_deleted_node_is_dropped_and_its_key_is_gone() {
    let (core, dir) = core();
    let (a, b) = (node_id(&core, "A"), node_id(&core, "B"));
    core.save_draft(&a, Some(json!({ "method": "POST" }))).unwrap();
    core.save_draft(&b, Some(json!({ "method": "PUT" }))).unwrap();
    core.delete_node(&a).unwrap();
    assert!(raw_get(&db_path(&dir), &draft_key(&a)).is_some(), "deleting a node leaves its draft for the next start");

    let drafts = core.load_session().unwrap().drafts;
    assert_eq!(drafts, vec![DraftDto { node_id: b.clone(), content: json!({ "method": "PUT" }) }]);
    assert_eq!(raw_get(&db_path(&dir), &draft_key(&a)), None);
    assert!(raw_get(&db_path(&dir), &draft_key(&b)).is_some());
}

#[test]
fn draft_that_is_not_json_is_dropped() {
    let (core, dir) = core();
    let (a, b) = (node_id(&core, "A"), node_id(&core, "B"));
    raw_set(&db_path(&dir), &draft_key(&a), "{broken");
    raw_set(&db_path(&dir), &draft_key(&b), "");
    assert!(core.load_session().unwrap().drafts.is_empty());
    assert_eq!(raw_get(&db_path(&dir), &draft_key(&a)), None);
    assert_eq!(raw_get(&db_path(&dir), &draft_key(&b)), None);
    // the next start is clean
    assert!(core.load_session().unwrap().drafts.is_empty());
}

#[test]
fn draft_that_is_not_an_object_is_dropped() {
    let (core, dir) = core();
    let db = db_path(&dir);
    let ids: Vec<String> = ["A", "B", "C", "D", "E"].iter().map(|name| node_id(&core, name)).collect();
    for (id, value) in ids.iter().zip([r#""abc""#, "[]", "5", "null"]) {
        raw_set(&db, &draft_key(id), value);
    }
    raw_set(&db, &draft_key(&ids[4]), r#"{ "method": "POST" }"#);

    let drafts = core.load_session().unwrap().drafts;
    assert_eq!(drafts, vec![DraftDto { node_id: ids[4].clone(), content: json!({ "method": "POST" }) }]);
    for id in &ids[..4] {
        assert_eq!(raw_get(&db, &draft_key(id)), None, "the key of a draft that is not an object is deleted");
    }
    assert_eq!(raw_get(&db, &draft_key(&ids[4])).as_deref(), Some(r#"{ "method": "POST" }"#), "the good draft stays");
}

#[test]
fn save_draft_for_an_unknown_node_writes_nothing() {
    let (core, dir) = core();
    assert_eq!(core.save_draft("no-such-node", Some(json!({ "method": "POST" }))), Ok(()));
    assert_eq!(raw_get(&db_path(&dir), &draft_key("no-such-node")), None);
    assert!(core.load_session().unwrap().drafts.is_empty());
}

#[test]
fn session_survives_reopening_the_database() {
    let (core, dir) = core();
    let a = node_id(&core, "A");
    let first = environment_id(&core, "First");
    let state = WindowState {
        sidebar_width: 333.0,
        placement: "bottom".to_string(),
        split: 61.5,
        tabs: vec![node_tab(&a), environment_tab(&first)],
        active: Some(a.clone()),
    };
    let settings = SettingsDto {
        timeout_ms: 1500,
        verify_tls: false,
        proxy: ProxyDto::Manual { url: "http://127.0.0.1:8888".to_string() },
        motion: MotionLevel::Rich,
        scripts_enabled: true,
        script_timeout_ms: 10_000,
    };
    core.save_window_state(state.clone()).unwrap();
    core.save_draft(&a, Some(json!({ "method": "DELETE", "url": "http://127.0.0.1/y" }))).unwrap();
    core.save_settings(settings.clone()).unwrap();
    drop(core);

    let core = open(&dir);
    let session = core.load_session().unwrap();
    assert_eq!(session.window, state);
    assert_eq!(session.drafts, vec![DraftDto { node_id: a, content: json!({ "method": "DELETE", "url": "http://127.0.0.1/y" }) }]);
    assert_eq!(core.get_settings().unwrap(), settings);
}

#[test]
fn default_settings() {
    let (core, dir) = core();
    assert_eq!(core.get_settings().unwrap(), default_settings_value());
    assert_eq!(
        as_json(&core.get_settings().unwrap()),
        json!({
            "timeout_ms": 0, "verify_tls": true, "proxy": { "mode": "system" }, "motion": "normal",
            "scripts_enabled": true, "script_timeout_ms": 10000
        })
    );
    assert_eq!(raw_get(&db_path(&dir), SETTINGS_KEY), None, "reading writes nothing");

    for broken in ["{broken", "", "[]", "null", r#"{ "timeout_ms": -1, "verify_tls": true, "proxy": { "mode": "system" } }"#,
        r#"{ "timeout_ms": 5, "verify_tls": true, "proxy": { "mode": "tunnel" } }"#, r#"{ "timeout_ms": 5 }"#]
    {
        raw_set(&db_path(&dir), SETTINGS_KEY, broken);
        assert_eq!(core.get_settings().unwrap(), default_settings_value(), "value: {broken}");
    }
}

#[test]
fn settings_round_trip() {
    let (core, dir) = core();
    let text = r#"{"timeout_ms":30000,"verify_tls":false,"proxy":{"mode":"manual","url":"http://127.0.0.1:8888"},"motion":"rich","scripts_enabled":false,"script_timeout_ms":2500}"#;
    let settings: SettingsDto = serde_json::from_str(text).unwrap();
    assert_eq!(
        settings,
        SettingsDto {
            timeout_ms: 30000,
            verify_tls: false,
            proxy: ProxyDto::Manual { url: "http://127.0.0.1:8888".to_string() },
            motion: MotionLevel::Rich,
            scripts_enabled: false,
            script_timeout_ms: 2500,
        }
    );
    core.save_settings(settings.clone()).unwrap();
    assert_eq!(core.get_settings().unwrap(), settings);
    assert_eq!(serde_json::to_string(&core.get_settings().unwrap()).unwrap(), text);
    assert_eq!(raw_get(&db_path(&dir), SETTINGS_KEY).as_deref(), Some(text));

    for proxy in [ProxyDto::Off, ProxyDto::System] {
        let settings = SettingsDto { timeout_ms: MAX_TIMEOUT_MS, proxy, motion: MotionLevel::Off, ..default_settings_value() };
        core.save_settings(settings.clone()).unwrap();
        assert_eq!(core.get_settings().unwrap(), settings);
    }
    assert_eq!(as_json(&SettingsDto { proxy: ProxyDto::Off, ..default_settings_value() })["proxy"], json!({ "mode": "off" }));

    // the address of a manual proxy is stored without the spaces around it
    let spaced = SettingsDto { proxy: ProxyDto::Manual { url: "  https://proxy.test:3128 \t".to_string() }, ..default_settings_value() };
    core.save_settings(spaced).unwrap();
    assert_eq!(core.get_settings().unwrap().proxy, ProxyDto::Manual { url: "https://proxy.test:3128".to_string() });
}

#[test]
fn settings_of_a_build_without_the_motion_level_stay_readable() {
    let (core, dir) = core();
    // what the previous build stored: no `motion` at all
    raw_set(&db_path(&dir), SETTINGS_KEY, r#"{"timeout_ms":1500,"verify_tls":false,"proxy":{"mode":"off"}}"#);
    assert_eq!(
        core.get_settings().unwrap(),
        SettingsDto { timeout_ms: 1500, verify_tls: false, proxy: ProxyDto::Off, ..default_settings_value() }
    );
    // and the send still takes them
    assert_eq!(
        AppSettings::from(core.get_settings().unwrap()),
        AppSettings {
            timeout: Some(Duration::from_millis(1500)),
            verify_tls: false,
            proxy: ProxySetting::Off,
            scripts_enabled: true,
            script_timeout: Duration::from_secs(10),
        }
    );
}

/// What a build without scripts stored: the two keys of the scripts are absent and read as the defaults,
/// and nothing of the rest is reset.
#[test]
fn settings_saved_before_stage_2_keep_their_values() {
    let (core, dir) = core();
    raw_set(&db_path(&dir), SETTINGS_KEY, r#"{"timeout_ms":30000,"verify_tls":false,"proxy":{"mode":"off"},"motion":"rich"}"#);
    assert_eq!(
        core.get_settings().unwrap(),
        SettingsDto {
            timeout_ms: 30000,
            verify_tls: false,
            proxy: ProxyDto::Off,
            motion: MotionLevel::Rich,
            scripts_enabled: true,
            script_timeout_ms: 10_000,
        }
    );
}

#[test]
fn script_timeout_out_of_range_is_refused() {
    let (core, dir) = core();
    let saved = SettingsDto { script_timeout_ms: 2500, ..default_settings_value() };
    core.save_settings(saved.clone()).unwrap();
    let stored = raw_get(&db_path(&dir), SETTINGS_KEY);

    for out_of_range in [99, 600_001] {
        assert_eq!(
            core.save_settings(SettingsDto { script_timeout_ms: out_of_range, ..default_settings_value() }),
            Err("script timeout is out of range".to_string()),
            "{out_of_range}"
        );
        assert_eq!(core.get_settings().unwrap(), saved, "{out_of_range}");
    }
    assert_eq!(raw_get(&db_path(&dir), SETTINGS_KEY), stored, "the stored value is the one saved before");

    for edge in [100, 600_000] {
        let settings = SettingsDto { script_timeout_ms: edge, ..default_settings_value() };
        core.save_settings(settings.clone()).unwrap();
        assert_eq!(core.get_settings().unwrap(), settings);
    }
    assert_eq!((MIN_SCRIPT_TIMEOUT_MS, MAX_SCRIPT_TIMEOUT_MS, DEFAULT_SCRIPT_TIMEOUT_MS), (100, 600_000, 10_000));
}

#[test]
fn an_unknown_motion_level_is_the_normal_one() {
    let (core, dir) = core();
    for (stored, level) in [
        (r#""off""#, MotionLevel::Off),
        (r#""normal""#, MotionLevel::Normal),
        (r#""rich""#, MotionLevel::Rich),
        (r#""extreme""#, MotionLevel::Normal),
        (r#""RICH""#, MotionLevel::Normal),
        (r#""""#, MotionLevel::Normal),
        ("null", MotionLevel::Normal),
        ("7", MotionLevel::Normal),
        (r#"{"level":"rich"}"#, MotionLevel::Normal),
    ] {
        let text = format!(r#"{{"timeout_ms":5,"verify_tls":true,"proxy":{{"mode":"system"}},"motion":{stored}}}"#);
        raw_set(&db_path(&dir), SETTINGS_KEY, &text);
        let settings = core.get_settings().unwrap();
        assert_eq!(settings.motion, level, "stored: {stored}");
        // the rest of the settings is not lost with it
        assert_eq!(settings.timeout_ms, 5, "stored: {stored}");
    }
    // the window sends the level the same way: what the core does not know is the normal one, not a refusal
    let sent: SettingsDto =
        serde_json::from_value(json!({ "timeout_ms": 0, "verify_tls": true, "proxy": { "mode": "system" }, "motion": "wild" })).unwrap();
    assert_eq!(sent.motion, MotionLevel::Normal);
    assert_eq!(as_json(&MotionLevel::Off), json!("off"));
    assert_eq!(as_json(&MotionLevel::Normal), json!("normal"));
    assert_eq!(as_json(&MotionLevel::Rich), json!("rich"));
}

#[test]
fn the_motion_level_changes_nothing_of_a_send() {
    let quiet = SettingsDto { motion: MotionLevel::Off, ..default_settings_value() };
    let rich = SettingsDto { motion: MotionLevel::Rich, ..default_settings_value() };
    assert_eq!(AppSettings::from(quiet), AppSettings::default());
    assert_eq!(AppSettings::from(rich), AppSettings::default());
}

#[test]
fn settings_refuse_a_bad_proxy_and_a_huge_timeout() {
    let (core, dir) = core();
    let saved = SettingsDto {
        timeout_ms: 30000,
        verify_tls: false,
        proxy: ProxyDto::Manual { url: "http://127.0.0.1:8888".to_string() },
        motion: MotionLevel::Rich,
        scripts_enabled: true,
        script_timeout_ms: 10_000,
    };
    core.save_settings(saved.clone()).unwrap();
    let stored = raw_get(&db_path(&dir), SETTINGS_KEY);

    for url in ["host:8080", "socks5://h:1", "", "   ", "127.0.0.1:8888", "http://"] {
        let refused = core.save_settings(SettingsDto { proxy: ProxyDto::Manual { url: url.to_string() }, ..default_settings_value() });
        let error = refused.expect_err(url);
        assert!(error.starts_with("invalid proxy address: ") && error.len() > "invalid proxy address: ".len(), "{url}: {error}");
        assert_eq!(core.get_settings().unwrap(), saved, "{url}");
    }
    assert_eq!(
        core.save_settings(SettingsDto { timeout_ms: 3_600_001, ..default_settings_value() }),
        Err("timeout is out of range".to_string())
    );
    assert_eq!(MAX_TIMEOUT_MS, 3_600_000);
    assert_eq!(core.get_settings().unwrap(), saved);
    assert_eq!(raw_get(&db_path(&dir), SETTINGS_KEY), stored, "the stored value is the one saved before");
}

#[test]
fn settings_dto_maps_to_app_settings() {
    assert_eq!(AppSettings::from(default_settings_value()), AppSettings::default());
    assert_eq!(AppSettings::from(default_settings_value()).timeout, None, "0 means no limit");
    let settings = SettingsDto {
        timeout_ms: 1500,
        verify_tls: false,
        proxy: ProxyDto::Off,
        motion: MotionLevel::Normal,
        scripts_enabled: false,
        script_timeout_ms: 2500,
    };
    assert_eq!(
        AppSettings::from(settings),
        AppSettings {
            timeout: Some(Duration::from_millis(1500)),
            verify_tls: false,
            proxy: ProxySetting::Off,
            scripts_enabled: false,
            script_timeout: Duration::from_millis(2500),
        }
    );
    let manual = SettingsDto { proxy: ProxyDto::Manual { url: "http://127.0.0.1:8888".to_string() }, ..default_settings_value() };
    assert_eq!(AppSettings::from(manual).proxy, ProxySetting::Manual("http://127.0.0.1:8888".to_string()));
    assert_eq!(AppSettings::from(default_settings_value()).proxy, ProxySetting::System);
}

/// The ids on the list of the collections whose scripts may run, as the database holds it.
fn allowed_list(dir: &TempDir) -> Vec<String> {
    serde_json::from_str(&raw_get(&db_path(dir), "scripts_allowed").expect("the list is stored")).unwrap()
}

#[test]
fn deleted_collection_leaves_the_allowed_list() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    let first = core.create_node(None, NodeKind::Collection, "First").unwrap();
    let second = core.create_node(None, NodeKind::Collection, "Second").unwrap();
    assert_eq!(allowed_list(&dir), [first.clone(), second.clone()]);

    core.delete_node(&first).unwrap();
    assert_eq!(allowed_list(&dir), [second]);
}

/// A database of a build without scripts has no list: every collection it holds gets the consent, once.
#[test]
fn database_without_the_allowed_list_gets_every_collection_once() {
    let (core, dir) = core();
    let collection = node_id(&core, "Synthetic");
    drop(core);
    rusqlite::Connection::open(db_path(&dir)).unwrap().execute("DELETE FROM app_state WHERE key = 'scripts_allowed'", []).unwrap();

    let core = open(&dir);
    assert_eq!(allowed_list(&dir), std::slice::from_ref(&collection));
    assert_eq!(core.scripts_allowed(&collection), Ok(true));

    // the list exists now: what the user forbade stays forbidden after a restart
    core.set_scripts_allowed(&collection, false).unwrap();
    drop(core);
    let core = open(&dir);
    assert_eq!(core.scripts_allowed(&collection), Ok(false));
    assert!(allowed_list(&dir).is_empty());
}
