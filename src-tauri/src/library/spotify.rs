//! Spotify playlists, from their links (no sign-in, no Premium): a public playlist's embed page (the small player
//! websites show) has its first 100 songs, and its own page the true number of songs. Up to 100 songs, that's the
//! whole playlist, read again at each update. Longer ones get their songs from a CSV file a website exports (any of
//! Chosic, Exportify, PlaylistExport, TuneMyMusic: their columns are recognised), and the embed only tells when the
//! playlist changed on Spotify since (the "!": update the list).
//!
//! Unofficial: the embed is Spotify's player for websites, not an interface for apps. If Spotify changes it, reading
//! fails and the playlist stays as it last was.
//!
//! A Spotify playlist syncs like Melue's own (playlist_sync.rs): Melue's version (its songs, with your changes) and
//! its reference (`SpotifyRef`: Spotify's list as last read). The device that reads a change on Spotify first puts it
//! in; the others take the reference from the share, so they don't add the same songs again.
//!
//! Its picture is Spotify's (640 px), downloaded by each device; your own cover goes over it, and it can show its
//! songs' covers instead (`songs_cover`, synced too).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::linked::{apply_file, TrackIndex};
use crate::sync::playlist_merge::SyncEntry;
use crate::sync::playlist_sync::{write_entries, NewEntry};
use crate::sync::same_song::Songs;
use super::m3u::{ExtInf, FileLine};
use super::{err, now_ms, Library, Result};

/// A Spotify playlist as read: its name, its songs (the first 100 at most, from the embed), how many it has, and
/// its picture's address.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpotifyList {
    pub name: String,
    pub songs: Vec<SpotifySong>,
    /// The playlist's true number of songs (None: not known; then `songs` is taken as all of them).
    pub total: Option<usize>,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifySong {
    pub title: String,
    /// The artists, as Spotify lists them ("ABBA", "Artist A, Artist B").
    pub artist: String,
    pub duration_ms: Option<i64>,
}

impl SpotifyList {
    /// All of the playlist's songs are here (100 or fewer).
    pub fn complete(&self) -> bool {
        self.total.is_none_or(|total| total <= self.songs.len())
    }
}

/// The playlist id in a link: "https://open.spotify.com/playlist/37i9…?si=…", "spotify:playlist:37i9…", or the id.
pub fn playlist_id(link: &str) -> Option<String> {
    let link = link.trim();
    let id = if let Some(rest) = link.split("/playlist/").nth(1) {
        rest.split(['?', '/', '#']).next()?
    } else if let Some(rest) = link.strip_prefix("spotify:playlist:") {
        rest
    } else {
        link
    };
    (id.len() >= 16 && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| id.to_string())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        // Not a full browser's: Spotify then sends its simple pages (with the song count), not its web player.
        .user_agent("Mozilla/5.0")
        .build()
        .into()
}

