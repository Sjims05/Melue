//! Playing from the library: the queue for a list ("album:12", "artist:3", "playlist:5", "songs",
//! "liked"), what playing counts as (play counts, skips, history), and likes.
//!
//! The player logs playback events (a song started, played to the end, or was left early) even while
//! Melue's UI isn't running (Android Auto), and hands them over once each; they're counted here:
//! - a play: played to the end, or left after half of it or 4 minutes (long songs);
//! - a skip: left in its first 30 seconds;
//! - anything between is neither.
//!
//! Counts are kept per song (track_stats), so they stay right when the history (plays) is trimmed to the
//! latest [`HISTORY_LIMIT`] songs started.

use std::path::Path;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri_plugin_native_audio::QueueItem;

use super::covers::{self, LARGE};
use super::tracks::{track_row, Track, TRACK_COLUMNS};
use super::{err, now_ms, Library, Result};

pub const HISTORY_LIMIT: i64 = 10_000;
/// Left before this: a skip.
const SKIP_BEFORE_MS: i64 = 30_000;
/// Left after this (or half the song, if shorter): a play.
const PLAY_AFTER_MS: i64 = 4 * 60_000;
/// How long counted events are remembered (to count a repeated one once).
const SEEN_FOR_MS: i64 = 30 * 24 * 3600 * 1000;

/// The columns of a queue item (see `queue_item`), from tracks `t`, folders `f` and covers `c`.
const ITEM_SELECT: &str = "SELECT t.id, f.kind, f.location, t.path, t.title, t.artist_credit, t.tag_album, c.file
    FROM tracks t JOIN folders f ON f.id = t.folder_id LEFT JOIN covers c ON c.id = t.cover_id";

/// A playback event from the player.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackEvent {
    pub id: String,
    /// "start", "complete" or "skip" (left before the end).
    #[serde(rename = "type")]
    pub kind: String,
    pub item_id: i64,
    pub position_ms: i64,
    pub at_ms: i64,
}

/// What a song being left counts as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Play,
    Skip,
    Neither,
}

/// What leaving a song at `position_ms` counts as (its length `duration_ms`, when known).
pub fn outcome(position_ms: i64, duration_ms: Option<i64>) -> Outcome {
    let play_after = duration_ms.filter(|d| *d > 0).map_or(PLAY_AFTER_MS, |d| (d / 2).min(PLAY_AFTER_MS));
    if position_ms >= play_after {
        Outcome::Play
    } else if position_ms < SKIP_BEFORE_MS {
        Outcome::Skip
    } else {
        Outcome::Neither
    }
}

