//! What adding and removing folders needs from Android: the permission to read the music on the phone
//! (for all music on the phone), and keeping access to folders chosen with the storage picker across
//! restarts. Elsewhere these are refused (those folder kinds are Android's).

use tauri::AppHandle;

use super::Result;

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

/// A folder chosen with the storage picker: keeps Melue's access to it after the app restarts.
pub fn keep_folder_access(app: &AppHandle, uri: &str) -> Result<()> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
        let uri = FsUri { uri: uri.to_string(), document_top_tree_uri: Some(uri.to_string()) };
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
        let uri = FsUri { uri: uri.to_string(), document_top_tree_uri: Some(uri.to_string()) };
        let _ = app.android_fs().picker().release_persisted_uri_permission(&uri);
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, uri);
}

/// Every song in Android's music index, as found files (their paths are the files' absolute paths).
pub fn list_music(app: &AppHandle) -> Result<Vec<super::scan::FoundFile>> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        let files = app.melue_android().list_music()?;
        Ok(files
            .into_iter()
            .map(|f| super::scan::FoundFile { path: f.path, name: f.name, size: f.size, modified_at: f.modified_at })
            .collect())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("all music on the phone is only for Android".into())
    }
}

/// The music files in a folder chosen with the storage picker (and its subfolders), as found files whose
/// paths are their content:// addresses. Hidden folders and folders with a `.nomedia` file are skipped.
pub fn list_tree(app: &AppHandle, uri: &str) -> Result<Vec<super::scan::FoundFile>> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, Entry, FsUri};
        let fs = app.android_fs();
        let root = FsUri { uri: uri.to_string(), document_top_tree_uri: Some(uri.to_string()) };
        let mut found = Vec::new();
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
            for entry in entries {
                match entry {
                    Entry::Dir { uri, name, .. } if !name.starts_with('.') => stack.push(uri),
                    Entry::File { uri, name, len, last_modified, .. } if !name.starts_with('.') && super::scan::is_audio(&name) => {
                        let modified_at = last_modified
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis() as i64);
                        found.push(super::scan::FoundFile { path: uri.uri, name, size: len as i64, modified_at });
                    }
                    _ => {}
                }
            }
        }
        Ok(found)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, uri);
        Err("chosen Android folders are only for Android".into())
    }
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