/// Reads playlist `id` from Spotify: its embed (name, first 100 songs) and its page (how many songs).
pub fn read_playlist(id: &str) -> Result<SpotifyList> {
    let embed = agent()
        .get(&format!("https://open.spotify.com/embed/playlist/{id}"))
        .call()
        .map_err(|e| format!("Spotify can’t be reached: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let mut list = parse_embed(&embed)?;
    // The true count is on the playlist's own page; without it, the embed's songs are taken as all.
    list.total = agent()
        .get(&format!("https://open.spotify.com/playlist/{id}"))
        .call()
        .ok()
        .and_then(|mut page| page.body_mut().read_to_string().ok())
        .and_then(|page| song_count(&page));
    Ok(list)
}

/// The playlist in an embed page (its __NEXT_DATA__ JSON).
pub fn parse_embed(html: &str) -> Result<SpotifyList> {
    #[derive(Deserialize)]
    struct Entity {
        name: String,
        #[serde(rename = "trackList")]
        track_list: Vec<Track>,
    }
    #[derive(Deserialize)]
    struct Image {
        url: String,
        #[serde(rename = "maxWidth")]
        max_width: Option<u32>,
    }
    #[derive(Deserialize)]
    struct Track {
        title: String,
        subtitle: String,
        duration: Option<i64>,
    }
    let start = html.find(r#"<script id="__NEXT_DATA__" type="application/json">"#).ok_or("Spotify didn’t show that playlist (is it public?)")?;
    let json = &html[start..];
    let json = &json[json.find('>').unwrap() + 1..];
    let json = &json[..json.find("</script>").ok_or("Spotify’s page has changed")?];
    let data: serde_json::Value = serde_json::from_str(json).map_err(|_| "Spotify’s page has changed".to_string())?;
    let entity = data.pointer("/props/pageProps/state/data/entity").ok_or("Spotify didn’t show that playlist (is it public?)")?;
    // Its picture: the largest (64, 300 and 640 px), or the one the player shows.
    let images: Vec<Image> = entity.pointer("/visualIdentity/image").and_then(|i| serde_json::from_value(i.clone()).ok()).unwrap_or_default();
    let cover = images
        .into_iter()
        .max_by_key(|i| i.max_width.unwrap_or(0))
        .map(|i| i.url)
        .or_else(|| entity.pointer("/coverArt/sources/0/url").and_then(|u| u.as_str()).map(String::from));
    let entity: Entity = serde_json::from_value(entity.clone()).map_err(|_| "Spotify’s page has changed".to_string())?;
    Ok(SpotifyList {
        name: entity.name,
        songs: entity.track_list.into_iter().map(|t| SpotifySong { title: t.title, artist: t.subtitle, duration_ms: t.duration }).collect(),
        total: None,
        cover,
    })
}

/// The number of songs a playlist page says it has (its "music:song_count").
pub fn song_count(page: &str) -> Option<usize> {
    let at = page.find(r#"name="music:song_count" content=""#).or_else(|| page.find(r#"property="music:song_count" content=""#))?;
    let rest = &page[at..];
    let rest = &rest[rest.find("content=\"")? + 9..];
    rest[..rest.find('"')?].parse().ok()
}

/// A song as a linked playlist's line. The line says what the song is itself ("spotify:" and its artist, title and
/// length as JSON): it's matched to the library by the same-song rule (linked.rs), from the embed's songs and a
/// CSV's alike, and again whenever the library changes.
pub fn line(song: &SpotifySong) -> FileLine {
    let what = serde_json::json!([song.artist, song.title, song.duration_ms]);
    FileLine {
        entry: format!("spotify:{what}"),
        info: Some(ExtInf { seconds: song.duration_ms.map(|ms| (ms + 500) / 1000), artist: Some(song.artist.clone()), title: song.title.clone() }),
    }
}

/// The song a Spotify line stands for (None: not a Spotify line).
pub fn from_line(entry: &str) -> Option<SpotifySong> {
    let (artist, title, duration_ms): (String, String, Option<i64>) = serde_json::from_str(entry.strip_prefix("spotify:")?).ok()?;
    Some(SpotifySong { title, artist, duration_ms })
}

/// What changing means for a long playlist (the "!"): its first songs and its count, as one text.
pub fn signature(list: &SpotifyList) -> String {
    let songs: Vec<String> = list.songs.iter().map(|s| format!("{} - {}", s.artist, s.title)).collect();
    format!("{}|{}", list.total.unwrap_or(list.songs.len()), songs.join("\n"))
}

/// The songs in a CSV a playlist website exported: the columns found by their names (title, artist, length), any
/// of the known sites' (and quotes, commas inside quotes, a ms or m:ss length).
pub fn parse_csv(text: &str) -> Result<Vec<SpotifySong>> {
    let mut rows = csv_rows(text.trim_start_matches('\u{feff}'));
    let header: Vec<String> = rows.next().ok_or("the file is empty")?.into_iter().map(|h| h.trim().to_lowercase()).collect();
    let find = |names: &[&str]| header.iter().position(|h| names.iter().any(|n| h == n || h.contains(n)));
    let title = find(&["track name", "song", "title", "name"]).ok_or("the file has no column for the songs’ titles")?;
    let artist = find(&["artist"]).ok_or("the file has no column for the artists")?;
    let duration = find(&["duration", "length", "time"]);
    let mut songs = Vec::new();
    for row in rows {
        let cell = |i: usize| row.get(i).map(|c| c.trim().to_string()).unwrap_or_default();
        let (title, artist) = (cell(title), cell(artist));
        if title.is_empty() {
            continue;
        }
        let duration_ms = duration.map(cell).and_then(|d| parse_length(&d));
        songs.push(SpotifySong { title, artist, duration_ms });
    }
    if songs.is_empty() {
        return Err("the file has no songs".into());
    }
    Ok(songs)
}

/// A Spotify playlist's reference, synced with it: Spotify's list as last read (its lines), its number of songs
/// there, its first songs and count at its last update (a long one's), the "!", and when a read last changed any of
/// it (two devices' reads: the newest wins).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyRef {
    pub id: String,
    pub lines: Vec<String>,
    #[serde(default)]
    pub total: Option<i64>,
    #[serde(default)]
    pub seen: Option<String>,
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub read_at: i64,
    /// Its picture's address on Spotify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    /// Its songs' covers shown instead of Spotify's picture.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub songs_cover: bool,
}

/// Playlist `id`'s Spotify reference (None: not a Spotify playlist).
pub(crate) fn spotify_ref(conn: &Connection, id: i64) -> Result<Option<SpotifyRef>> {
    type Row = (Option<String>, Option<i64>, Option<String>, bool, i64, Option<String>, bool);
    let row: Option<Row> = conn
        .query_row(
            "SELECT spotify_id, spotify_total, spotify_seen, spotify_changed, spotify_read_at, spotify_cover_url, songs_cover
             FROM playlists WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((Some(spotify), total, seen, changed, read_at, cover, songs_cover)) = row else { return Ok(None) };
    let mut stmt = conn.prepare("SELECT file_entry FROM playlist_snapshots WHERE playlist_id = ?1 ORDER BY position").map_err(err)?;
    let lines = stmt.query_map([id], |r| r.get(0)).map_err(err)?.collect::<rusqlite::Result<_>>().map_err(err)?;
    Ok(Some(SpotifyRef { id: spotify, lines, total, seen, changed, read_at, cover, songs_cover }))
}

/// After reading Spotify: a reference that changed counts as read now (the share gets it at the next sync).
fn mark_read(tx: &Transaction, id: i64, before: &Option<SpotifyRef>) -> Result<()> {
    if spotify_ref(tx, id)? != *before {
        tx.execute("UPDATE playlists SET spotify_read_at = ?2 WHERE id = ?1", params![id, now_ms()]).map_err(err)?;
    }
    Ok(())
}

/// A synced Spotify playlist as the share has it: Melue's version (`entries`) and its reference. Spotify's songs
/// are matched here; the reference's songs Melue's version doesn't have are greyed in their place (removed here,
/// with Restore); songs added in Melue are its own (here_only), and songs gone from Spotify that Melue kept are
/// marked (file_removed).
pub(crate) fn apply_synced(tx: &Transaction, id: i64, entries: &[SyncEntry], reference: &SpotifyRef, songs: &Songs, index: &TrackIndex) -> Result<()> {
    tx.execute(
        "UPDATE playlists SET kind = 'linked', file_path = ?2, spotify_id = ?3, spotify_total = ?4, spotify_seen = ?5, spotify_changed = ?6,
             spotify_read_at = ?7, spotify_cover_url = ?8, songs_cover = ?9
         WHERE id = ?1",
        params![
            id,
            format!("spotify:{}", reference.id),
            reference.id,
            reference.total,
            reference.seen,
            reference.changed,
            reference.read_at,
            reference.cover,
            reference.songs_cover
        ],
    )
    .map_err(err)?;
    tx.execute("DELETE FROM playlist_snapshots WHERE playlist_id = ?1", [id]).map_err(err)?;
    for (position, line) in reference.lines.iter().enumerate() {
        tx.execute("INSERT INTO playlist_snapshots (playlist_id, position, file_entry) VALUES (?1, ?2, ?3)", params![id, position as i64, line])
            .map_err(err)?;
    }

    /// A row of the playlist, and the reference's line it stands for.
    struct Row<'a> {
        entry: NewEntry<'a>,
        line: Option<usize>,
    }
    let find = |entry: &str| index.find("", &FileLine { entry: entry.into(), info: None });
    let mut taken = vec![false; reference.lines.len()];
    let mut rows: Vec<Row> = Vec::new();
    for entry in entries {
        if from_line(&entry.key).is_some() {
            let line = (0..reference.lines.len()).find(|&i| !taken[i] && reference.lines[i] == entry.key);
            if let Some(i) = line {
                taken[i] = true;
            }
            let state = if line.is_some() { "both" } else { "file_removed" };
            rows.push(Row { entry: NewEntry { state, file_entry: Some(entry.key.clone()), track: find(&entry.key), shown: None }, line });
        } else {
            // Added in Melue: the song itself, a copy of it here (keeping the key it came with), or not here.
            let (track, file_entry, shown) = match songs.find(entry) {
                Some((track, true)) => (Some(track), None, None),
                Some((track, false)) => (Some(track), Some(entry.key.clone()), None),
                None => (None, Some(entry.key.clone()), Some(entry)),
            };
            rows.push(Row { entry: NewEntry { state: "here_only", file_entry, track, shown }, line: None });
        }
    }
    // The reference's songs Melue's version doesn't have: greyed after the song before them on Spotify.
    for i in (0..reference.lines.len()).filter(|&i| !taken[i]) {
        let at = match i {
            0 => 0,
            _ => rows.iter().position(|r| r.line == Some(i - 1)).map_or(rows.len(), |p| p + 1),
        };
        let line = &reference.lines[i];
        let entry = NewEntry { state: "removed_here", file_entry: Some(line.clone()), track: find(line), shown: None };
        rows.insert(at, Row { entry, line: Some(i) });
    }
    let rows: Vec<NewEntry> = rows.into_iter().map(|r| r.entry).collect();
    write_entries(tx, id, &rows, songs)
}

impl Library {
    /// Adds a Spotify playlist from its link: a linked playlist that follows it (its songs matched to the library).
    /// Returns its id. One with more than 100 songs starts with its first 100; its full list comes from a CSV.
    pub fn add_spotify_playlist(&self, link: &str, index: &TrackIndex) -> Result<i64> {
        let id = playlist_id(link).ok_or("that isn’t a Spotify playlist link")?;
        if let Some(known) = self.conn().query_row("SELECT id FROM playlists WHERE spotify_id = ?1", [&id], |r| r.get::<_, i64>(0)).optional().map_err(err)? {
            return Ok(known);
        }
        self.add_spotify_list(&id, &read_playlist(&id)?, index)
    }

    /// Adds Spotify playlist `id` as read (`list`).
    pub fn add_spotify_list(&self, id: &str, list: &SpotifyList, index: &TrackIndex) -> Result<i64> {
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        tx.execute(
            "INSERT INTO playlists (name, kind, created_at, updated_at, file_path, spotify_id, spotify_total, spotify_seen, spotify_read_at,
                 spotify_cover_url)
             VALUES (?1, 'linked', ?2, ?2, ?3, ?4, ?5, ?6, ?2, ?7)",
            params![list.name, now, format!("spotify:{id}"), id, list.total.map(|t| t as i64), signature(list), list.cover],
        )
        .map_err(err)?;
        let playlist = tx.last_insert_rowid();
        let lines: Vec<FileLine> = list.songs.iter().map(line).collect();
        apply_file(&tx, playlist, "", &lines, index)?;
        tx.commit().map_err(err)?;
        Ok(playlist)
    }

    /// Reads a Spotify playlist again. Up to 100 songs: its changes come in, as a linked playlist's file's do. More:
    /// only whether it changed there since its list was last updated (`spotify_changed`: the "!"). Returns whether
    /// anything changed here.
    pub fn update_spotify_playlist(&self, playlist: i64, index: &TrackIndex) -> Result<bool> {
        let (id, seen): (String, Option<String>) = self
            .conn()
            .query_row("SELECT spotify_id, spotify_seen FROM playlists WHERE id = ?1", [playlist], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(err)?;
        self.update_from_list(playlist, seen, &read_playlist(&id)?, index)
    }

    /// Spotify playlist `playlist` as now read (`list`; `seen`: what it was at its last update).
    pub fn update_from_list(&self, playlist: i64, seen: Option<String>, list: &SpotifyList, index: &TrackIndex) -> Result<bool> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let before = spotify_ref(&tx, playlist)?;
        tx.execute(
            "UPDATE playlists SET spotify_total = ?2, spotify_cover_url = coalesce(?3, spotify_cover_url) WHERE id = ?1",
            params![playlist, list.total.map(|t| t as i64), list.cover],
        )
        .map_err(err)?;
        let changed = if list.complete() {
            let lines: Vec<FileLine> = list.songs.iter().map(line).collect();
            tx.execute("UPDATE playlists SET spotify_seen = ?2, spotify_changed = 0 WHERE id = ?1", params![playlist, signature(list)]).map_err(err)?;
            apply_file(&tx, playlist, "", &lines, index)?
        } else {
            let differs = seen.as_deref() != Some(signature(list).as_str());
            tx.execute("UPDATE playlists SET spotify_changed = ?2 WHERE id = ?1 AND spotify_changed != ?2", params![playlist, differs]).map_err(err)? > 0
        };
        mark_read(&tx, playlist, &before)?;
        tx.commit().map_err(err)?;
        Ok(changed)
    }

    /// Every Spotify playlist read again (the ones that fail are left as they were, and said). Returns the
    /// playlists that changed here.
    pub fn update_spotify_playlists(&self, index: &TrackIndex) -> Result<Vec<i64>> {
        let ids: Vec<i64> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT id FROM playlists WHERE spotify_id IS NOT NULL").map_err(err)?;
            let ids = stmt.query_map([], |r| r.get(0)).map_err(err)?;
            ids.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let mut changed = Vec::new();
        for id in ids {
            match self.update_spotify_playlist(id, index) {
                Ok(true) => changed.push(id),
                Ok(false) => {}
                Err(e) => eprintln!("melue: spotify playlist {id}: {e}"),
            }
        }
        Ok(changed)
    }

    /// Shows a Spotify playlist's songs' covers instead of Spotify's picture, or Spotify's again (on every device: it
    /// counts as its reference changing now).
    pub fn set_songs_cover(&self, playlist: i64, songs: bool) -> Result<()> {
        let changed = self
            .conn()
            .execute(
                "UPDATE playlists SET songs_cover = ?2, spotify_read_at = ?3, updated_at = ?3 WHERE id = ?1 AND spotify_id IS NOT NULL",
                params![playlist, songs, now_ms()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("no Spotify playlist {playlist}"));
        }
        Ok(())
    }

    /// Downloads the Spotify playlists' pictures that changed (or are new here). Returns the playlists whose
    /// picture changed; one that can't be downloaded now is tried again next time.
    pub fn update_spotify_covers(&self) -> Result<Vec<i64>> {
        let wanted: Vec<(i64, String)> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare("SELECT id, spotify_cover_url FROM playlists WHERE spotify_cover_url IS NOT NULL AND spotify_cover_of IS NOT spotify_cover_url")
                .map_err(err)?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let mut changed = Vec::new();
        for (playlist, url) in wanted {
            let picture = agent().get(&url).call().map_err(|e| e.to_string()).and_then(|mut r| {
                r.body_mut().with_config().limit(10 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())
            });
            let cover = match picture.and_then(|bytes| self.cover_for(&bytes)) {
                Ok(Some(cover)) => cover,
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("melue: spotify playlist {playlist}'s picture: {e}");
                    continue;
                }
            };
            self.conn()
                .execute("UPDATE playlists SET spotify_cover_id = ?2, spotify_cover_of = ?3 WHERE id = ?1", params![playlist, cover, url])
                .map_err(err)?;
            changed.push(playlist);
        }
        Ok(changed)
    }

    /// A long Spotify playlist's full list, from a CSV a website exported: its songs come in as a linked playlist's
    /// file's do, and it counts as up to date (its first songs and count as Spotify has them now, when reachable).
    pub fn import_spotify_csv(&self, playlist: i64, csv: &str, index: &TrackIndex) -> Result<bool> {
        let songs = parse_csv(csv)?;
        let id: String = self.conn().query_row("SELECT spotify_id FROM playlists WHERE id = ?1", [playlist], |r| r.get(0)).map_err(err)?;
        let now = read_playlist(&id).ok();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let before = spotify_ref(&tx, playlist)?;
        let lines: Vec<FileLine> = songs.iter().map(line).collect();
        let changed = apply_file(&tx, playlist, "", &lines, index)?;
        let (total, seen) = match &now {
            Some(list) => (list.total.map(|t| t as i64).unwrap_or(songs.len() as i64), Some(signature(list))),
            None => (songs.len() as i64, None),
        };
        tx.execute(
            "UPDATE playlists SET spotify_total = ?2, spotify_seen = coalesce(?3, spotify_seen), spotify_changed = 0 WHERE id = ?1",
            params![playlist, total, seen],
        )
        .map_err(err)?;
        mark_read(&tx, playlist, &before)?;
        tx.commit().map_err(err)?;
        Ok(changed)
    }
}

/// How often Spotify playlists are read again (and shortly after Melue starts).
const UPDATE_EVERY: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Keeps the Spotify playlists up to date by themselves: a while after start-up, then every half hour.
pub fn start(app: &tauri::AppHandle) {
    use tauri::Manager;
    let app = app.clone();
    let _ = std::thread::Builder::new().name("melue-spotify".into()).spawn(move || crate::guard::keep_running("Spotify playlists", move || {
        std::thread::sleep(std::time::Duration::from_secs(20));
        loop {
            let library = app.state::<Library>();
            let changed = library.track_index().and_then(|index| library.update_spotify_playlists(&index));
            let changed = changed.map(|mut ids| {
                ids.extend(library.update_spotify_covers().unwrap_or_default());
                ids
            });
            match changed {
                Ok(ids) if !ids.is_empty() => super::changes::emit(&app, super::changes::LibraryChange::playlists(super::changes::ChangeReason::Playlists, ids)),
                Ok(_) => {}
                Err(e) => eprintln!("melue: spotify playlists: {e}"),
            }
            std::thread::sleep(UPDATE_EVERY);
        }
    }));
}

/// Downloads the Spotify playlists' new pictures in the background (one just added, or one a sync brought), and
/// shows them.
pub fn update_covers_soon(app: &tauri::AppHandle) {
    use tauri::Manager;
    let app = app.clone();
    let _ = std::thread::Builder::new().name("melue-spotify-covers".into()).spawn(move || match app.state::<Library>().update_spotify_covers() {
        Ok(ids) if !ids.is_empty() => super::changes::emit(&app, super::changes::LibraryChange::playlists(super::changes::ChangeReason::Playlists, ids)),
        Ok(_) => {}
        Err(e) => eprintln!("melue: spotify playlists' pictures: {e}"),
    });
}

/// "215000" (ms), "3:35", "1:03:35".
fn parse_length(text: &str) -> Option<i64> {
    if text.contains(':') {
        let parts: Vec<i64> = text.split(':').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
        return Some(parts.iter().fold(0, |s, p| s * 60 + p) * 1000);
    }
    let n: f64 = text.parse().ok()?;
    // Seconds in some exports, milliseconds in others.
    Some(if n < 10_000.0 { (n * 1000.0) as i64 } else { n as i64 })
}

/// A CSV's rows (fields in double quotes may hold commas, quotes ("") and line breaks).
fn csv_rows(text: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    let mut chars = text.chars().peekable();
    std::iter::from_fn(move || {
        chars.peek()?;
        let (mut row, mut field, mut quoted) = (Vec::new(), String::new(), false);
        while let Some(c) = chars.next() {
            match (c, quoted) {
                ('"', true) if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                ('"', _) => quoted = !quoted,
                (',', false) => row.push(std::mem::take(&mut field)),
                ('\n', false) => break,
                ('\r', false) => {}
                (c, _) => field.push(c),
            }
        }
        row.push(field);
        Some(row)
    })
    .filter(|row| row.iter().any(|f| !f.trim().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playlists_id_from_its_link() {
        assert_eq!(playlist_id("https://open.spotify.com/playlist/37i9dQZF1DX4UtSsGT1Sbe?si=abc123").as_deref(), Some("37i9dQZF1DX4UtSsGT1Sbe"));
        assert_eq!(playlist_id("spotify:playlist:37i9dQZF1DX4UtSsGT1Sbe").as_deref(), Some("37i9dQZF1DX4UtSsGT1Sbe"));
        assert_eq!(playlist_id("not a link"), None);
    }

    #[test]
    fn the_embeds_songs_and_the_pages_count() {
        let embed = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"name":"Mix","trackList":[{"title":"Song One","subtitle":"Band A, Band B","duration":215000}]}}}}}}</script></html>"#;
        let list = parse_embed(embed).unwrap();
        assert_eq!(list.name, "Mix");
        assert_eq!(list.songs, vec![SpotifySong { title: "Song One".into(), artist: "Band A, Band B".into(), duration_ms: Some(215000) }]);
        assert_eq!(song_count(r#"<meta name="music:song_count" content="150"/>"#), Some(150));
    }

    #[test]
    fn a_websites_csv_with_its_own_columns() {
        // Exportify's columns (a quoted title with a comma, a length in ms).
        let exportify = "\"Track URI\",\"Track Name\",\"Artist Name(s)\",\"Duration (ms)\"\nspotify:track:1,\"Song, Two\",Band,215000\n";
        assert_eq!(parse_csv(exportify).unwrap(), vec![SpotifySong { title: "Song, Two".into(), artist: "Band".into(), duration_ms: Some(215000) }]);
        // Another site's: "Song", "Artist", "Time" in m:ss.
        let other = "#,Song,Artist,Album,Time\n1,Song Three,Band,Album,3:35\n";
        assert_eq!(parse_csv(other).unwrap()[0].duration_ms, Some(215000));
    }

    /// Spotify itself (run by hand: cargo test spotify_live -- --ignored).
    #[test]
    #[ignore]
    fn spotify_live() {
        let list = read_playlist("37i9dQZF1DX4UtSsGT1Sbe").unwrap();
        println!("{}: {} songs read, {:?} in all; first: {:?}", list.name, list.songs.len(), list.total, list.songs.first());
        assert_eq!(list.songs.len(), 100);
        assert!(!list.complete());
        // Its picture: the 640 px one.
        println!("cover: {:?}", list.cover);
        assert!(list.cover.is_some_and(|c| c.starts_with("https://")));
    }
}