impl Library {
    /// The queue for a list, ready to play: its songs that can be played, in its order. Errors for a list
    /// that isn't there (a deleted playlist).
    /// - "songs": all songs by title
    /// - "album:<id>": by disc and track
    /// - "artist:<id>": their albums oldest first (each by disc and track), then songs not on an album
    /// - "playlist:<id>": its songs in Melue's version, in order
    /// - "liked": liked songs, most recently liked first
    pub fn queue_items(&self, source: &str) -> Result<Vec<QueueItem>> {
        let (kind, id) = match source.split_once(':') {
            Some((kind, id)) => (kind, Some(id.parse::<i64>().map_err(|_| format!("not a list: {source}"))?)),
            None => (source, None),
        };
        let exists = |table: &str, id: i64| -> Result<()> {
            let sql = format!("SELECT EXISTS (SELECT 1 FROM {table} WHERE id = ?1)");
            let found: bool = self.conn().query_row(&sql, [id], |r| r.get(0)).map_err(err)?;
            if found { Ok(()) } else { Err(format!("no {source}")) }
        };
        let (join, filter, order) = match (kind, id) {
            ("songs", None) => ("", "", "t.sort_title, t.id"),
            ("album", Some(id)) => {
                exists("albums", id)?;
                ("", "AND t.album_id = ?1", "coalesce(t.disc_no, 1), t.track_no IS NULL, t.track_no, t.path")
            }
            ("artist", Some(id)) => {
                exists("artists", id)?;
                (
                    "LEFT JOIN albums a ON a.id = t.album_id",
                    "AND t.id IN (SELECT track_id FROM track_artists WHERE artist_id = ?1)",
                    "a.id IS NULL, a.year IS NULL, a.year, a.sort_title, a.id, coalesce(t.disc_no, 1), t.track_no IS NULL, t.track_no, t.sort_title",
                )
            }
            ("playlist", Some(id)) => {
                exists("playlists", id)?;
                (
                    "JOIN playlist_entries e ON e.track_id = t.id AND e.playlist_id = ?1 AND e.state IN ('both', 'here_only', 'file_removed')",
                    "",
                    "e.position, e.id",
                )
            }
            ("liked", None) => ("JOIN likes l ON l.track_id = t.id", "", "l.liked_at DESC, t.id"),
            _ => return Err(format!("not a list: {source}")),
        };
        let sql = format!("{ITEM_SELECT} {join} WHERE t.missing_since IS NULL AND f.enabled {filter} ORDER BY {order}");
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let to_item = |r: &rusqlite::Row| self.queue_item(r);
        // The lists with an id use it as ?1.
        let items = match id {
            Some(id) => stmt.query_map([id], to_item),
            None => stmt.query_map([], to_item),
        }
        .map_err(err)?;
        items.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Queue items for songs, in the order given (songs not there left out).
    pub fn queue_items_for(&self, ids: &[i64]) -> Result<Vec<QueueItem>> {
        let sql = format!("{ITEM_SELECT} WHERE t.missing_since IS NULL AND f.enabled AND t.id IN (SELECT value FROM json_each(?1))");
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let found: std::collections::HashMap<i64, QueueItem> = stmt
            .query_map([serde_json::to_string(ids).map_err(err)?], |r| self.queue_item(r))
            .map_err(err)?
            .map(|item| item.map(|i| (i.id.unwrap_or_default(), i)))
            .collect::<rusqlite::Result<_>>()
            .map_err(err)?;
        Ok(ids.iter().filter_map(|id| found.get(id).cloned()).collect())
    }

    /// A queue item from an [`ITEM_SELECT`] row.
    fn queue_item(&self, r: &rusqlite::Row) -> rusqlite::Result<QueueItem> {
        let (kind, location, path): (String, String, String) = (r.get(1)?, r.get(2)?, r.get(3)?);
        let src = if kind == "path" { Path::new(&location).join(&path).to_string_lossy().into_owned() } else { path };
        let cover: Option<String> = r.get(7)?;
        Ok(QueueItem {
            id: Some(r.get(0)?),
            src,
            title: Some(r.get(4)?),
            artist: r.get(5)?,
            album: r.get(6)?,
            artwork_url: cover.map(|c| self.covers_dir().join(covers::file_name(&c, LARGE)).to_string_lossy().into_owned()),
        })
    }

    /// Counts a playback event (once, however often it's handed over). Returns what a song being left
    /// counted as; None for a start, a repeated event, or a song that isn't in the library.
    pub fn record_playback(&self, event: &PlaybackEvent) -> Result<Option<Outcome>> {
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let new = tx
            .execute("INSERT INTO playback_events_seen (id, seen_at) VALUES (?1, ?2) ON CONFLICT DO NOTHING", params![event.id, now])
            .map_err(err)?;
        tx.execute("DELETE FROM playback_events_seen WHERE seen_at < ?1", [now - SEEN_FOR_MS]).map_err(err)?;
        let duration: Option<Option<i64>> =
            tx.query_row("SELECT duration_ms FROM tracks WHERE id = ?1", [event.item_id], |r| r.get(0)).optional().map_err(err)?;
        let (true, Some(duration)) = (new > 0, duration) else {
            tx.commit().map_err(err)?;
            return Ok(None);
        };
        let track = event.item_id;
        let result = match event.kind.as_str() {
            "start" => {
                tx.execute("INSERT INTO plays (track_id, started_at) VALUES (?1, ?2)", params![track, event.at_ms]).map_err(err)?;
                None
            }
            "complete" | "skip" => {
                let completed = event.kind == "complete";
                let counted = if completed { Outcome::Play } else { outcome(event.position_ms, duration) };
                // Its start in the history (or, if that was missed, a new entry).
                let updated = tx
                    .execute(
                        "UPDATE plays SET completed = ?2, position_ms = ?3 WHERE id = (
                             SELECT id FROM plays WHERE track_id = ?1 AND position_ms IS NULL AND started_at <= ?4
                             ORDER BY started_at DESC, id DESC LIMIT 1)",
                        params![track, completed, event.position_ms, event.at_ms],
                    )
                    .map_err(err)?;
                if updated == 0 {
                    tx.execute(
                        "INSERT INTO plays (track_id, started_at, completed, position_ms) VALUES (?1, ?2, ?3, ?4)",
                        params![track, event.at_ms - event.position_ms, completed, event.position_ms],
                    )
                    .map_err(err)?;
                }
                let (plays, skips) = match counted {
                    Outcome::Play => (1, 0),
                    Outcome::Skip => (0, 1),
                    Outcome::Neither => (0, 0),
                };
                if plays + skips > 0 {
                    tx.execute(
                        "INSERT INTO track_stats (track_id, play_count, skip_count, last_played_at) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT (track_id) DO UPDATE SET play_count = play_count + ?2, skip_count = skip_count + ?3,
                             last_played_at = CASE WHEN ?2 > 0 THEN max(coalesce(last_played_at, 0), ?4) ELSE last_played_at END",
                        params![track, plays, skips, if plays > 0 { Some(event.at_ms) } else { None }],
                    )
                    .map_err(err)?;
                }
                Some(counted)
            }
            _ => None,
        };
        // The history keeps the latest songs started.
        tx.execute(
            "DELETE FROM plays WHERE id <= (SELECT id FROM plays ORDER BY id DESC LIMIT 1 OFFSET ?1)",
            [HISTORY_LIMIT],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(result)
    }

    /// Likes or unlikes a song.
    pub fn set_liked(&self, track: i64, liked: bool) -> Result<()> {
        let conn = self.conn();
        if liked {
            conn.execute("INSERT INTO likes (track_id, liked_at) VALUES (?1, ?2) ON CONFLICT DO NOTHING", params![track, now_ms()])
                .map_err(|e| if e.to_string().contains("FOREIGN KEY") { format!("no song {track}") } else { err(e) })?;
        } else {
            conn.execute("DELETE FROM likes WHERE track_id = ?1", [track]).map_err(err)?;
        }
        Ok(())
    }

    /// Every liked song's id (missing ones too).
    pub fn liked_ids(&self) -> Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT track_id FROM likes ORDER BY track_id").map_err(err)?;
        let ids = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        ids.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Songs that are there, by when they were last started (latest first).
    pub fn recently_played(&self, limit: u32) -> Result<Vec<Track>> {
        self.track_list(
            "JOIN (SELECT track_id, max(started_at) AS last FROM plays GROUP BY track_id) p ON p.track_id = t.id",
            "p.last DESC",
            limit,
        )
    }

    /// Songs that are there and were played, by play count (most first).
    pub fn most_played(&self, limit: u32) -> Result<Vec<Track>> {
        self.track_list(
            "JOIN track_stats s ON s.track_id = t.id AND s.play_count > 0",
            "s.play_count DESC, s.last_played_at DESC",
            limit,
        )
    }

    fn track_list(&self, join: &str, order: &str, limit: u32) -> Result<Vec<Track>> {
        let sql = format!(
            "SELECT {TRACK_COLUMNS}
             FROM tracks t JOIN folders f ON f.id = t.folder_id LEFT JOIN covers c ON c.id = t.cover_id {join}
             WHERE t.missing_since IS NULL AND f.enabled ORDER BY {order}, t.id LIMIT ?1"
        );
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let tracks = stmt.query_map([limit], track_row).map_err(err)?;
        tracks.collect::<rusqlite::Result<_>>().map_err(err)
    }
}
