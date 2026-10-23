//! Remote playback (Phase 4): playing on another of your devices from this one, like Spotify Connect for your own
//! music. Devices say on the share that they're online and where they can be reached (presence.rs); they talk to
//! each other directly (server.rs), proving they're yours with the key on the share (key.rs), each playing its own
//! copy of the songs.
//!
//! "Let other devices play here" (this device's own): On (the default: your devices just play here), Ask first
//! (someone here says yes), or Off.

pub mod key;
pub mod presence;
pub mod server;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::library::Library;

/// The library setting for "Let other devices play here".
const ALLOW_SETTING: &str = "remote_allow";

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
}

pub fn settings(app: &AppHandle) -> Result<RemoteSettings, String> {
    let allow = match app.state::<Library>().setting(ALLOW_SETTING)?.as_deref() {
        Some("ask") => Allow::Ask,
        Some("off") => Allow::Off,
        _ => Allow::On,
    };
    Ok(RemoteSettings { allow })
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
    app.state::<Library>().set_setting(ALLOW_SETTING, value)?;
    // The others hear of it at once (not at the next heartbeat).
    presence::announce_soon();
    self::settings(&app)
}
