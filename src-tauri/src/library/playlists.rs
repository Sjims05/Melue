//! Playlists: made in Melue ("app"), or playlist files linked into the library ("linked", see linked.rs).
//! A name and songs in order; the same song can be in a playlist more than once (each entry has its own
//! id). Songs whose files are missing stay in their playlists (unavailable, shown greyed) and play again
//! when the file is back; songs that leave the library (their folder removed) leave app playlists (a linked
//! playlist keeps its file's line, as "not found").
//!
//! Editing a linked playlist edits Melue's version: added songs are "here only", removed songs from the
//! file stay greyed ("removed here", restorable). With write-back on, the caller then writes the file.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;

use super::grouping::file_name_of;
use super::tracks::{track_row, Track, TRACK_COLUMNS, TRACK_COLUMN_COUNT};
use super::{err, now_ms, Library, Result};

/// How many covers a playlist's picture is made of (the UI shows them as a grid).
const PLAYLIST_COVERS: usize = 4;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    /// "app": made in Melue. "linked": a playlist file.
    pub kind: String,
    /// Its songs in Melue's version, available or not (not counting greyed ones: removed here, suggested).
    pub entries: i64,
    /// Those that can be played.
    pub available: i64,
    /// Linked: songs new in the file, waiting to be added.
    pub suggested: i64,
    /// Linked: songs removed in Melue that the file still has.
    pub removed_here: i64,
    /// Linked: the file's name, whether it's gone, and the playlist's settings (see linked.rs).
    pub file: Option<String>,
    pub file_missing: bool,
    pub new_in_file: String,
    pub removed_from_file: String,
    pub write_back: bool,
    /// How long its available songs play.
    pub duration_ms: i64,
    /// The first different covers of its songs, in order, for its picture.
    pub covers: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A song in a playlist.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    /// The entry's own id (the same song twice is two entries).
    pub id: i64,
    pub position: i64,
    pub added_at: i64,
    /// "both" (app playlists: always), "here_only", "removed_here", "suggested" or "file_removed".
    pub state: String,
    /// Linked: its line in the file.
    pub file_entry: Option<String>,
    /// None: a line of the file that matches no song in the library ("not found").
    pub track: Option<Track>,
}

/// States of entries in Melue's version of a playlist (the others are shown greyed).
const IN_MELUE: &str = "('both', 'here_only', 'file_removed')";

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a playlist needs a name".into());
    }
    Ok(name.to_string())
}

/// Whether the playlist is linked to a file (refusing ones that aren't there).
fn is_linked(tx: &Transaction, id: i64) -> Result<bool> {
    let kind: Option<String> =
        tx.query_row("SELECT kind FROM playlists WHERE id = ?1", [id], |r| r.get(0)).optional().map_err(err)?;
    kind.map(|k| k == "linked").ok_or_else(|| format!("no playlist {id}"))
}

/// The playlist's entry ids in order.
fn entry_ids(tx: &Transaction, id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare("SELECT id FROM playlist_entries WHERE playlist_id = ?1 ORDER BY position, id").map_err(err)?;
    let ids = stmt.query_map([id], |r| r.get(0)).map_err(err)?;
    ids.collect::<rusqlite::Result<_>>().map_err(err)
}

/// Gives the entries positions 0, 1, 2... in the order of `ids`, writing only those that moved.
fn renumber(tx: &Transaction, playlist: i64, ids: &[i64]) -> Result<()> {
    let mut stmt = tx
        .prepare("UPDATE playlist_entries SET position = ?2 WHERE id = ?1 AND position != ?2")
        .map_err(err)?;
    for (position, id) in ids.iter().enumerate() {
        stmt.execute(params![id, position as i64]).map_err(err)?;
    }
    tx.execute("UPDATE playlists SET updated_at = ?2 WHERE id = ?1", params![playlist, now_ms()]).map_err(err)?;
    Ok(())
}

