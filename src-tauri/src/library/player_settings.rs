//! The player's settings (the volume curve, media keys, what happens at start and at the end of the queue, keeping
//! the player ready on Android): Melue keeps them, so the settings page shows them (the player can't read them back)
//! and they can sync across devices later. The frontend hands them to the player (setOptions, setSkipInterval),
//! which saves them too, so they hold even when the player starts without the UI (Android Auto).

use serde::{Deserialize, Serialize};

use super::{err, Library, Result};

const SETTING: &str = "player_settings";

/// The player's options, with its own defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerSettings {
    /// How the volume slider becomes loudness: "linear", "quadratic" or "cubic".
    pub volume_curve: String,
    /// Desktop: the system's media controls and the media keys.
    pub media_controls: bool,
    /// What previous / next do on media keys, headphones, the notification: 0 skips songs, above 0 seeks that many seconds.
    pub skip_seconds: u32,
    /// The last queue when Melue starts: "paused" (loaded, paused), "play", or "off" (starts empty).
    pub resume_last_queue: String,
    /// Previous restarts the song after this many seconds (0: always the song before).
    pub previous_restarts_after_seconds: u32,
    /// Next on the last song with repeat off: "nothing", or "first" (the first song, paused).
    pub next_at_end: String,
    /// Songs added to the queue stay when it repeats (otherwise they play once).
    pub repeat_added_tracks: bool,
    /// Android: how long the player stays ready (notification, car) after a pause.
    pub paused_keep_alive_minutes: u32,
    /// Android: stays ready while Android Auto is connected.
    pub keep_alive_while_car_connected: bool,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            volume_curve: "quadratic".into(),
            media_controls: true,
            skip_seconds: 0,
            resume_last_queue: "paused".into(),
            previous_restarts_after_seconds: 3,
            next_at_end: "nothing".into(),
            repeat_added_tracks: false,
            paused_keep_alive_minutes: 30,
            keep_alive_while_car_connected: true,
        }
    }
}

impl PlayerSettings {
    fn check(&self) -> Result<()> {
        let one_of = |value: &str, name: &str, allowed: &[&str]| {
            if allowed.contains(&value) { Ok(()) } else { Err(format!("{name} must be one of {}", allowed.join(", "))) }
        };
        let at_most = |value: u32, name: &str, max: u32| {
            if value <= max { Ok(()) } else { Err(format!("{name} must be at most {max}")) }
        };
        one_of(&self.volume_curve, "volumeCurve", &["linear", "quadratic", "cubic"])?;
        one_of(&self.resume_last_queue, "resumeLastQueue", &["off", "paused", "play"])?;
        one_of(&self.next_at_end, "nextAtEnd", &["nothing", "first"])?;
        at_most(self.skip_seconds, "skipSeconds", 120)?;
        at_most(self.previous_restarts_after_seconds, "previousRestartsAfterSeconds", 60)?;
        at_most(self.paused_keep_alive_minutes, "pausedKeepAliveMinutes", 24 * 60)
    }
}

impl Library {
    pub fn player_settings(&self) -> Result<PlayerSettings> {
        match self.setting(SETTING)? {
            Some(json) => serde_json::from_str(&json).map_err(err),
            None => Ok(PlayerSettings::default()),
        }
    }

    pub fn set_player_settings(&self, settings: &PlayerSettings) -> Result<()> {
        settings.check()?;
        self.set_setting(SETTING, &serde_json::to_string(settings).map_err(err)?)
    }
}
