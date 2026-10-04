//! Scanning: finding the music files in a folder and bringing the library in line with them.
//!
//! Incremental: new files are added; files whose size or modified time changed are updated; files that
//! are gone are marked missing (their likes, playlists and history stay, and they come back if the file
//! does); a missing file that turns up under another name (the same size and modified time) is a move,
//! so it keeps its library entry. Unchanged files cost a comparison.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;
use serde::Serialize;

use super::{err, now_ms, Library, Result};

/// The file types the player can play.
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "mp4", "aac", "ogg", "oga", "wav"];

/// A music file a folder listing found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    /// Relative to the folder (with `/`), or a content:// URI / absolute path for Android folders.
    pub path: String,
    /// The file name, for a title until tags are read.
    pub name: String,
    pub size: i64,
    /// Milliseconds since 1970.
    pub modified_at: i64,
}

/// What a scan changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub added: u32,
    pub changed: u32,
    /// Marked missing (the file is gone).
    pub missing: u32,
    /// Missing before, there again.
    pub returned: u32,
    /// Found under another name or folder (kept its entry).
    pub moved: u32,
    pub unchanged: u32,
}

impl ScanSummary {
    pub fn changed_anything(&self) -> bool {
        self.added + self.changed + self.missing + self.returned + self.moved > 0
    }

    pub fn add(&mut self, other: ScanSummary) {
        self.added += other.added;
        self.changed += other.changed;
        self.missing += other.missing;
        self.returned += other.returned;
        self.moved += other.moved;
        self.unchanged += other.unchanged;
    }
}

