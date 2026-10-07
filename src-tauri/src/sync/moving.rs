//! Moving songs to sync: local songs (in music folders that aren't synced) go to the share, into
//! `Artist/Album/` (`Uploads/` without tags), and into the synced folder here (copied locally, not downloaded
//! again); then the original is removed, its likes, plays and playlist entries carried over to the synced song.
//!
//! Local copies of songs that are synced already (the same artist, title and length, see same_song.rs) are
//! hidden in the library the same way, and the user decides: clear the space (remove them) or keep the hidden
//! copies.
//!
//! Removing: on a computer to the Recycle Bin; on a phone, all music on the phone to Android's trash (Android
//! asks first, and keeps them 30 days), songs in a chosen folder deleted.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::handoff::{self, share_path};
use super::local;
use super::run::Progress;
use super::layout::{layout_path, Layout};
use super::same_song::Songs;
use super::scheduler::{self, SyncFailure};
use crate::library::changes::{self, ChangeReason};
use crate::library::grouping::file_name_of;
use crate::library::tags::TagJob;
use crate::library::{err, now_ms, scanner, Folder, FolderKind, Library, Result};
use crate::share::commands::with_stores;
use crate::share::setup::{self, ShareSetup};
use crate::share::Share;

pub const COPIES_EVENT: &str = "song-copies";
/// The folder for songs without tags, on the share.
const UPLOADS: &str = "Uploads";
/// The longest folder or file name made (some systems limit a path's parts).
const MAX_NAME: usize = 120;

// ---- the rules (plain functions, tested in moving_tests.rs)

/// A folder or file name that works on every system: no characters Windows, SMB or Android refuse, no
/// trailing dots or spaces, not too long. Empty: "_".
pub fn clean_name(part: &str) -> String {
    let cleaned: String =
        part.chars().map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { c }).collect();
    let mut cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if cleaned.chars().count() > MAX_NAME {
        cleaned = cleaned.chars().take(MAX_NAME).collect::<String>().trim_end_matches(['.', ' ']).to_string();
    }
    if cleaned.is_empty() { "_".into() } else { cleaned }
}

/// Where a song goes on the share: `Artist/Album/<file>` (the album artist, else the artist), `Artist/<file>`
/// without an album, `Uploads/<file>` without an artist.
pub fn upload_path(album_artist: Option<&str>, artist: Option<&str>, album: Option<&str>, file_name: &str) -> String {
    fn given(s: Option<&str>) -> Option<&str> {
        s.map(str::trim).filter(|s| !s.is_empty())
    }
    let file = clean_name(file_name);
    match (given(album_artist).or(given(artist)), given(album)) {
        (Some(artist), Some(album)) => format!("{}/{}/{file}", clean_name(artist), clean_name(album)),
        (Some(artist), None) => format!("{}/{file}", clean_name(artist)),
        (None, _) => format!("{UPLOADS}/{file}"),
    }
}

/// `path`, or "<name> (2).<ext>", (3)... when it's taken.
pub fn free_path(path: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(path) {
        return path.to_string();
    }
    let (stem, ext) = match path.rfind('.').filter(|&dot| dot > path.rfind('/').map_or(0, |s| s + 1)) {
        Some(dot) => (&path[..dot], &path[dot..]),
        None => (path, ""),
    };
    (2..).map(|n| format!("{stem} ({n}){ext}")).find(|p| !taken(p)).expect("a free name")
}

// ---- the library's side

/// A local song that isn't on the share.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSong {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Its music folder's name.
    pub folder: String,
    pub size: i64,
}

/// Local copies of synced songs, hidden, waiting for the user's choice.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copies {
    pub songs: i64,
    pub bytes: i64,
}

/// A song's file and what its place on the share comes from.
struct SongFile {
    kind: FolderKind,
    location: String,
    path: String,
    folder_id: i64,
    album_artist: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

impl SongFile {
    fn file_name(&self) -> String {
        match self.kind {
            FolderKind::AndroidTree => file_name_of(&crate::library::linked::document_path(&self.path)),
            _ => file_name_of(&self.path.replace('\\', "/")),
        }
    }
}

/// What finding copies did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CopiesFound {
    /// Songs moved to sync whose synced copy is here now: their originals go.
    pub remove: Vec<i64>,
    /// Newly hidden copies (moved ones included).
    pub hidden: usize,
}

