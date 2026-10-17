//! Desktop: the tray icon, closing the window (hidden, the music keeps playing; or quit), starting with the system
//! (in the tray, without the window), and one Melue at a time. The Display settings "Start Melue with Windows" and
//! "Closing the window".

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

/// Starting with the system (the frontend switches it with @tauri-apps/plugin-autostart).
pub fn autostart() -> TauriPlugin<Wry> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN]))
}

/// One Melue at a time: opening it again shows the one that's running (it may be in the tray).
pub fn single_instance() -> TauriPlugin<Wry> {
    tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app))
}

/// At start: the window (unless the system started Melue), the tray, and starting with the system the first time.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    // The window starts hidden (tauri.windows.conf.json), so starting in the tray doesn't flash it.
    if !std::env::args().any(|a| a == HIDDEN) {
        show_window(app);
    }
    let show = MenuItem::with_id(app, "show", "Show Melue", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Melue", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &PredefinedMenuItem::separator(app)?, &quit_item])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Melue")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app),
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

/// Quit from the tray: where the song is now goes to the share first (like closing the window).
fn quit(app: &AppHandle) {
    crate::sync::handoff::save_now(app);
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
