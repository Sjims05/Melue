//! Songs every one of your devices has, each its own copy, but that aren't on the share ("Songs on all your
//! devices"): each device lists its songs that aren't on the share in its file there (data.rs), and sees which of
//! them all the others have too (the same song: same_song.rs; the others: the devices that synced in the last 30
//! days). Those are offered for the share: Ask (the Library card asks; a yes on any device counts), Automatically,
//! or Off. The device with the best copy (the higher bitrate, else the bigger file) moves its copy to sync
//! (moving.rs); the others get it with their next sync, and their own copy becomes a hidden copy of it (its likes,
//! plays and playlist places carried over).

use std::collections::HashSet;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::data::DeviceData;
use super::handoff::share_path;
use super::same_song::{plain_artist, plain_title, same_song, Songs};
use crate::library::{err, Folder, Library, Result};
use crate::guard::Locked;

/// A song a device has that isn't on the share, as the others see it (in its file on the share).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotOnShare {
    /// Its name, plain (for comparing).
    pub name: SongName,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub size: i64,
}

/// A song's artist and title, plain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SongName {
    pub artist: String,
    pub title: String,
}

/// A song here that's offered for the share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offered {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    /// Your other devices (all have it too), by name.
    pub devices: Vec<String>,
    /// This device has the best copy: it's the one that puts it on the share.
    pub best_here: bool,
    /// Someone said yes to it (on any device).
    pub approved: bool,
    #[serde(skip)]
    pub name: SongName,
}

/// What's offered now (the Library card shows it).
#[derive(Default)]
pub struct ToShare {
    offered: Mutex<Vec<Offered>>,
    /// Songs this device tried to put on the share since Melue started (not tried again and again when it fails).
    tried: Mutex<HashSet<i64>>,
}

/// The other devices that count: those that synced in the last this many days (an old phone doesn't hold it back).
pub const RECENT_DAYS: i64 = 30;

/// The offer changed: the app gets it.
pub const EVENT: &str = "to-share";
/// The songs someone here said to put on the share (a library setting: a list of SongName).
const APPROVED: &str = "to_share_approved";

/// A song here that isn't on the share: its id, title and artist, and what the others see.
pub struct Here {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub copy: NotOnShare,
}

impl Library {
    /// This device's songs that aren't on the share: in a music folder, without a synced copy here; or in the synced
    /// folder without having come through the share (put there by hand). Hidden copies aren't.
    pub fn songs_not_on_share(&self, folder: &Folder, songs: &Songs) -> Result<Vec<Here>> {
        let conn = self.conn();
        let on_share: HashSet<String> =
            conn.prepare("SELECT path FROM synced_files").and_then(|mut s| s.query_map([], |r| r.get::<_, String>(0))?.collect()).map_err(err)?;
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.folder_id, t.path, t.title, t.artist_credit, t.duration_ms, t.bitrate_kbps, t.size
                 FROM tracks t JOIN folders f ON f.id = t.folder_id
                 WHERE t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
            })
            .map_err(err)?;
        let mut here = Vec::new();
        for row in rows {
            let (id, folder_id, path, title, artist, duration_ms, bitrate, size) = row.map_err(err)?;
            let not_on_share =
                if folder_id == folder.id { share_path(folder, &path).is_some_and(|p| !on_share.contains(&p)) } else { songs.synced_copy(id).is_none() };
            if !not_on_share {
                continue;
            }
            let name = SongName { artist: plain_artist(artist.as_deref().unwrap_or_default()), title: plain_title(&title) };
            here.push(Here { id, title, artist, copy: NotOnShare { name, duration_ms, bitrate, size } });
        }
        Ok(here)
    }

    /// The songs someone here said to put on the share.
    pub fn approved_for_share(&self) -> Result<Vec<SongName>> {
        Ok(self.setting(APPROVED)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
    }

    fn set_approved_for_share(&self, names: &[SongName]) -> Result<()> {
        self.set_setting(APPROVED, &serde_json::to_string(names).map_err(err)?)
    }
}

