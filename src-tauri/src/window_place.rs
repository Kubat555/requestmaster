//! Where the window was when it was closed, so that the next run opens it there. Everything here is plain
//! Rust: the window itself is asked and moved by the glue in `main.rs`.

use serde::{Deserialize, Serialize};

use crate::core::{message, AppCore};

/// Key of `app_state` holding the place of the window: the JSON of [`WindowPlace`]. It exists so that the
/// window reopens where it was closed. Written by the program itself (the window glue in `main.rs`) when the
/// window is asked to close and when it is destroyed; read by the same glue once, at the start, before the
/// window is shown. The interface of the window neither reads nor writes it.
pub const WINDOW_PLACE_KEY: &str = "window_place";

/// The least part of the window, by the width and by the height in physical pixels, that must be visible on
/// one monitor for a stored place to be used.
const MIN_VISIBLE: i64 = 100;

/// The longest side of a window, in physical pixels, a stored place may have; anything above is taken for
/// a broken value.
const MAX_SIDE: u32 = 32_767;

/// The place of the window as it is stored: the outer rectangle of the normal (neither maximized nor
/// minimized) window in physical pixels, and whether the window was maximized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPlace {
    /// Left edge of the outer rectangle on the virtual desktop; negative on a monitor to the left of the
    /// main one.
    pub x: i32,
    /// Top edge of the outer rectangle on the virtual desktop.
    pub y: i32,
    /// Width of the outer rectangle.
    pub width: u32,
    /// Height of the outer rectangle.
    pub height: u32,
    /// Whether the window was maximized; the rectangle is then the one it returns to when un-maximized.
    pub maximized: bool,
}

/// The area of one monitor on the virtual desktop, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorArea {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// What the window says about itself at one moment: its outer rectangle in physical pixels and its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSnapshot {
    /// Left edge of the outer rectangle.
    pub x: i32,
    /// Top edge of the outer rectangle.
    pub y: i32,
    /// Width of the outer rectangle.
    pub width: u32,
    /// Height of the outer rectangle.
    pub height: u32,
    /// The window is maximized.
    pub maximized: bool,
    /// The window is minimized.
    pub minimized: bool,
}

/// Whether the stored place can be used with these monitors: its sides are sensible and at least 100x100
/// physical pixels of its rectangle lie on one of the monitors.
pub fn fits_on_monitors(place: &WindowPlace, monitors: &[MonitorArea]) -> bool {
    if place.width > MAX_SIDE || place.height > MAX_SIDE {
        return false;
    }
    monitors.iter().any(|monitor| {
        overlap(place.x, place.width, monitor.x, monitor.width) >= MIN_VISIBLE
            && overlap(place.y, place.height, monitor.y, monitor.height) >= MIN_VISIBLE
    })
}

/// Length of the common part of two segments given by their start and length; zero or less when they do
/// not meet.
fn overlap(start_a: i32, length_a: u32, start_b: i32, length_b: u32) -> i64 {
    let end_a = i64::from(start_a) + i64::from(length_a);
    let end_b = i64::from(start_b) + i64::from(length_b);
    end_a.min(end_b) - i64::from(start_a).max(i64::from(start_b))
}

/// The inner size that gives the window the outer size `stored_outer`, each pair being (width, height) in
/// physical pixels. The frame is what the window has now: `current_outer` minus `current_inner`; a frame
/// that comes out negative counts as none.
pub fn inner_size(stored_outer: (u32, u32), current_outer: (u32, u32), current_inner: (u32, u32)) -> (u32, u32) {
    let side = |stored: u32, outer: u32, inner: u32| stored.saturating_sub(outer.saturating_sub(inner));
    (
        side(stored_outer.0, current_outer.0, current_inner.0),
        side(stored_outer.1, current_outer.1, current_inner.1),
    )
}

/// Follows the window while it lives and knows the place to store when it ends.
///
/// The rectangle is taken only from a window that is neither maximized nor minimized, so a maximized window
/// keeps the rectangle it returns to, and a minimized window changes nothing at all.
///
/// On Windows a window being maximized reports itself twice: first moved, already with the maximized
/// rectangle but not yet marked as maximized, and only then resized, with the mark. The first report looks
/// like a normal window, so the tracker also keeps the place it had before the last change of the normal
/// rectangle and returns to it when the second report shows that the change was the maximizing.
#[derive(Debug, Default)]
pub struct PlaceTracker {
    /// The last known place; `None` until a normal window was seen and nothing was stored before.
    place: Option<WindowPlace>,
    /// The place before the normal rectangle changed last; `None` when there was none or it was already
    /// returned to.
    previous: Option<WindowPlace>,
}

impl PlaceTracker {
    /// A tracker that starts with the place the window was opened at; `None` — nothing is known yet.
    pub fn new(place: Option<WindowPlace>) -> Self {
        PlaceTracker { place, previous: None }
    }

    /// Takes the state of the window after it was moved or resized.
    pub fn observe(&mut self, window: WindowSnapshot) {
        if window.minimized {
            return;
        }
        let seen = WindowPlace {
            x: window.x,
            y: window.y,
            width: window.width,
            height: window.height,
            maximized: false,
        };
        if window.maximized {
            if self.place == Some(seen) {
                // The maximized rectangle was just taken for a normal one: go back to the place before it.
                self.place = self.previous.take();
            }
            if let Some(place) = &mut self.place {
                place.maximized = true;
            }
            return;
        }
        if self.place != Some(seen) {
            self.previous = self.place.replace(seen);
        }
    }

    /// The place to store; `None` while no normal rectangle is known.
    pub fn place(&self) -> Option<WindowPlace> {
        self.place
    }
}

impl AppCore {
    /// The stored place of the window; `None` when nothing is stored, the stored value cannot be read as
    /// [`WindowPlace`], or the store itself fails — the window then opens at its default place.
    pub fn window_place(&self) -> Option<WindowPlace> {
        let value = self.store().ok()?.state_get(WINDOW_PLACE_KEY).ok()??;
        serde_json::from_str(&value).ok()
    }

    /// Remembers the place of the window.
    pub fn save_window_place(&self, place: &WindowPlace) -> Result<(), String> {
        let value = serde_json::to_string(place).map_err(message)?;
        self.store()?.state_set(WINDOW_PLACE_KEY, &value).map_err(message)
    }
}
