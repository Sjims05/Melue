//! Reading songs back out of the library (the UI's lists build on this): the Library tabs (songs, albums, artists,
//! each sortable) and the pages of an album, an artist and a folder.

use std::path::PathBuf;

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

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
    /// Its album's page (null: not on an album).
    pub album_id: Option<i64>,
    /// Where the file is in its folder (a content:// URI for Android's media index and chosen folders).
    pub path: String,
    /// When the library first saw it (ms).
    pub added_at: i64,
    /// Its artists with their pages' ids: the main ones, then the featured ones, in the tag's order ("A feat. B":
    /// A, then B), for linking the names in `artist`.
    pub artists: Vec<ArtistLink>,
}

/// A song's columns, for [`track_row`]: `t` is the track, `f` its folder, `c` its cover.
pub(crate) const TRACK_COLUMNS: &str = "t.id, t.folder_id, t.title, t.artist_credit, t.tag_album, t.tag_album_artist, t.track_no,
    t.disc_no, t.year, t.genre, t.duration_ms, t.format, t.bitrate_kbps, t.sample_rate, c.file, t.tags_read_at IS NOT NULL,
    t.tags_error, t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled, EXISTS (SELECT 1 FROM likes l WHERE l.track_id = t.id),
    coalesce((SELECT s.play_count FROM all_track_stats s WHERE s.track_id = t.id), 0), t.album_id, t.path, t.added_at,
    (SELECT json_group_array(json_object('id', a.id, 'name', a.name)) FROM
       (SELECT ar.id, ar.name FROM track_artists x JOIN artists ar ON ar.id = x.artist_id
        WHERE x.track_id = t.id ORDER BY x.role = 'featured', x.position) a)";

/// How many columns [`TRACK_COLUMNS`] is: a query's own columns after it start there.
pub(crate) const TRACK_COLUMN_COUNT: usize = 24;

/// A song from a row selected with [`TRACK_COLUMNS`] first.
pub(crate) fn track_row(r: &Row) -> rusqlite::Result<Track> {
    let artists: Option<String> = r.get(23)?;
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
        album_id: r.get(20)?,
        path: r.get(21)?,
        added_at: r.get(22)?,
        artists: artists.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default(),
    })
}

/// An artist a song names, and their page's id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The album artist's page.
    pub artist_id: Option<i64>,
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

/// An album's page: the album, and its songs in the order they play (by disc, then track number).
#[derive(Debug, Clone, Serialize)]
pub struct AlbumPage {
    pub album: Album,
    pub tracks: Vec<Track>,
}

/// An artist's page: the artist, their albums (oldest first), their songs (in the order their albums go, then the
/// ones not on an album, by title) and the songs they're featured on (by title).
#[derive(Debug, Clone, Serialize)]
pub struct ArtistPage {
    pub artist: Artist,
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
    pub featured: Vec<Track>,
}

/// How the Songs tab is sorted. Title, artist and album go A–Z; date added and plays, newest and most first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackSort {
    #[default]
    Title,
    Artist,
    Album,
    Added,
    Plays,
}

impl TrackSort {
    /// "title", "artist", "album", "added", "plays" (as the UI names them).
    pub fn parse(name: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(name.into())).ok()
    }

    /// The songs' order, in SQL (`t` is the track).
    pub(crate) fn order(self) -> &'static str {
        match self {
            TrackSort::Title => "t.sort_title, t.id",
            // By the first main artist's sort name (songs without one last), then their albums' songs in order.
            TrackSort::Artist => {
                "(SELECT ar.sort_name FROM track_artists x JOIN artists ar ON ar.id = x.artist_id
                  WHERE x.track_id = t.id AND x.role = 'main' ORDER BY x.position LIMIT 1) IS NULL,
                 (SELECT ar.sort_name FROM track_artists x JOIN artists ar ON ar.id = x.artist_id
                  WHERE x.track_id = t.id AND x.role = 'main' ORDER BY x.position LIMIT 1),
                 (SELECT al.sort_title FROM albums al WHERE al.id = t.album_id), coalesce(t.disc_no, 1), t.track_no, t.sort_title, t.id"
            }
            // By album (songs not on one last), then disc and track.
            TrackSort::Album => {
                "t.album_id IS NULL, (SELECT al.sort_title FROM albums al WHERE al.id = t.album_id), t.album_id,
                 coalesce(t.disc_no, 1), t.track_no IS NULL, t.track_no, t.sort_title, t.id"
            }
            TrackSort::Added => "t.added_at DESC, t.id DESC",
            TrackSort::Plays => "coalesce((SELECT s.play_count FROM all_track_stats s WHERE s.track_id = t.id), 0) DESC, t.sort_title, t.id",
        }
    }
}

