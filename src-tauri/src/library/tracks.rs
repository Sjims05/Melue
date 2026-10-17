//! Reading songs back out of the library (the UI's lists build on this).

use std::collections::BTreeMap;
use rusqlite::{params, Row};
use serde::Serialize;

use super::{err, Library, Result};

/// A song as the UI shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub folder_id: i64,
    pub title: String,
    /// The artist as tagged.
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub duration_ms: Option<i64>,
    pub format: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: Option<u32>,
    /// The cover's file base name (see covers.rs), for its URL.
    pub cover: Option<String>,
    /// Whether its tags have been read yet (the second pass of a scan).
    pub tags_read: bool,
    /// Why its tags couldn't be read.
    pub tags_error: Option<String>,
    /// Whether it can be played: its file is there and its folder switched on. Lists of songs only have
    /// those; playlists keep the others too (shown greyed, they come back with their file).
    pub available: bool,
    pub liked: bool,
    /// How often it was played (see playback.rs for what counts).
    pub play_count: i64,
}

/// A song's columns, for [`track_row`]: `t` is the track, `f` its folder, `c` its cover.
pub(crate) const TRACK_COLUMNS: &str = "t.id, t.folder_id, t.title, t.artist_credit, t.tag_album, t.tag_album_artist, t.track_no,
    t.disc_no, t.year, t.genre, t.duration_ms, t.format, t.bitrate_kbps, t.sample_rate, c.file, t.tags_read_at IS NOT NULL,
    t.tags_error, t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled, EXISTS (SELECT 1 FROM likes l WHERE l.track_id = t.id),
    coalesce((SELECT s.play_count FROM all_track_stats s WHERE s.track_id = t.id), 0)";

/// How many columns [`TRACK_COLUMNS`] is: a query's own columns after it start there.
pub(crate) const TRACK_COLUMN_COUNT: usize = 20;

/// A song from a row selected with [`TRACK_COLUMNS`] first.
pub(crate) fn track_row(r: &Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        folder_id: r.get(1)?,
        title: r.get(2)?,
        artist: r.get(3)?,
        album: r.get(4)?,
        album_artist: r.get(5)?,
        track_no: r.get(6)?,
        disc_no: r.get(7)?,
        year: r.get(8)?,
        genre: r.get(9)?,
        duration_ms: r.get(10)?,
        format: r.get(11)?,
        bitrate_kbps: r.get(12)?,
        sample_rate: r.get(13)?,
        cover: r.get(14)?,
        tags_read: r.get(15)?,
        tags_error: r.get(16)?,
        available: r.get(17)?,
        liked: r.get(18)?,
        play_count: r.get(19)?,
    })
}

impl Library {
    /// Songs that are there (not missing), by title.
    pub fn tracks(&self, offset: u32, limit: u32) -> Result<Vec<Track>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS}
             FROM tracks t JOIN folders f ON f.id = t.folder_id LEFT JOIN covers c ON c.id = t.cover_id
             WHERE t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled
             ORDER BY t.sort_title, t.id LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let tracks = stmt.query_map(params![limit, offset], track_row).map_err(err)?;
        tracks.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// The songs with these ids (missing ones too, marked not available), in no particular order; ids not in the
    /// library are left out. For a list that has the ids (the player's queue): the songs' lengths, likes, covers.
    pub fn tracks_by_ids(&self, ids: &[i64]) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn();
        let marks = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT {TRACK_COLUMNS}
             FROM tracks t JOIN folders f ON f.id = t.folder_id LEFT JOIN covers c ON c.id = t.cover_id
             WHERE t.id IN ({marks})"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let tracks = stmt.query_map(rusqlite::params_from_iter(ids), track_row).map_err(err)?;
        tracks.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// These songs' artists, each with its artist page's id, by song id: the main ones, then the featured ones, in the
    /// tag's order ("A feat. B": A, then B). For linking the names the songs show. Songs without any are left out.
    pub fn track_artists(&self, track_ids: &[i64]) -> Result<BTreeMap<i64, Vec<ArtistLink>>> {
        let mut found: BTreeMap<i64, Vec<ArtistLink>> = BTreeMap::new();
        if track_ids.is_empty() {
            return Ok(found);
        }
        let conn = self.conn();
        let marks = vec!["?"; track_ids.len()].join(",");
        let sql = format!(
            "SELECT x.track_id, ar.id, ar.name FROM track_artists x JOIN artists ar ON ar.id = x.artist_id
             WHERE x.track_id IN ({marks}) ORDER BY x.track_id, x.role = 'featured', x.position"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(track_ids)).map_err(err)?;
        while let Some(r) = rows.next().map_err(err)? {
            found.entry(r.get(0).map_err(err)?).or_default().push(ArtistLink { id: r.get(1).map_err(err)?, name: r.get(2).map_err(err)? });
        }
        Ok(found)
    }
}

