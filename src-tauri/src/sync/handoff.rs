//! Synced playback: carrying on where another device left off. Each device keeps what it plays in its own
//! small file on the share, `.melue/playback/<device id>.json` (the queue by the songs' paths on the share,
//! the song, where in it, playing or not, shuffle and repeat), saved when the song or the queue changes, on
//! pause, on closing, and every 30 seconds while playing.
//!
//! Which is newest goes by the share's clock (the time the share gave each file when it was written), not
//! the devices' own clocks, which can be a little apart. A device's own clock difference from the share's
//! (its file's time on the share minus its own time in it) turns "now" into the share's time, to tell how
//! long ago another device played.
//!
//! At start-up, on coming back to the app, and when the share says a playback file changed, the newest other
//! device's playback is looked at. When it's newer than what this device last played (and than the last one
//! answered), it's offered: "Continue where you left off on <device>?" (the `playback-offer` event). Continue
//! loads its queue paused at its position. A device that is playing is never interrupted. The setting can also
//! follow other devices without asking, or switch it all off.
//!
//! Only songs on the share are carried over (the synced folder's): a song that's only on one device isn't
//! saved, and songs another device has that aren't here (not synced yet) are left out of its queue.

use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_native_audio::NativeAudioExt;

use crate::library::linked::document_path;
use crate::library::{err, now_ms, Folder, FolderKind, Library, Result};
use super::shared::with_share;
use crate::share::Share;

/// Where the playback files are on the share.
pub const DIR: &str = ".melue/playback";
pub const OFFER_EVENT: &str = "playback-offer";
/// Sent when another device's playback was loaded without asking ("follow", or recent enough).
pub const FOLLOWED_EVENT: &str = "playback-followed";
const SETTINGS: &str = "playback_sync";
const DEVICE_ID: &str = "device_id";
/// The newest other device's playback answered (Continue or Not now): its time on the share.
const ANSWERED: &str = "playback_answered";
/// How often the player is looked at.
const POLL: Duration = Duration::from_secs(5);
/// While playing, the position is saved this often.
pub const SAVE_PLAYING_MS: i64 = 30_000;
/// Paused, moving more than this (seconds) is saved.
const MOVED_S: f64 = 2.0;

/// One device's playback, as kept on the share.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackFile {
    pub device_id: String,
    pub device_name: String,
    /// When it changed (the device's own clock).
    pub saved_at_ms: i64,
    /// When the share wrote it (the share's clock, the same for every device). Not in the file: read with it.
    #[serde(skip)]
    pub share_ms: i64,
    #[serde(flatten)]
    pub state: Playback,
    /// The song's title and artist (to say what it is, even on a device that doesn't have it).
    pub title: Option<String>,
    pub artist: Option<String>,
}

/// What a device plays.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playback {
    /// The queue's songs, by their path on the share.
    pub queue: Vec<String>,
    /// The song in `queue`.
    pub index: usize,
    /// Seconds into the song.
    pub position_s: f64,
    pub playing: bool,
    pub shuffle: bool,
    /// "off", "all" or "one".
    pub repeat: String,
}

/// What to do with another device's playback.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Ask first: "Continue where you left off on <device>?".
    #[default]
    Ask,
    /// Load it (paused) without asking.
    Follow,
    /// Neither saved nor offered.
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackSettings {
    pub mode: Mode,
    /// This device's name for the others (None: the computer's name or the phone's).
    pub device_name: Option<String>,
    /// Ask: continue without asking when the other device's playback is at most this many minutes old (it's
    /// most likely a switch from that device to this one). None: always ask. Default 10.
    pub continue_within_minutes: Option<u32>,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self { mode: Mode::Ask, device_name: None, continue_within_minutes: Some(10) }
    }
}

/// Synced playback's state while Melue runs.
#[derive(Default)]
pub struct Handoff {
    /// The playback last saved (or loaded), to tell when it changes, and when it was saved.
    last: Mutex<Option<(Playback, i64)>>,
    /// Another device's playback waiting for an answer.
    offer: Mutex<Option<PlaybackFile>>,
    checks: Mutex<Option<Sender<()>>>,
}