/// How the Albums tab is sorted: by title, artist, year (newest first) or date added (newest first).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlbumSort {
    #[default]
    Title,
    Artist,
    Year,
    Added,
}

impl AlbumSort {
    fn order(self) -> &'static str {
        match self {
            AlbumSort::Title => "a.sort_title, ar.sort_name, a.id",
            AlbumSort::Artist => "ar.sort_name IS NULL, ar.sort_name, a.year IS NULL, a.year, a.sort_title, a.id",
            AlbumSort::Year => "a.year IS NULL, a.year DESC, a.sort_title, a.id",
            AlbumSort::Added => "(SELECT max(t.added_at) FROM tracks t WHERE t.album_id = a.id) DESC, a.sort_title, a.id",
        }
    }
}

/// How the Artists tab is sorted: by name, or by how many songs they have (most first).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtistSort {
    #[default]
    Name,
    Songs,
}

impl ArtistSort {
    fn order(self) -> &'static str {
        match self {
            ArtistSort::Name => "ar.sort_name, ar.id",
            ArtistSort::Songs => "main + guest DESC, ar.sort_name, ar.id",
        }
    }
}

/// A song that's there and shown (not missing, in a switched-on folder, not a hidden copy of a synced song).
const PRESENT: &str = "t.missing_since IS NULL AND t.copy_of IS NULL AND t.folder_id IN (SELECT id FROM folders WHERE enabled)";

/// The tables [`TRACK_COLUMNS`] reads (`t`, `f`, `c`).
const TRACKS_FROM: &str = "FROM tracks t JOIN folders f ON f.id = t.folder_id LEFT JOIN covers c ON c.id = t.cover_id";

/// An album's columns, for [`album_row`]: `a` is the album, `ar` its artist, `c` its cover.
const ALBUM_SELECT: &str = "SELECT a.id, a.title, ar.name, ar.id, a.compilation, a.year, c.file,
        (SELECT count(*) FROM tracks t WHERE t.album_id = a.id AND {PRESENT}) AS n
     FROM albums a LEFT JOIN artists ar ON ar.id = a.artist_id LEFT JOIN covers c ON c.id = a.cover_id";

fn album_row(r: &Row) -> rusqlite::Result<Album> {
    Ok(Album {
        id: r.get(0)?,
        title: r.get(1)?,
        artist: r.get(2)?,
        artist_id: r.get(3)?,
        compilation: r.get(4)?,
        year: r.get(5)?,
        cover: r.get(6)?,
        tracks: r.get(7)?,
    })
}

/// An artist's columns, for [`artist_row`]: `ar` is the artist.
fn artist_select() -> String {
    let count = |role: &str| {
        format!(
            "(SELECT count(*) FROM track_artists x JOIN tracks t ON t.id = x.track_id
              WHERE x.artist_id = ar.id AND x.role = '{role}' AND {PRESENT})"
        )
    };
    format!(
        "SELECT ar.id, ar.name, {} AS main, {} AS guest,
                (SELECT count(*) FROM albums a WHERE a.artist_id = ar.id
                   AND EXISTS (SELECT 1 FROM tracks t WHERE t.album_id = a.id AND {PRESENT})) AS albums
         FROM artists ar",
        count("main"),
        count("featured")
    )
}

fn artist_row(r: &Row) -> rusqlite::Result<Artist> {
    Ok(Artist { id: r.get(0)?, name: r.get(1)?, tracks: r.get(2)?, featured: r.get(3)?, albums: r.get(4)? })
}

