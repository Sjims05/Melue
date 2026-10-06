//! What adding and removing folders needs from Android: the permission to read the music on the phone
//! (for all music on the phone), and keeping access to folders chosen with the storage picker across
//! restarts. Elsewhere these are refused (those folder kinds are Android's).

use tauri::AppHandle;

use super::Result;

/// The setting for covers next to the songs (cover.jpg, folder.jpg) with all music on the phone: "1" when
/// on. Reading those needs the photos permission, so it's off unless the user switches it on.
pub const FOLDER_COVERS_SETTING: &str = "android_folder_covers";

/// Whether Melue may read the photos (images) on the phone, asking first when `request`: needed for
/// covers next to the songs with all music on the phone. Desktop: always granted.
pub fn photos_access(app: &AppHandle, request: bool) -> Result<String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        let media = app.melue_android();
        let state = if request { media.request_photos_permission()? } else { media.photos_permission()? };
        Ok(serde_json_name(state))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, request);
        Ok("granted".into())
    }
}

/// Whether Melue may read the music on the phone ("granted", "denied", "prompt", ...), asking first
/// when `request`; and how many songs the music index has once it may. Desktop: always granted.
pub fn music_access(app: &AppHandle, request: bool) -> Result<(String, Option<u64>)> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::{MelueAndroidExt, PermissionState};
        let media = app.melue_android();
        let state = if request { media.request_music_permission()? } else { media.music_permission()? };
        let count = if state == PermissionState::Granted { Some(media.count_music()?) } else { None };
        let name = serde_json_name(state);
        Ok((name, count))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, request);
        Ok(("granted".into(), None))
    }
}

#[cfg(target_os = "android")]
fn serde_json_name(state: tauri_plugin_melue_android::PermissionState) -> String {
    serde_json::to_value(state).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

/// All music on the phone needs the permission: asks for it, and refuses when it's not given.
pub fn ensure_music_permission(app: &AppHandle) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        let (state, _) = music_access(app, true)?;
        if state != "granted" {
            return Err("Melue needs permission to read the music on this phone (Settings → Apps → Melue → Permissions → Music and audio)".into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("all music on the phone is only for Android".into())
    }
}

/// A chosen folder's top address, the one Android grants lasting access to: the picker can give the folder
/// as ".../tree/<id>/document/<id>"; its top is ".../tree/<id>".
#[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
pub(crate) fn top_tree(uri: &str) -> String {
    uri.split_once("/document/").map_or(uri, |(top, _)| top).to_string()
}

/// A folder chosen with the storage picker: keeps Melue's access to it after the app restarts.
pub fn keep_folder_access(app: &AppHandle, uri: &str) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        let top = top_tree(uri);
        let uri = FsUri { uri: top.clone(), document_top_tree_uri: Some(top) };
        app.android_fs().picker().persist_uri_permission(&uri).map_err(|e| format!("can't keep access to that folder: {e}"))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, uri);
        Err("chosen Android folders are only for Android".into())
    }
}

/// The folder was removed from the library: gives its access back.
pub fn release_folder_access(app: &AppHandle, uri: &str) {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        let top = top_tree(uri);
        let uri = FsUri { uri: top.clone(), document_top_tree_uri: Some(top) };
        let _ = app.android_fs().picker().release_persisted_uri_permission(&uri);
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, uri);
}

/// Every song in Android's music index, as found files (their paths are the files' absolute paths), and
/// the playlist files it knows. With `folder_covers`, songs get the cover images next to them (with the
/// photos permission; without it, the images just aren't seen).
pub fn list_music(app: &AppHandle, folder_covers: bool) -> Result<super::scan::Listing> {
    #[cfg(target_os = "android")]
    {
        use std::collections::HashMap;
        use std::path::Path;
        use tauri_plugin_melue_android::MelueAndroidExt;
        let media = app.melue_android();
        let files = media.list_music()?;
        // Playlist files may not be readable with the music permission alone: then there are none.
        let playlists = media.list_playlists().unwrap_or_default();
        let mut covers: HashMap<String, Option<super::scan::FolderCover>> = HashMap::new();
        let songs = files
            .into_iter()
            .map(|f| {
                let folder_cover = if folder_covers {
                    let dir = Path::new(&f.path).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
                    covers.entry(dir).or_insert_with_key(|dir| super::scan::find_folder_cover(Path::new(dir))).clone()
                } else {
                    None
                };
                super::scan::FoundFile { path: f.path, name: f.name, size: f.size, modified_at: f.modified_at, folder_cover }
            })
            .collect();
        let playlists = playlists
            .into_iter()
            .map(|f| super::scan::FoundFile { path: f.path, name: f.name, size: f.size, modified_at: f.modified_at, folder_cover: None })
            .collect();
        Ok(super::scan::Listing { songs, playlists })
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, folder_covers);
        Err("all music on the phone is only for Android".into())
    }
}

