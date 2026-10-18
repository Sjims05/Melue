//! Linked playlists: playlist files (.m3u, .m3u8) in the music folders, kept as playlists in the library.
//!
//! Melue keeps its own version of a linked playlist plus a snapshot of the file as last read
//! (playlist_snapshots), so it can tell what changed on each side. Each entry has a state:
//! - both: in Melue's version and in the file;
//! - here_only: added in Melue, not in the file;
//! - removed_here: removed in Melue, still in the file (greyed, doesn't play, can be restored);
//! - suggested: new in the file, not taken into Melue's version yet (greyed, can be added);
//! - file_removed: removed from the file, kept in Melue (marked).
//!
//! When the file changes, lines new in it are added (or suggested: `new_in_file`), lines gone from it are
//! removed (or kept and marked: `removed_from_file`). If the file's order changed, Melue takes it (its own
//! songs staying after the song they followed); otherwise Melue's order stays. With `write_back` on, Melue's
//! changes are written into the file, in the file's style (m3u.rs); it's off by default.
//!
//! File lines are matched to songs by path (relative to the file, or absolute), then, for paths from another
//! computer, by the end of the path (at least "ABBA/Waterloo.mp3"), then by the #EXTINF artist and title.
//! Lines that match nothing stay as "not found" entries and match when the song turns up. A playlist whose
//! file is gone stays, marked "file gone"; a file renamed or moved within its folder is followed. Files
//! the user let go of (unlinked, or their playlist deleted) aren't linked again, until followed again (Settings).
//!
//! A linked playlist can stop following its file ("Unfollowed", `following` off): scans leave it alone (only a
//! renamed or gone file is still noticed) and write-back is off; edits are kept the usual way (songs removed here
//! stay as removed_here), only not shown greyed, so following again shows how it differs from the file again.
//! Deleting a linked playlist the user changed keeps it for 30 days (deleted_linked_playlists): restoring brings
//! it back as it was, followed or not, replacing the file's playlist if that was followed again meanwhile.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension, Transaction};

use super::grouping::{fold, percent_decode};
use super::m3u::{self, ExtInf, FileLine, Style};
use super::scan::{modified_ms, FoundFile};
use super::{err, now_ms, Folder, FolderKind, Library, Result};

/// Reading and writing playlist files wherever their folder is.
pub trait PlaylistFiles {
    fn read(&self, folder: &Folder, path: &str) -> Result<Vec<u8>>;
    /// Replaces the file's contents; returns its new size and modified time.
    fn write(&self, folder: &Folder, path: &str, bytes: &[u8]) -> Result<(i64, i64)>;
}

/// Playlist files on a disk.
pub struct LocalFiles;

/// A file of a folder on a disk (or, all music on Android, an absolute path).
pub fn local_path(folder: &Folder, path: &str) -> PathBuf {
    match folder.kind {
        FolderKind::Path => Path::new(&folder.location).join(path),
        _ => PathBuf::from(path),
    }
}

impl PlaylistFiles for LocalFiles {
    fn read(&self, folder: &Folder, path: &str) -> Result<Vec<u8>> {
        let path = local_path(folder, path);
        std::fs::read(&path).map_err(|e| format!("can't read {}: {e}", path.display()))
    }

    /// Written to a temporary file first, then put in place: never a half-written playlist.
    fn write(&self, folder: &Folder, path: &str, bytes: &[u8]) -> Result<(i64, i64)> {
        let target = local_path(folder, path);
        let mut temp = target.clone().into_os_string();
        temp.push(".melue-tmp");
        let temp = PathBuf::from(temp);
        std::fs::write(&temp, bytes).map_err(|e| format!("can't write {}: {e}", target.display()))?;
        if let Err(e) = std::fs::rename(&temp, &target) {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("can't write {}: {e}", target.display()));
        }
        let meta = std::fs::metadata(&target).map_err(err)?;
        Ok((meta.len() as i64, modified_ms(&meta)))
    }
}

// ---- matching lines to songs

/// One kind of path for every kind of folder, to match playlist lines with: absolute on a disk
/// ("C:/Music/a.mp3", "/storage/emulated/0/Music/a.mp3"), or Android's "volume:path" for chosen folders
/// ("primary:Music/a.mp3").
pub fn logical_path(kind: FolderKind, location: &str, path: &str) -> String {
    match kind {
        FolderKind::Path => m3u::normalize(&format!("{location}/{path}")),
        FolderKind::AndroidMedia => m3u::normalize(path),
        FolderKind::AndroidTree => m3u::normalize(&document_path(path)),
    }
}

/// "primary:Music/Album/01.mp3" from a content:// address in a chosen folder.
pub(crate) fn document_path(uri: &str) -> String {
    let part = uri
        .rsplit_once("/document/")
        .or_else(|| uri.rsplit_once("/tree/"))
        .map_or(uri, |(_, part)| part);
    percent_decode(part)
}

fn parent(path: &str) -> &str {
    path.rfind('/').map_or("", |end| &path[..end])
}

fn info_key(artist: &str, title: &str) -> String {
    format!("{}\u{1f}{}", fold(artist), fold(title))
}

