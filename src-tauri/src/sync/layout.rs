//! The share's folder layout (Phase 2 step 10b): songs in `Artist/Album/` (the app's default), or all in one
//! folder. Songs moved to sync are put there in the chosen layout (moving.rs); songs that land on the share by
//! other ways (dropped in by hand) are put in place after they're synced and in the library, once the share is
//! in that layout. Switching the layout doesn't move anything by itself: Melue says how many songs would move,
//! and reorganises when asked.
//!
//! The layout belongs to the share, so every device uses the same one: it's kept on the share
//! (`.melue/share.json`: the layout, and the layout the share was last put in completely), read at every sync
//! and kept here too (for when the share can't be reached). Changing it on a device changes it for all.
//!
//! Reorganising renames files on the share (and the copies in the synced folder here, so nothing is downloaded
//! again; other devices see the files as moved and move theirs). Playlist files (.m3u) on the share are updated
//! to the new paths, and so are Melue's synced playlists; folders left empty are removed.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::handoff::key;
use super::local;
use super::moving::{clean_name, free_path, upload_path};
use super::run::Progress;
use super::scheduler::{self, SyncFailure};
use crate::library::grouping::file_name_of;
use crate::library::{err, scanner, Folder, Library, Result};
use crate::share::commands::with_stores;
use crate::share::setup::{self, ShareSetup};
use crate::share::{Share, ShareError};

/// The layout the share was last put in completely (empty: never): new songs are tidied only when it's the
/// chosen one. Kept here as the share last said.
const APPLIED: &str = "share_layout_applied";
/// This device changed the layout while the share couldn't be reached: it goes to the share next time.
const PENDING: &str = "share_layout_pending";
/// Where the share keeps its layout (every device reads it).
pub const SHARE_FILE: &str = ".melue/share.json";

/// The share's own settings, in [`SHARE_FILE`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShareFile {
    pub layout: Layout,
    /// The layout the share was last put in completely.
    pub applied: Option<Layout>,
}

/// How songs are laid out on the share.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    /// `Artist/Album/<file>` (the album artist, else the artist; `Uploads/` without one).
    #[default]
    ArtistAlbum,
    /// Every song in the share's music folder itself.
    OneFolder,
}

impl Layout {
    fn as_str(self) -> &'static str {
        match self {
            Self::ArtistAlbum => "artistAlbum",
            Self::OneFolder => "oneFolder",
        }
    }
}

// ---- the rules (plain functions, tested in layout_tests.rs)

/// Where a song goes in a layout.
pub fn layout_path(layout: Layout, album_artist: Option<&str>, artist: Option<&str>, album: Option<&str>, file_name: &str) -> String {
    match layout {
        Layout::ArtistAlbum => upload_path(album_artist, artist, album, file_name),
        Layout::OneFolder => clean_name(file_name),
    }
}

fn split_name(path: &str) -> (&str, &str, &str) {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    match name.rfind('.').filter(|&dot| dot > 0) {
        Some(dot) => (dir, &name[..dot], &name[dot..]),
        None => (dir, name, ""),
    }
}

/// Whether a song at `path` is in place for `wanted`: the same, or the same with a number Melue added when
/// the name was taken ("01 Intro (2).mp3"). Compared without case.
pub fn in_place(path: &str, wanted: &str) -> bool {
    let (path, wanted) = (key(path), key(wanted));
    if path == wanted {
        return true;
    }
    let ((dir, stem, ext), (wanted_dir, wanted_stem, wanted_ext)) = (split_name(&path), split_name(&wanted));
    dir == wanted_dir
        && ext == wanted_ext
        && stem.strip_prefix(wanted_stem).and_then(|rest| rest.strip_prefix(" (")).and_then(|rest| rest.strip_suffix(')')).is_some_and(|n| {
            !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
        })
}

