//! Playlists' own covers across devices: the synced ones (Melue's own playlists that sync, and linked playlists
//! whose file is in the synced folder, so every device has them). Each has two files on the share,
//! `.melue/covers/<key>.json` (when it changed, by which device, whether there's a picture) and `<key>.jpg` (the
//! picture). The newest change wins: a new cover or Revert to default cover. Covers of playlists only on this
//! device (or linked to a file in a folder only here) stay here.
//!
//! The key: `p-<sync id>` for Melue's playlists, `f-<hash of the file's path in the synced folder>` for linked ones.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::library::covers::{file_name, LARGE};
use crate::library::{err, Folder, Library, Result};
use crate::share::{Share, ShareError};

/// Where the covers are on the share.
pub const DIR: &str = ".melue/covers";

/// A cover's description on the share.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoverFile {
    /// When it changed, on the share's clock.
    pub changed_at_ms: i64,
    pub changed_by: String,
    /// False: reverted to the default cover.
    pub picture: bool,
}

/// A synced playlist here and its cover.
struct Here {
    id: i64,
    key: String,
    /// When its cover last changed here (this device's clock); None: never.
    changed_at: Option<i64>,
    /// Its cover's file in the covers folder (None: the default).
    cover: Option<String>,
}

impl Library {
    fn synced_covers(&self, folder: &Folder) -> Result<Vec<Here>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare(
                "SELECT p.id, p.kind, p.sync_id, p.file_path, p.cover_changed_at, (SELECT c.file FROM covers c WHERE c.id = p.cover_id)
                 FROM playlists p
                 WHERE (p.kind = 'app' AND p.sync_id IS NOT NULL AND NOT p.local_only)
                    OR (p.kind = 'linked' AND p.file_folder_id = ?1)",
            )
            .map_err(err)?;
        let rows = statement
            .query_map(params![folder.id], |r| {
                let kind: String = r.get(1)?;
                let key = if kind == "app" {
                    format!("p-{}", r.get::<_, String>(2)?)
                } else {
                    let path: String = r.get(3)?;
                    format!("f-{:016x}", xxhash_rust::xxh3::xxh3_64(path.to_lowercase().as_bytes()))
                };
                Ok(Here { id: r.get(0)?, key, changed_at: r.get(4)?, cover: r.get(5)? })
            })
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// A cover from another device: set (or reverted), marked as changed when that device changed it.
    fn take_cover(&self, id: i64, picture: Option<&[u8]>, changed_at: i64) -> Result<()> {
        let cover = match picture {
            Some(bytes) => Some(self.cover_for(bytes)?.ok_or("the share's cover isn't a picture Melue can read")?),
            None => None,
        };
        self.conn()
            .execute("UPDATE playlists SET cover_id = ?2, cover_changed_at = ?3 WHERE id = ?1", params![id, cover, changed_at])
            .map_err(err)?;
        Ok(())
    }
}

/// Syncs the synced playlists' covers with the share (`ahead_ms`: how far the share's clock is ahead of this
/// device's). Returns the playlists whose cover changed here.
pub fn sync_covers(library: &Library, share: &dyn Share, folder: &Folder, me: &str, ahead_ms: i64) -> Result<Vec<i64>> {
    let mut changed = Vec::new();
    for here in library.synced_covers(folder)? {
        let json = format!("{DIR}/{}.json", here.key);
        let theirs: Option<CoverFile> = match share.read(&json) {
            Ok(bytes) => serde_json::from_slice(&bytes).ok(),
            Err(ShareError::NotFound { .. }) => None,
            Err(e) => return Err(e.to_string()),
        };
        let mine = here.changed_at.map(|at| at + ahead_ms);
        match (mine, theirs) {
            // Changed on another device since: theirs comes here.
            (mine, Some(theirs)) if mine.is_none_or(|mine| theirs.changed_at_ms > mine) => {
                let picture = if theirs.picture { Some(share.read(&format!("{DIR}/{}.jpg", here.key)).map_err(|e| e.to_string())?) } else { None };
                library.take_cover(here.id, picture.as_deref(), theirs.changed_at_ms - ahead_ms)?;
                changed.push(here.id);
            }
            // Changed here since: it goes to the share (the picture first, so a device never finds the description
            // without it).
            (Some(mine), theirs) if theirs.as_ref().is_none_or(|theirs| mine > theirs.changed_at_ms) => {
                share.create_dir_all(DIR)?;
                if let Some(cover) = &here.cover {
                    let bytes = std::fs::read(library.covers_dir().join(file_name(cover, LARGE))).map_err(err)?;
                    share.write(&format!("{DIR}/{}.jpg", here.key), &bytes)?;
                }
                let file = CoverFile { changed_at_ms: mine, changed_by: me.to_string(), picture: here.cover.is_some() };
                share.write(&json, &serde_json::to_vec(&file).map_err(err)?)?;
            }
            _ => {}
        }
    }
    Ok(changed)
}
