//! What changed in the library, for the UI (and, from step 10, the queue): one `library-changed` event
//! per change (a whole scan, a folder switched off, a playlist edited...), with the songs that changed and
//! the playable lists ("sources") whose songs changed.
//!
//! Found by comparing a snapshot of every song from before the change with one from after, so whatever
//! made the change, nothing is missed: a song added, edited (tags, cover), gone missing, back, moved, in
//! another album or with other artists, or hidden with its folder.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{err, Library, Result};

pub const CHANGE_EVENT: &str = "library-changed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeReason {
    /// A scan found changes.
    Scan,
    /// Music folders were added, removed, switched on or off, or renamed.
    Folders,
    /// A playlist was made, renamed, deleted, or its songs changed.
    Playlists,
    /// A song was liked or unliked.
    Likes,
    /// Whether sync is available changed (a share set up or removed, see sync.rs).
    Sync,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryChange {
    pub reason: ChangeReason,
    /// Songs added, changed, gone (missing, removed, hidden) or back, by id.
    pub tracks: Vec<i64>,
    /// The playable lists whose songs changed: "songs" (all songs), "album:<id>", "artist:<id>",
    /// "playlist:<id>", "liked", "folder:<id>".
    pub sources: Vec<String>,
}

impl LibraryChange {
    /// A playlist changed (its name, or which songs it has in what order): `tracks` were added, removed or
    /// moved.
    /// Playlists that changed (linked playlists brought in line with their files by a scan).
    pub fn playlists(reason: ChangeReason, mut ids: Vec<i64>) -> Self {
        ids.sort_unstable();
        ids.dedup();
        Self { reason, tracks: Vec::new(), sources: ids.into_iter().map(|id| format!("playlist:{id}")).collect() }
    }

    /// Sync became available or not.
    pub fn sync() -> Self {
        Self { reason: ChangeReason::Sync, tracks: Vec::new(), sources: Vec::new() }
    }

    /// A song liked or unliked.
    pub fn liked(track: i64) -> Self {
        Self { reason: ChangeReason::Likes, tracks: vec![track], sources: vec!["liked".into()] }
    }

    pub fn playlist(id: i64, tracks: Vec<i64>) -> Self {
        let mut tracks = tracks;
        tracks.sort_unstable();
        tracks.dedup();
        Self { reason: ChangeReason::Playlists, tracks, sources: vec![format!("playlist:{id}")] }
    }
}

/// What a song looks like to the UI, as far as changes go.
#[derive(Debug, Clone, PartialEq)]
struct TrackState {
    /// There: not missing, in a switched-on folder.
    present: bool,
    folder_id: i64,
    album_id: Option<i64>,
    artists: Vec<i64>,
    /// The playlists it's in.
    playlists: Vec<i64>,
    details: Details,
}

/// Everything else shown or played: the file and its tags.
#[derive(Debug, Clone, PartialEq)]
struct Details {
    path: String,
    size: i64,
    modified_at: i64,
    title: String,
    artist: Option<String>,
    cover_id: Option<i64>,
    track_no: Option<i64>,
    disc_no: Option<i64>,
    duration_ms: Option<i64>,
}

/// Every song's state at one moment.
#[derive(Debug, Default)]
pub struct Snapshot(HashMap<i64, TrackState>);

