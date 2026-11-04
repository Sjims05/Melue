//! The tray's player (desktop): a small window above the tray icon, like a tray app's own panel (right-click the icon;
//! a left-click opens Melue's window). The song, its cover, the seek bar, the player's buttons and the volume,
//! then Open Melue and Quit (shells/tray/TrayPlayer.vue). It hides when you click anywhere else.
//!
//! Made the first time it's opened, then kept hidden (it opens at once after). A volume shortcut with Melue hidden
//! shows it a moment, without taking the focus (shortcuts.rs).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Rect, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::guard::Locked;

const LABEL: &str = "tray";
/// Its width (logical pixels) and its room from the taskbar; its height is its page's (tray_fit), until known this.
const WIDTH: f64 = 340.0;
const HEIGHT: u32 = 260;
const MARGIN: f64 = 12.0;
/// Its page's height (logical pixels).
static FIT: AtomicU32 = AtomicU32::new(HEIGHT);
/// Shown by a shortcut: hidden again after this, unless it was clicked into.
const FLASH: Duration = Duration::from_millis(2500);

/// The tray icon's place when last clicked (the player opens above it).
static ICON: Mutex<Option<Rect>> = Mutex::new(None);
/// When it last hid by losing the focus: a click on the tray icon itself does that first, and mustn't open it again.
static HIDDEN_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Each flash's number: an older one's timer leaves a newer one alone.
static FLASHES: AtomicU64 = AtomicU64::new(0);

/// Its window is made (hidden) a moment after start, so even the first right-click opens it at once (its page has
/// loaded by then); after Melue's own start, so that's no slower.
const PRELOAD_AFTER: Duration = Duration::from_secs(5);

pub fn preload(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(PRELOAD_AFTER);
        window(&app);
    });
}

/// The tray icon right-clicked: the player opens above it, or closes. Off the tray's event (a window made in an event
/// handler can hang on Windows).
pub fn toggle(app: &AppHandle, icon: Rect) {
    let app = app.clone();
    std::thread::spawn(move || toggle_now(&app, icon));
}

fn toggle_now(app: &AppHandle, icon: Rect) {
    *ICON.locked() = Some(icon);
    let just_hidden = HIDDEN_AT.locked().is_some_and(|at| at.elapsed() < Duration::from_millis(300));
    let Some(window) = window(app) else { return };
    if window.is_visible().unwrap_or(false) || just_hidden {
        hidden(&window);
        return;
    }
    FLASHES.fetch_add(1, Ordering::SeqCst);
    place(&window);
    let _ = window.show();
    // Again once shown (a hidden window's place isn't always kept).
    place(&window);
    let _ = window.set_focus();
    let _ = window.emit_to(LABEL, SHOWN, ());
}

/// A volume shortcut with Melue hidden: shown a moment, without the focus (off the shortcut's event, as `toggle`).
pub fn flash(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || flash_now(&app));
}

fn flash_now(app: &AppHandle) {
    let Some(window) = window(app) else { return };
    let this = FLASHES.fetch_add(1, Ordering::SeqCst) + 1;
    if !window.is_visible().unwrap_or(false) {
        place(&window);
        let _ = window.show();
        place(&window);
        let _ = window.emit_to(LABEL, SHOWN, ());
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FLASH);
        // Not if another flash came since, or it was clicked into (then it hides as usual, on leaving it).
        if FLASHES.load(Ordering::SeqCst) == this {
            if let Some(window) = app.get_webview_window(LABEL).filter(|w| !w.is_focused().unwrap_or(false)) {
                hidden(&window);
            }
        }
    });
}

/// Its page is told when it shows (it fades in, as a menu does) and when it hides (ready, faded out, for next time).
const SHOWN: &str = "tray-shown";
const HIDDEN: &str = "tray-hidden";

fn hidden(window: &WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit_to(LABEL, HIDDEN, ());
}

/// One at a time makes the window (the preload and a right-click at once would make two).
static MAKING: Mutex<()> = Mutex::new(());

