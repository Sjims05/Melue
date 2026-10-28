//! Playlists across devices: Melue's own playlists and Spotify playlists (not ones linked to a file, and not
//! those set to "Only on this device") are kept on the share, one file each, `.melue/playlists/<id>.json`; every device edits the same
//! one. Each device remembers each playlist as it was at its last sync (the base), so when two devices changed
//! it while they couldn't sync, both sides' changes are merged (playlist_merge.rs). Changes that don't merge
//! (both reordered it, renamed it differently, or one deleted it while the other changed it) wait for the
//! user's choice, per playlist; until then that playlist isn't synced.
//!
//! Deleting a synced playlist deletes it everywhere (a deleted file stays on the share for a while, so devices
//! that were away hear of it); it can be restored for 30 days. "Only on this device" leaves the other devices'
//! copies, or takes it off them too (`unshared`: not kept for restoring there; syncing it again brings it back).
//!  A song a device doesn't have yet stays in the
//! playlist there, greyed, at its place, and plays once it's synced. Copies of the share's songs elsewhere
//! (another music folder) count as the same song (same_song.rs).

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::handoff;
use super::playlist_merge::{decide, Action, ConflictKind, PlaylistData, SyncEntry, Theirs};
use super::same_song::Songs;
use super::shared::with_share;
use crate::library::changes::{self, ChangeReason, LibraryChange};
use crate::library::spotify::SpotifyRef;
use crate::library::{err, now_ms, Folder, Library, Result};
use crate::share::{Share, ShareError};

/// Where the playlist files are on the share.
pub const DIR: &str = ".melue/playlists";
/// The playlists that sync (`p`): Melue's own, and Spotify playlists.
pub const SYNCED: &str = "(p.kind = 'app' OR p.spotify_id IS NOT NULL)";
pub const CONFLICTS_EVENT: &str = "playlist-conflicts";
/// How long a deleted playlist can be restored.
const RESTORABLE_MS: i64 = 30 * 24 * 3600 * 1000;
/// How long a deleted playlist's file stays on the share (for devices that were away).
const DELETED_KEPT_MS: i64 = 90 * 24 * 3600 * 1000;

/// A playlist's file on the share.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaylistFile {
    pub id: String,
    #[serde(flatten)]
    pub data: PlaylistData,
    pub deleted: bool,
    /// Deleted by being kept only on one device: the others let it go without keeping it for restoring.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unshared: bool,
    /// The device that wrote it last.
    pub changed_by: String,
    pub saved_at_ms: i64,
    /// When the share wrote it. Not in the file.
    #[serde(skip)]
    pub share_ms: i64,
}

/// A playlist waiting for the user's choice.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistConflict {
    pub sync_id: String,
    pub kind: ConflictKind,
    pub name: String,
    /// The device whose change it conflicts with.
    pub device_name: String,
    /// The playlist here (None: deleted here).
    pub playlist_id: Option<i64>,
    pub found_at: i64,
}

/// A deleted playlist that can be restored.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedPlaylist {
    pub sync_id: String,
    pub name: String,
    pub songs: usize,
    pub deleted_at: i64,
}

/// What a playlist sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaylistResult {
    /// Playlists here that were made, changed or deleted.
    pub changed: Vec<i64>,
    /// Whether the conflicts waiting changed.
    pub conflicts_changed: bool,
    pub uploaded: usize,
}

/// The user's choice for a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Choice {
    /// This device's version (deleted here: deleted everywhere).
    Mine,
    /// The other device's (deleted there: deleted here too).
    Theirs,
    /// Both (changed on both): this device's becomes a copy, "<name> (<this device>)".
    Both,
}

/// A new id for a playlist on the share.
fn new_sync_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_i64(now_ms());
    hasher.write_u32(std::process::id());
    format!("{:016x}{:012x}", hasher.finish(), now_ms())
}

