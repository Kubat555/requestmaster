//! The remembered place of the window, without a window: whether a stored place is still on some monitor,
//! which place is tracked while the window is moved, maximized and minimized, and how the place is kept in
//! the database. The database lives in a file, so that a test can put a broken value there through a second
//! connection.

use std::path::{Path, PathBuf};

use requestmaster_lib::window_place::{
    fits_on_monitors, inner_size, MonitorArea, PlaceTracker, WindowPlace, WindowSnapshot,
};
use requestmaster_lib::{AppCore, CoreConfig, WINDOW_PLACE_KEY};
use tempfile::TempDir;

/// A monitor of 1920x1080 whose left top corner is at (`x`, `y`).
fn monitor(x: i32, y: i32) -> MonitorArea {
    MonitorArea { x, y, width: 1920, height: 1080 }
}

fn place(x: i32, y: i32, width: u32, height: u32) -> WindowPlace {
    WindowPlace { x, y, width, height, maximized: false }
}

/// A window that is neither maximized nor minimized.
fn normal(x: i32, y: i32, width: u32, height: u32) -> WindowSnapshot {
    WindowSnapshot { x, y, width, height, maximized: false, minimized: false }
}

fn db_path(dir: &TempDir) -> PathBuf {
    dir.path().join("requestmaster.db")
}