/// `to` relative to the folder `from_dir` (paths with '/', relative to the share).
fn relative(from_dir: &str, to: &str) -> String {
    let from: Vec<&str> = from_dir.split('/').filter(|p| !p.is_empty()).collect();
    let target: Vec<&str> = to.split('/').filter(|p| !p.is_empty()).collect();
    let common = from.iter().zip(&target).take_while(|(a, b)| a.eq_ignore_ascii_case(b)).count();
    let mut parts = vec![".."; from.len() - common];
    parts.extend(&target[common..]);
    parts.join("/")
}

/// A playlist line's path relative to the share (None: not a path in the share, e.g. a full path or a URL).
fn resolve(dir: &str, line: &str) -> Option<String> {
    let line = line.trim().replace('\\', "/");
    if line.is_empty() || line.starts_with('#') || line.starts_with('/') || line.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in line.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

/// A playlist file's text with the moved songs' paths updated (None: nothing in it moved). `path`: the playlist
/// file's path on the share; `moved`: old path → new path (keys without case).
pub fn update_playlist_text(text: &str, path: &str, moved: &HashMap<String, String>) -> Option<String> {
    let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
    let backslashes = text.contains('\\');
    let mut changed = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let Some(new) = resolve(dir, line).and_then(|p| moved.get(&key(&p))) else { return line.to_string() };
            changed = true;
            let new = relative(dir, new);
            if backslashes { new.replace('/', "\\") } else { new }
        })
        .collect();
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    changed.then(|| lines.join(ending) + if text.ends_with('\n') { ending } else { "" })
}

// ---- the library's side

/// A song in the synced folder, and where it goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutMove {
    pub from: String,
    pub to: String,
}

/// A song's album artist, artist and album (what its place depends on).
type SongTags = (Option<String>, Option<String>, Option<String>);

impl Library {
    /// The synced folder's songs that aren't in place for `layout`: from their path on the share to where they go.
    pub fn layout_moves(&self, folder: &Folder, layout: Layout) -> Result<Vec<LayoutMove>> {
        let tags: HashMap<i64, SongTags> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare("SELECT id, tag_album_artist, artist_credit, tag_album FROM tracks WHERE folder_id = ?1 AND missing_since IS NULL")
                .map_err(err)?;
            let rows = stmt.query_map([folder.id], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?, r.get(3)?)))).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let mut moves = Vec::new();
        for (id, path) in self.synced_songs(folder)? {
            let Some((album_artist, artist, album)) = tags.get(&id) else { continue };
            let wanted = layout_path(layout, album_artist.as_deref(), artist.as_deref(), album.as_deref(), &file_name_of(&path));
            if !in_place(&path, &wanted) {
                moves.push(LayoutMove { from: path, to: wanted });
            }
        }
        moves.sort_by(|a, b| a.from.cmp(&b.from));
        Ok(moves)
    }

    /// The share's layout as this device last knew it (or set it).
    pub fn layout_here(&self) -> Result<ShareFile> {
        Ok(ShareFile { layout: self.sync_settings()?.share_layout, applied: self.applied_layout()? })
    }

    fn keep_layout_here(&self, file: ShareFile) -> Result<()> {
        let mut settings = self.sync_settings()?;
        if settings.share_layout != file.layout {
            settings.share_layout = file.layout;
            self.set_sync_settings(&settings)?;
        }
        match file.applied {
            Some(applied) => self.set_setting(APPLIED, applied.as_str()),
            None => self.conn().execute("DELETE FROM settings WHERE key = ?1", [APPLIED]).map(|_| ()).map_err(err),
        }
    }

    /// This device chose another layout: it goes to the share (now, or at the next sync).
    pub fn layout_changed_here(&self) -> Result<()> {
        self.set_setting(PENDING, "1")
    }

    /// The layout the share was last put in completely.
    pub fn applied_layout(&self) -> Result<Option<Layout>> {
        Ok(match self.setting(APPLIED)?.as_deref() {
            Some("artistAlbum") => Some(Layout::ArtistAlbum),
            Some("oneFolder") => Some(Layout::OneFolder),
            _ => None,
        })
    }
}

// ---- the layout on the share

