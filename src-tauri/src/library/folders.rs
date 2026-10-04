//! The music folders the library is built from: folders on a disk, all music on an Android phone (its
//! media index), or folders chosen through Android's storage picker.

use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{err, now_ms, Library, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderKind {
    /// A folder on a disk: desktop folders, or a plain path on Android.
    Path,
    /// All music on the phone, through Android's media index.
    AndroidMedia,
    /// A folder chosen through Android's storage picker (its location is a content:// tree URI).
    AndroidTree,
}

impl FolderKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::AndroidMedia => "android_media",
            Self::AndroidTree => "android_tree",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "path" => Some(Self::Path),
            "android_media" => Some(Self::AndroidMedia),
            "android_tree" => Some(Self::AndroidTree),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: i64,
    pub kind: FolderKind,
    pub location: String,
    pub name: String,
    pub enabled: bool,
    /// The folder a share syncs into (Phase 2).
    pub synced: bool,
    pub added_at: i64,
    pub last_scan_at: Option<i64>,
    /// Tracks found in it (not counting missing files).
    pub tracks: i64,
}

const SELECT: &str = "SELECT f.id, f.kind, f.location, f.name, f.enabled, f.synced, f.added_at, f.last_scan_at,
    (SELECT count(*) FROM tracks t WHERE t.folder_id = f.id AND t.missing_since IS NULL)
    FROM folders f";

fn folder_from(row: &Row) -> rusqlite::Result<Folder> {
    let kind: String = row.get(1)?;
    let location: String = row.get(2)?;
    let name: Option<String> = row.get(3)?;
    let kind = FolderKind::parse(&kind).unwrap_or(FolderKind::Path);
    Ok(Folder {
        id: row.get(0)?,
        name: name.unwrap_or_else(|| default_name(kind, &location)),
        kind,
        location,
        enabled: row.get(4)?,
        synced: row.get(5)?,
        added_at: row.get(6)?,
        last_scan_at: row.get(7)?,
        tracks: row.get(8)?,
    })
}

/// A folder's name when it wasn't given one: the last part of its path.
fn default_name(kind: FolderKind, location: &str) -> String {
    match kind {
        FolderKind::AndroidMedia => "All music on this phone".into(),
        FolderKind::Path => Path::new(location)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| location.to_string()),
        // content://.../tree/primary%3AMusic%2FRock -> Rock
        FolderKind::AndroidTree => {
            let last = location.rsplit('/').next().unwrap_or(location);
            let decoded = last.replace("%3A", ":").replace("%2F", "/").replace("%20", " ");
            decoded.rsplit(['/', ':']).find(|s| !s.is_empty()).unwrap_or(&decoded).to_string()
        }
    }
}

/// A folder path in one form: absolute, without `.`/`..`, without Windows' `\\?\` prefix or a trailing
/// separator, so the same folder typed two ways is recognized.
pub fn normalize_path(path: &str) -> Result<PathBuf> {
    let path = Path::new(path.trim());
    let meta = std::fs::metadata(path).map_err(|_| format!("{} doesn't exist or can't be read", path.display()))?;
    if !meta.is_dir() {
        return Err(format!("{} isn't a folder", path.display()));
    }
    let canonical = std::fs::canonicalize(path).map_err(err)?;
    let text = canonical.to_string_lossy();
    let plain = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text.into_owned()
    };
    Ok(PathBuf::from(plain))
}

/// Whether two normalized paths are the same, or one contains the other. Windows compares without case.
fn relation(a: &Path, b: &Path) -> Option<&'static str> {
    let key = |p: &Path| {
        let s = p.to_string_lossy().replace('\\', "/");
        let s = s.trim_end_matches('/').to_string();
        if cfg!(windows) { s.to_lowercase() } else { s }
    };
    let (a, b) = (key(a), key(b));
    if a == b {
        Some("same")
    } else if a.starts_with(&format!("{b}/")) {
        Some("inside")
    } else if b.starts_with(&format!("{a}/")) {
        Some("contains")
    } else {
        None
    }
}

