//! Library data across devices: each device keeps its likes, play counts and latest plays (of songs on the
//! share, by their path there) in its own file on the share, `.melue/library/<device id>.json`, and reads the
//! others'. Only one device writes each file, so they never overwrite each other.
//!
//! - Likes: the newest change of a song (a like or an unlike, on any device) wins.
//! - Play counts: each device counts its own plays; the other devices' are kept apart (remote_track_stats,
//!   remote_play_days) and added up with this device's (the all_* views), so nothing is counted twice. The
//!   plays per day go back a year (the longest Top songs period).
//! - History: each device keeps its own 10,000 plays; the shared history is the latest 200 of each.
//!
//! Times in the files are on the share's clock (see handoff.rs): this device's own file tells how far the
//! share's clock is from its own.
//!
//! Playlists are synced after this, with the same share (playlist_sync.rs). Both run a few seconds after a
//! change here, after each sync, when the share says one of their files changed, and at start-up.

use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::handoff::{self, key};
use super::shared::with_share;
use crate::library::changes::{self, ChangeReason, LibraryChange};
use crate::library::playback::DAY_MS;
use crate::library::{err, now_ms, Folder, Library, Result};
use crate::share::{Share, ShareError};

/// Where the device files are on the share.
pub const DIR: &str = ".melue/library";
/// How many of its latest plays a device shares.
pub const SHARED_HISTORY: i64 = 200;
/// How many days of plays per day a device shares (the longest Top songs period, and a day).
const SHARED_DAYS: i64 = 366;
/// How long after a change here the files are written (changes come in bursts).
const SETTLE: Duration = Duration::from_secs(5);

/// One device's library data, as kept on the share.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceData {
    pub device_id: String,
    pub device_name: String,
    /// When it was written (the device's own clock).
    pub saved_at_ms: i64,
    pub likes: Vec<LikeRecord>,
    pub counts: Vec<CountRecord>,
    pub days: Vec<DayRecord>,
    pub history: Vec<PlayRecord>,
    /// When the share wrote it (the share's clock). Not in the file.
    #[serde(skip)]
    pub share_ms: i64,
}

/// A song liked or unliked, and when (the share's clock).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LikeRecord {
    pub path: String,
    pub liked: bool,
    pub at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountRecord {
    pub path: String,
    pub plays: i64,
    pub last_played_ms: Option<i64>,
}

/// Plays of a song on a day (UTC days since 1970).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayRecord {
    pub path: String,
    pub day: i64,
    pub plays: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRecord {
    pub path: String,
    pub started_ms: i64,
}

// ---- the rules (plain functions, tested in data_tests.rs)

/// A song's like on this device, by the newest change among this device's (`here`) and the others' records:
/// Some((liked, at)) when another device's change is newer and says otherwise. Times on the share's clock;
/// `here` is None when the song was never liked or unliked here.
pub fn newer_like(here: Option<(bool, i64)>, others: &[(bool, i64)]) -> Option<(bool, i64)> {
    let newest = others.iter().copied().max_by_key(|(_, at)| *at)?;
    match here {
        Some((liked, at)) if at >= newest.1 || liked == newest.0 => None,
        // Never touched here: only a like is news (not liked is how it is already).
        None if !newest.0 => None,
        _ => Some(newest),
    }
}

/// How far the share's clock is ahead of this device's, from its own file (0 before it has written one).
pub fn share_clock_ahead(files: &[DeviceData], me: &str) -> i64 {
    files.iter().find(|f| f.device_id == me).map_or(0, |f| f.share_ms - f.saved_at_ms)
}

/// Whether two versions of a device's file say the same (when it was written aside).
pub fn same_data(a: &DeviceData, b: &DeviceData) -> bool {
    let plain = |d: &DeviceData| DeviceData { saved_at_ms: 0, share_ms: 0, device_name: String::new(), ..d.clone() };
    plain(a) == plain(b) && a.device_name == b.device_name
}