/// Every song in the library (missing ones too), findable by path, end of path, or artist and title.
pub struct TrackIndex {
    paths: HashMap<String, i64>,
    /// The last 2 and 3 parts of their paths.
    suffixes: [HashMap<String, Vec<i64>>; 2],
    infos: HashMap<String, Vec<i64>>,
}

impl TrackIndex {
    /// The song a playlist line names; `dir` is the playlist file's folder (a logical path).
    pub fn find(&self, dir: &str, line: &FileLine) -> Option<i64> {
        if let Some(path) = m3u::resolve(dir, &line.entry) {
            let key = path.to_lowercase();
            if let Some(id) = self.paths.get(&key) {
                return Some(*id);
            }
            let parts: Vec<&str> = key.split('/').filter(|p| !p.is_empty()).collect();
            // At least the folder and the file's name: a name alone ("01.mp3") says too little.
            for n in (2..=3.min(parts.len())).rev() {
                match self.suffixes[n - 2].get(&parts[parts.len() - n..].join("/")) {
                    Some(ids) if ids.len() == 1 => return Some(ids[0]),
                    Some(_) => break, // a shorter end of the path can't be less ambiguous
                    None => {}
                }
            }
        }
        let info = line.info.as_ref()?;
        let ids = self.infos.get(&info_key(info.artist.as_deref()?, &info.title))?;
        (ids.len() == 1).then(|| ids[0])
    }
}

/// An entry of a linked playlist while a file is applied to it.
struct Entry {
    id: Option<i64>,
    state: String,
    file_entry: Option<String>,
    track: Option<i64>,
    delete: bool,
    dirty: bool,
    /// The file line (index) it stands for.
    line: Option<usize>,
}

/// Lines that stand for a file line in Melue's version or as a greyed one.
fn file_backed(state: &str) -> bool {
    matches!(state, "both" | "removed_here" | "suggested")
}

fn take<'a>(counts: &mut HashMap<&'a str, usize>, line: &'a str) -> bool {
    match counts.get_mut(line) {
        Some(n) if *n > 0 => {
            *n -= 1;
            true
        }
        _ => false,
    }
}

fn counts<'a>(lines: impl Iterator<Item = &'a str>) -> HashMap<&'a str, usize> {
    let mut counts = HashMap::new();
    for line in lines {
        *counts.entry(line).or_default() += 1;
    }
    counts
}