impl Library {
    /// The synced playlists here (app and Spotify playlists not "Only on this device"): sync id (if synced before)
    /// and id.
    fn synced_playlists(&self) -> Result<Vec<(Option<String>, i64, String)>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!("SELECT sync_id, id, name FROM playlists p WHERE {SYNCED} AND NOT local_only ORDER BY id"))
            .map_err(err)?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// The sync ids of playlists kept only here that are to be taken off the other devices.
    fn unshare_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT sync_id FROM playlists WHERE unshare AND local_only AND sync_id IS NOT NULL").map_err(err)?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// The sync ids of playlists set to "Only on this device" (left alone, also on the share).
    fn local_only_ids(&self) -> Result<HashSet<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT sync_id FROM playlists WHERE local_only AND sync_id IS NOT NULL").map_err(err)?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// A playlist as it's synced: its name, and its songs by key (an entry matched to a copy here keeps the key
    /// it came with; a Spotify playlist's: greyed ones aren't in its version), and a Spotify playlist's reference.
    pub fn playlist_data(&self, id: i64, songs: &Songs) -> Result<PlaylistData> {
        let conn = self.conn();
        let name: String = conn.query_row("SELECT name FROM playlists WHERE id = ?1", [id], |r| r.get(0)).map_err(err)?;
        let spotify = crate::library::spotify::spotify_ref(&conn, id)?;
        let mut stmt = conn
            .prepare(
                "SELECT e.track_id, e.file_entry, coalesce(t.title, e.sync_title), coalesce(t.artist_credit, e.sync_artist),
                        coalesce(t.duration_ms, e.sync_duration_ms), e.chosen
                 FROM playlist_entries e LEFT JOIN tracks t ON t.id = e.track_id
                 WHERE e.playlist_id = ?1 AND e.state IN ('both', 'here_only', 'file_removed') ORDER BY e.position, e.id",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([id], |r| {
                Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<i64>>(4)?, r.get::<_, bool>(5)?))
            })
            .map_err(err)?;
        let mut entries = Vec::new();
        for row in rows {
            let (track, file_entry, title, artist, duration_ms, chosen) = row.map_err(err)?;
            // A song chosen by hand for a Spotify line: which one, for the other devices.
            let chosen = if chosen && file_entry.is_some() { track.map(|t| songs.key_of(t)) } else { None };
            let key = match (track, file_entry) {
                (_, Some(key)) => key,
                (Some(t), None) => songs.key_of(t),
                (None, None) => continue,
            };
            // A Spotify song not here: what it is, from its line.
            let line = crate::library::spotify::from_line(&key);
            let title = title.or_else(|| line.as_ref().map(|s| s.title.clone()));
            let artist = artist.or_else(|| line.as_ref().map(|s| s.artist.clone()));
            let duration_ms = duration_ms.or_else(|| line.as_ref().and_then(|s| s.duration_ms));
            entries.push(SyncEntry { key, title, artist, duration_ms, chosen });
        }
        Ok(PlaylistData { name, entries, spotify })
    }

    /// Puts `data` into a playlist here (`id`), or makes it (None). Songs this device doesn't have stay in it,
    /// greyed. A Spotify playlist follows its reference (spotify.rs); one that stopped following Spotify on another
    /// device stops here too. Returns the playlist's id.
    fn apply_playlist(&self, id: Option<i64>, sync_id: &str, data: &PlaylistData, songs: &Songs) -> Result<i64> {
        let following: bool = match id {
            Some(id) => self.conn().query_row("SELECT spotify_id IS NOT NULL FROM playlists WHERE id = ?1", [id], |r| r.get(0)).map_err(err)?,
            None => false,
        };
        if let Some(reference) = &data.spotify {
            return self.apply_spotify_playlist(id, sync_id, data, reference, songs);
        }
        if let (Some(id), true) = (id, following) {
            self.unlink_playlist(id)?;
        }
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let id = match id {
            Some(id) => {
                tx.execute("UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1", params![id, data.name, now]).map_err(err)?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO playlists (name, kind, created_at, updated_at, sync_id) VALUES (?1, 'app', ?2, ?2, ?3)",
                    params![data.name, now, sync_id],
                )
                .map_err(err)?;
                tx.last_insert_rowid()
            }
        };
        let rows: Vec<NewEntry> = data
            .entries
            .iter()
            .map(|entry| {
                // The song itself, a copy of it here (keeping the key it came with), or not here (greyed).
                let (track, file_entry, shown) = match songs.find(entry) {
                    Some((track, true)) => (Some(track), None, None),
                    Some((track, false)) => (Some(track), Some(entry.key.clone()), None),
                    None => (None, Some(entry.key.clone()), Some(entry)),
                };
                NewEntry { state: "both", track, file_entry, shown, chosen: false }
            })
            .collect();
        write_entries(&tx, id, &rows, songs)?;
        tx.commit().map_err(err)?;
        Ok(id)
    }

    fn apply_spotify_playlist(&self, id: Option<i64>, sync_id: &str, data: &PlaylistData, reference: &SpotifyRef, songs: &Songs) -> Result<i64> {
        let index = self.track_index()?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let id = match id {
            Some(id) => {
                tx.execute("UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1", params![id, data.name, now]).map_err(err)?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO playlists (name, kind, created_at, updated_at, sync_id, file_path) VALUES (?1, 'linked', ?2, ?2, ?3, ?4)",
                    params![data.name, now, sync_id, format!("spotify:{}", reference.id)],
                )
                .map_err(err)?;
                tx.last_insert_rowid()
            }
        };
        crate::library::spotify::apply_synced(&tx, id, &data.entries, reference, songs, &index)?;
        tx.commit().map_err(err)?;
        Ok(id)
    }

    fn sync_base(&self, sync_id: &str) -> Result<Option<PlaylistData>> {
        let json: Option<String> =
            self.conn().query_row("SELECT data FROM playlist_sync_base WHERE sync_id = ?1", [sync_id], |r| r.get(0)).optional().map_err(err)?;
        json.map(|j| serde_json::from_str(&j).map_err(err)).transpose()
    }

    fn set_sync_base(&self, sync_id: &str, data: Option<&PlaylistData>) -> Result<()> {
        let conn = self.conn();
        match data {
            Some(data) => conn.execute(
                "INSERT INTO playlist_sync_base (sync_id, data) VALUES (?1, ?2) ON CONFLICT DO UPDATE SET data = ?2",
                params![sync_id, serde_json::to_string(data).map_err(err)?],
            ),
            None => conn.execute("DELETE FROM playlist_sync_base WHERE sync_id = ?1", [sync_id]),
        }
        .map(|_| ())
        .map_err(err)
    }

    fn base_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT sync_id FROM playlist_sync_base").map_err(err)?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Keeps a deleted synced playlist for restoring (30 days).
    /// Keeps a deleted playlist for restoring. One with no songs isn't kept: there's nothing to get back.
    fn archive_playlist(&self, sync_id: &str, data: &PlaylistData) -> Result<()> {
        if data.entries.is_empty() {
            return self.conn().execute("DELETE FROM deleted_playlists WHERE sync_id = ?1", [sync_id]).map(|_| ()).map_err(err);
        }
        self.conn()
            .execute(
                "INSERT INTO deleted_playlists (sync_id, name, data, deleted_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET name = ?2, data = ?3, deleted_at = ?4",
                params![sync_id, data.name, serde_json::to_string(data).map_err(err)?, now_ms()],
            )
            .map(|_| ())
            .map_err(err)
    }

    /// Before deleting a playlist here: a synced one is kept for restoring (and deleted everywhere at the next
    /// sync).
    pub fn archive_before_delete(&self, id: i64) -> Result<()> {
        let sync_id: Option<String> = self
            .conn()
            .query_row(&format!("SELECT sync_id FROM playlists p WHERE id = ?1 AND {SYNCED} AND NOT local_only"), [id], |r| r.get(0))
            .optional()
            .map_err(err)?
            .flatten();
        let (Some(sync_id), Some(folder)) = (sync_id, self.synced_folder()?) else { return Ok(()) };
        let songs = self.songs_here(&folder, &handoff::device_id(self)?)?;
        let data = self.playlist_data(id, &songs)?;
        self.archive_playlist(&sync_id, &data)
    }

    /// Deleted playlists that can be restored, latest first.
    pub fn deleted_playlists(&self) -> Result<Vec<DeletedPlaylist>> {
        let conn = self.conn();
        conn.execute("DELETE FROM deleted_playlists WHERE deleted_at < ?1", [now_ms() - RESTORABLE_MS]).map_err(err)?;
        let mut stmt = conn.prepare("SELECT sync_id, name, data, deleted_at FROM deleted_playlists ORDER BY deleted_at DESC").map_err(err)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)))
            .map_err(err)?;
        let mut deleted = Vec::new();
        for row in rows {
            let (sync_id, name, data, deleted_at) = row.map_err(err)?;
            let songs = serde_json::from_str::<PlaylistData>(&data).map_or(0, |d| d.entries.len());
            // Kept before empty ones stopped being kept: not offered.
            if songs > 0 {
                deleted.push(DeletedPlaylist { sync_id, name, songs, deleted_at });
            }
        }
        Ok(deleted)
    }

    /// Restores a deleted playlist (it's synced again: restored everywhere). Returns its new id here.
    pub fn restore_playlist(&self, sync_id: &str) -> Result<i64> {
        let json: String = self
            .conn()
            .query_row("SELECT data FROM deleted_playlists WHERE sync_id = ?1", [sync_id], |r| r.get(0))
            .optional()
            .map_err(err)?
            .ok_or("that playlist can't be restored any more")?;
        let data: PlaylistData = serde_json::from_str(&json).map_err(err)?;
        let folder = self.synced_folder()?.ok_or("no share is set up")?;
        let songs = self.songs_here(&folder, &handoff::device_id(self)?)?;
        let id = self.apply_playlist(None, sync_id, &data, &songs)?;
        // Made here anew: the next sync puts it on the share over the deletion.
        self.set_sync_base(sync_id, None)?;
        self.conn().execute("DELETE FROM deleted_playlists WHERE sync_id = ?1", [sync_id]).map_err(err)?;
        self.conn().execute("DELETE FROM playlist_conflicts WHERE sync_id = ?1", [sync_id]).map_err(err)?;
        Ok(id)
    }

    /// "Only on this device": the playlist isn't synced any more (the other devices keep theirs, or with
    /// `remove_elsewhere` it's taken off them at the next sync), or is again.
    pub fn set_playlist_local_only(&self, id: i64, local_only: bool, remove_elsewhere: bool) -> Result<()> {
        let changed = self
            .conn()
            .execute(
                "UPDATE playlists AS p SET local_only = ?2, unshare = ?3 WHERE id = ?1 AND (p.kind = 'app' OR p.spotify_id IS NOT NULL)",
                params![id, local_only, local_only && remove_elsewhere],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err("only Melue's own playlists are synced".into());
        }
        Ok(())
    }

    /// Playlists waiting for the user's choice.
    pub fn playlist_conflicts(&self) -> Result<Vec<PlaylistConflict>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT c.sync_id, c.kind, c.device_name, c.found_at, c.theirs, p.id, p.name, d.name, b.data
                 FROM playlist_conflicts c LEFT JOIN playlists p ON p.sync_id = c.sync_id
                 LEFT JOIN deleted_playlists d ON d.sync_id = c.sync_id
                 LEFT JOIN playlist_sync_base b ON b.sync_id = c.sync_id
                 ORDER BY c.found_at",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                let theirs: Option<String> = r.get(4)?;
                let base: Option<String> = r.get(8)?;
                let named = |json: Option<String>| json.and_then(|j| serde_json::from_str::<PlaylistData>(&j).ok()).map(|d| d.name);
                let name = r.get::<_, Option<String>>(6)?.or(r.get(7)?).or_else(|| named(theirs)).or_else(|| named(base));
                Ok(PlaylistConflict {
                    sync_id: r.get(0)?,
                    kind: ConflictKind::parse(&r.get::<_, String>(1)?).unwrap_or(ConflictKind::Changed),
                    device_name: r.get(2)?,
                    found_at: r.get(3)?,
                    playlist_id: r.get(5)?,
                    name: name.unwrap_or_default(),
                })
            })
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    fn conflict_ids(&self) -> Result<HashSet<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT sync_id FROM playlist_conflicts").map_err(err)?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    fn add_conflict(&self, sync_id: &str, kind: ConflictKind, theirs: Option<&PlaylistData>, device_name: &str) -> Result<()> {
        let theirs = theirs.map(serde_json::to_string).transpose().map_err(err)?;
        self.conn()
            .execute(
                "INSERT INTO playlist_conflicts (sync_id, kind, theirs, device_name, found_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![sync_id, kind.as_str(), theirs, device_name, now_ms()],
            )
            .map(|_| ())
            .map_err(err)
    }

    /// Songs that were greyed in synced playlists (not on this device yet) and are here now: they play.
    fn fill_in_arrived(&self, songs: &Songs) -> Result<Vec<i64>> {
        let waiting: Vec<(i64, i64, SyncEntry)> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare(
                    "SELECT e.id, e.playlist_id, e.file_entry, e.sync_title, e.sync_artist, e.sync_duration_ms
                     FROM playlist_entries e JOIN playlists p ON p.id = e.playlist_id
                     WHERE (p.kind = 'app' OR (p.spotify_id IS NOT NULL AND e.state = 'here_only')) AND e.track_id IS NULL
                       AND e.file_entry IS NOT NULL",
                )
                .map_err(err)?;
            let rows = stmt
                .query_map([], |r| {
                    let entry = SyncEntry { key: r.get(2)?, title: r.get(3)?, artist: r.get(4)?, duration_ms: r.get(5)?, chosen: None };
                    Ok((r.get(0)?, r.get(1)?, entry))
                })
                .map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let mut changed = Vec::new();
        for (entry, playlist, wanted) in waiting {
            // The song itself (then known by itself), or a copy here (keeping the key it came with).
            let Some((track, itself)) = songs.find(&wanted) else { continue };
            self.conn()
                .execute(
                    "UPDATE playlist_entries SET track_id = ?2, file_entry = CASE WHEN ?3 THEN NULL ELSE file_entry END,
                         sync_title = NULL, sync_artist = NULL, sync_duration_ms = NULL
                     WHERE id = ?1",
                    params![entry, track, itself],
                )
                .map_err(err)?;
            changed.push(playlist);
        }
        changed.dedup();
        Ok(changed)
    }

    /// Deletes a playlist here because it was deleted elsewhere (kept for restoring).
    fn delete_synced_here(&self, id: i64, sync_id: &str, data: &PlaylistData) -> Result<()> {
        self.archive_playlist(sync_id, data)?;
        self.conn().execute("DELETE FROM playlists WHERE id = ?1", [id]).map_err(err)?;
        Ok(())
    }
}