// ---- the library's side

/// What a sync of the library data did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataResult {
    /// Songs liked or unliked here because another device did so later.
    pub likes_changed: Vec<i64>,
    /// Other devices whose data was read.
    pub devices: usize,
    /// Whether this device's file was written (something changed here).
    pub written: bool,
}

impl Library {
    /// This device's data for the share: songs of the synced folder only. `ahead_ms`: how far the share's
    /// clock is ahead of this device's.
    pub fn device_data(&self, folder: &Folder, device_id: &str, device_name: &str, ahead_ms: i64) -> Result<DeviceData> {
        let paths: HashMap<i64, String> = self.synced_songs(folder)?.into_iter().collect();
        let conn = self.conn();
        // (song, a number, maybe a time) rows.
        let rows = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Result<Vec<(i64, i64, Option<i64>)>> {
            let mut stmt = conn.prepare(sql).map_err(err)?;
            let rows = stmt.query_map(p, |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)
        };
        let path = |id: i64| paths.get(&id).cloned();
        let share_time = |t: i64| t + ahead_ms;
        let mut likes = Vec::new();
        for (track, at, _) in rows("SELECT track_id, liked_at, NULL FROM likes", &[])? {
            likes.extend(path(track).map(|path| LikeRecord { path, liked: true, at_ms: share_time(at) }));
        }
        for (track, at, _) in rows("SELECT track_id, unliked_at, NULL FROM unlikes", &[])? {
            likes.extend(path(track).map(|path| LikeRecord { path, liked: false, at_ms: share_time(at) }));
        }
        let counts = rows("SELECT track_id, play_count, last_played_at FROM track_stats WHERE play_count > 0", &[])?
            .into_iter()
            .filter_map(|(track, plays, last)| Some(CountRecord { path: path(track)?, plays, last_played_ms: last.map(share_time) }))
            .collect();
        let first_day = now_ms().div_euclid(DAY_MS) - SHARED_DAYS;
        let days = rows("SELECT track_id, plays, day FROM play_days WHERE day > ?1", &[&first_day])?
            .into_iter()
            .filter_map(|(track, plays, day)| Some(DayRecord { path: path(track)?, day: day?, plays }))
            .collect();
        // The latest plays of songs on the share (others are left out before counting the 200).
        let mut history = Vec::new();
        let mut stmt = conn.prepare("SELECT track_id, started_at FROM plays ORDER BY started_at DESC, id DESC").map_err(err)?;
        let mut all = stmt.query([]).map_err(err)?;
        while let Some(row) = all.next().map_err(err)? {
            let (track, started): (i64, i64) = (row.get(0).map_err(err)?, row.get(1).map_err(err)?);
            if let Some(path) = path(track) {
                history.push(PlayRecord { path, started_ms: share_time(started) });
                if history.len() as i64 == SHARED_HISTORY {
                    break;
                }
            }
        }
        let mut data = DeviceData {
            device_id: device_id.into(),
            device_name: device_name.into(),
            saved_at_ms: now_ms(),
            likes,
            counts,
            days,
            history,
            share_ms: 0,
        };
        // A steady order: the same data is the same file.
        data.likes.sort_by(|a, b| a.path.cmp(&b.path));
        data.counts.sort_by(|a, b| a.path.cmp(&b.path));
        data.days.sort_by(|a, b| (&a.path, a.day).cmp(&(&b.path, b.day)));
        Ok(data)
    }