impl Library {
    fn song_file(&self, id: i64) -> Result<SongFile> {
        self.conn()
            .query_row(
                "SELECT f.kind, f.location, t.path, t.folder_id, t.tag_album_artist, t.artist_credit, t.tag_album
                 FROM tracks t JOIN folders f ON f.id = t.folder_id WHERE t.id = ?1",
                [id],
                |r| {
                    Ok(SongFile {
                        kind: FolderKind::parse(&r.get::<_, String>(0)?).unwrap_or(FolderKind::Path),
                        location: r.get(1)?,
                        path: r.get(2)?,
                        folder_id: r.get(3)?,
                        album_artist: r.get(4)?,
                        artist: r.get(5)?,
                        album: r.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("no song {id}"))
    }

    /// Local songs that aren't on the share (not in the synced folder, and no copy of a synced song), by
    /// artist and title.
    pub fn not_on_share(&self, folder: &Folder, songs: &Songs) -> Result<Vec<LocalSong>> {
        let names: HashMap<i64, String> = self.folders()?.into_iter().map(|f| (f.id, f.name)).collect();
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.artist_credit, t.tag_album, f.id, t.size
                 FROM tracks t JOIN folders f ON f.id = t.folder_id
                 WHERE t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled AND t.folder_id != ?1
                 ORDER BY t.artist_credit IS NULL, t.artist_credit COLLATE NOCASE, t.sort_title, t.id",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([folder.id], |r| {
                let folder = names.get(&r.get::<_, i64>(4)?).cloned().unwrap_or_default();
                Ok(LocalSong { id: r.get(0)?, title: r.get(1)?, artist: r.get(2)?, album: r.get(3)?, folder, size: r.get(5)? })
            })
            .map_err(err)?;
        let all: Vec<LocalSong> = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
        Ok(all.into_iter().filter(|s| songs.synced_copy(s.id).is_none()).collect())
    }

    /// Carries a song's likes, plays and playlist entries over to another (its synced copy).
    pub fn carry_over(&self, from: i64, to: i64) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        // A like: kept when the synced song has none (its own like or unlike is newer news).
        tx.execute(
            "INSERT INTO likes (track_id, liked_at) SELECT ?2, liked_at FROM likes WHERE track_id = ?1
             AND NOT EXISTS (SELECT 1 FROM unlikes WHERE track_id = ?2) ON CONFLICT DO NOTHING",
            params![from, to],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO track_stats (track_id, play_count, skip_count, last_played_at)
             SELECT ?2, play_count, skip_count, last_played_at FROM track_stats WHERE track_id = ?1
             ON CONFLICT (track_id) DO UPDATE SET play_count = play_count + excluded.play_count,
                 skip_count = skip_count + excluded.skip_count,
                 last_played_at = max(coalesce(last_played_at, 0), coalesce(excluded.last_played_at, 0))",
            params![from, to],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO play_days (track_id, day, plays) SELECT ?2, day, plays FROM play_days WHERE track_id = ?1
             ON CONFLICT (track_id, day) DO UPDATE SET plays = plays + excluded.plays",
            params![from, to],
        )
        .map_err(err)?;
        for sql in [
            "DELETE FROM likes WHERE track_id = ?1",
            "DELETE FROM track_stats WHERE track_id = ?1",
            "DELETE FROM play_days WHERE track_id = ?1",
        ] {
            tx.execute(sql, [from]).map_err(err)?;
        }
        tx.execute("UPDATE plays SET track_id = ?2 WHERE track_id = ?1", params![from, to]).map_err(err)?;
        // Melue's own playlists (a linked playlist follows its file).
        tx.execute(
            "UPDATE playlist_entries SET track_id = ?2
             WHERE track_id = ?1 AND playlist_id IN (SELECT id FROM playlists WHERE kind = 'app')",
            params![from, to],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }

    /// Finds local copies of synced songs and hides them (their data carried over to the synced song): songs
    /// moved to sync whose synced copy is here now, and songs that are the same as a synced one. Shows copies
    /// again whose synced song is gone.
    pub fn find_copies(&self, folder: &Folder, songs: &Songs) -> Result<CopiesFound> {
        let mut found = CopiesFound::default();
        // The synced song gone (or not synced any more): the copy is shown again.
        self.conn()
            .execute(
                "UPDATE tracks SET copy_of = NULL, copy_kept = 0 WHERE copy_of IS NOT NULL AND copy_of IN
                     (SELECT id FROM tracks WHERE missing_since IS NOT NULL OR folder_id != ?1)",
                [folder.id],
            )
            .map_err(err)?;
        // Moved to sync: the synced copy is in the library.
        let moved: Vec<(i64, String)> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT track_id, share_path FROM moves").map_err(err)?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        for (track, path) in moved {
            let Some(synced) = songs.at_path(&path) else { continue };
            self.hide_copy(track, synced, true)?;
            self.conn().execute("DELETE FROM moves WHERE track_id = ?1", [track]).map_err(err)?;
            found.remove.push(track);
            found.hidden += 1;
        }
        // The same song as a synced one.
        let candidates: Vec<i64> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare(
                    "SELECT t.id FROM tracks t JOIN folders f ON f.id = t.folder_id
                     WHERE t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled AND t.folder_id != ?1",
                )
                .map_err(err)?;
            let rows = stmt.query_map([folder.id], |r| r.get(0)).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        for track in candidates {
            if let Some(synced) = songs.synced_copy(track) {
                self.hide_copy(track, synced, false)?;
                found.hidden += 1;
            }
        }
        Ok(found)
    }

