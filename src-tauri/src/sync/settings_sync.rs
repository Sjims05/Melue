//! Settings across devices: the settings that aren't this device's own (the app decides which: not the ones marked
//! per device, nor ones set to "This device only") are shared through one file on the share, `.melue/settings.json`:
//! each setting's value and when it was last changed. The newest change of each setting wins.
//!
//! The app reads and sets the settings (they live in different places: the app's own storage, Melue's settings, the
//! player's); it tells Melue what changed here (`record`), and Melue keeps a snapshot of them with the times. The
//! library data sync (data.rs) merges the snapshot with the share's file, and the settings another device changed
//! since come back to the app (an event) to set here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::library::{err, now_ms, Library, Result};
use crate::share::{Share, ShareError};

/// The file on the share.
pub const FILE: &str = ".melue/settings.json";
/// The snapshot here (Melue's settings).
const SNAPSHOT: &str = "shared_settings";
pub const EVENT: &str = "shared-settings";

/// A setting's value and when it was last changed (ms; 0: never changed, the device's first snapshot).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shared {
    pub value: Value,
    pub changed_at_ms: i64,
}

pub type Settings = BTreeMap<String, Shared>;

impl Library {
    fn shared_settings(&self) -> Result<Settings> {
        Ok(self.setting(SNAPSHOT)?.and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default())
    }

    fn keep_shared_settings(&self, settings: &Settings) -> Result<()> {
        self.set_setting(SNAPSHOT, &serde_json::to_string(settings).map_err(err)?)
    }

    /// The settings' values as last known here (what the app compares with, to tell what it changed).
    pub fn shared_settings_snapshot(&self) -> Result<BTreeMap<String, Value>> {
        Ok(self.shared_settings()?.into_iter().map(|(id, s)| (id, s.value)).collect())
    }

    /// Settings changed here. `first`: the device's first snapshot (its settings as they were before sharing):
    /// marked as never changed, so another device's choices win over them.
    pub fn record_settings(&self, changes: BTreeMap<String, Value>, first: bool) -> Result<()> {
        let mut settings = self.shared_settings()?;
        let at = if first { 0 } else { now_ms() };
        let ids: Vec<&String> = changes.keys().collect();
        eprintln!("melue: settings: changed here{}: {ids:?}", if first { " (the first snapshot)" } else { "" });
        for (id, value) in changes {
            settings.insert(id, Shared { value, changed_at_ms: at });
        }
        self.keep_shared_settings(&settings)
    }
}

/// Merges the snapshot here with the share's file, newest change of each setting winning. Returns the settings
/// another device changed since (to set here).
pub fn sync_settings(library: &Library, share: &dyn Share) -> Result<BTreeMap<String, Value>> {
    let theirs: Settings = match share.read(FILE) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(ShareError::NotFound { .. }) => Settings::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mine = library.shared_settings()?;
    let mut merged = mine.clone();
    let mut changed_here = BTreeMap::new();
    for (id, their) in &theirs {
        let newer = mine.get(id).is_none_or(|ours| their.changed_at_ms > ours.changed_at_ms);
        if newer {
            if mine.get(id).is_none_or(|ours| ours.value != their.value) {
                changed_here.insert(id.clone(), their.value.clone());
            }
            merged.insert(id.clone(), their.clone());
        }
    }
    if !changed_here.is_empty() {
        eprintln!("melue: settings: from another device: {:?}", changed_here.keys().collect::<Vec<_>>());
    }
    if merged != theirs {
        eprintln!("melue: settings: sent to the share");
        share.create_dir_all(".melue")?;
        share.write(FILE, &serde_json::to_vec(&merged).map_err(err)?)?;
    }
    if merged != mine {
        library.keep_shared_settings(&merged)?;
    }
    Ok(changed_here)
}
