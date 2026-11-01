//! Shortcuts that work anywhere (desktop): Melue's volume, play / pause, next and previous from any app, also with
//! Melue hidden in the tray. Set in Playback settings, this device's own (keyboards differ); on by default, Ctrl+Alt
//! with the arrows and Space. Taken by another app: said in the settings, the rest still work.
//!
//! Pressed: play / pause, next and previous go to the player here (as the tray's menu does); the volume to the window
//! ("shortcut" event: player/shortcuts.ts), which knows the volume steps and shows where it is (with the window hidden,
//! the tray's player shows a moment: tray.rs).

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::guard::Locked;
use crate::library::Library;

const SETTING: &str = "shortcuts";
/// The event the window acts on (its value: the action).
pub const EVENT: &str = "shortcut";

/// What each can do, in the settings' order.
pub const ACTIONS: [&str; 5] = ["volumeUp", "volumeDown", "playPause", "next", "previous"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Shortcuts {
    pub on: bool,
    /// Each action's keys ("Control+Alt+ArrowUp"); none: not set.
    pub keys: BTreeMap<String, String>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        let keys = [
            ("volumeUp", "Control+Alt+ArrowUp"),
            ("volumeDown", "Control+Alt+ArrowDown"),
            ("playPause", "Control+Alt+Space"),
            ("next", "Control+Alt+ArrowRight"),
            ("previous", "Control+Alt+ArrowLeft"),
        ];
        Self { on: true, keys: keys.into_iter().map(|(a, k)| (a.to_string(), k.to_string())).collect() }
    }
}

/// Held down, the volume goes on, like a phone's volume buttons: once at once, then after a pause again and again.
const REPEAT_AFTER: Duration = Duration::from_millis(400);
const REPEAT_EVERY: Duration = Duration::from_millis(100);
/// Each press's number (0: none held): a press's repeating stops once another press or its letting go changes it.
static HELD: AtomicU64 = AtomicU64::new(0);
static PRESSES: AtomicU64 = AtomicU64::new(0);

/// The registered shortcuts' actions, by the shortcut's id.
static REGISTERED: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);
/// The actions whose keys couldn't be had at the last registration.
static TAKEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state == ShortcutState::Released {
                return HELD.store(0, Ordering::SeqCst);
            }
            let action = REGISTERED.locked().as_ref().and_then(|r| r.get(&shortcut.id()).cloned());
            let Some(action) = action else { return };
            pressed(app, &action);
            if action.starts_with("volume") {
                repeat_while_held(app, action);
            }
        })
        .build()
}

/// A volume shortcut held down: again and again after a pause, until it's let go (or another is pressed).
fn repeat_while_held(app: &AppHandle, action: String) {
    let this = PRESSES.fetch_add(1, Ordering::SeqCst) + 1;
    HELD.store(this, Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(REPEAT_AFTER);
        while HELD.load(Ordering::SeqCst) == this {
            pressed(&app, &action);
            std::thread::sleep(REPEAT_EVERY);
        }
    });
}

fn pressed(app: &AppHandle, action: &str) {
    match action {
        "playPause" => crate::desktop::player(app, "play"),
        "next" | "previous" => crate::desktop::player(app, action),
        _ => {
            let _ = app.emit(EVENT, action);
            // The window hidden: the tray's player shows the volume a moment.
            let shown = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false));
            if !shown {
                crate::tray::flash(app);
            }
        }
    }
}

fn read(app: &AppHandle) -> Shortcuts {
    app.state::<Library>().setting(SETTING).ok().flatten().and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default()
}

/// Registers the shortcuts as set (none when off). Returns the actions whose keys couldn't be had (another app has
/// them, or they aren't keys this system knows).
pub fn apply(app: &AppHandle) -> Vec<String> {
    let set = read(app);
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    let mut registered = HashMap::new();
    let mut failed = Vec::new();
    if set.on {
        for action in ACTIONS {
            let Some(keys) = set.keys.get(action).filter(|k| !k.is_empty()) else { continue };
            match keys.parse::<Shortcut>().map_err(|e| e.to_string()).and_then(|s| shortcuts.register(s).map(|_| s).map_err(|e| e.to_string())) {
                Ok(shortcut) => {
                    registered.insert(shortcut.id(), action.to_string());
                }
                Err(e) => {
                    eprintln!("melue: shortcut {action} ({keys}): {e}");
                    failed.push(action.to_string());
                }
            }
        }
    }
    *REGISTERED.locked() = Some(registered);
    *TAKEN.locked() = failed.clone();
    failed
}

/// The shortcuts as set, the ones that couldn't be had, and each action's default keys (Playback settings).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutsNow {
    pub shortcuts: Shortcuts,
    pub taken: Vec<String>,
    pub defaults: BTreeMap<String, String>,
}

#[tauri::command(async)]
pub fn shortcuts_get(app: AppHandle) -> ShortcutsNow {
    ShortcutsNow { shortcuts: read(&app), taken: TAKEN.locked().clone(), defaults: Shortcuts::default().keys }
}

#[tauri::command(async)]
pub fn shortcuts_set(app: AppHandle, shortcuts: Shortcuts) -> Result<ShortcutsNow, String> {
    let json = serde_json::to_string(&shortcuts).map_err(|e| e.to_string())?;
    app.state::<Library>().set_setting(SETTING, &json).map_err(|e| e.to_string())?;
    let taken = apply(&app);
    Ok(ShortcutsNow { shortcuts, taken, defaults: Shortcuts::default().keys })
}