    /// Hides a copy of a synced song, carrying its data over. `kept`: not asked about (it's going anyway).
    fn hide_copy(&self, copy: i64, synced: i64, kept: bool) -> Result<()> {
        self.carry_over(copy, synced)?;
        self.conn()
            .execute("UPDATE tracks SET copy_of = ?2, copy_kept = ?3 WHERE id = ?1", params![copy, synced, kept])
            .map(|_| ())
            .map_err(err)
    }

    /// Hidden copies waiting for the user's choice.
    pub fn copies_waiting(&self) -> Result<Copies> {
        self.conn()
            .query_row(
                "SELECT count(*), coalesce(sum(size), 0) FROM tracks WHERE copy_of IS NOT NULL AND NOT copy_kept AND missing_since IS NULL",
                [],
                |r| Ok(Copies { songs: r.get(0)?, bytes: r.get(1)? }),
            )
            .map_err(err)
    }

    fn waiting_copy_ids(&self) -> Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT id FROM tracks WHERE copy_of IS NOT NULL AND NOT copy_kept AND missing_since IS NULL").map_err(err)?;
        let rows = stmt.query_map([], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Keeps the hidden copies (not asked about again).
    pub fn keep_copies(&self) -> Result<()> {
        self.conn().execute("UPDATE tracks SET copy_kept = 1 WHERE copy_of IS NOT NULL", []).map(|_| ()).map_err(err)
    }

    fn record_move(&self, track: i64, share_path: &str) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO moves (track_id, share_path, moved_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT DO UPDATE SET share_path = ?2, moved_at = ?3",
                params![track, share_path, now_ms()],
            )
            .map(|_| ())
            .map_err(err)
    }
}

// ---- moving (with the share)

/// What moving songs to sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveResult {
    /// Put on the share (and in the synced folder).
    pub moved: u32,
    /// On the share already (the same song): only the original goes.
    pub already_there: u32,
    /// Songs that couldn't be moved: (title or file, why). Left as they were.
    pub failed: Vec<(String, String)>,
    pub cancelled: bool,
}

/// What to move: a song in the library (it's removed after), or a file (copied: it stays where it is).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ToMove {
    Song { id: i64 },
    File { path: String },
}

