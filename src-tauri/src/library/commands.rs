//! The library's commands for the UI. They run off the main thread (`async`), so a slow query never
//! freezes the window.

use serde::Serialize;
use tauri::{AppHandle, State};

use super::scan::ScanSummary;
use super::changes::{self, tracking, ChangeReason, LibraryChange};
use super::follow::QueueFollow;
use super::playback::{Outcome, PlaybackEvent};
use super::playlists::{Playlist, PlaylistEntry};
use super::scanner::AppFiles;
use super::{android, scanner, Album, Artist, Folder, FolderKind, Library, Result, Track};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub schema_version: i64,
    pub folders: i64,
    /// Tracks whose files are there (not missing).
    pub tracks: i64,
    /// Albums and artists with songs that are there.
    pub albums: i64,
    pub artists: i64,
    pub playlists: i64,
}

#[tauri::command(async)]
pub fn library_status(library: State<'_, Library>) -> Result<LibraryStatus> {
    library.status()
}

/// An app setting saved in the library (null when it was never set).
#[tauri::command(async)]
pub fn library_setting(library: State<'_, Library>, key: String) -> Result<Option<String>> {
    library.setting(&key)
}

// ---- music folders

#[tauri::command(async)]
pub fn library_folders(library: State<'_, Library>) -> Result<Vec<Folder>> {
    library.folders()
}

/// Adds a music folder: a path; all music on the phone (asks for the permission); or a folder chosen
/// with Android's storage picker (keeps access to it).
#[tauri::command(async)]
pub fn library_add_folder(
    app: AppHandle,
    library: State<'_, Library>,
    kind: FolderKind,
    location: String,
    name: Option<String>,
) -> Result<Folder> {
    match kind {
        FolderKind::Path => {}
        FolderKind::AndroidMedia => android::ensure_music_permission(&app)?,
        FolderKind::AndroidTree => android::keep_folder_access(&app, &location)?,
    }
    let added = tracking(&app, ChangeReason::Folders, true, || library.add_folder(kind, &location, name.as_deref()));
    if added.is_err() && kind == FolderKind::AndroidTree {
        android::release_folder_access(&app, &location);
    }
    added
}

