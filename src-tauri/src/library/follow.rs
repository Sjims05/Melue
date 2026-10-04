//! The queue follows the library: when a change touches the list the player is playing (a song added to
//! the album, the playlist edited), the queue gets the list's new songs through the player's updateQueue,
//! with the user's settings. In Rust, so it happens even while the UI is asleep (Android Auto).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_native_audio::{NativeAudioExt, UpdateQueueOptions};

use super::changes::LibraryChange;
use super::{err, Library, Result};

const SETTING: &str = "queue_follow";

/// How the queue follows the library (the player's updateQueue options).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QueueFollow {
    pub enabled: bool,
    /// The playing song left the list: "finish" it, or "skip" to the next now.
    pub removed_current: String,
    /// Other songs that left it: "remove" them from the queue, or "keep" them.
    pub removed_items: String,
    /// New songs: "inPlace" (at their place in the list), "end", or "next".
    pub new_items: String,
    /// The queue's order: the "playlist"'s (the list's), or the "queue"'s own.
    pub order: String,
}

impl Default for QueueFollow {
    fn default() -> Self {
        Self {
            enabled: true,
            removed_current: "finish".into(),
            removed_items: "remove".into(),
            new_items: "inPlace".into(),
            order: "playlist".into(),
        }
    }
}

impl QueueFollow {
    fn check(&self) -> Result<()> {
        let one_of = |value: &str, name: &str, allowed: &[&str]| {
            if allowed.contains(&value) { Ok(()) } else { Err(format!("{name} must be one of {}", allowed.join(", "))) }
        };
        one_of(&self.removed_current, "removedCurrent", &["finish", "skip"])?;
        one_of(&self.removed_items, "removedItems", &["remove", "keep"])?;
        one_of(&self.new_items, "newItems", &["inPlace", "end", "next"])?;
        one_of(&self.order, "order", &["playlist", "queue"])
    }

    fn options(&self) -> UpdateQueueOptions {
        UpdateQueueOptions {
            source_id: None,
            removed_current: Some(self.removed_current.clone()),
            removed_items: Some(self.removed_items.clone()),
            new_items: Some(self.new_items.clone()),
            order: Some(self.order.clone()),
        }
    }
}

impl Library {
    pub fn queue_follow(&self) -> Result<QueueFollow> {
        match self.setting(SETTING)? {
            Some(json) => serde_json::from_str(&json).map_err(err),
            None => Ok(QueueFollow::default()),
        }
    }

    pub fn set_queue_follow(&self, settings: &QueueFollow) -> Result<()> {
        settings.check()?;
        self.set_setting(SETTING, &serde_json::to_string(settings).map_err(err)?)
    }
}

/// Whether a change touches the list a queue plays.
pub fn affects(change: &LibraryChange, source: Option<&str>) -> bool {
    source.is_some_and(|source| change.sources.iter().any(|s| s == source))
}

/// After a change: brings the playing queue in line with its list, when the change touched it.
pub fn follow(app: &AppHandle, change: &LibraryChange) {
    let library = app.state::<Library>();
    let settings = match library.queue_follow() {
        Ok(settings) if settings.enabled => settings,
        _ => return,
    };
    let player = app.native_audio();
    let queue = match player.get_queue() {
        Ok(queue) => queue,
        Err(e) => return eprintln!("melue: can't read the queue: {e}"),
    };
    let source = queue.source_id.as_deref();
    if queue.current_index < 0 || !affects(change, source) {
        return;
    }
    // A list that's gone (a deleted playlist) leaves the queue as it is.
    let Ok(items) = library.queue_items(source.unwrap_or_default()) else { return };
    if let Err(e) = player.update_queue(items, settings.options()) {
        eprintln!("melue: can't update the queue: {e}");
    }
}