/// Moves songs to sync: to the share, and into the synced folder here; a song's original goes once its synced
/// copy is in the library (after the scan this starts). One at a time with the sync (its progress shows like
/// a sync's).
pub fn move_to_sync(app: &AppHandle, items: Vec<ToMove>) -> std::result::Result<MoveResult, SyncFailure> {
    let (settings, share) = with_stores(app, |library, stores| setup::current(library, stores, &setup::open))
        .and_then(|r| r)
        .map_err(|error| SyncFailure::Setup { error })?;
    let library = app.state::<Library>();
    let folder = library.folder(settings.sync_folder_id)?.ok_or_else(|| "the synced folder isn't in the library any more".to_string())?;
    // Played straight from a drive: the share is the synced folder (no copy here).
    let target = match settings.setup {
        ShareSetup::Drive { stream: true, .. } => None,
        _ => Some(local::target(app, &folder)?),
    };
    let songs = library.songs_here(&folder, &handoff::device_id(&library)?)?;
    let layout = library.sync_settings()?.share_layout;
    let result = scheduler::exclusive(app, |report| {
        let mut result = MoveResult::default();
        let total = items.len() as u32;
        for (done, item) in items.iter().enumerate() {
            if scheduler::cancelled(app) {
                result.cancelled = true;
                break;
            }
            let label = match item {
                ToMove::Song { id } => library.conn().query_row("SELECT title FROM tracks WHERE id = ?1", [id], |r| r.get(0)).unwrap_or_default(),
                ToMove::File { path } => file_name_of(&path.replace('\\', "/")),
            };
            report(&Progress { files_done: done as u32, files_total: total, current: Some(label.clone()), ..Default::default() });
            match move_one(app, &library, share.as_ref(), target.as_deref(), &songs, layout, item) {
                Ok(true) => result.moved += 1,
                Ok(false) => result.already_there += 1,
                Err(e) => result.failed.push((label, e)),
            }
        }
        Ok(result)
    })?;
    // The synced copies come into the library; then the originals go (see after_scan).
    scanner::scan_soon(app);
    Ok(result)
}

/// Opens the file to move (once for the share, once for the synced folder here).
type OpenFile = dyn Fn() -> Result<std::fs::File>;

/// Moves one: returns whether it was put on the share (false: the same song was there already).
fn move_one(app: &AppHandle, library: &Library, share: &dyn Share, target: Option<&dyn Share>, songs: &Songs, layout: Layout, item: &ToMove) -> Result<bool> {
    // A song on this computer's disk can be moved into the synced folder itself (no second copy).
    let mut original: Option<std::path::PathBuf> = None;
    let (open, dest, song): (Box<OpenFile>, String, Option<i64>) = match item {
        ToMove::Song { id } => {
            // A copy of a synced song: nothing to put on the share, only the original goes.
            if let Some(synced) = songs.synced_copy(*id) {
                library.hide_copy(*id, synced, true)?;
                let path = library.conn().query_row("SELECT path FROM tracks WHERE id = ?1", [synced], |r| r.get::<_, String>(0)).map_err(err)?;
                let folder = library.synced_folder()?.ok_or("no share is set up")?;
                library.record_move(*id, &share_path(&folder, &path).unwrap_or(path))?;
                return Ok(false);
            }
            let file = library.song_file(*id)?;
            let dest = layout_path(layout, file.album_artist.as_deref(), file.artist.as_deref(), file.album.as_deref(), &file.file_name());
            if file.kind == FolderKind::Path {
                original = Some(std::path::Path::new(&file.location).join(&file.path));
            }
            let job = TagJob { track_id: *id, kind: file.kind, location: file.location.clone(), path: file.path.clone(), folder_cover: None };
            let app = app.clone();
            (Box::new(move || scanner::open_file(&app, &job, &job.path)), dest, Some(*id))
        }
        ToMove::File { path } => {
            let mut file = std::fs::File::open(path).map_err(|e| format!("can't open {path}: {e}"))?;
            let tags = crate::library::tags::read_file(&mut file).unwrap_or_default();
            let name = file_name_of(&path.replace('\\', "/"));
            let dest = layout_path(layout, tags.album_artist.as_deref(), tags.artist.as_deref(), tags.album.as_deref(), &name);
            let path = path.clone();
            (Box::new(move || std::fs::File::open(&path).map_err(|e| format!("can't open {path}: {e}"))), dest, None)
        }
    };
    let dest = free_path(&dest, |p| share.stat(p).ok().flatten().is_some());
    if let Some(parent) = dest.rfind('/').map(|i| &dest[..i]) {
        share.create_dir_all(parent)?;
    }
    share.write_from(&dest, &mut open()?, &mut |_| {})?;
    let on_share = share.stat(&dest)?.ok_or("the share lost the song right after it was written")?;
    match target {
        Some(target) => {
            if let Some(parent) = dest.rfind('/').map(|i| &dest[..i]) {
                target.create_dir_all(parent)?;
            }
            // On the same drive: moved (instant, no second copy, nothing for the Recycle Bin); else copied.
            if !original.is_some_and(|from| move_into(library, &from, &dest)) {
                target.write_from(&dest, &mut open()?, &mut |_| {})?;
            }
            let here = target.stat(&dest)?.ok_or("the synced folder lost the song right after it was written")?;
            library.record_synced(&on_share, &here)?;
        }
        None => library.record_synced(&on_share, &on_share)?,
    }
    if let Some(song) = song {
        library.record_move(song, &dest)?;
    }
    Ok(true)
}

