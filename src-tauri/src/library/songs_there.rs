//! Songs already in a folder, before it's chosen for the synced music (Library: the share's "Synced music goes to"):
//! a folder with music in it (one that holds an earlier synced folder, say) gets the share's songs beside its own, at
//! other places, so the same songs would be there twice. The app says so, and where most of them are.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;

use super::grouping::percent_decode;
use super::{FolderKind, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongsThere {
    pub songs: u32,
    /// The folder inside it with the most of them (more than half), and how many.
    pub most_in: Option<String>,
    pub most: u32,
}

/// The songs' places within the folder (`a/b.mp3`), counted by their first folder.
pub fn count(relative: impl IntoIterator<Item = String>) -> SongsThere {
    let mut there = SongsThere::default();
    let mut by_folder: HashMap<String, u32> = HashMap::new();
    for path in relative {
        there.songs += 1;
        if let Some((first, _)) = path.split_once('/') {
            *by_folder.entry(first.to_string()).or_default() += 1;
        }
    }
    if let Some((folder, n)) = by_folder.into_iter().max_by_key(|(_, n)| *n) {
        if n * 2 > there.songs {
            there.most_in = Some(folder);
            there.most = n;
        }
    }
    there
}

/// A tree URI's file, relative to the tree's folder (Android: `…/document/primary%3AMusic%2FMelue%2Fa.mp3` within
/// `primary:Music` is `Melue/a.mp3`).
fn tree_relative(folder: &str, file: &str) -> Option<String> {
    let doc = |uri: &str| uri.rsplit_once("/document/").map(|(_, d)| percent_decode(d));
    let (folder, file) = (doc(folder)?, doc(file)?);
    file.strip_prefix(&folder).map(|rest| rest.trim_start_matches('/').to_string())
}

/// How many songs `location` has now, and where most of them are.
#[tauri::command(async)]
pub fn library_songs_there(app: AppHandle, kind: FolderKind, location: String) -> Result<SongsThere> {
    let listing = match kind {
        FolderKind::Path => super::scan::list_folder(Path::new(&location))?,
        FolderKind::AndroidTree => super::android::list_tree(&app, &location)?,
        FolderKind::AndroidMedia => return Ok(SongsThere::default()),
    };
    let relative = listing.songs.into_iter().filter_map(|song| match kind {
        FolderKind::AndroidTree => tree_relative(&location, &song.path),
        _ => Some(song.path),
    });
    Ok(count(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_are_counted_by_their_first_folder() {
        let paths = ["Melue/a.mp3", "Melue/b.mp3", "Melue/Artist/c.mp3", "d.mp3"].map(String::from);
        assert_eq!(count(paths), SongsThere { songs: 4, most_in: Some("Melue".into()), most: 3 });
        assert_eq!(count(["a.mp3", "b.mp3"].map(String::from)).most_in, None);
    }

    #[test]
    fn a_tree_files_place_within_its_folder() {
        let folder = "content://com.android.externalstorage.documents/tree/primary%3AMusic/document/primary%3AMusic";
        let file = "content://com.android.externalstorage.documents/tree/primary%3AMusic/document/primary%3AMusic%2FMelue%2FWaterloo%20-%20ABBA.mp3";
        assert_eq!(tree_relative(folder, file).as_deref(), Some("Melue/Waterloo - ABBA.mp3"));
    }
}