impl Library {
    pub fn folders(&self) -> Result<Vec<Folder>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY f.added_at, f.id")).map_err(err)?;
        let rows = stmt.query_map([], folder_from).map_err(err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)
    }

    pub fn folder(&self, id: i64) -> Result<Option<Folder>> {
        self.conn()
            .query_row(&format!("{SELECT} WHERE f.id = ?1"), [id], folder_from)
            .optional()
            .map_err(err)
    }

    /// Adds a music folder. `location`: a path (Path), '' (AndroidMedia) or a tree URI (AndroidTree).
    /// Refuses one that's already there, or (for paths) inside or around one that is, since its files
    /// would be in the library twice; and all music on the phone together with chosen folders.
    pub fn add_folder(&self, kind: FolderKind, location: &str, name: Option<&str>) -> Result<Folder> {
        let location = match kind {
            FolderKind::Path => normalize_path(location)?.to_string_lossy().into_owned(),
            FolderKind::AndroidMedia => String::new(),
            FolderKind::AndroidTree => {
                let uri = location.trim();
                if !uri.starts_with("content://") {
                    return Err("a chosen Android folder needs its content:// address".into());
                }
                uri.to_string()
            }
        };
        let existing = self.folders()?;
        for folder in &existing {
            if folder.kind == kind && folder.location == location {
                return Err(format!("{} is already in the library", folder.name));
            }
            if kind == FolderKind::Path && folder.kind == FolderKind::Path {
                match relation(Path::new(&location), Path::new(&folder.location)) {
                    Some("same") => return Err(format!("{} is already in the library", folder.name)),
                    Some("inside") => {
                        return Err(format!("this folder is inside {}, which is already in the library", folder.location))
                    }
                    Some("contains") => {
                        return Err(format!(
                            "this folder contains {}, which is already in the library: remove that one first",
                            folder.location
                        ))
                    }
                    _ => {}
                }
            }
        }
        check_android_choice(&existing, kind, None)?;
        let conn = self.conn();
        conn.execute(
            "INSERT INTO folders (kind, location, name, added_at) VALUES (?1, ?2, ?3, ?4)",
            params![kind.as_str(), location, name.map(str::trim).filter(|n| !n.is_empty()), now_ms()],
        )
        .map_err(err)?;
        let id = conn.last_insert_rowid();
        drop(conn);
        Ok(self.folder(id)?.expect("the folder just added"))
    }

    /// Removes a folder and its tracks (with their likes, playlist entries and history). To keep them,
    /// switch the folder off instead.
    pub fn remove_folder(&self, id: i64) -> Result<()> {
        let removed = self.conn().execute("DELETE FROM folders WHERE id = ?1", [id]).map_err(err)?;
        if removed == 0 {
            return Err(format!("no folder {id}"));
        }
        Ok(())
    }

    /// Switches a folder on or off. Off hides its tracks but keeps them (and their likes and history).
    pub fn set_folder_enabled(&self, id: i64, enabled: bool) -> Result<Folder> {
        let folder = self.folder(id)?.ok_or_else(|| format!("no folder {id}"))?;
        if enabled {
            check_android_choice(&self.folders()?, folder.kind, Some(id))?;
        }
        self.conn().execute("UPDATE folders SET enabled = ?2 WHERE id = ?1", params![id, enabled]).map_err(err)?;
        Ok(self.folder(id)?.expect("the folder"))
    }

    pub fn rename_folder(&self, id: i64, name: &str) -> Result<Folder> {
        let name = name.trim();
        let changed = self
            .conn()
            .execute("UPDATE folders SET name = ?2 WHERE id = ?1", params![id, Some(name).filter(|n| !n.is_empty())])
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("no folder {id}"));
        }
        Ok(self.folder(id)?.expect("the folder"))
    }
}