/// The player's window: made the first time.
fn window(app: &AppHandle) -> Option<WebviewWindow> {
    let _making = MAKING.locked();
    if let Some(window) = app.get_webview_window(LABEL) {
        return Some(window);
    }
    // The main window's browser arguments too (tauri.windows.conf.json): two windows' WebViews have to be started
    // alike, or the second can't be made ("not in the correct state").
    let args = app.config().app.windows.first().and_then(|w| w.additional_browser_args.clone());
    let mut builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()));
    if let Some(args) = &args {
        builder = builder.additional_browser_args(args);
    }
    let made = builder
        .title("Melue")
        .inner_size(WIDTH, f64::from(FIT.load(Ordering::SeqCst)))
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .visible(false)
        .build();
    match made {
        Ok(window) => {
            let hide = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    *HIDDEN_AT.locked() = Some(Instant::now());
                    hidden(&hide);
                }
            });
            Some(window)
        }
        Err(e) => {
            eprintln!("melue: tray player: {e}");
            None
        }
    }
}

/// Above the tray icon (or below it, with the taskbar at the top), inside the screen's work area. Never clicked yet
/// (a shortcut first): the work area's corner by the clock.
fn place(window: &WebviewWindow) {
    let icon = *ICON.locked();
    let monitor = match icon {
        Some(icon) => {
            let at = icon.position.to_physical::<f64>(1.0);
            window.monitor_from_point(at.x, at.y).ok().flatten()
        }
        None => None,
    }
    .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (w, h, margin) = (WIDTH * scale, f64::from(FIT.load(Ordering::SeqCst)) * scale, MARGIN * scale);
    let (left, top) = (area.position.x as f64, area.position.y as f64);
    let (right, bottom) = (left + area.size.width as f64, top + area.size.height as f64);
    // Above the icon itself (in the taskbar, or in the hidden icons' pop-up above it), or below it with the taskbar at
    // the top; inside the work area.
    let (centre, icon_top, icon_bottom) = match icon {
        Some(icon) => {
            let at = icon.position.to_physical::<f64>(scale);
            let size: PhysicalSize<f64> = icon.size.to_physical(scale);
            (at.x + size.width / 2.0, at.y, at.y + size.height)
        }
        None => (right - w / 2.0 - margin, bottom, bottom),
    };
    let x = (centre - w / 2.0).clamp(left + margin, right - w - margin);
    let gap = 8.0 * scale;
    let y = if icon_top < top + (bottom - top) / 2.0 {
        (icon_bottom.max(top) + gap).min(bottom - h - margin)
    } else {
        (icon_top.min(bottom) - h - gap).max(top + margin)
    };
    // Moved, then sized in this screen's pixels, then moved again: Windows rescales a window that comes to a screen of
    // another scaling (or when the scaling changes, as after sleep or a display change), and the page's height alone
    // wouldn't size it again, so it grew taller than aimed for (its bottom on the icon).
    let at = PhysicalPosition::new(x.round() as i32, y.round() as i32);
    let _ = window.set_position(at);
    let _ = window.set_size(PhysicalSize::new(w.round() as u32, h.round() as u32));
    let _ = window.set_position(at);
}

/// The player's page, as tall as it is (logical pixels): the window takes its height, in its place.
#[tauri::command]
pub fn tray_fit(app: AppHandle, height: u32) {
    let height = height.clamp(120, 600);
    if FIT.swap(height, Ordering::SeqCst) == height {
        return;
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_size(LogicalSize::new(WIDTH, f64::from(height)));
        if window.is_visible().unwrap_or(false) {
            place(&window);
        }
    }
}

/// The player's "Open Melue": the window first (hiding the player first hands the focus back to the app before, and
/// Melue's window couldn't come to the front then), then the player goes.
#[tauri::command]
pub fn tray_open_main(app: AppHandle) {
    crate::desktop::show_window(&app);
    if let Some(window) = app.get_webview_window(LABEL) {
        hidden(&window);
    }
}

/// The player's "Quit": Melue quits (as from the tray's menu).
#[tauri::command]
pub fn tray_quit(app: AppHandle) {
    crate::desktop::quit(&app);
}