// ---- the rules (plain functions, tested in handoff_tests.rs)

/// Whether a changed file on the share is a playback file.
pub fn is_playback_file(path: &str) -> bool {
    path.strip_prefix(DIR).is_some_and(|rest| rest.starts_with('/') && rest.ends_with(".json"))
}

/// A song's path on the share, from its path in the synced folder (None: a folder that isn't synced).
pub fn share_path(folder: &Folder, track_path: &str) -> Option<String> {
    match folder.kind {
        FolderKind::Path => Some(track_path.replace('\\', "/")),
        // A chosen Android folder's songs are content:// addresses: their document paths, within the folder's.
        FolderKind::AndroidTree => {
            let root = document_path(&folder.location);
            document_path(track_path).strip_prefix(&format!("{}/", root.trim_end_matches('/'))).map(str::to_string)
        }
        FolderKind::AndroidMedia => None,
    }
}

/// Paths compare without case (like the sync's).
pub(crate) fn key(path: &str) -> String {
    path.to_lowercase()
}

/// The queue to save: its songs on the share (others left out) and the song's place among them. None: the
/// song isn't on the share (then nothing is saved).
pub fn on_share(ids: &[Option<i64>], index: usize, paths: &HashMap<i64, String>) -> Option<(Vec<String>, usize)> {
    let path = |id: &Option<i64>| id.and_then(|id| paths.get(&id).cloned());
    path(ids.get(index)?)?;
    let before = ids[..index].iter().filter(|id| path(id).is_some()).count();
    Some((ids.iter().filter_map(path).collect(), before))
}

/// The songs here for a saved queue, and the song's place among them. None: the song isn't here.
pub fn here(playback: &Playback, songs: &HashMap<String, i64>) -> Option<(Vec<i64>, usize)> {
    let id = |path: &String| songs.get(&key(path)).copied();
    id(playback.queue.get(playback.index)?)?;
    let before = playback.queue[..playback.index].iter().filter(|p| id(p).is_some()).count();
    Some((playback.queue.iter().filter_map(id).collect(), before))
}

/// Whether the playback changed enough to save: another song or queue, play or pause, shuffle or repeat;
/// while playing every [`SAVE_PLAYING_MS`]; paused, moved to another place in the song.
pub fn needs_saving(last: Option<&(Playback, i64)>, now: &Playback, now_ms: i64) -> bool {
    let Some((last, saved_ms)) = last else { return true };
    let other_song = last.queue != now.queue || last.index != now.index;
    let other_settings = last.playing != now.playing || last.shuffle != now.shuffle || last.repeat != now.repeat;
    other_song
        || other_settings
        || (now.playing && now_ms - saved_ms >= SAVE_PLAYING_MS)
        || (!now.playing && (now.position_s - last.position_s).abs() > MOVED_S)
}

/// Whether another device's playback (written at `share_ms`, the share's clock) is loaded without asking:
/// "follow", or "ask" with playback recent enough. `now_ms`: now, on the share's clock.
pub fn continues_by_itself(settings: &PlaybackSettings, share_ms: i64, now_ms: i64) -> bool {
    match settings.mode {
        Mode::Follow => true,
        Mode::Ask => settings.continue_within_minutes.is_some_and(|m| now_ms - share_ms <= i64::from(m) * 60_000),
        Mode::Off => false,
    }
}

/// How far the share's clock is ahead of this device's (from this device's own file: the share's time for it
/// minus the device's). 0 before this device has saved anything.
pub fn share_clock_ahead(files: &[PlaybackFile], me: &str) -> i64 {
    files.iter().find(|f| f.device_id == me).map_or(0, |f| f.share_ms - f.saved_at_ms)
}

