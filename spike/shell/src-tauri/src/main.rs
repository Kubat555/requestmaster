// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // First statement: the reference point for `mark_ready`.
    let started = std::time::Instant::now();
    requestmaster_spike_lib::run(started)
}