impl Library {
    pub fn create_playlist(&self, name: &str) -> Result<Playlist> {
        let name = clean_name(name)?;
        let now = now_ms();
        let id = {
            let conn = self.conn();
            conn.execute("INSERT INTO playlists (name, kind, created_at, updated_at) VALUES (?1, 'app', ?2, ?2)", params![name, now])
                .map_err(err)?;
            conn.last_insert_rowid()
        };
        self.playlist(id)
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<Playlist> {
        let name = clean_name(name)?;
        let changed = self
            .conn()
            .execute("UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1", params![id, name, now_ms()])
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("no playlist {id}"));
        }
        self.playlist(id)
    }

    /// Deletes a playlist (its songs stay in the library). A linked playlist's file is left alone, and not
    /// linked again.
    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        super::linked::ignore_file(&tx, id)?;
        let deleted = tx.execute("DELETE FROM playlists WHERE id = ?1", [id]).map_err(err)?;
        if deleted == 0 {
            return Err(format!("no playlist {id}"));
        }
        tx.commit().map_err(err)
    }

    /// Every playlist, by name.
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let ids: Vec<i64> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT id FROM playlists ORDER BY name COLLATE NOCASE, id").map_err(err)?;
            let ids = stmt.query_map([], |r| r.get(0)).map_err(err)?;
            ids.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        ids.into_iter().map(|id| self.playlist(id)).collect()
    }

    pub fn playlist(&self, id: i64) -> Result<Playlist> {
        let conn = self.conn();
        let sql = format!(
            "SELECT p.id, p.name, p.kind, p.created_at, p.updated_at,
                    count(e.id) FILTER (WHERE e.state IN {IN_MELUE}),
                    count(t.id) FILTER (WHERE e.state IN {IN_MELUE}),
                    coalesce(sum(t.duration_ms) FILTER (WHERE e.state IN {IN_MELUE}), 0),
                    count(e.id) FILTER (WHERE e.state = 'suggested'),
                    count(e.id) FILTER (WHERE e.state = 'removed_here'),
                    p.file_path, p.file_missing_since IS NOT NULL, p.new_in_file, p.removed_from_file, p.write_back
             FROM playlists p
             LEFT JOIN playlist_entries e ON e.playlist_id = p.id
             LEFT JOIN tracks t ON t.id = e.track_id AND t.missing_since IS NULL
                  AND t.folder_id IN (SELECT id FROM folders WHERE enabled)
             WHERE p.id = ?1 GROUP BY p.id"
        );
        let mut playlist = conn
            .query_row(&sql, [id], |r| {
                Ok(Playlist {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    kind: r.get(2)?,
                    created_at: r.get(3)?,
                    updated_at: r.get(4)?,
                    entries: r.get(5)?,
                    available: r.get(6)?,
                    duration_ms: r.get(7)?,
                    suggested: r.get(8)?,
                    removed_here: r.get(9)?,
                    file: r.get::<_, Option<String>>(10)?.map(|path| file_name_of(&path)),
                    file_missing: r.get(11)?,
                    new_in_file: r.get(12)?,
                    removed_from_file: r.get(13)?,
                    write_back: r.get(14)?,
                    covers: Vec::new(),
                })
            })
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("no playlist {id}"))?;
        let sql = format!(
            "SELECT c.file FROM playlist_entries e JOIN tracks t ON t.id = e.track_id JOIN covers c ON c.id = t.cover_id
             WHERE e.playlist_id = ?1 AND t.missing_since IS NULL AND e.state IN {IN_MELUE} ORDER BY e.position, e.id"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        for cover in stmt.query_map([id], |r| r.get::<_, String>(0)).map_err(err)? {
            let cover = cover.map_err(err)?;
            if !playlist.covers.contains(&cover) {
                playlist.covers.push(cover);
                if playlist.covers.len() == PLAYLIST_COVERS {
                    break;
                }
            }
        }
        Ok(playlist)
    }

    /// A playlist's entries in order: its songs, available or not, and (linked) greyed ones and lines of the
    /// file that match no song.
    pub fn playlist_entries(&self, id: i64) -> Result<Vec<PlaylistEntry>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS}, e.id, e.position, e.added_at, e.state, e.file_entry
             FROM playlist_entries e LEFT JOIN tracks t ON t.id = e.track_id LEFT JOIN folders f ON f.id = t.folder_id
             LEFT JOIN covers c ON c.id = t.cover_id
             WHERE e.playlist_id = ?1 ORDER BY e.position, e.id"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let entries = stmt
            .query_map([id], |r| {
                let track = if r.get::<_, Option<i64>>(0)?.is_some() { Some(track_row(r)?) } else { None };
                Ok(PlaylistEntry {
                    track,
                    id: r.get(TRACK_COLUMN_COUNT)?,
                    position: r.get(TRACK_COLUMN_COUNT + 1)?,
                    added_at: r.get(TRACK_COLUMN_COUNT + 2)?,
                    state: r.get(TRACK_COLUMN_COUNT + 3)?,
                    file_entry: r.get(TRACK_COLUMN_COUNT + 4)?,
                })
            })
            .map_err(err)?;
        entries.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Adds songs to a playlist, in order, at `position` (from 0; the end when left out or past it). In a
    /// linked playlist they're "here only" (not in the file). Returns the new entries' ids.
    pub fn add_to_playlist(&self, id: i64, tracks: &[i64], position: Option<usize>) -> Result<Vec<i64>> {
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let state = if is_linked(&tx, id)? { "here_only" } else { "both" };
        let mut ids = entry_ids(&tx, id)?;
        let at = position.unwrap_or(ids.len()).min(ids.len());
        let mut added = Vec::with_capacity(tracks.len());
        for track in tracks {
            let exists: bool =
                tx.query_row("SELECT EXISTS (SELECT 1 FROM tracks WHERE id = ?1)", [track], |r| r.get(0)).map_err(err)?;
            if !exists {
                return Err(format!("no song {track}"));
            }
            tx.execute(
                "INSERT INTO playlist_entries (playlist_id, position, track_id, state, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, (ids.len() + added.len()) as i64, track, state, now],
            )
            .map_err(err)?;
            added.push(tx.last_insert_rowid());
        }
        ids.splice(at..at, added.iter().copied());
        renumber(&tx, id, &ids)?;
        tx.commit().map_err(err)?;
        Ok(added)
    }

    /// Removes entries from a playlist (by entry id); the songs that were removed. In a linked playlist,
    /// songs the file has stay greyed ("removed here", restorable); a song suggested by the file is turned
    /// down the same way.
    pub fn remove_from_playlist(&self, id: i64, entries: &[i64]) -> Result<Vec<i64>> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let linked = is_linked(&tx, id)?;
        let mut removed = Vec::new();
        for entry in entries {
            let found: Option<(String, Option<i64>)> = tx
                .query_row("SELECT state, track_id FROM playlist_entries WHERE id = ?1 AND playlist_id = ?2", params![entry, id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()
                .map_err(err)?;
            let Some((state, track)) = found else { continue };
            if linked && matches!(state.as_str(), "both" | "suggested") {
                tx.execute("UPDATE playlist_entries SET state = 'removed_here' WHERE id = ?1", [entry]).map_err(err)?;
            } else if !linked || state != "removed_here" {
                tx.execute("DELETE FROM playlist_entries WHERE id = ?1", [entry]).map_err(err)?;
            }
            removed.extend(track);
        }
        let ids = entry_ids(&tx, id)?;
        renumber(&tx, id, &ids)?;
        tx.commit().map_err(err)?;
        Ok(removed)
    }

    /// Moves an entry to `position` (from 0; the end when past it). Returns its song.
    pub fn move_in_playlist(&self, id: i64, entry: i64, position: usize) -> Result<Option<i64>> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        is_linked(&tx, id)?;
        let mut ids = entry_ids(&tx, id)?;
        let from = ids.iter().position(|e| *e == entry).ok_or_else(|| format!("no entry {entry} in playlist {id}"))?;
        let moved = ids.remove(from);
        ids.insert(position.min(ids.len()), moved);
        renumber(&tx, id, &ids)?;
        let track = tx.query_row("SELECT track_id FROM playlist_entries WHERE id = ?1", [entry], |r| r.get(0)).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(track)
    }
}