/// Moves a song's file on this computer into the synced folder (a disk folder), when they're on the same drive.
pub(crate) fn move_into(library: &Library, from: &std::path::Path, dest: &str) -> bool {
    let Ok(Some(folder)) = library.synced_folder() else { return false };
    if folder.kind != FolderKind::Path {
        return false;
    }
    let to = std::path::Path::new(&folder.location).join(dest);
    to.parent().is_some_and(|dir| std::fs::create_dir_all(dir).is_ok()) && std::fs::rename(from, &to).is_ok()
}

/// Removes songs' files (originals moved to sync, or copies the user cleared): to the Recycle Bin on a
/// computer, Android's trash for all music on the phone (Android asks first), deleted in a chosen folder.
/// Returns how many were removed or handed to Android.
pub fn remove_files(app: &AppHandle, tracks: &[i64]) -> Result<usize> {
    let library = app.state::<Library>();
    let mut by_folder: HashMap<i64, Vec<SongFile>> = HashMap::new();
    for &track in tracks {
        let file = library.song_file(track)?;
        by_folder.entry(file.folder_id).or_default().push(file);
    }
    let mut removed = 0;
    for (folder_id, files) in by_folder {
        let Some(folder) = library.folder(folder_id)? else { continue };
        match folder.kind {
            FolderKind::Path => {
                for file in &files {
                    let full = std::path::Path::new(&folder.location).join(&file.path);
                    // Moved into the synced folder already (the same drive): nothing left to remove.
                    if !full.exists() {
                        continue;
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    let gone = trash::delete(&full).map_err(|e| e.to_string());
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    let gone = std::fs::remove_file(&full).map_err(|e| e.to_string());
                    match gone {
                        Ok(()) => removed += 1,
                        Err(e) => eprintln!("melue: can't remove {}: {e}", full.display()),
                    }
                }
            }
            FolderKind::AndroidMedia => {
                #[cfg(target_os = "android")]
                {
                    use tauri_plugin_melue_android::MelueAndroidExt;
                    let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
                    removed += app.melue_android().trash_music(&paths)? as usize;
                }
            }
            FolderKind::AndroidTree => {
                let target = local::target(app, &folder)?;
                for file in &files {
                    let Some(relative) = share_path(&folder, &file.path) else { continue };
                    match target.delete(&relative) {
                        Ok(()) => removed += 1,
                        Err(e) => eprintln!("melue: can't remove {relative}: {e}"),
                    }
                }
            }
        }
    }
    scanner::scan_soon(app);
    Ok(removed)
}

/// After each scan: hides copies of synced songs (their data carried over), removes the originals of songs
/// moved to sync whose synced copy is in now, and tells the UI about copies waiting for a choice.
pub fn after_scan(app: &AppHandle) {
    let library = app.state::<Library>();
    let Ok(Some(folder)) = library.synced_folder() else { return };
    let Ok(me) = handoff::device_id(&library) else { return };
    let found = changes::tracking(app, ChangeReason::Scan, false, || {
        let songs = library.songs_here(&folder, &me)?;
        library.find_copies(&folder, &songs)
    });
    match found {
        Ok(found) => {
            if !found.remove.is_empty() {
                if let Err(e) = remove_files(app, &found.remove) {
                    eprintln!("melue: can't remove the originals of songs moved to sync: {e}");
                }
            }
            if found.hidden > 0 {
                emit_copies(app);
            }
            let songs = library.songs_here(&folder, &me);
            if let Err(e) = songs.and_then(|songs| move_new_songs(app, &folder, &songs)) {
                eprintln!("melue: can't move new songs to sync: {e}");
            }
            super::layout::tidy_new_songs(app);
        }
        Err(e) => eprintln!("melue: can't look for copies of synced songs: {e}"),
    }
}

/// The setting "Move new songs to sync automatically": when it was switched on (songs added after that move).
const MOVE_NEW_SINCE: &str = "move_new_songs_since";

/// The setting switched on or off: songs added from now on are moved.
pub fn moving_new_songs(library: &Library, on: bool) -> Result<()> {
    match on {
        true if library.setting(MOVE_NEW_SINCE)?.is_none() => library.set_setting(MOVE_NEW_SINCE, &now_ms().to_string()),
        true => Ok(()),
        false => library.conn().execute("DELETE FROM settings WHERE key = ?1", [MOVE_NEW_SINCE]).map(|_| ()).map_err(err),
    }
}

/// With the setting on: moves songs added to the other music folders since it was switched on (in the
/// background).
fn move_new_songs(app: &AppHandle, folder: &Folder, songs: &Songs) -> Result<()> {
    let library = app.state::<Library>();
    if !library.sync_settings()?.move_new_songs {
        return Ok(());
    }
    let Some(since) = library.setting(MOVE_NEW_SINCE)?.and_then(|s| s.parse::<i64>().ok()) else { return Ok(()) };
    let new: Vec<i64> = {
        let conn = library.conn();
        let mut stmt = conn.prepare("SELECT id FROM tracks WHERE added_at > ?1").map_err(err)?;
        let rows = stmt.query_map([since], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)?
    };
    let items: Vec<ToMove> = library
        .not_on_share(folder, songs)?
        .into_iter()
        .filter(|s| new.contains(&s.id))
        .map(|s| ToMove::Song { id: s.id })
        .collect();
    if items.is_empty() {
        return Ok(());
    }
    let app = app.clone();
    std::thread::spawn(move || match move_to_sync(&app, items) {
        Ok(r) => eprintln!("melue: moved {} new songs to sync ({} failed)", r.moved, r.failed.len()),
        Err(e) => eprintln!("melue: can't move new songs to sync: {e:?}"),
    });
    Ok(())
}

/// What a look for copies found (the spike's "Look for copies now").
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyCheck {
    pub synced_songs: usize,
    pub other_songs: usize,
    /// Copies hidden by this look.
    pub hidden_now: usize,
    /// All hidden copies, and those waiting for a choice.
    pub hidden: i64,
    pub waiting: Copies,
    /// Songs with a synced song's title that weren't taken as copies, and why (artist, length).
    pub near_misses: Vec<String>,
}

/// Looks for copies of synced songs now (as after a scan), and says what it found.
pub fn check_copies(app: &AppHandle) -> Result<CopyCheck> {
    let library = app.state::<Library>();
    let folder = library.synced_folder()?.ok_or("no share is set up")?;
    let me = handoff::device_id(&library)?;
    let found = changes::tracking(app, ChangeReason::Scan, false, || {
        let songs = library.songs_here(&folder, &me)?;
        library.find_copies(&folder, &songs)
    })?;
    if !found.remove.is_empty() {
        remove_files(app, &found.remove)?;
    }
    emit_copies(app);
    let songs = library.songs_here(&folder, &me)?;
    let (synced_songs, other_songs) = songs.counts();
    let hidden = library.conn().query_row("SELECT count(*) FROM tracks WHERE copy_of IS NOT NULL", [], |r| r.get(0)).map_err(err)?;
    Ok(CopyCheck { synced_songs, other_songs, hidden_now: found.hidden, hidden, waiting: library.copies_waiting()?, near_misses: songs.near_misses() })
}

/// Tells the UI the hidden copies waiting for a choice (the `song-copies` event).
pub fn emit_copies(app: &AppHandle) {
    if let Ok(copies) = app.state::<Library>().copies_waiting() {
        let _ = app.emit(COPIES_EVENT, &copies);
    }
}

/// Clear space: removes the hidden copies waiting for a choice.
pub fn clear_copies(app: &AppHandle) -> Result<usize> {
    let ids = app.state::<Library>().waiting_copy_ids()?;
    let removed = remove_files(app, &ids)?;
    // Removed or not (Android's dialog can be turned down), they're not asked about again.
    app.state::<Library>().keep_copies()?;
    emit_copies(app);
    Ok(removed)
}
