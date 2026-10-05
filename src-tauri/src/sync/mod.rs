//! The sync engine (Phase 2): the share's music folder copied one way into the synced folder on this device.
//! The share is the master copy: changes on the device are undone at the next sync, except songs put into the
//! folder by hand (they stay, "only on this device").
//!
//! - plan.rs: what a sync would do (the dry run).
//! - run.rs: doing it, safely (local.rs and, on Android, tree.rs: the synced folder).
//!
//! Only what Melue uses is synced: songs, playlist files and cover images. Melue's own data on the share
//! (synced playback, playlists, likes...) is in its own hidden folder, synced by its own steps.

pub mod commands;
pub mod handoff;
#[cfg(test)]
mod handoff_tests;
pub mod local;
pub mod plan;
#[cfg(test)]
mod plan_tests;
pub mod run;
#[cfg(test)]
mod run_tests;
pub mod schedule;
pub mod scheduler;
#[cfg(target_os = "android")]
mod tree;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::library::{err, now_ms, Library, Result};
use crate::share::Entry;

/// The sync's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncSettings {
    /// Remove the local copies of songs gone from the share (off: they're kept, and listed for a clean-up).
    pub remove_gone: bool,
    /// Ask before a sync removes more than this many songs (0: always ask).
    pub ask_before_removing: u32,
    /// Sync when Melue starts.
    pub at_start: bool,
    /// Sync when the user comes back to the app, if the last sync is at least this many minutes ago (None: no).
    pub on_return_after_minutes: Option<u32>,
    /// Sync when the share says its music changed (SMB change notifications, while Melue runs).
    pub on_change: bool,
    /// Also sync every this many minutes while Melue runs (None: no).
    pub every_minutes: Option<u32>,
    /// Copy songs on metered connections (mobile data, a phone's hotspot). Off: they wait for Wi-Fi; moves,
    /// renames and removals still happen.
    pub copy_on_metered: bool,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            remove_gone: false,
            ask_before_removing: 5,
            at_start: true,
            on_return_after_minutes: Some(15),
            on_change: true,
            every_minutes: None,
            copy_on_metered: false,
        }
    }
}

const SETTINGS: &str = "sync_settings";

/// A file the sync copied: as it was on the share, and as the local copy was right after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub path: String,
    pub share_size: u64,
    pub share_modified_ms: i64,
    pub local_size: u64,
    pub local_modified_ms: i64,
}

/// What the sync copies: what Melue uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    Song,
    Playlist,
    Cover,
}

/// Whether a file is synced, and as what. Hidden files never are (the walks leave them out).
pub fn kind_of(name: &str) -> Option<FileKind> {
    if crate::library::scan::is_audio(name) {
        Some(FileKind::Song)
    } else if crate::library::m3u::is_playlist(name) {
        Some(FileKind::Playlist)
    } else if crate::library::scan::cover_rank(name).is_some() {
        Some(FileKind::Cover)
    } else {
        None
    }
}

impl Library {
    pub fn sync_settings(&self) -> Result<SyncSettings> {
        match self.setting(SETTINGS)? {
            Some(json) => serde_json::from_str(&json).map_err(err),
            None => Ok(SyncSettings::default()),
        }
    }

    pub fn set_sync_settings(&self, settings: &SyncSettings) -> Result<()> {
        self.set_setting(SETTINGS, &serde_json::to_string(settings).map_err(err)?)
    }

    /// Every file synced so far.
    pub fn sync_records(&self) -> Result<Vec<Record>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT path, share_size, share_modified_ms, local_size, local_modified_ms FROM synced_files ORDER BY path")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Record {
                    path: r.get(0)?,
                    share_size: r.get::<_, i64>(1)? as u64,
                    share_modified_ms: r.get(2)?,
                    local_size: r.get::<_, i64>(3)? as u64,
                    local_modified_ms: r.get(4)?,
                })
            })
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Remembers a file as synced: `share` as it is on the share, `local` as the copy is now.
    pub fn record_synced(&self, share: &Entry, local: &Entry) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO synced_files (path, share_size, share_modified_ms, local_size, local_modified_ms, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (path) DO UPDATE SET share_size = ?2, share_modified_ms = ?3, local_size = ?4,
                     local_modified_ms = ?5, synced_at = ?6",
                params![share.path, share.size as i64, share.modified_ms, local.size as i64, local.modified_ms, now_ms()],
            )
            .map(|_| ())
            .map_err(err)
    }

    /// Forgets a synced file (gone from the share and from here, or removed).
    pub fn forget_synced(&self, path: &str) -> Result<()> {
        self.conn().execute("DELETE FROM synced_files WHERE path = ?1", [path]).map(|_| ()).map_err(err)
    }

    /// Forgets every synced file (the share removed, or another share set up).
    pub fn forget_all_synced(&self) -> Result<()> {
        self.conn().execute("DELETE FROM synced_files", []).map(|_| ()).map_err(err)
    }
}