impl Library {
    pub fn snapshot(&self) -> Result<Snapshot> {
        let conn = self.conn();
        let lists = |sql: &str| -> Result<HashMap<i64, Vec<i64>>> {
            let mut found: HashMap<i64, Vec<i64>> = HashMap::new();
            let mut stmt = conn.prepare(sql).map_err(err)?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).map_err(err)?;
            for row in rows {
                let (track, other) = row.map_err(err)?;
                found.entry(track).or_default().push(other);
            }
            Ok(found)
        };
        let mut artists = lists("SELECT track_id, artist_id FROM track_artists ORDER BY track_id, position")?;
        let mut playlists = lists(
            "SELECT DISTINCT track_id, playlist_id FROM playlist_entries WHERE track_id IS NOT NULL ORDER BY track_id, playlist_id",
        )?;
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled, t.album_id, t.path, t.size, t.modified_at, t.title,
                        t.artist_credit, t.cover_id, t.track_no, t.disc_no, t.duration_ms, t.folder_id
                 FROM tracks t JOIN folders f ON f.id = t.folder_id",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                let id: i64 = r.get(0)?;
                let state = TrackState {
                    present: r.get(1)?,
                    folder_id: r.get(12)?,
                    album_id: r.get(2)?,
                    artists: Vec::new(),
                    playlists: Vec::new(),
                    details: Details {
                        path: r.get(3)?,
                        size: r.get(4)?,
                        modified_at: r.get(5)?,
                        title: r.get(6)?,
                        artist: r.get(7)?,
                        cover_id: r.get(8)?,
                        track_no: r.get(9)?,
                        disc_no: r.get(10)?,
                        duration_ms: r.get(11)?,
                    },
                };
                Ok((id, state))
            })
            .map_err(err)?;
        let mut snapshot = Snapshot::default();
        for row in rows {
            let (id, mut state) = row.map_err(err)?;
            state.artists = artists.remove(&id).unwrap_or_default();
            state.playlists = playlists.remove(&id).unwrap_or_default();
            snapshot.0.insert(id, state);
        }
        Ok(snapshot)
    }
}

impl Snapshot {
    /// What changed from this snapshot to `after`; None when nothing did.
    pub fn changes(&self, after: &Snapshot, reason: ChangeReason) -> Option<LibraryChange> {
        let mut tracks = BTreeSet::new();
        // (0, 0): all songs; (1, id): an album; (2, id): an artist; (3, id): a playlist; (4, id): a folder.
        // Sorted that way.
        let mut sources: BTreeSet<(u8, i64)> = BTreeSet::new();
        let ids: BTreeSet<i64> = self.0.keys().chain(after.0.keys()).copied().collect();
        for id in ids {
            let (old, new) = (self.0.get(&id), after.0.get(&id));
            let was = old.filter(|s| s.present);
            let is = new.filter(|s| s.present);
            let changed = match (was, is) {
                (None, None) => false,
                (Some(old), Some(new)) => old != new,
                _ => true,
            };
            if !changed {
                continue;
            }
            tracks.insert(id);
            sources.insert((0, 0));
            for state in [was, is].into_iter().flatten() {
                sources.extend(state.album_id.map(|album| (1, album)));
                sources.extend(state.artists.iter().map(|artist| (2, *artist)));
                sources.extend(state.playlists.iter().map(|playlist| (3, *playlist)));
                sources.insert((4, state.folder_id));
            }
        }
        if tracks.is_empty() {
            return None;
        }
        let sources = sources
            .into_iter()
            .map(|(kind, id)| match kind {
                0 => "songs".to_string(),
                1 => format!("album:{id}"),
                2 => format!("artist:{id}"),
                3 => format!("playlist:{id}"),
                _ => format!("folder:{id}"),
            })
            .collect();
        Some(LibraryChange { reason, tracks: tracks.into_iter().collect(), sources })
    }
}

/// Tells the UI about a change, sends Android Auto the library (once things settle), and brings the
/// playing queue in line when it touches its list (in the background: the player is asked what it plays).
pub fn emit(app: &AppHandle, change: LibraryChange) {
    let _ = app.emit(CHANGE_EVENT, &change);
    super::auto::send_soon(app);
    // Likes and playlists go to the share; songs arriving can fill in synced playlists' greyed songs.
    if matches!(change.reason, ChangeReason::Likes | ChangeReason::Playlists | ChangeReason::Scan) {
        crate::sync::data::soon(app);
    }
    let app = app.clone();
    std::thread::spawn(move || super::follow::follow(&app, &change));
}

/// Runs `change` and tells the UI what it changed. With `always`, it's told even when no song changed
/// (the folder list itself did).
pub fn tracking<T>(app: &AppHandle, reason: ChangeReason, always: bool, change: impl FnOnce() -> Result<T>) -> Result<T> {
    let library = app.state::<Library>();
    let before = library.snapshot()?;
    let result = change()?;
    let found = before.changes(&library.snapshot()?, reason);
    let found = found.or_else(|| always.then(|| LibraryChange { reason, tracks: Vec::new(), sources: Vec::new() }));
    if let Some(found) = found {
        emit(app, found);
    }
    Ok(result)
}