pub fn is_audio(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The music files under a folder on disk, with paths relative to it. Hidden files and folders (starting
/// with a dot, which includes macOS' `._` files) and folders with a `.nomedia` file are skipped, like
/// Android does.
pub fn list_folder(root: &Path) -> Result<Vec<FoundFile>> {
    if !root.is_dir() {
        return Err(format!("{} isn't there (unplugged, or moved?)", root.display()));
    }
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let entries: Vec<_> = entries.flatten().collect();
        if dir != root && entries.iter().any(|e| e.file_name() == ".nomedia") {
            continue;
        }
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else { continue };
            // Symlinked folders are skipped (they could loop, or point into another library folder).
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() && is_audio(&name) {
                let Ok(meta) = entry.metadata() else { continue };
                let relative = entry.path().strip_prefix(root).unwrap_or(&entry.path()).to_string_lossy().replace('\\', "/");
                found.push(FoundFile { path: relative, name, size: meta.len() as i64, modified_at: modified_ms(&meta) });
            }
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

fn modified_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

/// A title from a file name, until tags are read: the name without its extension.
pub fn title_from_name(name: &str) -> String {
    let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = stem.trim();
    if stem.is_empty() { name.to_string() } else { stem.to_string() }
}

fn format_from_name(name: &str) -> Option<String> {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase())
}

struct Existing {
    id: i64,
    size: i64,
    modified_at: i64,
    missing: bool,
}

impl Library {
    /// Brings folder `folder_id`'s tracks in line with `files` (everything the folder has now).
    pub fn apply_scan(&self, folder_id: i64, files: &[FoundFile]) -> Result<ScanSummary> {
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let mut summary = ScanSummary::default();

        let mut existing: HashMap<String, Existing> = HashMap::new();
        {
            let mut stmt = tx
                .prepare("SELECT id, path, size, modified_at, missing_since IS NOT NULL FROM tracks WHERE folder_id = ?1")
                .map_err(err)?;
            let rows = stmt
                .query_map([folder_id], |r| {
                    Ok((r.get::<_, String>(1)?, Existing { id: r.get(0)?, size: r.get(2)?, modified_at: r.get(3)?, missing: r.get(4)? }))
                })
                .map_err(err)?;
            for row in rows {
                let (path, e) = row.map_err(err)?;
                existing.insert(path, e);
            }
        }

        // New paths, for after the known ones: some may be moves of files that are gone.
        let mut new_files = Vec::new();
        for file in files {
            match existing.remove(&file.path) {
                Some(e) => {
                    let changed = e.size != file.size || e.modified_at != file.modified_at;
                    if changed {
                        tx.execute(
                            "UPDATE tracks SET size = ?2, modified_at = ?3, missing_since = NULL WHERE id = ?1",
                            params![e.id, file.size, file.modified_at],
                        )
                        .map_err(err)?;
                        summary.changed += 1;
                    } else if e.missing {
                        tx.execute("UPDATE tracks SET missing_since = NULL WHERE id = ?1", [e.id]).map_err(err)?;
                        summary.returned += 1;
                    } else {
                        summary.unchanged += 1;
                    }
                }
                None => new_files.push(file),
            }
        }

        // What's left in `existing` isn't there any more. A gone file whose size and modified time match
        // exactly one new file is taken to have moved there.
        let mut gone_by_identity: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
        for e in existing.values() {
            gone_by_identity.entry((e.size, e.modified_at)).or_default().push(e.id);
        }
        let mut new_by_identity: HashMap<(i64, i64), u32> = HashMap::new();
        for file in &new_files {
            *new_by_identity.entry((file.size, file.modified_at)).or_default() += 1;
        }
        let mut moved_ids = std::collections::HashSet::new();
        for file in new_files {
            let identity = (file.size, file.modified_at);
            let unique = new_by_identity.get(&identity) == Some(&1)
                && gone_by_identity.get(&identity).is_some_and(|ids| ids.len() == 1);
            if unique {
                let id = gone_by_identity[&identity][0];
                tx.execute(
                    "UPDATE tracks SET path = ?2, missing_since = NULL WHERE id = ?1",
                    params![id, file.path],
                )
                .map_err(err)?;
                moved_ids.insert(id);
                summary.moved += 1;
            } else {
                let title = title_from_name(&file.name);
                tx.execute(
                    "INSERT INTO tracks (folder_id, path, size, modified_at, title, sort_title, format, added_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![folder_id, file.path, file.size, file.modified_at, title, title.to_lowercase(), format_from_name(&file.name), now],
                )
                .map_err(err)?;
                summary.added += 1;
            }
        }
        for e in existing.values() {
            if !e.missing && !moved_ids.contains(&e.id) {
                tx.execute("UPDATE tracks SET missing_since = ?2 WHERE id = ?1", params![e.id, now]).map_err(err)?;
                summary.missing += 1;
            }
        }
        tx.execute("UPDATE folders SET last_scan_at = ?2 WHERE id = ?1", params![folder_id, now]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testkit::{Tags, TestLibrary};
    use crate::library::FolderKind;
    use std::time::{Duration, SystemTime};

    fn scan(t: &TestLibrary, folder_id: i64, root: &Path) -> ScanSummary {
        t.library.apply_scan(folder_id, &list_folder(root).unwrap()).unwrap()
    }

    fn titles(t: &TestLibrary) -> Vec<String> {
        let conn = t.library.conn();
        let mut stmt = conn.prepare("SELECT title FROM tracks WHERE missing_since IS NULL ORDER BY title").unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    #[test]
    fn listing_finds_music_files_and_skips_the_rest() {
        let t = TestLibrary::new("scan-list");
        let m = t.music_folder("Music");
        m.add("Artist/Album/01 One.wav", &Tags::default());
        m.add_sample("sample.flac", "Artist/Album/02 Two.flac");
        m.add_sample("sample.mp3", "Loose.MP3");
        m.add_text("Artist/Album/cover.jpg", "not audio");
        m.add_text("notes.txt", "x");
        m.add("._One.wav", &Tags::default()); // macOS leftovers
        m.add(".hidden/secret.wav", &Tags::default());
        m.add("Podcasts/ep.wav", &Tags::default());
        m.add_text("Podcasts/.nomedia", "");
        let found = list_folder(&m.root).unwrap();
        let paths: Vec<&str> = found.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["Artist/Album/01 One.wav", "Artist/Album/02 Two.flac", "Loose.MP3"]);
        assert!(found.iter().all(|f| f.size > 0 && f.modified_at > 0));
        assert!(list_folder(&m.root.join("gone")).is_err());
    }

    #[test]
    fn the_first_scan_adds_and_a_second_changes_nothing() {
        let t = TestLibrary::new("scan-first");
        let m = t.music_folder("Music");
        m.add("01 One.wav", &Tags::default());
        m.add("02 Two.wav", &Tags::default());
        let f = t.library.add_folder(FolderKind::Path, m.root.to_str().unwrap(), None).unwrap();
        let first = scan(&t, f.id, &m.root);
        assert_eq!((first.added, first.unchanged), (2, 0));
        assert_eq!(titles(&t), vec!["01 One", "02 Two"]);
        let again = scan(&t, f.id, &m.root);
        assert_eq!(again, ScanSummary { unchanged: 2, ..Default::default() });
        assert!(!again.changed_anything());
        assert!(t.library.folder(f.id).unwrap().unwrap().last_scan_at.is_some());
    }

    #[test]
    fn changed_missing_and_returning_files() {
        let t = TestLibrary::new("scan-changes");
        let m = t.music_folder("Music");
        m.add("a.wav", &Tags::default());
        m.add("b.wav", &Tags::default());
        let f = t.library.add_folder(FolderKind::Path, m.root.to_str().unwrap(), None).unwrap();
        scan(&t, f.id, &m.root);
        let id_of = |path: &str| -> i64 {
            t.library.conn().query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| r.get(0)).unwrap()
        };
        let b = id_of("b.wav");

        m.retag("a.wav", &Tags::song("New", "X", "Y", 1));
        let backup = std::fs::read(m.path("b.wav")).unwrap();
        let b_time = std::fs::metadata(m.path("b.wav")).unwrap().modified().unwrap();
        m.remove("b.wav");
        let s = scan(&t, f.id, &m.root);
        assert_eq!((s.changed, s.missing, s.added), (1, 1, 0));
        assert_eq!(t.library.status().unwrap().tracks, 1);

        // It's back (an SD card put back in): the same entry, not a new one.
        std::fs::write(m.path("b.wav"), backup).unwrap();
        m.touch("b.wav", b_time);
        let s = scan(&t, f.id, &m.root);
        assert_eq!((s.returned, s.added), (1, 0));
        assert_eq!(id_of("b.wav"), b);
    }

    #[test]
    fn a_moved_or_renamed_file_keeps_its_entry() {
        let t = TestLibrary::new("scan-moves");
        let m = t.music_folder("Music");
        m.add("Old/song.wav", &Tags::song("Song", "X", "Y", 1));
        m.add("other.wav", &Tags::default());
        let f = t.library.add_folder(FolderKind::Path, m.root.to_str().unwrap(), None).unwrap();
        scan(&t, f.id, &m.root);
        let before: i64 = t.library.conn().query_row("SELECT id FROM tracks WHERE path = 'Old/song.wav'", [], |r| r.get(0)).unwrap();
        t.library.conn().execute("INSERT INTO likes VALUES (?1, 1)", [before]).unwrap();

        m.rename("Old/song.wav", "New/Renamed song.wav");
        let s = scan(&t, f.id, &m.root);
        assert_eq!((s.moved, s.added, s.missing), (1, 0, 0));
        let (id, path): (i64, String) = t
            .library
            .conn()
            .query_row("SELECT id, path FROM tracks WHERE id = ?1", [before], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((id, path.as_str()), (before, "New/Renamed song.wav"));
        assert_eq!(t.library.conn().query_row("SELECT count(*) FROM likes", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    #[test]
    fn identical_looking_files_are_not_mistaken_for_moves() {
        let t = TestLibrary::new("scan-ambiguous");
        let m = t.music_folder("Music");
        let time = SystemTime::now() - Duration::from_secs(3600);
        m.add_untagged("a.wav", 100);
        m.touch("a.wav", time);
        let f = t.library.add_folder(FolderKind::Path, m.root.to_str().unwrap(), None).unwrap();
        scan(&t, f.id, &m.root);
        // a.wav is gone and two files with its size and time appear: which one is it? Neither is assumed.
        m.remove("a.wav");
        for name in ["x.wav", "y.wav"] {
            m.add_untagged(name, 100);
            m.touch(name, time);
        }
        let s = scan(&t, f.id, &m.root);
        assert_eq!((s.moved, s.added, s.missing), (0, 2, 1));
    }

    #[test]
    fn scanning_a_big_folder_again_is_quick() {
        let t = TestLibrary::new("scan-big");
        let m = t.music_folder("Music");
        for album in 0..40 {
            for track in 0..25 {
                m.add_untagged(&format!("Artist {}/Album {album}/{track:02} Song.wav", album % 7), 10);
            }
        }
        let f = t.library.add_folder(FolderKind::Path, m.root.to_str().unwrap(), None).unwrap();
        let start = std::time::Instant::now();
        assert_eq!(scan(&t, f.id, &m.root).added, 1000);
        let first = start.elapsed();
        let start = std::time::Instant::now();
        assert_eq!(scan(&t, f.id, &m.root).unchanged, 1000);
        let again = start.elapsed();
        println!("1000 files: first scan {first:?}, rescan {again:?}");
        assert!(again < Duration::from_secs(5), "rescan took {again:?}");
    }
}