/// Brings linked playlist `id`'s entries in line with the file's `lines` (see the module docs), and makes
/// them its snapshot. Returns whether any entry changed.
fn apply_file(tx: &Transaction, id: i64, dir: &str, lines: &[FileLine], index: &TrackIndex) -> Result<bool> {
    let (new_in_file, removed_from_file): (String, String) = tx
        .query_row("SELECT new_in_file, removed_from_file FROM playlists WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(err)?;
    let old: Vec<String> = {
        let mut stmt = tx.prepare("SELECT file_entry FROM playlist_snapshots WHERE playlist_id = ?1 ORDER BY position").map_err(err)?;
        let rows = stmt.query_map([id], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)?
    };
    let mut entries: Vec<Entry> = {
        let mut stmt = tx
            .prepare("SELECT id, state, file_entry, track_id FROM playlist_entries WHERE playlist_id = ?1 ORDER BY position, id")
            .map_err(err)?;
        let rows = stmt
            .query_map([id], |r| {
                Ok(Entry { id: r.get(0)?, state: r.get(1)?, file_entry: r.get(2)?, track: r.get(3)?, delete: false, dirty: false, line: None })
            })
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)?
    };
    let existing = entries.len();
    let new: Vec<&str> = lines.iter().map(|l| l.entry.as_str()).collect();

    // Which of the file's lines were there last time (kept), and which of last time's are gone.
    let mut old_left = counts(old.iter().map(String::as_str));
    let kept: Vec<bool> = new.iter().map(|l| take(&mut old_left, l)).collect();
    let mut new_left = counts(new.iter().copied());
    let removed: Vec<&str> = old.iter().map(String::as_str).filter(|l| !take(&mut new_left, l)).collect();

    // Lines gone from the file (of a song that's in twice, a greyed copy goes first).
    for line in removed {
        let matches = |e: &Entry, greyed: bool| {
            !e.delete && file_backed(&e.state) && (e.state != "both") == greyed && e.file_entry.as_deref() == Some(line)
        };
        let found = entries.iter().position(|e| matches(e, true)).or_else(|| entries.iter().position(|e| matches(e, false)));
        if let Some(e) = found.map(|at| &mut entries[at]) {
            if e.state == "both" && removed_from_file == "kept" {
                e.state = "file_removed".into();
                e.dirty = true;
            } else {
                e.delete = true;
            }
        }
    }

    // Each of the file's lines gets its entry: the one it had, a new one, or (back in the file) one that was
    // kept after leaving it.
    for (i, line) in lines.iter().enumerate() {
        let free = |e: &Entry| !e.delete && e.line.is_none() && e.file_entry.as_deref() == Some(line.entry.as_str());
        let found = if kept[i] {
            entries.iter().position(|e| free(e) && file_backed(&e.state))
        } else {
            entries.iter().position(|e| free(e) && e.state == "file_removed")
        };
        let at = match found {
            Some(at) => {
                let e = &mut entries[at];
                if e.state == "file_removed" {
                    e.state = "both".into();
                    e.dirty = true;
                }
                if e.track.is_none() {
                    e.track = index.find(dir, line);
                    e.dirty |= e.track.is_some();
                }
                at
            }
            None => {
                // New in the file, or (kept) lost when its song left the library: back as a line.
                let state = if kept[i] || new_in_file == "added" { "both" } else { "suggested" };
                entries.push(Entry {
                    id: None,
                    state: state.into(),
                    file_entry: Some(line.entry.clone()),
                    track: index.find(dir, line),
                    delete: false,
                    dirty: true,
                    line: None,
                });
                entries.len() - 1
            }
        };
        entries[at].line = Some(i);
    }

    // The order. If the lines that stayed are in the same order in the file as last time, Melue's order
    // stays and new lines go before the line after them (or at the end); otherwise the file's order is
    // taken, with Melue's own entries staying after the entry they followed.
    let survivors: Vec<usize> = (0..existing).filter(|&i| !entries[i].delete).collect();
    let mut snapshot_left = counts(new.iter().copied());
    let old_kept: Vec<&str> = old.iter().map(String::as_str).filter(|l| take(&mut snapshot_left, l)).collect();
    let new_kept: Vec<&str> = new.iter().zip(&kept).filter(|(_, k)| **k).map(|(l, _)| *l).collect();
    let by_line: HashMap<usize, usize> = entries.iter().enumerate().filter_map(|(i, e)| e.line.map(|l| (l, i))).collect();
    let order: Vec<usize> = if old_kept == new_kept {
        let mut order = survivors;
        for line in (0..lines.len()).rev() {
            let at = by_line[&line];
            if at < existing {
                continue;
            }
            let next = by_line.get(&(line + 1)).and_then(|next| order.iter().position(|i| i == next));
            order.insert(next.unwrap_or(order.len()), at);
        }
        order
    } else {
        let mut head = Vec::new();
        let mut after: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut anchor = None;
        for i in survivors {
            if entries[i].line.is_some() {
                anchor = Some(i);
            } else {
                match anchor {
                    Some(a) => after.entry(a).or_default().push(i),
                    None => head.push(i),
                }
            }
        }
        let mut order = head;
        for line in 0..lines.len() {
            let at = by_line[&line];
            order.push(at);
            order.extend(after.remove(&at).unwrap_or_default());
        }
        order
    };

    // Saving.
    let now = now_ms();
    let mut changed = false;
    for e in &entries {
        match (e.id, e.delete) {
            (Some(entry), true) => {
                tx.execute("DELETE FROM playlist_entries WHERE id = ?1", [entry]).map_err(err)?;
                changed = true;
            }
            (Some(entry), false) if e.dirty => {
                tx.execute(
                    "UPDATE playlist_entries SET state = ?2, file_entry = ?3, track_id = ?4 WHERE id = ?1",
                    params![entry, e.state, e.file_entry, e.track],
                )
                .map_err(err)?;
                changed = true;
            }
            _ => {}
        }
    }
    let mut ids = Vec::with_capacity(order.len());
    for i in order {
        let e = &entries[i];
        let entry = match e.id {
            Some(entry) => entry,
            None => {
                tx.execute(
                    "INSERT INTO playlist_entries (playlist_id, position, track_id, file_entry, state, added_at)
                     VALUES (?1, -1, ?2, ?3, ?4, ?5)",
                    params![id, e.track, e.file_entry, e.state, now],
                )
                .map_err(err)?;
                changed = true;
                tx.last_insert_rowid()
            }
        };
        ids.push(entry);
    }
    {
        let mut stmt = tx.prepare("UPDATE playlist_entries SET position = ?2 WHERE id = ?1 AND position != ?2").map_err(err)?;
        for (position, entry) in ids.iter().enumerate() {
            changed |= stmt.execute(params![entry, position as i64]).map_err(err)? > 0;
        }
    }
    if old != new {
        tx.execute("DELETE FROM playlist_snapshots WHERE playlist_id = ?1", [id]).map_err(err)?;
        let mut stmt = tx
            .prepare("INSERT INTO playlist_snapshots (playlist_id, position, file_entry) VALUES (?1, ?2, ?3)")
            .map_err(err)?;
        for (position, line) in new.iter().enumerate() {
            stmt.execute(params![id, position as i64, line]).map_err(err)?;
        }
    }
    if changed {
        tx.execute("UPDATE playlists SET updated_at = ?2 WHERE id = ?1", params![id, now]).map_err(err)?;
    }
    Ok(changed)
}

/// A linked playlist's file is let go of: later scans don't link it again.
pub(crate) fn ignore_file(tx: &Transaction, id: i64) -> Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO playlist_files_ignored (folder_id, path)
         SELECT file_folder_id, file_path FROM playlists WHERE id = ?1 AND kind = 'linked' AND file_folder_id IS NOT NULL",
        [id],
    )
    .map(|_| ())
    .map_err(err)
}