/// A playlist entry as a sync writes it: its state, its song here, its key when it isn't the song's own (a copy, a
/// song not here, a Spotify line), and what it shows when the song isn't here.
pub(crate) struct NewEntry<'a> {
    pub state: &'static str,
    pub track: Option<i64>,
    pub file_entry: Option<String>,
    pub shown: Option<&'a SyncEntry>,
    /// Its song chosen by hand (Find song).
    pub chosen: bool,
}

/// Writes a synced playlist's entries as `rows`, in order. The entries of songs that stay are kept (their ids
/// too): the app's rows for them stay, so the songs added or removed on another device unfold or fold away and
/// the others stay put.
pub(crate) fn write_entries(tx: &Transaction, id: i64, rows: &[NewEntry], songs: &Songs) -> Result<()> {
    let key = |track: Option<i64>, file_entry: &Option<String>| file_entry.clone().or_else(|| track.map(|t| songs.key_of(t)));
    let mut free: HashMap<Option<String>, VecDeque<i64>> = HashMap::new();
    {
        let mut stmt = tx.prepare("SELECT id, track_id, file_entry FROM playlist_entries WHERE playlist_id = ?1 ORDER BY position, id").map_err(err)?;
        let existing = stmt.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<String>>(2)?))).map_err(err)?;
        for row in existing {
            let (entry, track, file_entry) = row.map_err(err)?;
            free.entry(key(track, &file_entry)).or_default().push_back(entry);
        }
    }
    let now = now_ms();
    for (position, row) in rows.iter().enumerate() {
        let title = row.shown.and_then(|e| e.title.as_ref());
        let artist = row.shown.and_then(|e| e.artist.as_ref());
        let duration = row.shown.and_then(|e| e.duration_ms);
        match free.get_mut(&key(row.track, &row.file_entry)).and_then(VecDeque::pop_front) {
            Some(entry) => tx.execute(
                "UPDATE playlist_entries SET position = ?2, track_id = ?3, file_entry = ?4, state = ?5, sync_title = ?6, sync_artist = ?7,
                     sync_duration_ms = ?8, chosen = ?9
                 WHERE id = ?1",
                params![entry, position as i64, row.track, row.file_entry, row.state, title, artist, duration, row.chosen],
            ),
            None => tx.execute(
                "INSERT INTO playlist_entries
                     (playlist_id, position, track_id, file_entry, state, added_at, sync_title, sync_artist, sync_duration_ms, chosen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![id, position as i64, row.track, row.file_entry, row.state, now, title, artist, duration, row.chosen],
            ),
        }
        .map_err(err)?;
    }
    for entry in free.into_values().flatten() {
        tx.execute("DELETE FROM playlist_entries WHERE id = ?1", [entry]).map_err(err)?;
    }
    Ok(())
}

