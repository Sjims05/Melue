//! The library's commands for the UI. They run off the main thread (`async`), so a slow query never
//! freezes the window.

use serde::Serialize;
use tauri::{AppHandle, State};

use super::scan::ScanSummary;
use super::changes::{self, tracking, ChangeReason, LibraryChange};
use super::playlists::{Playlist, PlaylistEntry};
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
pub fn library_set_setting(library: State<'_, Library>, key: String, value: String) -> Result<()> {
    library.set_setting(&key, &value)
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
    let tracks = library.playlist_entries(id)?.into_iter().map(|e| e.track.id).collect();
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
    Ok(added)
}

/// Removes entries (by entry id: one copy of a song that's in twice).
#[tauri::command(async)]
pub fn library_remove_from_playlist(app: AppHandle, library: State<'_, Library>, id: i64, entries: Vec<i64>) -> Result<()> {
    let removed = library.remove_from_playlist(id, &entries)?;
    changes::emit(&app, LibraryChange::playlist(id, removed));
    Ok(())
}

#[tauri::command(async)]
pub fn library_move_in_playlist(app: AppHandle, library: State<'_, Library>, id: i64, entry: i64, position: usize) -> Result<()> {
    let track = library.move_in_playlist(id, entry, position)?;
    changes::emit(&app, LibraryChange::playlist(id, vec![track]));
    Ok(())
}