/// The other device's playback to offer: the newest on the share's clock, if it's newer than this device's
/// own and than `answered_ms` (the last one answered).
pub fn newest_other(files: Vec<PlaybackFile>, me: &str, answered_ms: i64) -> Option<PlaybackFile> {
    let mine = files.iter().find(|f| f.device_id == me).map_or(0, |f| f.share_ms);
    let after = mine.max(answered_ms);
    files
        .into_iter()
        .filter(|f| f.device_id != me && !f.state.queue.is_empty() && f.share_ms > after)
        .max_by_key(|f| f.share_ms)
}

// ---- this device

/// This device's id (made the first time: it stays when the device is renamed).
pub fn device_id(library: &Library) -> Result<String> {
    if let Some(id) = library.setting(DEVICE_ID)?.filter(|id| !id.is_empty()) {
        return Ok(id);
    }
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_i64(now_ms());
    hasher.write_u32(std::process::id());
    let id = format!("{:016x}", hasher.finish());
    library.set_setting(DEVICE_ID, &id)?;
    Ok(id)
}

/// The computer's name, or the phone's (as set in its settings, or its model).
fn system_name(app: &AppHandle) -> String {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        app.melue_android().device_name().unwrap_or_else(|_| "Phone".into())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        ["COMPUTERNAME", "HOSTNAME"]
            .iter()
            .find_map(|name| std::env::var(name).ok().filter(|n| !n.trim().is_empty()))
            .unwrap_or_else(|| "Computer".into())
    }
}

// ---- with the library and the share, without the app (the car uses these too: car.rs)

/// The settings as saved (the device name: None when it's the system's).
pub fn stored_settings(library: &Library) -> Result<PlaybackSettings> {
    match library.setting(SETTINGS)? {
        Some(json) => serde_json::from_str(&json).map_err(err),
        None => Ok(PlaybackSettings::default()),
    }
}

/// The time on the share of the newest other device's playback answered (Continue, Not now, or loaded).
/// Connections of remote playback (remote/: this device playing on another, or another playing on this one): while
/// there's one, synced playback is off here (nothing saved, nothing offered); it's back when they end.
static CONNECTIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A connection began (remote playback): synced playback rests until it ends ([`connection_ended`]).
pub fn connection_began() {
    CONNECTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

pub fn connection_ended() {
    let _ = CONNECTIONS.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |n| n.checked_sub(1));
}

fn resting() -> bool {
    CONNECTIONS.load(std::sync::atomic::Ordering::SeqCst) > 0
}

/// A device whose playback isn't offered or followed here, until a time (this device's clock): the one this device
/// plays on (Connect device, remote/link.rs), and a minute after, so the move itself isn't offered back.
static IGNORED: Mutex<Option<(String, i64)>> = Mutex::new(None);

/// Ignores device `id`'s playback until `until_ms` (`i64::MAX`: until said otherwise).
pub fn ignore(id: &str, until_ms: i64) {
    *IGNORED.lock().unwrap() = Some((id.to_string(), until_ms));
}

fn ignored() -> Option<String> {
    IGNORED.lock().unwrap().as_ref().filter(|(_, until)| now_ms() < *until).map(|(id, _)| id.clone())
}

pub fn answered_ms(library: &Library) -> i64 {
    library.setting(ANSWERED).ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(0)
}

pub fn mark_answered(library: &Library, other: &PlaybackFile) -> Result<()> {
    library.set_setting(ANSWERED, &other.share_ms.to_string())
}

