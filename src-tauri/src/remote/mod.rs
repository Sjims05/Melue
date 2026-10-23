//! Remote playback (Phase 4): playing on another of your devices from this one, like Spotify Connect for your own
//! music. Devices say on the share that they're online and where they can be reached (presence.rs); they talk to
//! each other directly (server.rs), proving they're yours with the key on the share (key.rs), each playing its own
//! copy of the songs.
//!
//! "Let other devices play here" (this device's own): On (the default: your devices just play here), Ask first
//! (someone here says yes), or Off. And the volume's steps when playing on another device: a phone's (levels a step)
//! and, from a phone's volume buttons, a computer's (% a press).

pub mod control;
pub mod key;
pub mod link;
pub mod presence;
pub mod server;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::library::Library;

/// The library setting for "Let other devices play here".
const ALLOW_SETTING: &str = "remote_allow";
/// "Volume steps on a phone": the levels one step moves a phone's volume (phones with more than 30).
const PHONE_STEP_SETTING: &str = "remote_phone_step";
/// "Volume buttons on another device": the % one press of a phone's buttons moves a computer's volume.
const BUTTON_STEP_SETTING: &str = "remote_button_step";
const PHONE_STEP_DEFAULT: u32 = 10;
const BUTTON_STEP_DEFAULT: u32 = 5;

/// Whether the other devices may play here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Allow {
    #[default]
    On,
    /// Someone at this device says yes first.
    Ask,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSettings {
    pub allow: Allow,
    pub phone_step: u32,
    pub button_step: u32,
}

pub fn settings(app: &AppHandle) -> Result<RemoteSettings, String> {
    let allow = match app.state::<Library>().setting(ALLOW_SETTING)?.as_deref() {
        Some("ask") => Allow::Ask,
        Some("off") => Allow::Off,
        _ => Allow::On,
    };
    let library = app.state::<Library>();
    let number = |key: &str, default: u32| Ok::<_, String>(library.setting(key)?.and_then(|v| v.parse().ok()).filter(|n| *n > 0).unwrap_or(default));
    Ok(RemoteSettings { allow, phone_step: number(PHONE_STEP_SETTING, PHONE_STEP_DEFAULT)?, button_step: number(BUTTON_STEP_SETTING, BUTTON_STEP_DEFAULT)? })
}

#[tauri::command(async)]
pub fn remote_settings(app: AppHandle) -> Result<RemoteSettings, String> {
    settings(&app)
}

#[tauri::command(async)]
pub fn remote_set_settings(app: AppHandle, settings: RemoteSettings) -> Result<RemoteSettings, String> {
    let value = match settings.allow {
        Allow::On => "on",
        Allow::Ask => "ask",
        Allow::Off => "off",
    };
    let library = app.state::<Library>();
    library.set_setting(ALLOW_SETTING, value)?;
    library.set_setting(PHONE_STEP_SETTING, &settings.phone_step.max(1).to_string())?;
    library.set_setting(BUTTON_STEP_SETTING, &settings.button_step.max(1).to_string())?;
    // The others hear of it at once (not at the next heartbeat).
    presence::announce_soon();
    self::settings(&app)
}
