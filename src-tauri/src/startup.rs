//! Start-up: when the UI has drawn itself (main.ts says so), and how long things took, in the log ("melue:
//! start-up: ..."). Background work the first screen doesn't need (the start-up scan, Android Auto's library) waits
//! for it, so the first screen isn't slowed down; it waits a few seconds at most (Melue can be started without its UI:
//! the car, the tray).

use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::AppHandle;

static STARTED: OnceLock<Instant> = OnceLock::new();
static UI_READY: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// The longest background work waits for the UI.
pub const UI_AT_MOST: Duration = Duration::from_secs(4);

/// First thing in run(): the clock the other times count from.
pub fn begin() {
    STARTED.get_or_init(Instant::now);
}

/// Milliseconds since [`begin`].
pub fn ms() -> u128 {
    STARTED.get().map_or(0, |s| s.elapsed().as_millis())
}

/// Waits until the UI has drawn itself, `at_most` that long.
pub fn wait_for_ui(at_most: Duration) {
    let (ready, changed) = &UI_READY;
    let ready = ready.lock().unwrap();
    let _ = changed.wait_timeout_while(ready, at_most, |ready| !*ready).unwrap();
}

/// The UI has drawn itself, `page_ms` after the page started loading: the desktop window shows (if it's waiting to),
/// and the work waiting for it goes ahead.
#[tauri::command]
pub fn window_ready(#[allow(unused_variables)] app: AppHandle, page_ms: Option<f64>) {
    let (ready, changed) = &UI_READY;
    let mut ready = ready.lock().unwrap();
    if !*ready {
        eprintln!("melue: start-up: the first screen drawn at {} ms (the page took {:.0} ms of that)", ms(), page_ms.unwrap_or(0.0));
    }
    *ready = true;
    changed.notify_all();
    drop(ready);
    #[cfg(desktop)]
    crate::desktop::ui_ready(&app);
}
