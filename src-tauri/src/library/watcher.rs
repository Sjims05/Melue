//! Watching the music folders (a computer): a song added, changed, moved or deleted in one (or a playlist file)
//! starts a scan once things have been quiet for a moment (scanner.rs), so it shows up without Rescan; a song a
//! playlist was missing fills in by itself. The watched folders follow the switched-on ones at each scan. A folder
//! that can't be watched (some network drives) is scanned as before: at start-up, after a sync, and with Rescan.
//! Android tells Melue itself when its music changes (android.rs).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Manager};

use super::{m3u, scan, FolderKind, Library};

/// The watcher, and the folders it watches (by folder id).
static WATCHING: Mutex<Option<(RecommendedWatcher, HashMap<i64, PathBuf>)>> = Mutex::new(None);

/// Whether a change is one a scan would see: a song or playlist file, or a folder (moved or deleted whole). Not
/// hidden files and folders (the scan skips them; Melue's own `.melue` folder on the share), not downloads still
/// being written (their own names until they're done), and not only its attributes changing.
fn matters(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_))) {
        return false;
    }
    event.paths.iter().any(|path| {
        let hidden = path.components().any(|c| c.as_os_str().to_str().is_some_and(|c| c.starts_with('.') && c != "." && c != ".."));
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        !hidden && (scan::is_audio(name) || m3u::is_playlist(name) || Path::new(name).extension().is_none())
    })
}

/// Watches the switched-on music folders on disk (and stops watching the others).
pub fn refresh(app: &AppHandle) {
    let wanted: HashMap<i64, PathBuf> = match app.state::<Library>().folders() {
        Ok(folders) => folders.into_iter().filter(|f| f.enabled && f.kind == FolderKind::Path).map(|f| (f.id, PathBuf::from(f.location))).collect(),
        Err(e) => return eprintln!("melue: can't watch the music folders: {e}"),
    };
    let mut watching = WATCHING.lock().unwrap();
    if watching.is_none() {
        let handle = app.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            if event.is_ok_and(|e| matters(&e)) {
                super::scanner::scan_soon(&handle);
            }
        });
        match watcher {
            Ok(watcher) => *watching = Some((watcher, HashMap::new())),
            Err(e) => return eprintln!("melue: can't watch the music folders: {e}"),
        }
    }
    let Some((watcher, watched)) = watching.as_mut() else { return };
    watched.retain(|id, path| {
        let keep = wanted.get(id) == Some(path);
        if !keep {
            let _ = watcher.unwatch(path);
        }
        keep
    });
    for (id, path) in wanted {
        if watched.contains_key(&id) {
            continue;
        }
        match watcher.watch(&path, RecursiveMode::Recursive) {
            Ok(()) => {
                watched.insert(id, path);
            }
            Err(e) => eprintln!("melue: can't watch {}: {e} (it's scanned at start-up and with Rescan)", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind, RenameMode};

    fn event(kind: EventKind, path: &str) -> Event {
        Event::new(kind).add_path(PathBuf::from(path))
    }

    #[test]
    fn songs_playlists_and_folders_matter_downloads_and_hidden_files_dont() {
        let created = EventKind::Create(CreateKind::File);
        assert!(matters(&event(created, r"D:\Music\Band\Song.flac")));
        assert!(matters(&event(created, r"D:\Music\Mix.m3u8")));
        // A folder moved or deleted whole.
        assert!(matters(&event(EventKind::Remove(notify::event::RemoveKind::Any), r"D:\Music\Band")));
        // A download done: renamed to its song's name.
        assert!(matters(&event(EventKind::Modify(ModifyKind::Name(RenameMode::To)), r"D:\Music\Song.mp3")));
        assert!(!matters(&event(created, r"D:\Music\Song.mp3.crdownload")));
        assert!(!matters(&event(created, r"D:\Music\.melue\playlists\x.json")));
        assert!(!matters(&event(created, r"D:\Music\Band\cover.jpg")));
        assert!(!matters(&event(EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::Any)), r"D:\Music\Song.mp3")));
    }
}