    /// Takes in the other devices' data: their plays (replacing what was read before; devices whose file is
    /// gone are forgotten) and their newer likes. Returns the songs whose like changed here.
    pub fn import_device_data(&self, folder: &Folder, others: &[DeviceData], ahead_ms: i64) -> Result<Vec<i64>> {
        let songs: HashMap<String, i64> = self.synced_songs(folder)?.into_iter().map(|(id, p)| (key(&p), id)).collect();
        let track = |path: &str| songs.get(&key(path)).copied();
        let here_time = |t: i64| t - ahead_ms;
        {
            let mut conn = self.conn();
            let tx = conn.transaction().map_err(err)?;
            let devices: Vec<&str> = others.iter().map(|d| d.device_id.as_str()).collect();
            let known = serde_json::to_string(&devices).map_err(err)?;
            for table in ["remote_track_stats", "remote_play_days", "remote_plays"] {
                let sql = format!("DELETE FROM {table} WHERE device_id NOT IN (SELECT value FROM json_each(?1))");
                tx.execute(&sql, [&known]).map_err(err)?;
            }
            for data in others {
                let device = &data.device_id;
                for table in ["remote_track_stats", "remote_play_days", "remote_plays"] {
                    tx.execute(&format!("DELETE FROM {table} WHERE device_id = ?1"), [device]).map_err(err)?;
                }
                for c in &data.counts {
                    let Some(t) = track(&c.path) else { continue };
                    tx.execute(
                        "INSERT INTO remote_track_stats (device_id, track_id, play_count, last_played_at) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT DO UPDATE SET play_count = play_count + ?3, last_played_at = max(coalesce(last_played_at, 0), ?4)",
                        params![device, t, c.plays, c.last_played_ms.map(here_time)],
                    )
                    .map_err(err)?;
                }
                for d in &data.days {
                    let Some(t) = track(&d.path) else { continue };
                    tx.execute(
                        "INSERT INTO remote_play_days (device_id, track_id, day, plays) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT DO UPDATE SET plays = plays + ?4",
                        params![device, t, d.day, d.plays],
                    )
                    .map_err(err)?;
                }
                for p in &data.history {
                    let Some(t) = track(&p.path) else { continue };
                    tx.execute(
                        "INSERT INTO remote_plays (device_id, track_id, started_at) VALUES (?1, ?2, ?3)",
                        params![device, t, here_time(p.started_ms)],
                    )
                    .map_err(err)?;
                }
            }
            tx.commit().map_err(err)?;
        }

        // Likes: the newest change wins.
        let mut theirs: HashMap<i64, Vec<(bool, i64)>> = HashMap::new();
        for like in others.iter().flat_map(|d| &d.likes) {
            if let Some(t) = track(&like.path) {
                theirs.entry(t).or_default().push((like.liked, like.at_ms));
            }
        }
        let mut changed = Vec::new();
        for (t, records) in theirs {
            let here = self.like_here(t)?.map(|(liked, at)| (liked, at + ahead_ms));
            if let Some((liked, at)) = newer_like(here, &records) {
                self.set_liked_at(t, liked, here_time(at))?;
                changed.push(t);
            }
        }
        changed.sort_unstable();
        Ok(changed)
    }

    /// A song's like here, and when it was liked or unliked (None: neither, ever).
    fn like_here(&self, track: i64) -> Result<Option<(bool, i64)>> {
        let conn = self.conn();
        let at = |sql: &str| -> Result<Option<i64>> {
            use rusqlite::OptionalExtension;
            conn.query_row(sql, [track], |r| r.get(0)).optional().map_err(err)
        };
        if let Some(liked_at) = at("SELECT liked_at FROM likes WHERE track_id = ?1")? {
            return Ok(Some((true, liked_at)));
        }
        Ok(at("SELECT unliked_at FROM unlikes WHERE track_id = ?1")?.map(|t| (false, t)))
    }
}

