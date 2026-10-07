//! Synced playback in the car with Melue closed (Phase 2 step 10a). When Android Auto starts Melue's player
//! by itself, only the player runs (native-audio's Kotlin service): Melue's app, and with it handoff.rs, doesn't.
//! The player asks Melue's queue provider (CarQueue.kt in Melue's Android plugin), which calls these functions
//! through JNI; they use the same library, share and rules as the app, without it:
//! - the player starts with nothing loaded: another device's newer playback is loaded (paused) when it's at
//!   most the setting's minutes old (10; "follow": always, "off": never), else the player keeps its own;
//! - the "Recently played" tab starts with "Continue from <device>" when another device has newer playback;
//! - what the car plays is saved to the share like the app does (on changes; every 30 s while playing).
//!
//! The share's password: Android's secure store (handed over by the Kotlin side), or the plain file next to
//! the library; asked each time: no share in the car.

// Called from Android only (the tests use it on desktop).
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri_plugin_native_audio::QueueItem;

use super::handoff::{
    self, continues_by_itself, mark_answered, needs_saving, newest_other, playback_on_share, queue_here, read_playback_files,
    share_clock_ahead, stored_settings, write_playback_file, Mode, Playback, PlaybackFile,
};
use crate::library::{now_ms, Library, Result};
use crate::share::secrets::{FileSecrets, PasswordStore, Secrets};
use crate::share::setup::{self, PASSWORD_KEY};
use crate::share::Share;

/// The folder (its id in the Android Auto library) that gets the "Continue from" entry.
pub const ENTRY_FOLDER: &str = "recent";

/// A queue for the player.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarQueue {
    pub items: Vec<QueueItem>,
    pub index: usize,
    pub position_ms: i64,
    pub shuffle: bool,
    pub repeat: String,
}

/// An entry at the top of the car's "Recently played".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarEntry {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
}

/// What the car plays (from the player).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CarPlayback {
    pub item_ids: Vec<Option<i64>>,
    pub index: usize,
    pub position_ms: i64,
    pub playing: bool,
    pub shuffle: bool,
    pub repeat_mode: String,
    pub title: Option<String>,
    pub artist: Option<String>,
}

/// The library and share, kept open while the player runs; and the playback last saved.
pub struct Car {
    data_dir: PathBuf,
    library: Library,
    share: Option<Box<dyn Share>>,
    last: Option<(Playback, i64)>,
}

static CAR: Mutex<Option<Car>> = Mutex::new(None);

/// Runs `work` with the car's library (opened the first time) from the app's data folder.
pub fn with_car<T>(data_dir: &Path, work: impl FnOnce(&mut Car) -> Result<T>) -> Result<T> {
    let mut car = CAR.lock().unwrap_or_else(|p| p.into_inner());
    if car.as_ref().is_none_or(|c| c.data_dir != data_dir) {
        let library = Library::open(&data_dir.join("library.db"))?;
        *car = Some(Car { data_dir: data_dir.to_path_buf(), library, share: None, last: None });
    }
    work(car.as_mut().expect("opened"))
}