/// An entry of a playlist being written to its file, and its song.
struct WriteRow {
    entry: i64,
    state: String,
    file_entry: Option<String>,
    track: Option<WriteTrack>,
}

struct WriteTrack {
    kind: String,
    location: String,
    path: String,
    title: String,
    artist: Option<String>,
    duration_ms: Option<i64>,
}

struct LinkedFile {
    id: i64,
    path: String,
    size: Option<i64>,
    modified_at: Option<i64>,
    missing: bool,
    following: bool,
}

fn stem(name: &str) -> String {
    let stem = Path::new(name).file_stem().map_or(name.to_string(), |s| s.to_string_lossy().into_owned());
    if stem.trim().is_empty() { name.to_string() } else { stem.trim().to_string() }
}

impl Library {
    /// Every song, for matching playlist lines. A hidden copy of a synced song (moved to sync, or the same song)
    /// stands for that synced song: a line naming the copy's file lands on the song that's there.
    pub fn track_index(&self) -> Result<TrackIndex> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT coalesce(t.copy_of, t.id), f.kind, f.location, t.path, t.title, t.artist_credit
                 FROM tracks t JOIN folders f ON f.id = t.folder_id ORDER BY t.id",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, Option<String>>(5)?))
            })
            .map_err(err)?;
        let mut index = TrackIndex { paths: HashMap::new(), suffixes: Default::default(), infos: HashMap::new() };
        for row in rows {
            let (id, kind, location, path, title, artist) = row.map_err(err)?;
            let kind = FolderKind::parse(&kind).unwrap_or(FolderKind::Path);
            let key = logical_path(kind, &location, &path).to_lowercase();
            let parts: Vec<&str> = key.split('/').filter(|p| !p.is_empty()).collect();
            for n in 2..=3.min(parts.len()) {
                index.suffixes[n - 2].entry(parts[parts.len() - n..].join("/")).or_default().push(id);
            }
            index.paths.entry(key).or_insert(id);
            if let Some(artist) = artist {
                index.infos.entry(info_key(&artist, &title)).or_default().push(id);
            }
        }
        Ok(index)
    }

    /// Brings a folder's linked playlists in line with the playlist files found in it: new files become
    /// playlists, changed ones are read again, gone ones are marked "file gone" (or followed, when renamed
    /// or moved within the folder). Returns the playlists that changed.
    pub fn sync_playlists(&self, folder: &Folder, files: &[FoundFile], access: &dyn PlaylistFiles, index: &TrackIndex) -> Result<Vec<i64>> {
        let known: Vec<LinkedFile> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare(
                    "SELECT id, file_path, file_size, file_modified_at, file_missing_since IS NOT NULL, following FROM playlists
                     WHERE kind = 'linked' AND file_folder_id = ?1",
                )
                .map_err(err)?;
            let rows = stmt
                .query_map([folder.id], |r| {
                    Ok(LinkedFile { id: r.get(0)?, path: r.get(1)?, size: r.get(2)?, modified_at: r.get(3)?, missing: r.get(4)?, following: r.get(5)? })
                })
                .map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let ignored: HashSet<String> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT path FROM playlist_files_ignored WHERE folder_id = ?1").map_err(err)?;
            let paths = stmt.query_map([folder.id], |r| r.get(0)).map_err(err)?;
            paths.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let files: Vec<&FoundFile> = files.iter().filter(|f| !ignored.contains(&f.path)).collect();
        let found: HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
        let mut gone: Vec<&LinkedFile> = known.iter().filter(|k| !found.contains(k.path.as_str())).collect();
        let mut changed = Vec::new();
        let now = now_ms();
        for file in files {
            let same = |k: &LinkedFile| k.size == Some(file.size) && k.modified_at == Some(file.modified_at);
            let playlist = match known.iter().find(|k| k.path == file.path) {
                // Unfollowed: the file's changes don't come in; only that it's there again counts.
                Some(k) if !k.following => {
                    if k.missing {
                        self.conn().execute("UPDATE playlists SET file_missing_since = NULL WHERE id = ?1", [k.id]).map_err(err)?;
                        changed.push(k.id);
                    }
                    continue;
                }
                Some(k) if !k.missing && same(k) => continue,
                Some(k) => Some(k.id),
                None => {
                    // Renamed or moved within the folder: the same size and time as one that's gone.
                    let moved: Vec<usize> = (0..gone.len()).filter(|&i| same(gone[i])).collect();
                    if let [i] = moved[..] {
                        let k = gone.remove(i);
                        self.conn()
                            .execute(
                                "UPDATE playlists SET file_path = ?2, file_missing_since = NULL, updated_at = ?3 WHERE id = ?1",
                                params![k.id, file.path, now],
                            )
                            .map_err(err)?;
                        changed.push(k.id);
                        continue;
                    }
                    None
                }
            };
            let bytes = match access.read(folder, &file.path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("melue: can't read playlist {}: {e}", file.path);
                    continue;
                }
            };
            let (lines, _) = m3u::parse(&bytes);
            let dir = parent(&logical_path(folder.kind, &folder.location, &file.path)).to_string();
            let mut conn = self.conn();
            let tx = conn.transaction().map_err(err)?;
            let id = match playlist {
                Some(id) => id,
                None => {
                    tx.execute(
                        "INSERT INTO playlists (name, kind, created_at, updated_at, file_folder_id, file_path)
                         VALUES (?1, 'linked', ?2, ?2, ?3, ?4)",
                        params![stem(&file.name), now, folder.id, file.path],
                    )
                    .map_err(err)?;
                    tx.last_insert_rowid()
                }
            };
            apply_file(&tx, id, &dir, &lines, index)?;
            tx.execute(
                "UPDATE playlists SET file_size = ?2, file_modified_at = ?3, file_missing_since = NULL WHERE id = ?1",
                params![id, file.size, file.modified_at],
            )
            .map_err(err)?;
            tx.commit().map_err(err)?;
            changed.push(id);
        }
        for k in gone {
            if !k.missing {
                self.conn().execute("UPDATE playlists SET file_missing_since = ?2 WHERE id = ?1", params![k.id, now]).map_err(err)?;
                changed.push(k.id);
            }
        }
        Ok(changed)
    }

    /// After songs changed: linked playlists' lines are matched again (songs that turned up, or lines whose
    /// entries left with their songs), and playlists whose folder was removed are marked "file gone".
    /// Returns the playlists that changed.
    pub fn rematch_playlists(&self, index: &TrackIndex) -> Result<Vec<i64>> {
        let now = now_ms();
        let mut changed: Vec<i64> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare(
                    "UPDATE playlists SET file_missing_since = ?1
                     WHERE kind = 'linked' AND file_folder_id IS NULL AND file_missing_since IS NULL RETURNING id",
                )
                .map_err(err)?;
            let ids = stmt.query_map([now], |r| r.get(0)).map_err(err)?;
            ids.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let linked: Vec<(i64, String, String, String)> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare(
                    "SELECT p.id, f.kind, f.location, p.file_path FROM playlists p JOIN folders f ON f.id = p.file_folder_id
                     WHERE p.kind = 'linked'",
                )
                .map_err(err)?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        for (id, kind, location, path) in linked {
            let kind = FolderKind::parse(&kind).unwrap_or(FolderKind::Path);
            let dir = parent(&logical_path(kind, &location, &path)).to_string();
            let mut conn = self.conn();
            let tx = conn.transaction().map_err(err)?;
            let lines: Vec<FileLine> = {
                let mut stmt = tx.prepare("SELECT file_entry FROM playlist_snapshots WHERE playlist_id = ?1 ORDER BY position").map_err(err)?;
                let rows = stmt.query_map([id], |r| Ok(FileLine { entry: r.get(0)?, info: None })).map_err(err)?;
                rows.collect::<rusqlite::Result<_>>().map_err(err)?
            };
            if apply_file(&tx, id, &dir, &lines, index)? {
                changed.push(id);
            }
            tx.commit().map_err(err)?;
        }
        changed.sort_unstable();
        changed.dedup();
        Ok(changed)
    }

    /// Writes Melue's version of a linked playlist into its file (in the file's style), when write-back is
    /// on for it. Returns whether it wrote.
    pub fn write_back_if_on(&self, id: i64, access: &dyn PlaylistFiles) -> Result<bool> {
        let on: Option<bool> = self
            .conn()
            .query_row("SELECT write_back AND following FROM playlists WHERE id = ?1 AND kind = 'linked'", [id], |r| r.get(0))
            .optional()
            .map_err(err)?;
        if on == Some(true) {
            self.write_back(id, access)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Writes Melue's version of a linked playlist into its file: its songs (both, added here, kept after
    /// leaving the file) in order. Afterwards the file and Melue agree: everything is "both", and songs
    /// removed here or only suggested are gone.
    pub fn write_back(&self, id: i64, access: &dyn PlaylistFiles) -> Result<()> {
        let (folder_id, path, missing): (Option<i64>, String, bool) = self
            .conn()
            .query_row(
                "SELECT file_folder_id, file_path, file_missing_since IS NOT NULL FROM playlists WHERE id = ?1 AND kind = 'linked'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("no linked playlist {id}"))?;
        let folder = match folder_id {
            Some(folder_id) if !missing => self.folder(folder_id)?.ok_or("the playlist's folder is gone")?,
            _ => return Err("the playlist's file is gone".into()),
        };
        if folder.kind == FolderKind::AndroidMedia {
            return Err("Melue can't write playlist files with all music on the phone (choose the folder instead)".into());
        }
        let style = access.read(&folder, &path).ok().map(|bytes| m3u::parse(&bytes).1).unwrap_or_else(|| Style::new(cfg!(windows)));
        let dir = parent(&logical_path(folder.kind, &folder.location, &path)).to_string();

        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let rows: Vec<WriteRow> = {
            let mut stmt = tx
                .prepare(
                    "SELECT e.id, e.state, e.file_entry, t.id IS NOT NULL, f.kind, f.location, t.path, t.title, t.artist_credit, t.duration_ms
                     FROM playlist_entries e LEFT JOIN tracks t ON t.id = e.track_id LEFT JOIN folders f ON f.id = t.folder_id
                     WHERE e.playlist_id = ?1 ORDER BY e.position, e.id",
                )
                .map_err(err)?;
            let rows = stmt
                .query_map([id], |r| {
                    let track = if r.get::<_, bool>(3)? {
                        Some(WriteTrack {
                            kind: r.get(4)?,
                            location: r.get(5)?,
                            path: r.get(6)?,
                            title: r.get(7)?,
                            artist: r.get(8)?,
                            duration_ms: r.get(9)?,
                        })
                    } else {
                        None
                    };
                    Ok(WriteRow { entry: r.get(0)?, state: r.get(1)?, file_entry: r.get(2)?, track })
                })
                .map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let mut lines = Vec::new();
        let mut written = Vec::new();
        for WriteRow { entry, state, file_entry, track } in rows {
            if !matches!(state.as_str(), "both" | "here_only" | "file_removed") {
                tx.execute("DELETE FROM playlist_entries WHERE id = ?1", [entry]).map_err(err)?;
                continue;
            }
            let info = track.as_ref().map(|t| ExtInf {
                seconds: t.duration_ms.map(|ms| ms / 1000),
                artist: t.artist.clone(),
                title: t.title.clone(),
            });
            let line = match (file_entry, &track) {
                (Some(line), _) => line,
                (None, Some(t)) => {
                    let kind = FolderKind::parse(&t.kind).unwrap_or(FolderKind::Path);
                    let target = logical_path(kind, &t.location, &t.path);
                    let line = if style.absolute { None } else { m3u::relative(&dir, &target) }.unwrap_or(target);
                    if style.backslashes { line.replace('/', "\\") } else { line }
                }
                (None, None) => continue,
            };
            written.push((entry, line.clone()));
            lines.push(FileLine { entry: line, info });
        }
        let (size, modified_at) = access.write(&folder, &path, &m3u::write(&lines, &style))?;
        for (entry, line) in &written {
            tx.execute("UPDATE playlist_entries SET state = 'both', file_entry = ?2 WHERE id = ?1", params![entry, line]).map_err(err)?;
        }
        let ids: Vec<i64> = written.iter().map(|(entry, _)| *entry).collect();
        {
            let mut stmt = tx.prepare("UPDATE playlist_entries SET position = ?2 WHERE id = ?1").map_err(err)?;
            for (position, entry) in ids.iter().enumerate() {
                stmt.execute(params![entry, position as i64]).map_err(err)?;
            }
        }
        tx.execute("DELETE FROM playlist_snapshots WHERE playlist_id = ?1", [id]).map_err(err)?;
        for (position, line) in lines.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_snapshots (playlist_id, position, file_entry) VALUES (?1, ?2, ?3)",
                params![id, position as i64, line.entry],
            )
            .map_err(err)?;
        }
        tx.execute(
            "UPDATE playlists SET file_size = ?2, file_modified_at = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, size, modified_at, now_ms()],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }

    /// A linked playlist's settings (each left as it is when None): songs new in the file are "added" or
    /// "suggested"; songs removed from the file are "removed" or "kept"; write_back. Switching write-back on
    /// writes the file right away (the caller does, see write_back_if_on).
    pub fn set_playlist_options(&self, id: i64, new_in_file: Option<&str>, removed_from_file: Option<&str>, write_back: Option<bool>) -> Result<()> {
        if new_in_file.is_some_and(|v| !matches!(v, "added" | "suggested")) {
            return Err("new songs in the file are 'added' or 'suggested'".into());
        }
        if removed_from_file.is_some_and(|v| !matches!(v, "removed" | "kept")) {
            return Err("songs removed from the file are 'removed' or 'kept'".into());
        }
        let changed = self
            .conn()
            .execute(
                "UPDATE playlists SET new_in_file = coalesce(?2, new_in_file), removed_from_file = coalesce(?3, removed_from_file),
                     write_back = coalesce(?4, write_back), updated_at = ?5
                 WHERE id = ?1 AND kind = 'linked'",
                params![id, new_in_file, removed_from_file, write_back, now_ms()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("no linked playlist {id}"));
        }
        Ok(())
    }

    /// Turns a linked playlist into an app playlist: Melue's version stays (greyed songs removed here or
    /// only suggested go), the file is left alone, no longer followed and not linked again. It stays only on this
    /// device (not synced): the other devices have the file's playlist linked themselves, when they have the file.
    pub fn unlink_playlist(&self, id: i64) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        ignore_file(&tx, id)?;
        let changed = tx
            .execute(
                "UPDATE playlists SET kind = 'app', file_folder_id = NULL, file_path = NULL, file_size = NULL,
                     file_modified_at = NULL, file_missing_since = NULL, write_back = 0, local_only = 1, updated_at = ?2
                 WHERE id = ?1 AND kind = 'linked'",
                params![id, now_ms()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("no linked playlist {id}"));
        }
        tx.execute("DELETE FROM playlist_entries WHERE playlist_id = ?1 AND (state IN ('removed_here', 'suggested') OR track_id IS NULL)", [id])
            .map_err(err)?;
        tx.execute("UPDATE playlist_entries SET state = 'both', file_entry = NULL WHERE playlist_id = ?1", [id]).map_err(err)?;
        tx.execute("DELETE FROM playlist_snapshots WHERE playlist_id = ?1", [id]).map_err(err)?;
        let ids: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM playlist_entries WHERE playlist_id = ?1 ORDER BY position, id").map_err(err)?;
            let ids = stmt.query_map([id], |r| r.get(0)).map_err(err)?;
            ids.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        for (position, entry) in ids.iter().enumerate() {
            tx.execute("UPDATE playlist_entries SET position = ?2 WHERE id = ?1", params![entry, position as i64]).map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    /// Takes a greyed song (removed here, or suggested by the file) into Melue's version. Returns its song.
    pub fn restore_playlist_entry(&self, id: i64, entry: i64) -> Result<Option<i64>> {
        self.conn()
            .query_row(
                "UPDATE playlist_entries SET state = 'both' WHERE id = ?2 AND playlist_id = ?1 AND state IN ('removed_here', 'suggested')
                 RETURNING track_id",
                params![id, entry],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("entry {entry} isn't a greyed song of playlist {id}"))
    }
}

/// A linked playlist being deleted: kept for 30 days when the user changed it (songs added, removed or kept here,
/// suggestions waiting, its order or its name), with its settings, its entries and the file as last read. A newer
/// copy of the same file's playlist replaces an older one.
pub(crate) fn keep_deleted(tx: &Transaction, id: i64) -> Result<()> {
    let row: Option<(String, bool, Option<i64>, String, String, String, bool, i64)> = tx
        .query_row(
            "SELECT name, following, file_folder_id, file_path, new_in_file, removed_from_file, write_back, created_at
             FROM playlists WHERE id = ?1 AND kind = 'linked'",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((name, following, Some(folder_id), path, new_in_file, removed_from_file, write_back, created_at)) = row else { return Ok(()) };
    let entries: Vec<(Option<i64>, Option<String>, String, i64)> = {
        let mut stmt = tx
            .prepare("SELECT track_id, file_entry, state, added_at FROM playlist_entries WHERE playlist_id = ?1 ORDER BY position, id")
            .map_err(err)?;
        let rows = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)?
    };
    let snapshot: Vec<String> = {
        let mut stmt = tx.prepare("SELECT file_entry FROM playlist_snapshots WHERE playlist_id = ?1 ORDER BY position").map_err(err)?;
        let rows = stmt.query_map([id], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)?
    };
    let file_order: Vec<&str> = entries.iter().filter(|e| e.2 == "both").filter_map(|e| e.1.as_deref()).collect();
    let snapshot_order: Vec<&str> = snapshot.iter().map(String::as_str).filter(|line| file_order.contains(line)).collect();
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(&path);
    let changed = entries.iter().any(|e| e.2 != "both") || file_order != snapshot_order || name != stem(file_name);
    if !changed {
        return Ok(());
    }
    let songs = entries.iter().filter(|e| e.2 != "suggested" && e.2 != "removed_here").count() as i64;
    let data = serde_json::json!({
        "following": following,
        "newInFile": new_in_file,
        "removedFromFile": removed_from_file,
        "writeBack": write_back,
        "createdAt": created_at,
        "entries": entries.iter().map(|(track, line, state, added)| serde_json::json!([track, line, state, added])).collect::<Vec<_>>(),
        "snapshot": snapshot,
    });
    tx.execute("DELETE FROM deleted_linked_playlists WHERE folder_id = ?1 AND file_path = ?2", params![folder_id, path]).map_err(err)?;
    tx.execute(
        "INSERT INTO deleted_linked_playlists (name, songs, deleted_at, folder_id, file_path, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![name, songs, now_ms(), folder_id, path, data.to_string()],
    )
    .map_err(err)?;
    Ok(())
}

/// How long a deleted linked playlist is kept (30 days).
const KEPT_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// A linked playlist deleted with the user's changes, kept to restore (Recently deleted).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedLinkedPlaylist {
    pub id: i64,
    pub name: String,
    pub songs: i64,
    pub deleted_at: i64,
    /// The playlist file's name.
    pub file: String,
}

/// A playlist file let go of (its playlist unlinked or deleted): scans don't link it again, until it's followed again.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LetGoFile {
    pub folder_id: i64,
    /// Its music folder's name.
    pub folder: String,
    /// Where it is in that folder.
    pub path: String,
}

impl Library {
    /// The playlist files let go of, by folder and path (Settings: "Follow again").
    pub fn let_go_files(&self) -> Result<Vec<LetGoFile>> {
        let names: HashMap<i64, String> = self.folders()?.into_iter().map(|f| (f.id, f.name)).collect();
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT folder_id, path FROM playlist_files_ignored ORDER BY folder_id, path").map_err(err)?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(err)?;
        let files = rows.collect::<rusqlite::Result<Vec<_>>>().map_err(err)?;
        Ok(files
            .into_iter()
            .map(|(folder_id, path)| LetGoFile { folder: names.get(&folder_id).cloned().unwrap_or_default(), folder_id, path })
            .collect())
    }

    /// Follows a let-go playlist file again: the next scan links it as a new playlist.
    pub fn follow_file_again(&self, folder_id: i64, path: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM playlist_files_ignored WHERE folder_id = ?1 AND path = ?2", params![folder_id, path])
            .map(|_| ())
            .map_err(err)
    }

    /// Stops or starts following a linked playlist's file. Following again: the next scan reads the file and shows
    /// how the playlist differs from it (it's read even when unchanged since).
    pub fn set_playlist_following(&self, id: i64, following: bool) -> Result<()> {
        let changed = self
            .conn()
            .execute(
                "UPDATE playlists SET following = ?2, write_back = CASE WHEN ?2 THEN write_back ELSE 0 END,
                     file_size = CASE WHEN ?2 THEN NULL ELSE file_size END, updated_at = ?3
                 WHERE id = ?1 AND kind = 'linked'",
                params![id, following, now_ms()],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(format!("no linked playlist {id}"));
        }
        Ok(())
    }

    /// Linked playlists deleted with the user's changes in the last 30 days, latest first (older ones go).
    pub fn deleted_linked_playlists(&self) -> Result<Vec<DeletedLinkedPlaylist>> {
        let conn = self.conn();
        conn.execute("DELETE FROM deleted_linked_playlists WHERE deleted_at < ?1", [now_ms() - KEPT_MS]).map_err(err)?;
        let mut stmt = conn
            .prepare("SELECT id, name, songs, deleted_at, file_path FROM deleted_linked_playlists ORDER BY deleted_at DESC, id DESC")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                let path: String = r.get(4)?;
                Ok(DeletedLinkedPlaylist {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    songs: r.get(2)?,
                    deleted_at: r.get(3)?,
                    file: path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string(),
                })
            })
            .map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Restores a deleted linked playlist as it was (followed or not, with its changes), replacing the file's
    /// playlist if it was followed again meanwhile; its file isn't let go of any more. The next scan reads the file
    /// again. Returns the playlist's id.
    pub fn restore_linked_playlist(&self, kept: i64) -> Result<i64> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let (name, folder_id, path, data): (String, i64, String, String) = tx
            .query_row("SELECT name, folder_id, file_path, data FROM deleted_linked_playlists WHERE id = ?1", [kept], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .optional()
            .map_err(err)?
            .ok_or_else(|| format!("no deleted playlist {kept}"))?;
        let data: serde_json::Value = serde_json::from_str(&data).map_err(|e| format!("a deleted playlist that can't be read: {e}"))?;
        let now = now_ms();
        tx.execute("DELETE FROM playlists WHERE kind = 'linked' AND file_folder_id = ?1 AND file_path = ?2", params![folder_id, path])
            .map_err(err)?;
        tx.execute(
            "INSERT INTO playlists (name, kind, created_at, updated_at, file_folder_id, file_path, new_in_file, removed_from_file, write_back, following)
             VALUES (?1, 'linked', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                name,
                data["createdAt"].as_i64().unwrap_or(now),
                now,
                folder_id,
                path,
                data["newInFile"].as_str().unwrap_or("added"),
                data["removedFromFile"].as_str().unwrap_or("removed"),
                data["writeBack"].as_bool().unwrap_or(false),
                data["following"].as_bool().unwrap_or(true),
            ],
        )
        .map_err(err)?;
        let id = tx.last_insert_rowid();
        let mut position = 0i64;
        for entry in data["entries"].as_array().into_iter().flatten() {
            // A song that left the library since: its file line stays (matched again when it's back).
            let track = entry[0].as_i64().filter(|&t| tx.query_row("SELECT 1 FROM tracks WHERE id = ?1", [t], |_| Ok(())).optional().ok().flatten().is_some());
            let line = entry[1].as_str();
            if track.is_none() && line.is_none() {
                continue;
            }
            tx.execute(
                "INSERT INTO playlist_entries (playlist_id, position, track_id, file_entry, state, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, position, track, line, entry[2].as_str().unwrap_or("both"), entry[3].as_i64().unwrap_or(now)],
            )
            .map_err(err)?;
            position += 1;
        }
        for (i, line) in data["snapshot"].as_array().into_iter().flatten().filter_map(|l| l.as_str()).enumerate() {
            tx.execute("INSERT INTO playlist_snapshots (playlist_id, position, file_entry) VALUES (?1, ?2, ?3)", params![id, i as i64, line])
                .map_err(err)?;
        }
        tx.execute("DELETE FROM playlist_files_ignored WHERE folder_id = ?1 AND path = ?2", params![folder_id, path]).map_err(err)?;
        tx.execute("DELETE FROM deleted_linked_playlists WHERE id = ?1", [kept]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(id)
    }
}