/// The share's layout, agreed with the share: the share's (kept here too), or this device's when the share has
/// none yet or this device changed it while away (then it's written to the share).
pub fn agree(library: &Library, share: &dyn Share) -> Result<ShareFile> {
    let here = library.layout_here()?;
    let changed_here = library.setting(PENDING)?.is_some();
    let on_share = match share.read(SHARE_FILE) {
        Ok(bytes) if !changed_here => serde_json::from_slice::<ShareFile>(&bytes).ok(),
        Ok(_) | Err(ShareError::NotFound { .. }) => None,
        Err(e) => return Err(e.into()),
    };
    match on_share {
        Some(file) => {
            library.keep_layout_here(file)?;
            Ok(file)
        }
        // Nothing on the share, and this device never chose or applied a layout: its default isn't a choice.
        None if !changed_here && here.applied.is_none() => Ok(here),
        None => {
            share.create_dir_all(".melue")?;
            share.write(SHARE_FILE, &serde_json::to_vec_pretty(&here).map_err(err)?)?;
            library.conn().execute("DELETE FROM settings WHERE key = ?1", [PENDING]).map_err(err)?;
            Ok(here)
        }
    }
}

/// The layout with the share when it can be reached (the shared open share), else as known here.
fn agree_now(app: &AppHandle) -> Result<ShareFile> {
    let library = app.state::<Library>();
    match super::shared::with_share(app, |share| agree(&library, share)) {
        Ok(file) => Ok(file),
        Err(_) => library.layout_here(),
    }
}

/// The layout was changed on this device: to the share, in the background.
pub fn layout_changed(app: &AppHandle) -> Result<()> {
    app.state::<Library>().layout_changed_here()?;
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = agree_now(&app);
    });
    Ok(())
}

// ---- reorganising (with the share)

/// The layout and how far the share is from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutStatus {
    pub layout: Layout,
    /// The share is in this layout (new songs are put in place by themselves).
    pub applied: bool,
    /// Songs that would move.
    pub to_move: usize,
    /// A few of them, to show.
    pub examples: Vec<LayoutMove>,
}

pub fn status(app: &AppHandle) -> Result<LayoutStatus> {
    let library = app.state::<Library>();
    let ShareFile { layout, applied } = agree_now(app)?;
    let folder = library.synced_folder()?.ok_or("no share is set up")?;
    let moves = library.layout_moves(&folder, layout)?;
    Ok(LayoutStatus {
        layout,
        applied: applied == Some(layout),
        to_move: moves.len(),
        examples: moves.into_iter().take(5).collect(),
    })
}

/// What reorganising did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorganiseResult {
    pub moved: u32,
    /// Not on the share any more (another device took them off): left for the sync (Remove songs gone).
    #[serde(skip)]
    pub gone: Vec<String>,
    /// Playlist files (.m3u) and Melue playlists updated to the new paths.
    pub playlists_updated: u32,
    pub failed: Vec<(String, String)>,
}

/// Puts the share in the chosen layout: renames songs on the share and in the synced folder here, updates
/// playlists, removes folders left empty. `only_if_applied`: only when the share is in the layout already (new
/// songs, by themselves); otherwise this is the user's "Reorganise now", and the layout counts as applied after.
/// Songs a tidy found gone from the share: not tried again until the next sync (which sees to them), so a scan
/// doesn't walk the share for them every time.
static GONE: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// A sync ran: songs found gone are tried again by the next tidy.
pub fn forget_gone() {
    *GONE.lock().unwrap() = None;
}

fn known_gone(path: &str) -> bool {
    GONE.lock().unwrap().as_ref().is_some_and(|gone| gone.contains(&key(path)))
}