/// "1:23" for 83 seconds.
fn minutes(seconds: f64) -> String {
    let s = seconds.max(0.0) as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl Car {
    #[cfg(test)]
    pub fn new(data_dir: &Path, library: Library) -> Self {
        Self { data_dir: data_dir.to_path_buf(), library, share: None, last: None }
    }

    #[cfg(test)]
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// Runs `work` with the share (opened when needed; a failure closes it). `password`: from Android's secure
    /// store, when that's where it's kept.
    fn with_share<T>(&mut self, password: Option<&str>, work: impl FnOnce(&Library, &dyn Share) -> Result<T>) -> Result<T> {
        if self.share.is_none() {
            let settings = self.library.share_settings()?.ok_or("no share is set up")?;
            let password = match &settings.setup {
                setup::ShareSetup::Smb { password_store: PasswordStore::Secure, .. } => password.map(str::to_string),
                setup::ShareSetup::Smb { password_store: PasswordStore::File, .. } => {
                    FileSecrets::new(self.data_dir.join("secrets.json")).get(PASSWORD_KEY)?
                }
                setup::ShareSetup::Smb { .. } => return Err("the share's password is asked each time: not in the car".into()),
                setup::ShareSetup::Drive { .. } => None,
            };
            self.share = Some(setup::open(&settings.setup, password.as_deref()).map_err(|e| format!("{e:?}"))?);
        }
        let result = work(&self.library, self.share.as_deref().expect("opened"));
        if result.is_err() {
            self.share = None;
        }
        result
    }

    /// The other device's playback to continue, read from the share (newer than this device's own, and not
    /// answered yet), and how far the share's clock is ahead.
    fn newest_other(&mut self, password: Option<&str>) -> Result<Option<(PlaybackFile, i64)>> {
        if stored_settings(&self.library)?.mode == Mode::Off {
            return Ok(None);
        }
        let me = handoff::device_id(&self.library)?;
        let answered = handoff::answered_ms(&self.library);
        let files = self.with_share(password, |_, share| read_playback_files(share))?;
        let ahead = share_clock_ahead(&files, &me);
        Ok(newest_other(files, &me, answered).map(|f| (f, ahead)))
    }

    /// Loads another device's playback as a queue here (marked answered: not offered again).
    fn take(&mut self, other: &PlaybackFile) -> Result<Option<CarQueue>> {
        let Some(folder) = self.library.synced_folder()? else { return Ok(None) };
        let Some((items, index)) = queue_here(&self.library, &folder, &other.state)? else { return Ok(None) };
        mark_answered(&self.library, other)?;
        // Theirs, not news to tell the others (saved once it changes here).
        self.last = Some((Playback { playing: false, ..other.state.clone() }, now_ms()));
        let s = &other.state;
        Ok(Some(CarQueue { items, index, position_ms: (s.position_s * 1000.0) as i64, shuffle: s.shuffle, repeat: s.repeat.clone() }))
    }

    /// The player started with nothing loaded: another device's playback when it's recent enough.
    pub fn queue_at_start(&mut self, password: Option<&str>) -> Result<Option<CarQueue>> {
        let Some((other, ahead)) = self.newest_other(password)? else { return Ok(None) };
        if !continues_by_itself(&stored_settings(&self.library)?, other.share_ms, now_ms() + ahead) {
            return Ok(None);
        }
        self.take(&other)
    }

    /// "Continue from <device>" for the car's library folder `folder_id` (only "Recently played").
    pub fn entries(&mut self, password: Option<&str>, folder_id: &str) -> Result<Vec<CarEntry>> {
        if folder_id != ENTRY_FOLDER {
            return Ok(Vec::new());
        }
        let Some((other, _)) = self.newest_other(password)? else { return Ok(Vec::new()) };
        let Some(folder) = self.library.synced_folder()? else { return Ok(Vec::new()) };
        if queue_here(&self.library, &folder, &other.state)?.is_none() {
            return Ok(Vec::new());
        }
        let song = match (&other.title, &other.artist) {
            (Some(title), Some(artist)) => format!("{title} — {artist}"),
            (Some(title), None) => title.clone(),
            _ => other.state.queue.get(other.state.index).cloned().unwrap_or_default(),
        };
        Ok(vec![CarEntry {
            id: format!("{}@{}", other.device_id, other.share_ms),
            title: format!("Continue from {}", other.device_name),
            subtitle: Some(format!("{song} · {}", minutes(other.state.position_s))),
        }])
    }

    /// The "Continue from" entry was picked: that device's playback (its newest).
    pub fn queue_for_entry(&mut self, password: Option<&str>, entry_id: &str) -> Result<Option<CarQueue>> {
        let device = entry_id.split('@').next().unwrap_or_default();
        let me = handoff::device_id(&self.library)?;
        let files = self.with_share(password, |_, share| read_playback_files(share))?;
        let Some(other) = files.into_iter().filter(|f| f.device_id == device && f.device_id != me).max_by_key(|f| f.share_ms) else {
            return Ok(None);
        };
        self.take(&other)
    }

    /// What the car plays: saved to the share when it changed (like the app does). `name`: this phone's name.
    pub fn playback(&mut self, password: Option<&str>, name: &str, car: &CarPlayback) -> Result<bool> {
        let settings = stored_settings(&self.library)?;
        if settings.mode == Mode::Off {
            return Ok(false);
        }
        let Some(folder) = self.library.synced_folder()? else { return Ok(false) };
        let modes = (car.playing, car.shuffle, car.repeat_mode.as_str());
        let position_s = car.position_ms as f64 / 1000.0;
        let Some(playback) = playback_on_share(&self.library, &folder, &car.item_ids, car.index, position_s, modes)? else { return Ok(false) };
        let now = now_ms();
        // The first look: the queue the player started with isn't news (its own last queue, or one taken).
        if self.last.is_none() {
            self.last = Some((playback, now));
            return Ok(false);
        }
        if !needs_saving(self.last.as_ref(), &playback, now) {
            return Ok(false);
        }
        let file = PlaybackFile {
            device_id: handoff::device_id(&self.library)?,
            device_name: settings.device_name.unwrap_or_else(|| name.to_string()),
            saved_at_ms: now,
            share_ms: 0,
            state: playback,
            title: car.title.clone(),
            artist: car.artist.clone(),
        };
        self.with_share(password, |_, share| write_playback_file(share, &file))?;
        self.last = Some((file.state, now));
        Ok(true)
    }
}

/// The Android side: what CarQueue.kt calls (JSON in and out; nothing here ever crashes the player).
#[cfg(target_os = "android")]
mod android {
    use std::ffi::CString;
    use std::path::Path;

    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use jni::JNIEnv;

    use super::{with_car, CarPlayback};

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(priority: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
    }

    /// To Android's log (logcat), tag "MelueCar".
    fn log(text: &str) {
        let (tag, text) = (CString::new("MelueCar").unwrap(), CString::new(text.replace('\0', " ")).unwrap_or_default());
        unsafe { __android_log_write(4, tag.as_ptr(), text.as_ptr()) };
    }

    fn text(env: &mut JNIEnv, s: &JString) -> Option<String> {
        if s.is_null() {
            return None;
        }
        env.get_string(s).ok().map(Into::into)
    }

    /// Runs `work` (caught: an error or a panic answers null), answering its JSON.
    fn answer<T: serde::Serialize>(env: &mut JNIEnv, what: &str, work: impl FnOnce() -> crate::library::Result<Option<T>>) -> jstring {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        let json = match result {
            Ok(Ok(Some(value))) => serde_json::to_string(&value).ok(),
            Ok(Ok(None)) => None,
            Ok(Err(e)) => {
                log(&format!("{what}: {e}"));
                None
            }
            Err(_) => {
                log(&format!("{what}: crashed"));
                None
            }
        };
        match json.and_then(|j| env.new_string(j).ok()) {
            Some(s) => s.into_raw(),
            None => std::ptr::null_mut(),
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_simon_melue_android_CarQueue_nativeQueueAtStart<'l>(
        mut env: JNIEnv<'l>,
        _class: JClass<'l>,
        data_dir: JString<'l>,
        password: JString<'l>,
    ) -> jstring {
        let (dir, password) = (text(&mut env, &data_dir).unwrap_or_default(), text(&mut env, &password));
        answer(&mut env, "queue at start", || {
            let queue = with_car(Path::new(&dir), |car| car.queue_at_start(password.as_deref()))?;
            log(&format!("queue at start: {}", if queue.is_some() { "another device's" } else { "the player's own" }));
            Ok(queue)
        })
    }

    #[no_mangle]
    pub extern "system" fn Java_com_simon_melue_android_CarQueue_nativeEntries<'l>(
        mut env: JNIEnv<'l>,
        _class: JClass<'l>,
        data_dir: JString<'l>,
        password: JString<'l>,
        folder_id: JString<'l>,
    ) -> jstring {
        let (dir, password, folder) = (text(&mut env, &data_dir).unwrap_or_default(), text(&mut env, &password), text(&mut env, &folder_id));
        answer(&mut env, "entries", || {
            with_car(Path::new(&dir), |car| car.entries(password.as_deref(), &folder.unwrap_or_default())).map(Some)
        })
    }

    #[no_mangle]
    pub extern "system" fn Java_com_simon_melue_android_CarQueue_nativeQueueForEntry<'l>(
        mut env: JNIEnv<'l>,
        _class: JClass<'l>,
        data_dir: JString<'l>,
        password: JString<'l>,
        entry_id: JString<'l>,
    ) -> jstring {
        let (dir, password, entry) = (text(&mut env, &data_dir).unwrap_or_default(), text(&mut env, &password), text(&mut env, &entry_id));
        answer(&mut env, "queue for entry", || {
            with_car(Path::new(&dir), |car| car.queue_for_entry(password.as_deref(), &entry.unwrap_or_default()))
        })
    }

    #[no_mangle]
    pub extern "system" fn Java_com_simon_melue_android_CarQueue_nativePlayback<'l>(
        mut env: JNIEnv<'l>,
        _class: JClass<'l>,
        data_dir: JString<'l>,
        password: JString<'l>,
        name: JString<'l>,
        playback: JString<'l>,
    ) -> jstring {
        let (dir, password) = (text(&mut env, &data_dir).unwrap_or_default(), text(&mut env, &password));
        let (name, playback) = (text(&mut env, &name).unwrap_or_default(), text(&mut env, &playback).unwrap_or_default());
        answer(&mut env, "playback", || {
            let playback: CarPlayback = serde_json::from_str(&playback).map_err(|e| e.to_string())?;
            let saved = with_car(Path::new(&dir), |car| car.playback(password.as_deref(), &name, &playback))?;
            if saved {
                log("playback saved to the share");
            }
            Ok(None::<()>)
        })
    }
}