/// The order an album's songs play in (by disc, then track number; untracked ones after, by file).
const ALBUM_ORDER: &str = "coalesce(t.disc_no, 1), t.track_no IS NULL, t.track_no, t.path";

/// A song whose file can't be found (deleted, moved, or on a drive that isn't there now): kept in the library, with
/// its likes, plays and playlist places, in case it comes back.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingSong {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    /// Its music folder's name, and where the file was in it.
    pub folder: String,
    pub path: String,
    /// Since when (ms).
    pub missing_since: i64,
}

impl Library {
    /// The songs that can't be found, in switched-on folders (most recently gone first).
    pub fn missing_songs(&self) -> Result<Vec<MissingSong>> {
        // The folders' names as the library has them (a name can be the path's, made when read: none in the table).
        let names: std::collections::HashMap<i64, String> = self.folders()?.into_iter().map(|f| (f.id, f.name)).collect();
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.artist_credit, t.folder_id, t.path, t.missing_since FROM tracks t JOIN folders f ON f.id = t.folder_id
                 WHERE t.missing_since IS NOT NULL AND t.copy_of IS NULL AND f.enabled ORDER BY t.missing_since DESC, t.title",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                let folder = names.get(&r.get::<_, i64>(3)?).cloned().unwrap_or_default();
                Ok(MissingSong { id: r.get(0)?, title: r.get(1)?, artist: r.get(2)?, folder, path: r.get(4)?, missing_since: r.get(5)? })
            })
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }
}

impl Library {
    /// Songs that are there (not missing), sorted.
    pub fn tracks(&self, offset: u32, limit: u32, sort: TrackSort) -> Result<Vec<Track>> {
        self.select_tracks(
            &format!("WHERE {PRESENT} ORDER BY {} LIMIT ?1 OFFSET ?2", sort.order()),
            params![limit, offset],
        )
    }

