//! The library's commands for the UI. They run off the main thread (`async`), so a slow query never
//! freezes the window.

use serde::Serialize;
use tauri::{AppHandle, State};

use super::{android, Folder, FolderKind, Library, Result};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub schema_version: i64,
    pub folders: i64,
    /// Tracks whose files are there (not missing).
    pub tracks: i64,
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
    let added = library.add_folder(kind, &location, name.as_deref());
    if added.is_err() && kind == FolderKind::AndroidTree {
        android::release_folder_access(&app, &location);
    }
    added
}

/// Removes a folder and its tracks from the library (the files themselves stay).
#[tauri::command(async)]
pub fn library_remove_folder(app: AppHandle, library: State<'_, Library>, id: i64) -> Result<()> {
    let folder = library.folder(id)?.ok_or_else(|| format!("no folder {id}"))?;
    library.remove_folder(id)?;
    if folder.kind == FolderKind::AndroidTree {
        android::release_folder_access(&app, &folder.location);
    }
    Ok(())
}

#[tauri::command(async)]
pub fn library_set_folder_enabled(library: State<'_, Library>, id: i64, enabled: bool) -> Result<Folder> {
    library.set_folder_enabled(id, enabled)
}

#[tauri::command(async)]
pub fn library_rename_folder(library: State<'_, Library>, id: i64, name: String) -> Result<Folder> {
    library.rename_folder(id, &name)
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