pub fn reorganise(app: &AppHandle, only_if_applied: bool) -> std::result::Result<ReorganiseResult, SyncFailure> {
    let library = app.state::<Library>();
    // As last known here first (no connecting after every scan for nothing), then as the share says.
    let here = library.layout_here()?;
    if only_if_applied && here.applied != Some(here.layout) {
        return Ok(ReorganiseResult::default());
    }
    let folder = library.synced_folder()?.ok_or_else(|| "no share is set up".to_string())?;
    if only_if_applied && library.layout_moves(&folder, here.layout)?.iter().all(|m| known_gone(&m.from)) {
        return Ok(ReorganiseResult::default());
    }
    let (settings, share) = with_stores(app, |library, stores| setup::current(library, stores, &setup::open))
        .and_then(|r| r)
        .map_err(|error| SyncFailure::Setup { error })?;
    let ShareFile { layout, applied } = agree(&library, share.as_ref())?;
    if only_if_applied && applied != Some(layout) {
        return Ok(ReorganiseResult::default());
    }
    let mut moves = library.layout_moves(&folder, layout)?;
    if only_if_applied {
        moves.retain(|m| !known_gone(&m.from));
    }
    let done = |library: &Library| -> Result<()> {
        library.keep_layout_here(ShareFile { layout, applied: Some(layout) })?;
        let file = ShareFile { layout, applied: Some(layout) };
        Ok(share.write(SHARE_FILE, &serde_json::to_vec_pretty(&file).map_err(err)?)?)
    };
    if moves.is_empty() {
        done(&library)?;
        return Ok(ReorganiseResult::default());
    }
    let target = match settings.setup {
        ShareSetup::Drive { stream: true, .. } => None,
        _ => Some(local::target(app, &folder)?),
    };
    let result = scheduler::exclusive(app, |report| {
        Ok(reorganise_files(&library, share.as_ref(), target.as_deref(), &moves, report, &|| scheduler::cancelled(app))?)
    })?;
    if !result.gone.is_empty() {
        GONE.lock().unwrap().get_or_insert_with(HashSet::new).extend(result.gone.iter().map(|p| key(p)));
    }
    if result.failed.is_empty() {
        done(&library)?;
    }
    // Only when something moved: a scan after a tidy that moved nothing would tidy again, and again.
    if result.moved > 0 || result.playlists_updated > 0 {
        scanner::scan_soon(app);
        super::data::soon(app);
    }
    Ok(result)
}

/// Moves songs on the share (and their copies here, `target`), updates the playlists naming them, and removes
/// folders left empty. `cancelled`: stops after the song it's on.
pub fn reorganise_files(
    library: &Library,
    share: &dyn Share,
    target: Option<&dyn Share>,
    moves: &[LayoutMove],
    report: &mut dyn FnMut(&Progress),
    cancelled: &dyn Fn() -> bool,
) -> Result<ReorganiseResult> {
    let mut result = ReorganiseResult::default();
    let mut taken: HashSet<String> = share.walk("")?.into_iter().map(|e| key(&e.path)).collect();
    let mut moved: HashMap<String, String> = HashMap::new();
    let mut left: HashSet<String> = HashSet::new();
    let total = moves.len() as u32;
    for (done, m) in moves.iter().enumerate() {
        if cancelled() {
            break;
        }
        report(&Progress { files_done: done as u32, files_total: total, current: Some(m.from.clone()), ..Default::default() });
        // Not on the share any more (another device took it off): nothing to put in place. The sync sees to it
        // (Remove songs gone from the share).
        if !taken.contains(&key(&m.from)) {
            result.gone.push(m.from.clone());
            continue;
        }
        let to = free_path(&m.to, |p| taken.contains(&key(p)));
        match move_file(library, share, target, &m.from, &to) {
            Ok(()) => {
                taken.remove(&key(&m.from));
                taken.insert(key(&to));
                moved.insert(key(&m.from), to.clone());
                if let Some((dir, _)) = m.from.rsplit_once('/') {
                    left.insert(dir.to_string());
                }
                result.moved += 1;
            }
            Err(e) => result.failed.push((m.from.clone(), e)),
        }
    }
    result.playlists_updated += update_playlist_files(library, share, target, &moved)?;
    result.playlists_updated += update_synced_playlists(share, &moved)?;
    // Folders left empty (deepest first, and the folders above them).
    let mut dirs: Vec<String> = left.into_iter().collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for dir in dirs {
        let mut dir = dir.as_str();
        loop {
            let _ = share.delete_empty_dir(dir);
            if let Some(target) = target {
                let _ = target.delete_empty_dir(dir);
            }
            match dir.rsplit_once('/') {
                Some((parent, _)) => dir = parent,
                None => break,
            }
        }
    }
    Ok(result)
}