/// A core over the database file of `dir`.
fn open(dir: &TempDir) -> AppCore {
    AppCore::new(CoreConfig {
        db_path: Some(db_path(dir)),
        bodies_dir: dir.path().join("bodies"),
        history_keep: None,
        measure_file: None,
    })
    .unwrap()
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

#[test]
fn a_place_inside_a_monitor_fits() {
    assert!(fits_on_monitors(&place(100, 100, 1280, 800), &[monitor(0, 0)]));
}

#[test]
fn a_place_partly_outside_fits_while_100_by_100_pixels_are_visible() {
    // Exactly 100x100 of the window is left in the right bottom corner of the monitor.
    assert!(fits_on_monitors(&place(1820, 980, 1280, 800), &[monitor(0, 0)]));
    // One pixel less by the width, then by the height.
    assert!(!fits_on_monitors(&place(1821, 980, 1280, 800), &[monitor(0, 0)]));
    assert!(!fits_on_monitors(&place(1820, 981, 1280, 800), &[monitor(0, 0)]));
    // The same over the left top corner.
    assert!(fits_on_monitors(&place(-1180, -700, 1280, 800), &[monitor(0, 0)]));
    assert!(!fits_on_monitors(&place(-1181, -700, 1280, 800), &[monitor(0, 0)]));
}

#[test]
fn the_visible_part_is_counted_on_one_monitor_not_summed_over_two() {
    // 60 pixels on the left monitor and 60 on the right one: neither shows 100.
    let window = place(-60, 100, 120, 800);
    assert!(!fits_on_monitors(&window, &[monitor(-1920, 0), monitor(0, 0)]));
}

#[test]
fn a_place_on_a_monitor_that_is_gone_does_not_fit() {
    let on_the_second_monitor = place(2200, 100, 1280, 800);
    assert!(fits_on_monitors(&on_the_second_monitor, &[monitor(0, 0), monitor(1920, 0)]));
    assert!(!fits_on_monitors(&on_the_second_monitor, &[monitor(0, 0)]));
    assert!(!fits_on_monitors(&on_the_second_monitor, &[]));
}

#[test]
fn negative_coordinates_of_a_left_hand_monitor_fit() {
    let window = place(-1800, -50, 1280, 800);
    assert!(fits_on_monitors(&window, &[monitor(0, 0), monitor(-1920, -200)]));
    assert!(!fits_on_monitors(&window, &[monitor(0, 0)]));
}

#[test]
fn a_zero_or_absurd_size_does_not_fit() {
    let monitors = [monitor(0, 0)];
    assert!(!fits_on_monitors(&place(100, 100, 0, 0), &monitors));
    assert!(!fits_on_monitors(&place(100, 100, 1280, 0), &monitors));
    assert!(!fits_on_monitors(&place(100, 100, 0, 800), &monitors));
    assert!(!fits_on_monitors(&place(100, 100, 99, 800), &monitors));
    assert!(!fits_on_monitors(&place(0, 0, u32::MAX, 800), &monitors));
    assert!(!fits_on_monitors(&place(0, 0, 1280, 40_000), &monitors));
    // The extreme numbers do not overflow.
    assert!(!fits_on_monitors(&place(i32::MAX, i32::MAX, u32::MAX, u32::MAX), &monitors));
    assert!(!fits_on_monitors(&place(i32::MIN, i32::MIN, 1280, 800), &monitors));
    // A monitor without an area shows nothing.
    assert!(!fits_on_monitors(&place(0, 0, 1280, 800), &[MonitorArea { x: 0, y: 0, width: 0, height: 0 }]));
}

#[test]
fn a_maximized_place_is_checked_by_its_normal_rectangle() {
    let stored = WindowPlace { maximized: true, ..place(2200, 100, 1280, 800) };
    assert!(fits_on_monitors(&stored, &[monitor(0, 0), monitor(1920, 0)]));
    assert!(!fits_on_monitors(&stored, &[monitor(0, 0)]));
}

#[test]
fn the_inner_size_is_the_outer_one_without_the_frame() {
    assert_eq!(inner_size((1280, 800), (1296, 839), (1280, 800)), (1264, 761));
    // A frame measured wrongly (the inner size above the outer one) changes nothing.
    assert_eq!(inner_size((1280, 800), (1000, 700), (1280, 800)), (1280, 800));
    // A stored size smaller than the frame does not go below zero.
    assert_eq!(inner_size((10, 10), (1296, 839), (1280, 800)), (0, 0));
}

#[test]
fn the_tracker_starts_with_the_stored_place_or_with_nothing() {
    assert_eq!(PlaceTracker::new(None).place(), None);
    let stored = WindowPlace { maximized: true, ..place(10, 20, 1280, 800) };
    assert_eq!(PlaceTracker::new(Some(stored)).place(), Some(stored));
}

#[test]
fn the_tracker_follows_the_normal_window() {
    let mut tracker = PlaceTracker::new(None);
    tracker.observe(normal(10, 20, 1280, 800));
    tracker.observe(normal(300, 40, 1000, 700));
    assert_eq!(tracker.place(), Some(place(300, 40, 1000, 700)));
}

#[test]
fn a_maximized_window_keeps_its_previous_normal_place() {
    let mut tracker = PlaceTracker::new(None);
    tracker.observe(normal(300, 40, 1000, 700));
    tracker.observe(WindowSnapshot { maximized: true, ..normal(-8, -8, 1936, 1056) });
    assert_eq!(tracker.place(), Some(WindowPlace { maximized: true, ..place(300, 40, 1000, 700) }));
    // Back to normal: the flag is dropped and the place follows the window again.
    tracker.observe(normal(320, 60, 1000, 700));
    assert_eq!(tracker.place(), Some(place(320, 60, 1000, 700)));
}

#[test]
fn a_minimized_window_changes_nothing() {
    let mut tracker = PlaceTracker::new(None);
    tracker.observe(normal(300, 40, 1000, 700));
    tracker.observe(WindowSnapshot { minimized: true, ..normal(-32000, -32000, 160, 28) });
    assert_eq!(tracker.place(), Some(place(300, 40, 1000, 700)));

    // Minimized from the maximized state: still "maximized, with the previous normal place".
    tracker.observe(WindowSnapshot { maximized: true, ..normal(-8, -8, 1936, 1056) });
    tracker.observe(WindowSnapshot { minimized: true, ..normal(-32000, -32000, 160, 28) });
    tracker.observe(WindowSnapshot { minimized: true, maximized: true, ..normal(-32000, -32000, 160, 28) });
    assert_eq!(tracker.place(), Some(WindowPlace { maximized: true, ..place(300, 40, 1000, 700) }));
}

#[test]
fn a_maximized_window_without_a_known_normal_place_gives_nothing() {
    let mut tracker = PlaceTracker::new(None);
    tracker.observe(WindowSnapshot { maximized: true, ..normal(-8, -8, 1936, 1056) });
    assert_eq!(tracker.place(), None);
}

#[test]
fn the_place_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    assert_eq!(core.window_place(), None);

    let stored = WindowPlace { x: -1800, y: 40, width: 1000, height: 700, maximized: true };
    core.save_window_place(&stored).unwrap();
    assert_eq!(core.window_place(), Some(stored));
    assert_eq!(
        raw_get(&db_path(&dir), WINDOW_PLACE_KEY).as_deref(),
        Some(r#"{"x":-1800,"y":40,"width":1000,"height":700,"maximized":true}"#)
    );
    drop(core);
    assert_eq!(open(&dir).window_place(), Some(stored));
}

#[test]
fn an_unreadable_place_is_no_place() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    for broken in [
        "not json",
        "null",
        "[]",
        r#"{"x":1,"y":2}"#,
        r#"{"x":"left","y":2,"width":1000,"height":700,"maximized":false}"#,
        r#"{"x":1,"y":2,"width":-1000,"height":700,"maximized":false}"#,
    ] {
        raw_set(&db_path(&dir), WINDOW_PLACE_KEY, broken);
        assert_eq!(core.window_place(), None, "{broken}");
    }
    // The next save replaces the broken value.
    core.save_window_place(&place(1, 2, 1000, 700)).unwrap();
    assert_eq!(core.window_place(), Some(place(1, 2, 1000, 700)));
}
