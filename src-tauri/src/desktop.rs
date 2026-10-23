//! Desktop: the tray icon (show the window, play / pause, next, previous, quit), closing the window (hidden, the music keeps playing; or quit), starting with the system
//! (in the tray, without the window; or with it), and one Melue at a time. The Display settings "Start Melue with
//! Windows", "When Windows starts it" and "Closing the window".

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::plugin::TauriPlugin;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, CloseRequestApi, Manager, Window, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use crate::library::Library;

/// The library setting for closing the window: "tray" (the default: hidden, the music keeps playing) or "quit".
const CLOSE_SETTING: &str = "close_window";
/// Set once Melue has switched starting with the system on (its default), so switching it off sticks.
const AUTOSTART_SET: &str = "autostart_set";
/// What the system starts Melue with: it starts in the tray, without its window.
const HIDDEN: &str = "--hidden";
/// The library setting for starting with the system: "tray" (the default: no window) or "window" (shown).
const AUTOSTART_WINDOW_SETTING: &str = "autostart_window";

/// Starting with the system (the frontend switches it with @tauri-apps/plugin-autostart).
pub fn autostart() -> TauriPlugin<Wry> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN]))
}

/// One Melue at a time: opening it again shows the one that's running (it may be in the tray).
pub fn single_instance() -> TauriPlugin<Wry> {
    tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app))
}

/// The window waits to be shown until the UI is ready (see [`ui_ready`]).
static SHOW_WHEN_READY: AtomicBool = AtomicBool::new(false);
/// The longest the window waits for the UI (a slow start, or the UI failing to load: never a Melue without a window).
const READY_AT_MOST: Duration = Duration::from_secs(5);

/// First thing at start: the window, unless the system started Melue (then it stays in the tray). The window starts
/// hidden (tauri.windows.conf.json) and shows once the UI has drawn itself, not as a white window before that.
pub fn show_at_start(app: &AppHandle) {
    if !std::env::args().any(|a| a == HIDDEN) {
        show_when_ready(app);
    }
}

fn show_when_ready(app: &AppHandle) {
    SHOW_WHEN_READY.store(true, Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(READY_AT_MOST);
        ui_ready(&app);
    });
}

/// The UI has drawn itself (main.ts): the window shows, if it's waiting to.
pub fn ui_ready(app: &AppHandle) {
    if SHOW_WHEN_READY.swap(false, Ordering::SeqCst) {
        show_window(app);
    }
}

/// At start, after the rest: the tray, and starting with the system the first time.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Melue", true, None::<&str>)?;
    // The player, without opening the window.
    let play = MenuItem::with_id(app, "play", "Play / pause", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Next song", true, None::<&str>)?;
    let previous = MenuItem::with_id(app, "previous", "Previous song", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Melue", true, None::<&str>)?;
    let line = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(app, &[&show, &line()?, &play, &next, &previous, &line()?, &quit_item])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Melue")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app),
            "play" | "next" | "previous" => player(app, event.id.as_ref()),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    first_autostart(app);
    // Started by the system, set to start with its window: shown now (show_at_start couldn't read the setting yet).
    let with_window = app.state::<Library>().setting(AUTOSTART_WINDOW_SETTING).ok().flatten();
    if std::env::args().any(|a| a == HIDDEN) && with_window.as_deref() == Some("window") {
        show_when_ready(app);
    }
    Ok(())
}

/// Closing the window: hidden in the tray (the music keeps playing) unless "Quit" is picked. True when it was hidden.
pub fn on_close(window: &Window, api: &CloseRequestApi) -> bool {
    let mode = window.app_handle().state::<Library>().setting(CLOSE_SETTING).ok().flatten();
    if mode.as_deref() == Some("quit") {
        return false;
    }
    api.prevent_close();
    let _ = window.hide();
    true
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// The tray's player items (the window follows: the player tells it, as with media keys).
fn player(app: &AppHandle, action: &str) {
    use tauri_plugin_native_audio::NativeAudioExt;
    let audio = app.native_audio();
    let done = match action {
        "next" => audio.next(),
        "previous" => audio.previous(),
        _ if audio.get_state().is_ok_and(|s| s.is_playing) => audio.pause(),
        _ => audio.play(),
    };
    if let Err(e) = done {
        eprintln!("melue: tray: can't {action}: {e}");
    }
}

/// Quit from the tray: where the song is now goes to the share first (like closing the window).
fn quit(app: &AppHandle) {
    crate::sync::handoff::save_now(app);
    // The other devices hear at once that this one is gone.
    crate::remote::presence::leave(app);
    app.exit(0);
}

/// Starting with the system is on by default: switched on the first time Melue runs (then it's the user's choice).
/// Not in dev builds, which would start the dev build.
fn first_autostart(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let library = app.state::<Library>();
    if library.setting(AUTOSTART_SET).ok().flatten().is_some() {
        return;
    }
    match app.autolaunch().enable() {
        Ok(()) => {
            let _ = library.set_setting(AUTOSTART_SET, "1");
        }
        Err(e) => eprintln!("melue: can't start with the system: {e}"),
    }
}