/// Moves a song on the share, and its copy here; the sync's record follows (no download again).
fn move_file(library: &Library, share: &dyn Share, target: Option<&dyn Share>, from: &str, to: &str) -> Result<()> {
    if let Some((dir, _)) = to.rsplit_once('/') {
        share.create_dir_all(dir)?;
    }
    share.rename(from, to)?;
    let on_share = share.stat(to)?.ok_or("the share lost the song right after it was moved")?;
    library.forget_synced(from)?;
    match target {
        Some(target) => {
            if let Some((dir, _)) = to.rsplit_once('/') {
                target.create_dir_all(dir)?;
            }
            // Not here (yet): the next sync brings it.
            match target.rename(from, to) {
                Ok(()) | Err(ShareError::NotFound { .. }) => {}
                Err(e) => return Err(e.into()),
            }
            if let Some(here) = target.stat(to)? {
                library.record_synced(&on_share, &here)?;
            }
        }
        None => library.record_synced(&on_share, &on_share)?,
    }
    Ok(())
}

/// Playlist files on the share (.m3u, .m3u8) naming moved songs: updated there and here.
fn update_playlist_files(library: &Library, share: &dyn Share, target: Option<&dyn Share>, moved: &HashMap<String, String>) -> Result<u32> {
    if moved.is_empty() {
        return Ok(0);
    }
    let mut updated = 0;
    for entry in share.walk("")?.into_iter().filter(|e| crate::library::m3u::is_playlist(&e.name)) {
        let Ok(bytes) = share.read(&entry.path) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let Some(new) = update_playlist_text(&text, &entry.path, moved) else { continue };
        share.write(&entry.path, new.as_bytes())?;
        let on_share = share.stat(&entry.path)?.ok_or("the share lost a playlist right after it was written")?;
        if let Some(target) = target {
            target.write(&entry.path, new.as_bytes())?;
            if let Some(here) = target.stat(&entry.path)? {
                library.record_synced(&on_share, &here)?;
            }
        }
        updated += 1;
    }
    Ok(updated)
}

/// Melue's synced playlists name songs by their path on the share: their files follow the moves.
fn update_synced_playlists(share: &dyn Share, moved: &HashMap<String, String>) -> Result<u32> {
    if moved.is_empty() {
        return Ok(0);
    }
    let dir = super::playlist_sync::DIR;
    let entries = match share.list(dir) {
        Ok(entries) => entries,
        Err(ShareError::NotFound { .. }) => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    let mut updated = 0;
    for entry in entries.iter().filter(|e| !e.is_dir && e.name.ends_with(".json")) {
        let Ok(bytes) = share.read(&entry.path) else { continue };
        let Ok(mut file) = serde_json::from_slice::<serde_json::Value>(&bytes) else { continue };
        let mut changed = false;
        if let Some(songs) = file.get_mut("entries").and_then(|e| e.as_array_mut()) {
            for song in songs {
                let new = song.get("key").and_then(|k| k.as_str()).and_then(|k| moved.get(&key(k))).cloned();
                if let Some(new) = new {
                    song["key"] = new.into();
                    changed = true;
                }
            }
        }
        if changed {
            share.write(&entry.path, &serde_json::to_vec(&file).map_err(err)?)?;
            updated += 1;
        }
    }
    Ok(updated)
}

/// After a scan: new songs on the share put in place, when the share is in the chosen layout (in the
/// background; skipped while a sync runs, tried after the next scan).
pub fn tidy_new_songs(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || match reorganise(&app, true) {
        Ok(r) => {
            if r.moved > 0 {
                eprintln!("melue: put {} new songs in place on the share", r.moved);
            }
            if !r.failed.is_empty() {
                eprintln!("melue: couldn't put {} songs in place on the share: {:?}", r.failed.len(), r.failed);
            }
        }
        Err(SyncFailure::Running) => {}
        Err(e) => eprintln!("melue: can't put new songs in place on the share: {e:?}"),
    });
}
