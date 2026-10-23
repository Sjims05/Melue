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

/// Whether `their` setting wins over `ours`: the newer change; changed at the same time (both never changed: each
/// device's first snapshot), the same value wins on every device (the larger, as JSON), so they settle on one instead
/// of each writing its own back to the share, over and over.
fn wins(their: &Shared, ours: &Shared) -> bool {
    their.changed_at_ms > ours.changed_at_ms
        || (their.changed_at_ms == ours.changed_at_ms && their.value.to_string() > ours.value.to_string())
}

/// The file on the share.
pub const FILE: &str = ".melue/settings.json";
/// The snapshot here (Melue's settings).
const SNAPSHOT: &str = "shared_settings";
pub const EVENT: &str = "shared-settings";
/// After the first merge since Melue started: all the shared settings (the app makes sure it has every one of them).
pub const CHECK_EVENT: &str = "shared-settings-check";

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
        let newer = mine.get(id).is_none_or(|ours| wins(their, ours));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testkit::TestLibrary;
    use crate::share::testkit::TestShare;

    fn never_changed(value: Value) -> Shared {
        Shared { value, changed_at_ms: 0 }
    }

    #[test]
    fn two_devices_with_different_untouched_settings_settle_on_one() {
        let share = TestShare::new("settings-tie");
        let (pc, phone) = (TestLibrary::new("settings-tie-pc"), TestLibrary::new("settings-tie-phone"));
        pc.library.keep_shared_settings(&Settings::from([("playlists.topSize".into(), never_changed(100.into()))])).unwrap();
        phone.library.keep_shared_settings(&Settings::from([("playlists.topSize".into(), never_changed(50.into()))])).unwrap();
        for _ in 0..3 {
            sync_settings(&pc.library, &share.share).unwrap();
            sync_settings(&phone.library, &share.share).unwrap();
        }
        let (on_pc, on_phone) = (pc.library.shared_settings().unwrap(), phone.library.shared_settings().unwrap());
        assert_eq!(on_pc, on_phone, "one value on both");
        // Settled: another round writes nothing (the file stays as it is).
        let before = share.share.stat(FILE).unwrap().unwrap().modified_ms;
        std::thread::sleep(std::time::Duration::from_millis(20));
        sync_settings(&pc.library, &share.share).unwrap();
        sync_settings(&phone.library, &share.share).unwrap();
        assert_eq!(share.share.stat(FILE).unwrap().unwrap().modified_ms, before, "nothing written again");
    }
}