/// Every device's playback on the share, with the share's time for each (files that can't be read are left
/// out).
pub fn read_playback_files(share: &dyn Share) -> Result<Vec<PlaybackFile>> {
    let entries = match share.list(DIR) {
        Ok(entries) => entries,
        Err(crate::share::ShareError::NotFound { .. }) => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(entries
        .iter()
        .filter(|e| !e.is_dir && e.name.ends_with(".json"))
        .filter_map(|e| {
            let file: PlaybackFile = serde_json::from_slice(&share.read(&e.path).ok()?).ok()?;
            Some(PlaybackFile { share_ms: e.modified_ms, ..file })
        })
        .collect())
}

/// Writes a device's playback to the share.
pub fn write_playback_file(share: &dyn Share, file: &PlaybackFile) -> Result<()> {
    let json = serde_json::to_vec_pretty(file).map_err(err)?;
    share.create_dir_all(DIR)?;
    Ok(share.write(&format!("{DIR}/{}.json", file.device_id), &json)?)
}

/// The queue here for another device's playback: its songs that are here, and the song's place among them
/// (None: the song isn't here, not synced yet).
pub fn queue_here(library: &Library, folder: &Folder, other: &Playback) -> Result<Option<(Vec<tauri_plugin_native_audio::QueueItem>, usize)>> {
    let songs: HashMap<String, i64> = library.synced_songs(folder)?.into_iter().map(|(id, p)| (key(&p), id)).collect();
    let Some((ids, index)) = here(other, &songs) else { return Ok(None) };
    // The songs that can play (one that's here but missing, or hidden as a copy, is left out of the queue): the song's
    // place counted among them, not among all.
    let playing = ids[index];
    let items = library.queue_items_for(&ids)?;
    let Some(index) = items.iter().position(|item| item.id == Some(playing)) else { return Ok(None) };
    Ok(Some((items, index)))
}

/// What plays, on the share's terms, from the queue's item ids (None: the song isn't on the share).
pub fn playback_on_share(library: &Library, folder: &Folder, ids: &[Option<i64>], index: usize, position_s: f64, modes: (bool, bool, &str)) -> Result<Option<Playback>> {
    let paths: HashMap<i64, String> = library.synced_songs(folder)?.into_iter().collect();
    let Some((queue, index)) = on_share(ids, index, &paths) else { return Ok(None) };
    let (playing, shuffle, repeat) = modes;
    let repeat = if repeat.is_empty() { "off" } else { repeat };
    Ok(Some(Playback { queue, index, position_s: position_s.max(0.0), playing, shuffle, repeat: repeat.into() }))
}

// ---- with the app

pub fn settings(app: &AppHandle) -> Result<PlaybackSettings> {
    let saved = stored_settings(&app.state::<Library>())?;
    let device_name = saved.device_name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| system_name(app));
    Ok(PlaybackSettings { device_name: Some(device_name), ..saved })
}

pub fn set_settings(app: &AppHandle, settings: &PlaybackSettings) -> Result<()> {
    let name = settings.device_name.as_deref().map(str::trim).filter(|n| !n.is_empty() && *n != system_name(app));
    let saved = PlaybackSettings { device_name: name.map(str::to_string), ..settings.clone() };
    app.state::<Library>().set_setting(SETTINGS, &serde_json::to_string(&saved).map_err(err)?)
}