/// Reads every device's file on the share, with the share's time for each (unreadable ones left out).
pub fn read_device_files(share: &dyn Share) -> Result<Vec<DeviceData>> {
    let entries = match share.list(DIR) {
        Ok(entries) => entries,
        Err(ShareError::NotFound { .. }) => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(entries
        .iter()
        .filter(|e| !e.is_dir && e.name.ends_with(".json"))
        .filter_map(|e| {
            let data: DeviceData = serde_json::from_slice(&share.read(&e.path).ok()?).ok()?;
            Some(DeviceData { share_ms: e.modified_ms, ..data })
        })
        .collect())
}

/// Syncs the library data with the share: takes in the other devices', and writes this device's when it
/// changed. Returns what it did, and how far the share's clock is ahead of this device's.
pub fn sync_device_data(library: &Library, share: &dyn Share, folder: &Folder, me: &str, name: &str) -> Result<(DataResult, i64)> {
    let files = read_device_files(share)?;
    let ahead = share_clock_ahead(&files, me);
    let (mine, others): (Vec<DeviceData>, Vec<DeviceData>) = files.into_iter().partition(|f| f.device_id == me);
    let likes_changed = library.import_device_data(folder, &others, ahead)?;
    let data = library.device_data(folder, me, name, ahead)?;
    let written = !mine.first().is_some_and(|old| same_data(old, &data));
    if written {
        share.create_dir_all(DIR)?;
        share.write(&format!("{DIR}/{me}.json"), &serde_json::to_vec(&data).map_err(err)?)?;
    }
    Ok((DataResult { likes_changed, devices: others.len(), written }, ahead))
}

// ---- running it

#[derive(Default)]
pub struct LibrarySync {
    soon: Mutex<Option<Sender<()>>>,
}

/// Whether a changed file on the share is library data or a playlist.
pub fn is_library_file(path: &str) -> bool {
    [DIR, super::playlist_sync::DIR].iter().any(|dir| path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/') && rest.ends_with(".json")))
}

/// Something changed (here: likes, playlists, plays; or on the share): the library data is synced once
/// things have been quiet for a moment.
pub fn soon(app: &AppHandle) {
    if let Some(sync) = app.try_state::<LibrarySync>() {
        if let Some(soon) = sync.soon.lock().unwrap().as_ref() {
            let _ = soon.send(());
        }
    }
}

/// Syncs the library data and the playlists now (on this thread), and tells the UI what changed.
pub fn run(app: &AppHandle) -> Result<()> {
    let library = app.state::<Library>();
    if !library.sync_available()? {
        return Ok(());
    }
    let Some(folder) = library.synced_folder()? else { return Ok(()) };
    let me = handoff::device_id(&library)?;
    let name = handoff::settings(app)?.device_name.unwrap_or_default();
    let (data, playlists) = with_share(app, |share| {
        let (data, ahead) = sync_device_data(&library, share, &folder, &me, &name)?;
        let playlists = super::playlist_sync::sync_playlists(&library, share, &folder, &me, &name, ahead)?;
        Ok((data, playlists))
    })?;
    if !data.likes_changed.is_empty() {
        let change = LibraryChange { reason: ChangeReason::Likes, tracks: data.likes_changed, sources: vec!["liked".into()] };
        changes::emit(app, change);
    }
    if !playlists.changed.is_empty() {
        changes::emit(app, LibraryChange::playlists(ChangeReason::Playlists, playlists.changed.clone()));
    }
    if playlists.conflicts_changed {
        super::playlist_sync::emit_conflicts(app);
    }
    Ok(())
}

/// Starts syncing the library data: shortly after start-up, then a few seconds after each burst of changes.
pub fn start(app: &AppHandle) {
    let (sender, wake) = mpsc::channel::<()>();
    *app.state::<LibrarySync>().soon.lock().unwrap() = Some(sender.clone());
    let app = app.clone();
    std::thread::Builder::new()
        .name("melue-library-sync".into())
        .spawn(move || {
            // After the library's start-up scan has had a moment.
            std::thread::sleep(Duration::from_secs(5));
            let _ = sender.send(());
            while wake.recv().is_ok() {
                loop {
                    match wake.recv_timeout(SETTLE) {
                        Ok(()) => continue,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                if let Err(e) = run(&app) {
                    eprintln!("melue: library data: can't sync: {e}");
                }
            }
        })
        .expect("the library sync thread");
}
