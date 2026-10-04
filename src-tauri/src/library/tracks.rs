//! Reading songs back out of the library (the UI's lists build on this).

use rusqlite::params;
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
}

impl Library {
    /// Songs that are there (not missing), by title.
    pub fn tracks(&self, offset: u32, limit: u32) -> Result<Vec<Track>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.folder_id, t.title, t.artist_credit, t.tag_album, t.tag_album_artist, t.track_no, t.disc_no,
                        t.year, t.genre, t.duration_ms, t.format, t.bitrate_kbps, t.sample_rate, c.file,
                        t.tags_read_at IS NOT NULL, t.tags_error
                 FROM tracks t JOIN folders f ON f.id = t.folder_id LEFT JOIN covers c ON c.id = t.cover_id
                 WHERE t.missing_since IS NULL AND f.enabled
                 ORDER BY t.sort_title, t.id LIMIT ?1 OFFSET ?2",
            )
            .map_err(err)?;
        let tracks = stmt
            .query_map(params![limit, offset], |r| {
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
                })
            })
            .map_err(err)?;
        tracks.collect::<rusqlite::Result<_>>().map_err(err)
    }
}