/// Removes a folder and its tracks from the library (the files themselves stay).
#[tauri::command(async)]
pub fn library_remove_folder(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<()> {
    let folder = library.folder(id)?.ok_or_else(|| format!("no folder {id}"))?;
    tracking(&app, ChangeReason::Folders, true, || library.remove_folder(id))?;
    if folder.kind == FolderKind::AndroidTree {
        android::release_folder_access(&app, &folder.location);
    }
    // Tidies up the covers only its songs used.
    scanner::scan_soon(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn library_set_folder_enabled(app: AppHandle, library: State<'_, Library>, id: i64, enabled: bool) -> Result<Folder> {
    tracking(&app, ChangeReason::Folders, true, || library.set_folder_enabled(id, enabled))
}

#[tauri::command(async)]
pub fn library_rename_folder(app: AppHandle, library: State<'_, Library>, id: i64, name: String) -> Result<Folder> {
    tracking(&app, ChangeReason::Folders, true, || library.rename_folder(id, &name))
}

/// Scans every switched-on folder (the refresh button) and returns what changed; null when a scan was
/// already running (it goes once more instead). Progress also comes as `library-scan` events.
#[tauri::command(async)]
pub fn library_scan(app: AppHandle) -> Result<Option<ScanSummary>> {
    scanner::scan_all(&app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicAccess {
    /// "granted", "denied", "prompt" (not asked yet) or "prompt-with-rationale" (denied once).
    pub permission: String,
    /// Android: how many songs the phone's music index has, once allowed.
    pub songs: Option<u64>,
}

/// Android: whether Melue may read the music on the phone (asking first when `request`). Desktop: granted.
#[tauri::command(async)]
pub fn library_music_access(app: AppHandle, request: bool) -> Result<MusicAccess> {
    let (permission, songs) = android::music_access(&app, request)?;
    Ok(MusicAccess { permission, songs })
}

#[tauri::command(async)]
pub fn library_set_setting(app: AppHandle, library: State<'_, Library>, key: String, value: String) -> Result<()> {
    let sync_before = library.sync_available()?;
    library.set_setting(&key, &value)?;
    if library.sync_available()? != sync_before {
        changes::emit(&app, LibraryChange::sync());
    }
    Ok(())
}

/// Whether sync is available: a share is set up (the UI shows sync buttons only then).
#[tauri::command(async)]
pub fn library_sync_available(library: State<'_, Library>) -> Result<bool> {
    library.sync_available()
}

// ---- songs

/// Songs that are there, by title (`limit` from `offset`).
#[tauri::command(async)]
pub fn library_tracks(library: State<'_, Library>, offset: u32, limit: u32) -> Result<Vec<Track>> {
    library.tracks(offset, limit)
}

/// Albums with songs that are there, by title.
#[tauri::command(async)]
pub fn library_albums(library: State<'_, Library>, offset: u32, limit: u32) -> Result<Vec<Album>> {
    library.albums(offset, limit)
}

/// Artists with songs that are there, by name (sort name: "The" ignored).
#[tauri::command(async)]
pub fn library_artists(library: State<'_, Library>, offset: u32, limit: u32) -> Result<Vec<Artist>> {
    library.artists(offset, limit)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCovers {
    /// Whether all music on the phone uses the cover images next to the songs (cover.jpg, folder.jpg).
    pub enabled: bool,
    /// The photos permission they need: "granted", "denied", "prompt"... Desktop: granted.
    pub permission: String,
}

/// Android, all music on the phone: whether cover images next to the songs are used (off unless switched on).
#[tauri::command(async)]
pub fn library_folder_covers(app: AppHandle, library: State<'_, Library>) -> Result<FolderCovers> {
    let enabled = library.setting(android::FOLDER_COVERS_SETTING)?.as_deref() == Some("1");
    Ok(FolderCovers { enabled, permission: android::photos_access(&app, false)? })
}

/// Switches cover images next to the songs on or off, then rescans. Switching on asks for the photos
/// permission, and is refused (stays off) without it.
#[tauri::command(async)]
pub fn library_set_folder_covers(app: AppHandle, library: State<'_, Library>, enabled: bool) -> Result<FolderCovers> {
    if enabled && android::photos_access(&app, true)? != "granted" {
        return Err("Melue needs permission to read photos for the cover images next to the songs".into());
    }
    library.set_setting(android::FOLDER_COVERS_SETTING, if enabled { "1" } else { "0" })?;
    scanner::scan_soon(&app);
    library_folder_covers(app, library)
}

// ---- playlists (each change is told to the UI as a library change)

/// Every playlist, by name.
#[tauri::command(async)]
pub fn library_playlists(library: State<'_, Library>) -> Result<Vec<Playlist>> {
    library.playlists()
}

#[tauri::command(async)]
pub fn library_playlist(library: State<'_, Library>, id: i64) -> Result<Playlist> {
    library.playlist(id)
}

/// A playlist's songs in order (unavailable ones too: shown greyed).
#[tauri::command(async)]
pub fn library_playlist_entries(library: State<'_, Library>, id: i64) -> Result<Vec<PlaylistEntry>> {
    library.playlist_entries(id)
}

#[tauri::command(async)]
pub fn library_create_playlist(app: AppHandle, library: State<'_, Library>, name: String) -> Result<Playlist> {
    let playlist = library.create_playlist(&name)?;
    changes::emit(&app, LibraryChange::playlist(playlist.id, Vec::new()));
    Ok(playlist)
}

#[tauri::command(async)]
pub fn library_rename_playlist(app: AppHandle, library: State<'_, Library>, id: i64, name: String) -> Result<Playlist> {
    let playlist = library.rename_playlist(id, &name)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    Ok(playlist)
}

#[tauri::command(async)]
pub fn library_delete_playlist(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<()> {
    let tracks = library.playlist_entries(id)?.into_iter().filter_map(|e| e.track.map(|t| t.id)).collect();
    library.delete_playlist(id)?;
    changes::emit(&app, LibraryChange::playlist(id, tracks));
    Ok(())
}

/// Adds songs at `position` (the end when left out); returns the new entries' ids.
#[tauri::command(async)]
pub fn library_add_to_playlist(
    app: AppHandle,
    library: State<'_, Library>,
    id: i64,
    tracks: Vec<i64>,
    position: Option<usize>,
) -> Result<Vec<i64>> {
    let added = library.add_to_playlist(id, &tracks, position)?;
    changes::emit(&app, LibraryChange::playlist(id, tracks));
    write_back(&app, &library, id)?;
    Ok(added)
}

/// Removes entries (by entry id: one copy of a song that's in twice).
#[tauri::command(async)]
pub fn library_remove_from_playlist(app: AppHandle, library: State<'_, Library>, id: i64, entries: Vec<i64>) -> Result<()> {
    let removed = library.remove_from_playlist(id, &entries)?;
    changes::emit(&app, LibraryChange::playlist(id, removed));
    write_back(&app, &library, id)
}

#[tauri::command(async)]
pub fn library_move_in_playlist(app: AppHandle, library: State<'_, Library>, id: i64, entry: i64, position: usize) -> Result<()> {
    let track = library.move_in_playlist(id, entry, position)?;
    changes::emit(&app, LibraryChange::playlist(id, track.into_iter().collect()));
    write_back(&app, &library, id)
}

/// After an edit of a linked playlist with write-back on: Melue's version into the file. The edit is saved
/// either way; a failure to write is reported.
fn write_back(app: &AppHandle, library: &Library, id: i64) -> Result<()> {
    match library.write_back_if_on(id, &AppFiles(app)) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("saved in Melue, but the playlist file couldn't be written: {e}")),
    }
}

/// Takes a greyed song of a linked playlist (removed here, or suggested by the file) into Melue's version.
#[tauri::command(async)]
pub fn library_restore_playlist_entry(app: AppHandle, library: State<'_, Library>, id: i64, entry: i64) -> Result<()> {
    let track = library.restore_playlist_entry(id, entry)?;
    changes::emit(&app, LibraryChange::playlist(id, track.into_iter().collect()));
    write_back(&app, &library, id)
}

/// A linked playlist's settings (each left as it is when null): songs new in the file 'added' or
/// 'suggested', songs removed from it 'removed' or 'kept', and writing Melue's changes into the file
/// (switching it on writes the file right away).
#[tauri::command(async)]
pub fn library_set_playlist_options(
    app: AppHandle,
    library: State<'_, Library>,
    id: i64,
    new_in_file: Option<String>,
    removed_from_file: Option<String>,
    write_back: Option<bool>,
) -> Result<Playlist> {
    library.set_playlist_options(id, new_in_file.as_deref(), removed_from_file.as_deref(), write_back)?;
    let result = if write_back == Some(true) { self::write_back(&app, &library, id) } else { Ok(()) };
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    result?;
    library.playlist(id)
}

/// Turns a linked playlist into an app playlist (the file is left alone and no longer followed).
#[tauri::command(async)]
pub fn library_unlink_playlist(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<Playlist> {
    library.unlink_playlist(id)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    library.playlist(id)
}

// ---- playing (see playback.rs)

/// The queue for a list ("songs", "album:<id>", "artist:<id>", "playlist:<id>", "liked"), ready to play.
#[tauri::command(async)]
pub fn library_queue_items(library: State<'_, Library>, source: String) -> Result<Vec<tauri_plugin_native_audio::QueueItem>> {
    library.queue_items(&source)
}

/// Counts one of the player's playback events (once, however often it's handed over): "play", "skip" or
/// "neither" for a song being left, null otherwise.
#[tauri::command(async)]
pub fn library_record_playback(library: State<'_, Library>, event: PlaybackEvent) -> Result<Option<Outcome>> {
    library.record_playback(&event)
}

#[tauri::command(async)]
pub fn library_set_liked(app: AppHandle, library: State<'_, Library>, id: i64, liked: bool) -> Result<()> {
    library.set_liked(id, liked)?;
    changes::emit(&app, LibraryChange::liked(id));
    Ok(())
}

/// Every liked song's id (for the player's like button).
#[tauri::command(async)]
pub fn library_liked_ids(library: State<'_, Library>) -> Result<Vec<i64>> {
    library.liked_ids()
}

#[tauri::command(async)]
pub fn library_recently_played(library: State<'_, Library>, limit: u32) -> Result<Vec<Track>> {
    library.recently_played(limit)
}

#[tauri::command(async)]
pub fn library_most_played(library: State<'_, Library>, limit: u32) -> Result<Vec<Track>> {
    library.most_played(limit)
}

/// How the queue follows library changes (on, and the player's updateQueue options).
#[tauri::command(async)]
pub fn library_queue_follow(library: State<'_, Library>) -> Result<QueueFollow> {
    library.queue_follow()
}

#[tauri::command(async)]
pub fn library_set_queue_follow(library: State<'_, Library>, settings: QueueFollow) -> Result<QueueFollow> {
    library.set_queue_follow(&settings)?;
    library.queue_follow()
}