/// Every playlist file on the share, by id (unreadable ones left out).
fn read_playlist_files(share: &dyn Share) -> Result<HashMap<String, PlaylistFile>> {
    let entries = match share.list(DIR) {
        Ok(entries) => entries,
        Err(ShareError::NotFound { .. }) => return Ok(HashMap::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(entries
        .iter()
        .filter(|e| !e.is_dir && e.name.ends_with(".json"))
        .filter_map(|e| {
            let file: PlaylistFile = serde_json::from_slice(&share.read(&e.path).ok()?).ok()?;
            Some((file.id.clone(), PlaylistFile { share_ms: e.modified_ms, ..file }))
        })
        .collect())
}

fn write_playlist_file(share: &dyn Share, sync_id: &str, data: Option<&PlaylistData>, name: &str) -> Result<()> {
    write_file(share, sync_id, data, false, name)
}

/// A playlist kept only on this device and taken off the others: deleted on the share, `unshared`.
fn write_unshared_file(share: &dyn Share, sync_id: &str, name: &str) -> Result<()> {
    write_file(share, sync_id, None, true, name)
}

fn write_file(share: &dyn Share, sync_id: &str, data: Option<&PlaylistData>, unshared: bool, name: &str) -> Result<()> {
    let file = PlaylistFile {
        id: sync_id.into(),
        data: data.cloned().unwrap_or_default(),
        deleted: data.is_none(),
        unshared,
        changed_by: name.into(),
        saved_at_ms: now_ms(),
        share_ms: 0,
    };
    share.create_dir_all(DIR)?;
    Ok(share.write(&format!("{DIR}/{sync_id}.json"), &serde_json::to_vec(&file).map_err(err)?)?)
}

/// Syncs the playlists with the share. `me` and `name`: this device; `ahead_ms`: how far the share's clock is
/// ahead of this device's.
pub fn sync_playlists(library: &Library, share: &dyn Share, folder: &Folder, me: &str, name: &str, ahead_ms: i64) -> Result<PlaylistResult> {
    let mut result = PlaylistResult::default();
    let mut files = read_playlist_files(share)?;
    let songs = library.songs_here(folder, me)?;
    let waiting = library.conflict_ids()?;
    let local_only = library.local_only_ids()?;
    result.changed.extend(library.fill_in_arrived(&songs)?);

    // Kept only here and taken off the other devices: off the share. Synced again later, it goes up as new.
    for sync_id in library.unshare_ids()? {
        if files.get(&sync_id).is_some_and(|f| !f.deleted) {
            write_unshared_file(share, &sync_id, name)?;
        }
        library.set_sync_base(&sync_id, None)?;
        library.conn().execute("UPDATE playlists SET unshare = 0 WHERE sync_id = ?1", [&sync_id]).map_err(err)?;
    }

    // Playlists here, by sync id. Ones never synced get an id: the share's playlist of the same name when there
    // is one (made on two devices before syncing: merged), or a new one.
    let mut here: HashMap<String, i64> = HashMap::new();
    let mut unsynced = Vec::new();
    for (sync_id, id, playlist_name) in library.synced_playlists()? {
        match sync_id {
            Some(s) => {
                here.insert(s, id);
            }
            None => unsynced.push((id, playlist_name)),
        }
    }
    for (id, playlist_name) in unsynced {
        let same_name = files
            .values()
            .find(|f| !f.deleted && !here.contains_key(&f.id) && f.data.name.eq_ignore_ascii_case(&playlist_name))
            .map(|f| f.id.clone());
        let sync_id = same_name.unwrap_or_else(new_sync_id);
        library.conn().execute("UPDATE playlists SET sync_id = ?2 WHERE id = ?1", params![id, sync_id]).map_err(err)?;
        here.insert(sync_id, id);
    }

    let mut ids: HashSet<String> = here.keys().cloned().collect();
    ids.extend(files.keys().cloned());
    ids.extend(library.base_ids()?);
    let mut ids: Vec<String> = ids.into_iter().filter(|s| !waiting.contains(s) && !local_only.contains(s)).collect();
    ids.sort();
    for sync_id in ids {
        let file = files.remove(&sync_id);
        let theirs = match &file {
            None => Theirs::Missing,
            Some(f) if f.deleted => Theirs::Deleted,
            Some(f) => Theirs::Live(f.data.clone()),
        };
        let changed_by = file.as_ref().map_or_else(String::new, |f| f.changed_by.clone());
        let unshared = file.as_ref().is_some_and(|f| f.unshared);
        let base = library.sync_base(&sync_id)?;
        let local_id = here.get(&sync_id).copied();
        let mine = local_id.map(|id| library.playlist_data(id, &songs)).transpose()?;
        match decide(base.as_ref(), mine.as_ref(), &theirs) {
            Action::Nothing => {}
            Action::Use { result: data, here: to_here, share: to_share } => {
                if to_here {
                    result.changed.push(library.apply_playlist(local_id, &sync_id, &data, &songs)?);
                }
                if to_share {
                    write_playlist_file(share, &sync_id, Some(&data), name)?;
                    result.uploaded += 1;
                }
                library.set_sync_base(&sync_id, Some(&data))?;
            }
            Action::DeleteHere => {
                if let (Some(id), Some(data)) = (local_id, &mine) {
                    if unshared {
                        library.conn().execute("DELETE FROM playlists WHERE id = ?1", [id]).map_err(err)?;
                    } else {
                        library.delete_synced_here(id, &sync_id, data)?;
                    }
                    result.changed.push(id);
                }
                library.set_sync_base(&sync_id, None)?;
            }
            Action::DeleteOnShare => {
                write_playlist_file(share, &sync_id, None, name)?;
                library.set_sync_base(&sync_id, None)?;
            }
            Action::Forget => library.set_sync_base(&sync_id, None)?,
            Action::Conflict(kind) => {
                let theirs = match &theirs {
                    Theirs::Live(data) => Some(data),
                    _ => None,
                };
                library.add_conflict(&sync_id, kind, theirs, &changed_by)?;
                result.conflicts_changed = true;
            }
        }
    }

    // Deleted playlists' files go after a while (devices away longer get it as a playlist missing here).
    let now_on_share = now_ms() + ahead_ms;
    for file in files.values().filter(|f| f.deleted && now_on_share - f.share_ms > DELETED_KEPT_MS) {
        let _ = share.delete(&format!("{DIR}/{}.json", file.id));
    }
    Ok(result)
}

/// The user's choice for a conflict: applied now (with the share).
pub fn answer_conflict(library: &Library, share: &dyn Share, folder: &Folder, me: &str, name: &str, sync_id: &str, choice: Choice) -> Result<Vec<i64>> {
    let (kind, theirs): (String, Option<String>) = library
        .conn()
        .query_row("SELECT kind, theirs FROM playlist_conflicts WHERE sync_id = ?1", [sync_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(err)?
        .ok_or("that conflict was answered already")?;
    let kind = ConflictKind::parse(&kind).ok_or("not a conflict")?;
    let theirs: Option<PlaylistData> = theirs.map(|t| serde_json::from_str(&t).map_err(err)).transpose()?;
    let songs = library.songs_here(folder, me)?;
    let local_id: Option<i64> = library
        .conn()
        .query_row("SELECT id FROM playlists WHERE sync_id = ?1", [sync_id], |r| r.get(0))
        .optional()
        .map_err(err)?;
    let mine = local_id.map(|id| library.playlist_data(id, &songs)).transpose()?;
    let mut changed = Vec::new();
    match (kind, choice) {
        // Changed on both / deleted there, changed here: this device's version goes everywhere.
        (ConflictKind::Changed | ConflictKind::DeletedThere, Choice::Mine) => {
            let mine = mine.ok_or("the playlist isn't here any more")?;
            write_playlist_file(share, sync_id, Some(&mine), name)?;
            library.set_sync_base(sync_id, Some(&mine))?;
        }
        (ConflictKind::Changed, Choice::Theirs | Choice::Both) => {
            let theirs = theirs.ok_or("the other version isn't known")?;
            if choice == Choice::Both {
                // This device's version as a copy (synced as a new playlist).
                if let Some(mine) = &mine {
                    // Its own playlist (not following Spotify: one playlist follows it).
                    let copy = PlaylistData { name: format!("{} ({name})", mine.name), entries: mine.entries.clone(), spotify: None };
                    let copy_id = library.apply_playlist(None, &new_sync_id(), &copy, &songs)?;
                    changed.push(copy_id);
                }
            }
            changed.push(library.apply_playlist(local_id, sync_id, &theirs, &songs)?);
            library.set_sync_base(sync_id, Some(&theirs))?;
        }
        // Deleted there, changed here: deleted here too (restorable).
        (ConflictKind::DeletedThere, _) => {
            if let (Some(id), Some(mine)) = (local_id, &mine) {
                library.delete_synced_here(id, sync_id, mine)?;
                changed.push(id);
            }
            library.set_sync_base(sync_id, None)?;
        }
        // Deleted here, changed there: deleted everywhere.
        (ConflictKind::DeletedHere, Choice::Mine) => {
            write_playlist_file(share, sync_id, None, name)?;
            library.set_sync_base(sync_id, None)?;
        }
        // Deleted here, changed there: brought back with their changes.
        (ConflictKind::DeletedHere, _) => {
            let theirs = theirs.ok_or("the other version isn't known")?;
            changed.push(library.apply_playlist(local_id, sync_id, &theirs, &songs)?);
            library.set_sync_base(sync_id, Some(&theirs))?;
            library.conn().execute("DELETE FROM deleted_playlists WHERE sync_id = ?1", [sync_id]).map_err(err)?;
        }
    }
    library.conn().execute("DELETE FROM playlist_conflicts WHERE sync_id = ?1", [sync_id]).map_err(err)?;
    Ok(changed)
}

/// Tells the UI the conflicts waiting (the `playlist-conflicts` event).
pub fn emit_conflicts(app: &AppHandle) {
    if let Ok(conflicts) = app.state::<Library>().playlist_conflicts() {
        let _ = app.emit(CONFLICTS_EVENT, &conflicts);
    }
}

/// Answers a conflict (with the share), and tells the UI.
pub fn answer(app: &AppHandle, sync_id: &str, choice: Choice) -> Result<()> {
    let library = app.state::<Library>();
    let folder = library.synced_folder()?.ok_or("no share is set up")?;
    let me = handoff::device_id(&library)?;
    let name = handoff::settings(app)?.device_name.unwrap_or_default();
    let changed = with_share(app, |share| answer_conflict(&library, share, &folder, &me, &name, sync_id, choice))?;
    if !changed.is_empty() {
        changes::emit(app, LibraryChange::playlists(ChangeReason::Playlists, changed));
    }
    emit_conflicts(app);
    Ok(())
}