/// The music and playlist files in a folder chosen with the storage picker (and its subfolders), as found
/// files whose paths are their content:// addresses. Hidden folders and folders with a `.nomedia` file are
/// skipped.
pub fn list_tree(app: &AppHandle, uri: &str) -> Result<super::scan::Listing> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, Entry, FsUri};
        let fs = app.android_fs();
        let root = FsUri { uri: uri.to_string(), document_top_tree_uri: Some(top_tree(uri)) };
        let mut found = super::scan::Listing::default();
        let mut stack = vec![root];
        let mut first = true;
        while let Some(dir) = stack.pop() {
            let entries = match fs.read_dir(&dir) {
                Ok(entries) => entries,
                // The chosen folder itself can't be read: unplugged SD card, or access lost.
                Err(e) if first => return Err(format!("can't read the folder (unplugged, or access lost?): {e}")),
                Err(_) => continue,
            };
            first = false;
            if entries.iter().any(|e| matches!(e, Entry::File { name, .. } if name == ".nomedia")) && dir.uri != uri {
                continue;
            }
            let mut songs = Vec::new();
            let mut images = Vec::new();
            for entry in entries {
                match entry {
                    Entry::Dir { uri, name, .. } if !name.starts_with('.') => stack.push(uri),
                    Entry::File { uri, name, len, last_modified, .. } if !name.starts_with('.') => {
                        let modified_at = last_modified
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis() as i64);
                        if super::scan::is_audio(&name) {
                            let size = len as i64;
                            songs.push(super::scan::FoundFile { path: uri.uri, name, size, modified_at, folder_cover: None });
                        } else if super::scan::cover_rank(&name).is_some() {
                            images.push((name, super::scan::FolderCover { path: uri.uri, modified_at }));
                        } else if super::m3u::is_playlist(&name) {
                            let size = len as i64;
                            found.playlists.push(super::scan::FoundFile { path: uri.uri, name, size, modified_at, folder_cover: None });
                        }
                    }
                    _ => {}
                }
            }
            let cover = super::scan::pick_cover(images);
            found.songs.extend(songs.into_iter().map(|song| super::scan::FoundFile { folder_cover: cover.clone(), ..song }));
        }
        Ok(found)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, uri);
        Err("chosen Android folders are only for Android".into())
    }
}

/// Opens a file in a folder chosen with the storage picker (`path` is its content:// address).
pub fn open_tree_file(app: &AppHandle, location: &str, path: &str) -> Result<std::fs::File> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        let uri = FsUri { uri: path.to_string(), document_top_tree_uri: Some(top_tree(location)) };
        app.android_fs().open_file_readable(&uri).map_err(|e| format!("can't open {path}: {e}"))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, location, path);
        Err("chosen Android folders are only for Android".into())
    }
}

/// Replaces a file's contents in a folder chosen with the storage picker; returns its new size and
/// modified time.
pub fn write_tree_file(app: &AppHandle, location: &str, path: &str, bytes: &[u8]) -> Result<(i64, i64)> {
    #[cfg(target_os = "android")]
    {
        use std::io::Write;
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        let uri = FsUri { uri: path.to_string(), document_top_tree_uri: Some(top_tree(location)) };
        let fs = app.android_fs();
        let mut file = fs.open_file_writable(&uri).map_err(|e| format!("can't write {path}: {e}"))?;
        file.write_all(bytes).map_err(|e| format!("can't write {path}: {e}"))?;
        drop(file);
        let meta = fs.get_metadata(&uri).map_err(|e| format!("can't read {path}: {e}"))?;
        Ok((meta.len() as i64, super::scan::modified_ms(&meta)))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, location, path, bytes);
        Err("chosen Android folders are only for Android".into())
    }
}

/// Hides a chosen folder's music from other apps (a `.nomedia` file in it: Android's music index skips the
/// folder) or shows it again.
pub fn set_hidden_from_other_apps(app: &AppHandle, location: &str, hidden: bool) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        let fs = app.android_fs();
        let dir = FsUri { uri: location.to_string(), document_top_tree_uri: Some(top_tree(location)) };
        let existing = fs.resolve_file_uri(&dir, ".nomedia").ok();
        match (hidden, existing) {
            (true, None) => fs.create_new_file(&dir, ".nomedia", None).map(|_| ()).map_err(|e| format!("can't hide the folder: {e}")),
            (false, Some(file)) => fs.remove_file(&file).map_err(|e| format!("can't show the folder: {e}")),
            _ => Ok(()),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, location, hidden);
        Err("hiding a folder from other apps is only for Android".into())
    }
}

/// Android: calls `handler` when the app comes back to the foreground. Elsewhere: nothing (desktop: see the
/// window's focus in lib.rs).
pub fn on_resume(app: &AppHandle, handler: impl Fn() + Send + Sync + 'static) {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        if let Err(e) = app.melue_android().listen("resumed", handler) {
            eprintln!("melue: can't follow the app coming back: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, handler);
}

/// Android: calls `handler` when the network the app uses changes (Wi-Fi, mobile data, a VPN coming up).
/// Elsewhere: nothing.
pub fn on_network_change(app: &AppHandle, handler: impl Fn() + Send + Sync + 'static) {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        if let Err(e) = app.melue_android().listen("networkChanged", handler) {
            eprintln!("melue: can't follow the network: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, handler);
}

/// Android: calls `handler` when Cancel is pressed on the "Syncing music" notification.
pub fn on_sync_cancel(app: &AppHandle, handler: impl Fn() + Send + Sync + 'static) {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        if let Err(e) = app.melue_android().listen("syncCancel", handler) {
            eprintln!("melue: can't follow the sync notification: {e}");
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, handler);
}

/// Android: calls `handler` when the music index changes or the app comes back to the foreground.
/// Elsewhere: nothing to listen to.
pub fn on_music_changes(app: &AppHandle, handler: impl Fn() + Send + Sync + Clone + 'static) {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        for event in ["musicChanged", "resumed"] {
            if let Err(e) = app.melue_android().listen(event, handler.clone()) {
                eprintln!("melue: can't follow Android's {event}: {e}");
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, handler);
}

#[cfg(test)]
mod tests {
    use super::top_tree;

    #[test]
    fn a_chosen_folder_s_top_address() {
        let top = "content://com.android.externalstorage.documents/tree/primary%3AMusic%2FMelue";
        assert_eq!(top_tree(top), top);
        assert_eq!(top_tree(&format!("{top}/document/primary%3AMusic%2FMelue")), top);
        assert_eq!(top_tree(&format!("{top}/document/primary%3AMusic%2FMelue%2FABBA%2F01.mp3")), top);
    }
}