/// On Android, all music on the phone and chosen folders are alternatives: the media index already covers
/// the chosen folders, so having both on would put songs in the library twice.
fn check_android_choice(existing: &[Folder], kind: FolderKind, except: Option<i64>) -> Result<()> {
    let enabled = |k: FolderKind| existing.iter().any(|f| f.kind == k && f.enabled && Some(f.id) != except);
    match kind {
        FolderKind::AndroidTree if enabled(FolderKind::AndroidMedia) => Err(
            "all music on this phone is already in the library, which includes this folder; switch it off to choose folders instead"
                .into(),
        ),
        FolderKind::AndroidMedia if enabled(FolderKind::AndroidTree) => Err(
            "chosen folders are in the library; switch them off to use all music on this phone instead".into(),
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testkit::{TestLibrary, Tags};

    #[test]
    fn folders_are_added_listed_renamed_and_removed() {
        let t = TestLibrary::new("folders-basic");
        let music = t.music_folder("Music");
        let more = t.music_folder("More music");
        let a = t.library.add_folder(FolderKind::Path, music.root.to_str().unwrap(), None).unwrap();
        let b = t.library.add_folder(FolderKind::Path, more.root.to_str().unwrap(), Some("  Extra  ")).unwrap();
        assert_eq!((a.name.as_str(), a.enabled, a.tracks), ("Music", true, 0));
        assert_eq!(b.name, "Extra");
        assert_eq!(t.library.folders().unwrap().iter().map(|f| f.id).collect::<Vec<_>>(), vec![a.id, b.id]);

        assert_eq!(t.library.rename_folder(a.id, "Main").unwrap().name, "Main");
        assert_eq!(t.library.rename_folder(a.id, " ").unwrap().name, "Music", "back to its own name");
        t.library.remove_folder(b.id).unwrap();
        assert_eq!(t.library.folders().unwrap().len(), 1);
        assert!(t.library.remove_folder(b.id).is_err());
    }

    #[test]
    fn a_folder_must_exist_and_be_a_folder() {
        let t = TestLibrary::new("folders-exist");
        let music = t.music_folder("Music");
        let file = music.add("song.wav", &Tags::default());
        assert!(t.library.add_folder(FolderKind::Path, music.root.join("nope").to_str().unwrap(), None).is_err());
        let error = t.library.add_folder(FolderKind::Path, file.to_str().unwrap(), None).unwrap_err();
        assert!(error.contains("isn't a folder"), "{error}");
    }

    #[test]
    fn the_same_folder_written_differently_is_recognized() {
        let t = TestLibrary::new("folders-same");
        let music = t.music_folder("Music");
        let path = music.root.to_str().unwrap().to_string();
        t.library.add_folder(FolderKind::Path, &path, None).unwrap();
        let mut variants = vec![format!("{path}/"), format!("{path}/sub/.."), format!("  {path}  ")];
        if cfg!(windows) {
            variants.push(path.to_uppercase());
            variants.push(path.replace('\\', "/"));
        }
        for variant in variants {
            let error = t.library.add_folder(FolderKind::Path, &variant, None).unwrap_err();
            assert!(error.contains("already in the library"), "{variant}: {error}");
        }
    }

    #[test]
    fn folders_inside_or_around_another_are_refused() {
        let t = TestLibrary::new("folders-nested");
        let music = t.music_folder("Music");
        std::fs::create_dir_all(music.path("Rock/Live")).unwrap();
        std::fs::create_dir_all(music.root.parent().unwrap().join("Music2")).unwrap();
        t.library.add_folder(FolderKind::Path, music.path("Rock").to_str().unwrap(), None).unwrap();
        let inside = t.library.add_folder(FolderKind::Path, music.path("Rock/Live").to_str().unwrap(), None).unwrap_err();
        assert!(inside.contains("is inside"), "{inside}");
        let around = t.library.add_folder(FolderKind::Path, music.root.to_str().unwrap(), None).unwrap_err();
        assert!(around.contains("contains"), "{around}");
        // A name that merely starts the same isn't inside.
        let sibling = music.root.parent().unwrap().join("Music2");
        t.library.add_folder(FolderKind::Path, sibling.to_str().unwrap(), None).unwrap();
    }

    #[test]
    fn switching_off_hides_and_keeps() {
        let t = TestLibrary::new("folders-enabled");
        let music = t.music_folder("Music");
        let f = t.library.add_folder(FolderKind::Path, music.root.to_str().unwrap(), None).unwrap();
        assert!(!t.library.set_folder_enabled(f.id, false).unwrap().enabled);
        assert!(t.library.set_folder_enabled(f.id, true).unwrap().enabled);
    }

    #[test]
    fn all_music_on_the_phone_and_chosen_folders_are_either_or() {
        let t = TestLibrary::new("folders-android");
        let media = t.library.add_folder(FolderKind::AndroidMedia, "", None).unwrap();
        assert_eq!(media.name, "All music on this phone");
        assert!(t.library.add_folder(FolderKind::AndroidMedia, "", None).is_err(), "once");
        let tree = "content://com.android.externalstorage.documents/tree/1234-5678%3AMusic%2FRock";
        let refused = t.library.add_folder(FolderKind::AndroidTree, tree, None).unwrap_err();
        assert!(refused.contains("switch it off"), "{refused}");

        t.library.set_folder_enabled(media.id, false).unwrap();
        let chosen = t.library.add_folder(FolderKind::AndroidTree, tree, None).unwrap();
        assert_eq!(chosen.name, "Rock");
        assert!(t.library.set_folder_enabled(media.id, true).is_err(), "not both on");
        t.library.set_folder_enabled(chosen.id, false).unwrap();
        t.library.set_folder_enabled(media.id, true).unwrap();
        assert!(t.library.add_folder(FolderKind::AndroidTree, "/sdcard/Music", None).is_err(), "needs a content:// address");
    }

    #[test]
    fn removing_a_folder_removes_its_tracks() {
        let t = TestLibrary::new("folders-remove");
        let music = t.music_folder("Music");
        let f = t.library.add_folder(FolderKind::Path, music.root.to_str().unwrap(), None).unwrap();
        t.library
            .conn()
            .execute(
                "INSERT INTO tracks (folder_id, path, size, modified_at, title, sort_title, added_at) VALUES (?1, 'a.mp3', 1, 1, 'a', 'a', 1)",
                [f.id],
            )
            .unwrap();
        assert_eq!(t.library.folder(f.id).unwrap().unwrap().tracks, 1);
        t.library.remove_folder(f.id).unwrap();
        assert_eq!(t.library.status().unwrap().tracks, 0);
    }
}