impl Library {
    /// The synced folder's songs: (track id, path on the share).
    pub(crate) fn synced_songs(&self, folder: &Folder) -> Result<Vec<(i64, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, path FROM tracks WHERE folder_id = ?1 AND missing_since IS NULL").map_err(err)?;
        let rows = stmt.query_map([folder.id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(err)?;
        let rows: Vec<(i64, String)> = rows.collect::<rusqlite::Result<_>>().map_err(err)?;
        Ok(rows.into_iter().filter_map(|(id, path)| share_path(folder, &path).map(|p| (id, p))).collect())
    }

    /// The synced folder (None: no share set up).
    pub(crate) fn synced_folder(&self) -> Result<Option<Folder>> {
        match self.share_settings()? {
            Some(settings) => self.folder(settings.sync_folder_id),
            None => Ok(None),
        }
    }
}

/// The network changed: the share is opened again (on the new network), and the others are looked at.
pub fn network_changed(app: &AppHandle) {
    super::shared::reset(app);
    check(app);
}

/// What plays here now, on the share's terms, and the song's (title, artist).
type PlayingHere = (Playback, (Option<String>, Option<String>));

/// What plays here now (None: nothing, or a song that isn't on the share).
pub(crate) fn playing_here(app: &AppHandle, library: &Library, folder: &Folder) -> Result<Option<PlayingHere>> {
    let player = app.native_audio();
    let state = player.get_state()?;
    if state.queue_index < 0 {
        return Ok(None);
    }
    let queue = player.get_queue()?;
    let Ok(index) = usize::try_from(queue.current_index) else { return Ok(None) };
    let ids: Vec<Option<i64>> = queue.items.iter().map(|i| i.id).collect();
    let modes = (state.is_playing, state.shuffle, state.repeat_mode.as_str());
    let Some(playback) = playback_on_share(library, folder, &ids, index, state.current_time, modes)? else { return Ok(None) };
    let current = &queue.items[index];
    Ok(Some((playback, (current.title.clone(), current.artist.clone()))))
}

/// Saves what plays here when it changed (`force`: also when it didn't, e.g. on closing).
fn save(app: &AppHandle, force: bool) -> Result<()> {
    let library = app.state::<Library>();
    if settings(app)?.mode == Mode::Off {
        return Ok(());
    }
    let Some(folder) = library.synced_folder()? else { return Ok(()) };
    let Some((playback, (title, artist))) = playing_here(app, &library, &folder)? else { return Ok(()) };
    let handoff = app.state::<Handoff>();
    let now = now_ms();
    {
        let mut last = handoff.last.lock().unwrap();
        // Melue just started: the player's queue from last time isn't something new to tell the others.
        if last.is_none() && !force {
            *last = Some((playback, now));
            return Ok(());
        }
        if !force && !needs_saving(last.as_ref(), &playback, now) {
            return Ok(());
        }
    }
    let file = PlaybackFile {
        device_id: device_id(&library)?,
        device_name: settings(app)?.device_name.unwrap_or_default(),
        saved_at_ms: now,
        share_ms: 0,
        state: playback,
        title,
        artist,
    };
    with_share(app, |share| write_playback_file(share, &file))?;
    // Only once it's on the share (otherwise it's tried again on the next look at the player).
    *handoff.last.lock().unwrap() = Some((file.state, now));
    Ok(())
}

/// Saves what plays here now (closing the app).
pub fn save_now(app: &AppHandle) {
    if let Err(e) = save(app, true) {
        eprintln!("melue: playback: can't save: {e}");
    }
}

/// Every device's playback on the share, with the share's time for each (files that can't be read are left
/// out).
fn read_all(app: &AppHandle) -> Result<Vec<PlaybackFile>> {
    with_share(app, read_playback_files)
}

/// Looks at the other devices' playback: offers the newest (or loads it, with "follow").
fn look(app: &AppHandle) -> Result<()> {
    let library = app.state::<Library>();
    let settings = settings(app)?;
    if settings.mode == Mode::Off || !library.sync_available()? {
        return Ok(());
    }
    let mut files = read_all(app)?;
    let me = device_id(&library)?;
    let now_on_share = now_ms() + share_clock_ahead(&files, &me);
    // The device this one plays on: its playback is this device's own, in a way (not offered back).
    if let Some(ignored) = ignored() {
        files.retain(|f| f.device_id != ignored);
    }
    let Some(other) = newest_other(files, &me, answered_ms(&library)) else { return Ok(()) };
    // Never interrupt: playing here wins (and soon saves, which makes it the newest).
    if app.native_audio().get_state()?.is_playing {
        return Ok(());
    }
    // A song that isn't here yet (not synced) can't be continued.
    let Some(folder) = library.synced_folder()? else { return Ok(()) };
    if queue_here(&library, &folder, &other.state)?.is_none() {
        return Ok(());
    }
    let handoff = app.state::<Handoff>();
    if continues_by_itself(&settings, other.share_ms, now_on_share) {
        load(app, &other)?;
        *handoff.offer.lock().unwrap() = None;
        let _ = app.emit(FOLLOWED_EVENT, &other);
    } else {
        let mut offer = handoff.offer.lock().unwrap();
        if offer.as_ref().is_some_and(|o| o.share_ms >= other.share_ms) {
            return Ok(());
        }
        *offer = Some(other.clone());
        let _ = app.emit(OFFER_EVENT, &other);
    }
    Ok(())
}

/// Loads another device's playback, paused at its position.
fn load(app: &AppHandle, other: &PlaybackFile) -> Result<()> {
    let library = app.state::<Library>();
    let folder = library.synced_folder()?.ok_or("no share is set up")?;
    let (items, index) = queue_here(&library, &folder, &other.state)?.ok_or("that song isn't here (not synced yet?)")?;
    let player = app.native_audio();
    player.set_queue(items, index, other.state.position_s, None)?;
    player.set_shuffle(other.state.shuffle)?;
    player.set_repeat_mode(&other.state.repeat)?;
    mark_answered(&library, other)?;
    // Not something new to tell the others: it's theirs (saved once it changes here).
    let loaded = playing_here(app, &library, &folder)?.map(|(p, ..)| p).unwrap_or_else(|| Playback { playing: false, ..other.state.clone() });
    *app.state::<Handoff>().last.lock().unwrap() = Some((loaded, now_ms()));
    Ok(())
}

/// The offer waiting for an answer.
pub fn offer(app: &AppHandle) -> Option<PlaybackFile> {
    app.state::<Handoff>().offer.lock().unwrap().clone()
}

/// The offered song here (its id, for its cover), if this device has it.
pub fn offer_song(app: &AppHandle) -> Result<Option<i64>> {
    let Some(other) = offer(app) else { return Ok(None) };
    let library = app.state::<Library>();
    let Some(folder) = library.synced_folder()? else { return Ok(None) };
    let Some(path) = other.state.queue.get(other.state.index) else { return Ok(None) };
    Ok(library.synced_songs(&folder)?.into_iter().find(|(_, p)| key(p) == key(path)).map(|(id, _)| id))
}

/// The answer to the offer: Continue (load it) or Not now (it isn't offered again).
pub fn answer(app: &AppHandle, continue_here: bool) -> Result<()> {
    let Some(other) = app.state::<Handoff>().offer.lock().unwrap().take() else { return Ok(()) };
    if continue_here {
        load(app, &other)
    } else {
        mark_answered(&app.state::<Library>(), &other)
    }
}

/// Asks for a look at the other devices' playback (start-up, coming back, a playback file changed).
pub fn check(app: &AppHandle) {
    if let Some(checks) = app.state::<Handoff>().checks.lock().unwrap().as_ref() {
        let _ = checks.send(());
    }
}

/// Starts synced playback: a look shortly after start-up, then saving what plays here every few seconds
/// when it changed, and looking again when asked.
pub fn start(app: &AppHandle) {
    let (sender, checks) = mpsc::channel::<()>();
    *app.state::<Handoff>().checks.lock().unwrap() = Some(sender.clone());
    let app = app.clone();
    std::thread::Builder::new()
        .name("melue-playback".into())
        .spawn(move || {
            // After the player has restored its queue.
            std::thread::sleep(Duration::from_secs(3));
            let _ = sender.send(());
            loop {
                // Mobile data, "Nothing": synced playback waits for Wi-Fi too; and it rests during remote playback.
                let waiting = resting() || app.state::<Library>().sync_settings().is_ok_and(|s| super::mobile::waits_for_wifi(&app, &s));
                match checks.recv_timeout(POLL) {
                    Ok(()) if waiting => while checks.try_recv().is_ok() {},
                    Ok(()) => {
                        while checks.try_recv().is_ok() {}
                        if let Err(e) = look(&app) {
                            eprintln!("melue: playback: can't look at the other devices: {e}");
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                if !waiting && app.state::<Library>().sync_available().unwrap_or(false) {
                    if let Err(e) = save(&app, false) {
                        eprintln!("melue: playback: can't save: {e}");
                    }
                }
            }
        })
        .expect("the playback thread");
}