/// An artist a song names, and their page's id.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArtistLink {
    pub id: i64,
    pub name: String,
}

/// An album as the UI shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: i64,
    pub title: String,
    /// The album artist; null for a compilation.
    pub artist: Option<String>,
    pub compilation: bool,
    pub year: Option<i64>,
    pub cover: Option<String>,
    /// Its songs that are there.
    pub tracks: i64,
}

/// An artist as the UI shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: i64,
    pub name: String,
    /// Songs they're a main artist of.
    pub tracks: i64,
    /// Songs they're featured on.
    pub featured: i64,
    /// Albums they're the album artist of.
    pub albums: i64,
}

/// Songs that are there: not missing, in a switched-on folder.
/// A song that's there and shown (not a hidden copy of a synced song).
const PRESENT: &str = "t.missing_since IS NULL AND t.copy_of IS NULL AND t.folder_id IN (SELECT id FROM folders WHERE enabled)";

impl Library {
    /// Albums with songs that are there, by title.
    pub fn albums(&self, offset: u32, limit: u32) -> Result<Vec<Album>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT a.id, a.title, ar.name, a.compilation, a.year, c.file,
                    (SELECT count(*) FROM tracks t WHERE t.album_id = a.id AND {PRESENT}) AS n
             FROM albums a LEFT JOIN artists ar ON ar.id = a.artist_id LEFT JOIN covers c ON c.id = a.cover_id
             WHERE n > 0 ORDER BY a.sort_title, ar.sort_name, a.id LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let albums = stmt
            .query_map(params![limit, offset], |r| {
                Ok(Album {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    artist: r.get(2)?,
                    compilation: r.get(3)?,
                    year: r.get(4)?,
                    cover: r.get(5)?,
                    tracks: r.get(6)?,
                })
            })
            .map_err(err)?;
        albums.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Artists with songs that are there, by name.
    pub fn artists(&self, offset: u32, limit: u32) -> Result<Vec<Artist>> {
        let conn = self.conn();
        let count = |role: &str| {
            format!(
                "(SELECT count(*) FROM track_artists x JOIN tracks t ON t.id = x.track_id
                  WHERE x.artist_id = ar.id AND x.role = '{role}' AND {PRESENT})"
            )
        };
        let sql = format!(
            "SELECT ar.id, ar.name, {} AS main, {} AS guest,
                    (SELECT count(*) FROM albums a WHERE a.artist_id = ar.id
                       AND EXISTS (SELECT 1 FROM tracks t WHERE t.album_id = a.id AND {PRESENT})) AS albums
             FROM artists ar WHERE main + guest + albums > 0 ORDER BY ar.sort_name, ar.id LIMIT ?1 OFFSET ?2",
            count("main"),
            count("featured")
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let artists = stmt
            .query_map(params![limit, offset], |r| {
                Ok(Artist { id: r.get(0)?, name: r.get(1)?, tracks: r.get(2)?, featured: r.get(3)?, albums: r.get(4)? })
            })
            .map_err(err)?;
        artists.collect::<rusqlite::Result<_>>().map_err(err)
    }
}