/// What's offered for the share here: the songs here that all the other devices have too (not on the share);
/// which device has the best copy of each, and whether anyone said yes. `others`: the other devices that count
/// (synced lately).
pub fn offer(here: &[Here], others: &[DeviceData], me: &str, approved_here: &[SongName]) -> Vec<Offered> {
    if others.is_empty() {
        return Vec::new();
    }
    let approved: HashSet<&SongName> = approved_here.iter().chain(others.iter().flat_map(|d| &d.put_on_share)).collect();
    let same = |a: &NotOnShare, b: &NotOnShare| same_song((&a.name.artist, &a.name.title, a.duration_ms), (&b.name.artist, &b.name.title, b.duration_ms));
    // Better: the higher bitrate, else the bigger file; the same: the device first by id.
    let better = |a: &NotOnShare, a_device: &str, b: &NotOnShare, b_device: &str| (a.bitrate.unwrap_or(0), a.size, std::cmp::Reverse(a_device)) > (b.bitrate.unwrap_or(0), b.size, std::cmp::Reverse(b_device));
    here.iter()
        .filter_map(|song| {
            let theirs: Vec<(&DeviceData, &NotOnShare)> =
                others.iter().filter_map(|d| d.local_songs.iter().find(|c| same(&song.copy, c)).map(|c| (d, c))).collect();
            if theirs.len() < others.len() {
                return None;
            }
            Some(Offered {
                id: song.id,
                title: song.title.clone(),
                artist: song.artist.clone(),
                devices: theirs.iter().map(|(d, _)| d.device_name.clone()).collect(),
                best_here: theirs.iter().all(|(d, c)| better(&song.copy, me, c, &d.device_id)),
                approved: approved.contains(&song.copy.name),
                name: song.copy.name.clone(),
            })
        })
        .collect()
}

/// After the library data sync (data.rs): the offer for the sync card, and the songs this device puts on the share
/// (its best copies, with Automatically or a yes). Approvals of songs no longer offered here are dropped.
pub fn after_data_sync(app: &AppHandle, offered: Vec<Offered>) {
    let library = app.state::<Library>();
    if let Ok(approved) = library.approved_for_share() {
        let still: Vec<SongName> = approved.iter().filter(|n| offered.iter().any(|o| &o.name == *n)).cloned().collect();
        if still.len() != approved.len() {
            let _ = library.set_approved_for_share(&still);
        }
    }
    let state = app.state::<ToShare>();
    let changed = *state.offered.locked() != offered;
    *state.offered.locked() = offered.clone();
    if changed {
        let _ = app.emit(EVENT, &offered);
    }
    let mode = library.sync_settings().map(|s| s.songs_on_more_devices).unwrap_or_default();
    if mode == Mode::Off {
        return;
    }
    let ids: Vec<i64> = {
        let mut tried = state.tried.locked();
        offered.iter().filter(|o| o.best_here && (mode == Mode::Auto || o.approved) && tried.insert(o.id)).map(|o| o.id).collect()
    };
    if ids.is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let items = ids.into_iter().map(|id| super::moving::ToMove::Song { id }).collect();
        match super::moving::move_to_sync(&app, items, None) {
            Ok(result) => result.failed.iter().for_each(|(song, why)| eprintln!("melue: can't put {song} on the share: {why}")),
            Err(e) => eprintln!("melue: can't put songs on the share: {e:?}"),
        }
        super::data::soon(&app);
    });
}

/// "Songs on more than one device" (a sync setting, shared across devices).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Ask,
    Auto,
    Off,
}

/// What's offered for the share now.
#[tauri::command]
pub fn sync_to_share(app: AppHandle) -> Vec<Offered> {
    app.state::<ToShare>().offered.locked().clone()
}

/// "Put on share": a yes to songs offered now (`ids`; none given: all of them). This device puts its best copies
/// there at once; the others hear of the yes with the next library data sync, and put theirs.
#[tauri::command(async)]
pub fn sync_put_on_share(app: AppHandle, ids: Option<Vec<i64>>) -> Result<()> {
    let library = app.state::<Library>();
    let offered = sync_to_share(app.clone());
    let chosen = |o: &Offered| ids.as_ref().is_none_or(|ids| ids.contains(&o.id));
    let mut approved = library.approved_for_share()?;
    for song in offered.iter().filter(|o| chosen(o)) {
        if !approved.contains(&song.name) {
            approved.push(song.name.clone());
        }
    }
    library.set_approved_for_share(&approved)?;
    let offered: Vec<Offered> = offered.into_iter().map(|o| Offered { approved: o.approved || chosen(&o), ..o }).collect();
    after_data_sync(&app, offered);
    super::data::soon(&app);
    Ok(())
}
