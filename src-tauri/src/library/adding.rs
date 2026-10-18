//! Adding songs from anywhere (files dropped on Melue or picked with Add songs…) into a music folder on this
//! computer: copied into `Artist/Album/` (`Uploads/` without tags), the files themselves stay where they are. (To
//! the share instead: sync/moving.rs.) Either way they can go into a playlist too: they wait here until a scan has
//! found them, then they're added in the order they came.

use serde::{Deserialize, Serialize};

use super::grouping::file_name_of;
use super::{FolderKind, Library, Result};
use crate::sync::layout::{layout_path, Layout};
use crate::sync::moving::free_path;

/// Songs waiting for a scan to find them, to go into a playlist (kept with the settings, so a restart keeps them).
const WAITING: &str = "waiting_for_playlist";
/// Scans a waiting song is looked for before it's given up on (its file never came: the copy failed later).
const SCANS: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Waiting {
    folder_id: i64,
    /// Where it is in its folder (with "/").
    path: String,
    playlist: i64,
    scans_left: u8,
}

/// What adding files did.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    pub added: u32,
    /// Files that couldn't be added: (file name, why).
    pub failed: Vec<(String, String)>,
    /// Where the added ones went in the folder (with "/"), in order.
    #[serde(skip)]
    pub placed: Vec<String>,
}

impl Library {
    /// Copies music files into a music folder on this computer, into `Artist/Album/` by their tags (a name that's
    /// taken gets " (2)"). The scan after brings them into the library.
    pub fn copy_songs_into(&self, folder_id: i64, files: &[String]) -> Result<AddResult> {
        let folder = self.folder(folder_id)?.ok_or_else(|| format!("no folder {folder_id}"))?;
        if folder.kind != FolderKind::Path {
            return Err("songs can only be added to a folder on this computer".into());
        }
        let root = std::path::Path::new(&folder.location);
        let mut result = AddResult::default();
        for file in files {
            let name = file_name_of(&file.replace('\\', "/"));
            match copy_one(root, file, &name) {
                Ok(dest) => {
                    result.added += 1;
                    result.placed.push(dest);
                }
                Err(e) => result.failed.push((name, e)),
            }
        }
        Ok(result)
    }

    /// Songs on their way into a folder (`paths` in it) that go into a playlist once a scan has found them.
    pub fn wait_for_songs(&self, folder_id: i64, paths: &[String], playlist: i64) -> Result<()> {
        let mut waiting = self.waiting()?;
        waiting.extend(paths.iter().map(|path| Waiting { folder_id, path: path.clone(), playlist, scans_left: SCANS }));
        self.keep_waiting(&waiting)
    }

    /// After a scan: the waiting songs it found go into their playlists, in the order they came. Returns the
    /// playlists that changed.
    pub fn add_waiting_songs(&self) -> Result<Vec<i64>> {
        let waiting = self.waiting()?;
        if waiting.is_empty() {
            return Ok(Vec::new());
        }
        let mut found: Vec<(i64, Vec<i64>)> = Vec::new();
        let mut still = Vec::new();
        for w in waiting {
            let track: Option<i64> = self
                .conn()
                .query_row("SELECT id FROM tracks WHERE folder_id = ?1 AND path = ?2 AND missing_since IS NULL", rusqlite::params![w.folder_id, w.path], |r| r.get(0))
                .ok();
            match track {
                Some(track) => match found.iter_mut().find(|(p, _)| *p == w.playlist) {
                    Some((_, tracks)) => tracks.push(track),
                    None => found.push((w.playlist, vec![track])),
                },
                None if w.scans_left > 1 => still.push(Waiting { scans_left: w.scans_left - 1, ..w }),
                None => {}
            }
        }
        self.keep_waiting(&still)?;
        let mut changed = Vec::new();
        for (playlist, tracks) in found {
            // A playlist deleted meanwhile: they just don't go anywhere.
            if self.add_to_playlist(playlist, &tracks, None).is_ok() {
                changed.push(playlist);
            }
        }
        Ok(changed)
    }

    fn waiting(&self) -> Result<Vec<Waiting>> {
        Ok(self.setting(WAITING)?.and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default())
    }

    fn keep_waiting(&self, waiting: &[Waiting]) -> Result<()> {
        self.set_setting(WAITING, &serde_json::to_string(waiting).map_err(|e| e.to_string())?)
    }
}

/// Copies one file into the folder; returns where it went.
fn copy_one(root: &std::path::Path, file: &str, name: &str) -> Result<String> {
    let mut opened = std::fs::File::open(file).map_err(|e| format!("can't open it: {e}"))?;
    let tags = super::tags::read_file(&mut opened).unwrap_or_default();
    let wanted = layout_path(Layout::ArtistAlbum, tags.album_artist.as_deref(), tags.artist.as_deref(), tags.album.as_deref(), name);
    let dest = free_path(&wanted, |p| root.join(p).exists());
    let to = root.join(&dest);
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't make its folder: {e}"))?;
    }
    std::fs::copy(file, &to).map_err(|e| format!("can't copy it: {e}"))?;
    Ok(dest)
}