    /// The songs with these ids (missing ones too, marked not available), in no particular order; ids not in the
    /// library are left out. For a list that has the ids (the player's queue): the songs' lengths, likes, covers.
    pub fn tracks_by_ids(&self, ids: &[i64]) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let marks = vec!["?"; ids.len()].join(",");
        self.select_tracks(&format!("WHERE t.id IN ({marks})"), rusqlite::params_from_iter(ids))
    }

    /// Albums with songs that are there, sorted.
    pub fn albums(&self, offset: u32, limit: u32, sort: AlbumSort) -> Result<Vec<Album>> {
        let sql = format!("{} WHERE n > 0 ORDER BY {} LIMIT ?1 OFFSET ?2", album_select(), sort.order());
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let albums = stmt.query_map(params![limit, offset], album_row).map_err(err)?;
        albums.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Artists with songs that are there, sorted.
    pub fn artists(&self, offset: u32, limit: u32, sort: ArtistSort) -> Result<Vec<Artist>> {
        let sql = format!("{} WHERE main + guest + albums > 0 ORDER BY {} LIMIT ?1 OFFSET ?2", artist_select(), sort.order());
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let artists = stmt.query_map(params![limit, offset], artist_row).map_err(err)?;
        artists.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// An album's page (None: no such album). Its songs are the ones that are there, in the order they play.
    pub fn album_page(&self, id: i64) -> Result<Option<AlbumPage>> {
        let album = {
            let conn = self.conn();
            conn.query_row(&format!("{} WHERE a.id = ?1", album_select()), [id], album_row).optional().map_err(err)?
        };
        let Some(album) = album else { return Ok(None) };
        let tracks = self.select_tracks(&format!("WHERE t.album_id = ?1 AND {PRESENT} ORDER BY {ALBUM_ORDER}"), [id])?;
        Ok(Some(AlbumPage { album, tracks }))
    }

    /// An artist's page (None: no such artist).
    pub fn artist_page(&self, id: i64) -> Result<Option<ArtistPage>> {
        let (artist, albums) = {
            let conn = self.conn();
            let artist = conn.query_row(&format!("{} WHERE ar.id = ?1", artist_select()), [id], artist_row).optional().map_err(err)?;
            let Some(artist) = artist else { return Ok(None) };
            let sql = format!("{} WHERE a.artist_id = ?1 AND n > 0 ORDER BY a.year IS NULL, a.year, a.sort_title, a.id", album_select());
            let mut stmt = conn.prepare(&sql).map_err(err)?;
            let albums = stmt.query_map([id], album_row).map_err(err)?.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?;
            (artist, albums)
        };
        let of_role = |role: &str| format!("t.id IN (SELECT track_id FROM track_artists WHERE artist_id = ?1 AND role = '{role}')");
        // Their songs as their albums go (oldest first), then the ones not on an album.
        let tracks = self.select_tracks(
            &format!(
                "LEFT JOIN albums al ON al.id = t.album_id WHERE {} AND {PRESENT}
                 ORDER BY al.id IS NULL, al.year IS NULL, al.year, al.sort_title, al.id, {ALBUM_ORDER}, t.sort_title",
                of_role("main")
            ),
            [id],
        )?;
        let featured = self.select_tracks(&format!("WHERE {} AND {PRESENT} ORDER BY t.sort_title, t.id", of_role("featured")), [id])?;
        Ok(Some(ArtistPage { artist, albums, tracks, featured }))
    }

    /// The songs of a Made for you list, in its order (what it plays): "songs" (all, by title), "liked" (newest
    /// liked first), "top:<period>" (most played first).
    pub fn auto_playlist_tracks(&self, source: &str) -> Result<Vec<Track>> {
        if let Some(period) = source.strip_prefix("top:") {
            let period = super::auto_playlists::TopPeriod::parse(period).ok_or_else(|| format!("not a list: {source}"))?;
            let ids = self.top_songs(period)?;
            let mut found: std::collections::HashMap<i64, Track> = self.tracks_by_ids(&ids)?.into_iter().map(|t| (t.id, t)).collect();
            return Ok(ids.iter().filter_map(|id| found.remove(id)).filter(|t| t.available).collect());
        }
        match source {
            "songs" => self.tracks(0, u32::MAX, TrackSort::Title),
            "liked" => self.select_tracks(
                &format!("JOIN likes l ON l.track_id = t.id WHERE {PRESENT} ORDER BY l.liked_at DESC, t.id"),
                [],
            ),
            _ => Err(format!("not a list: {source}")),
        }
    }

    /// A music folder's songs that are there, by where they are in it (subfolders together).
    pub fn folder_tracks(&self, folder_id: i64) -> Result<Vec<Track>> {
        self.select_tracks(&format!("WHERE t.folder_id = ?1 AND {PRESENT} ORDER BY t.path, t.id"), [folder_id])
    }

    /// Where a song's file is on this device's disk (for "Show in folder"); None for songs that aren't files on a
    /// disk (Android's media index and chosen folders) or not in the library.
    pub fn track_file(&self, id: i64) -> Result<Option<PathBuf>> {
        let found: Option<(i64, String)> = self
            .conn()
            .query_row("SELECT folder_id, path FROM tracks WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(err)?;
        let Some((folder_id, path)) = found else { return Ok(None) };
        let Some(folder) = self.folder(folder_id)? else { return Ok(None) };
        Ok((folder.kind == super::FolderKind::Path).then(|| super::linked::local_path(&folder, &path)))
    }

    /// The songs a query finds: `rest` follows the songs' joins (more joins, WHERE, ORDER BY).
    fn select_tracks(&self, rest: &str, params: impl rusqlite::Params) -> Result<Vec<Track>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("SELECT {TRACK_COLUMNS} {TRACKS_FROM} {rest}")).map_err(err)?;
        let tracks = stmt.query_map(params, track_row).map_err(err)?;
        tracks.collect::<rusqlite::Result<_>>().map_err(err)
    }
}

/// [`ALBUM_SELECT`] with what counts as there filled in.
fn album_select() -> String {
    ALBUM_SELECT.replace("{PRESENT}", PRESENT)
}
