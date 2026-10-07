//! Running syncs: the Sync now button, and by themselves when schedule.rs's rules say so (start-up, coming
//! back to the app, the share's change notifications, an optional periodic check). One sync at a time; its
//! progress and the last sync's result are the sync status, sent to the UI as `sync-status` events and kept
//! across restarts. On Android a running sync shows a "Syncing music" notification with a progress bar
//! (which also keeps it going in the background), and on a metered connection song copies wait for Wi-Fi.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::handoff;
use super::local;
use super::plan::{make_plan, SyncPlan};
use super::run::{run, Progress, RunOptions, SyncError, SyncResult};
use super::schedule::{retry_after_ms, should_sync, Trigger, NETWORK_SETTLE_MS, SETTLE_MS};
use crate::library::{err, now_ms, scanner, Folder, Library, Result};
use crate::share::commands::with_stores;
use crate::share::setup::{self, SetupError, ShareSetup};
use crate::share::{Share, ShareError, Watch};

pub const STATUS_EVENT: &str = "sync-status";
const LAST_SYNC: &str = "sync_last";
/// How often the scheduler looks at the periodic check.
const TICK: Duration = Duration::from_secs(60);

/// Why a sync didn't happen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SyncFailure {
    /// The share couldn't be used (see SetupError: unreachable, password needed...).
    Setup { error: SetupError },
    /// Not enough free space (bytes).
    NotEnoughSpace { needed: u64, free: u64 },
    /// A sync is running already.
    Running,
    Other { detail: String },
}

impl From<String> for SyncFailure {
    fn from(detail: String) -> Self {
        Self::Other { detail }
    }
}

/// The last sync: when, why, and what it did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastSync {
    pub at_ms: i64,
    pub trigger: Trigger,
    pub result: SyncResult,
}

/// The sync status, for the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// A sync is running: how far it is.
    pub syncing: Option<Progress>,
    pub last: Option<LastSync>,
    /// Why the last try didn't sync (cleared by the next sync).
    pub problem: Option<SyncFailure>,
    /// The share couldn't be reached: when it's tried again (and sooner if the network changes).
    pub retry_at_ms: Option<i64>,
    /// Something about the connection (the home address didn't answer at home, so the main one was used).
    pub note: Option<String>,
}

/// The running sync's state, and the scheduler's.
#[derive(Default)]
pub struct Syncing {
    running: AtomicBool,
    cancel: AtomicBool,
    status: Mutex<SyncStatus>,
    triggers: Mutex<Option<Sender<Trigger>>>,
    /// Watching the share for changes (with the share it runs on).
    watch: Mutex<Option<(Box<dyn Share>, Watch)>>,
    /// The share couldn't be reached: tries so far, and when the next one is due.
    retry: Mutex<Option<(u32, Instant)>>,
}

impl SyncFailure {
    /// The share couldn't be reached (off, asleep, no network, the VPN not there yet): worth trying again.
    pub fn unreachable(&self) -> bool {
        matches!(self, Self::Setup { error: SetupError::Share { error: ShareError::Unreachable { .. } } })
    }
}

impl Library {
    pub fn last_sync(&self) -> Result<Option<LastSync>> {
        Ok(self.setting(LAST_SYNC)?.and_then(|json| serde_json::from_str(&json).ok()))
    }

    fn save_last_sync(&self, last: &LastSync) -> Result<()> {
        self.set_setting(LAST_SYNC, &serde_json::to_string(last).map_err(err)?)
    }
}

/// The share, the synced folder, and the plan for them.
pub fn plan_now(app: &AppHandle) -> std::result::Result<(Box<dyn Share>, Folder, SyncPlan), SetupError> {
    let (settings, share) = with_stores(app, |library, stores| setup::current(library, stores, &setup::open))??;
    if matches!(settings.setup, ShareSetup::Drive { stream: true, .. }) {
        return Err("played straight from the share: there's nothing to copy".to_string().into());
    }
    let library = app.state::<Library>();
    let folder =
        library.folder(settings.sync_folder_id)?.ok_or_else(|| "the synced folder isn't in the library any more".to_string())?;
    let on_share = share.walk("")?;
    let here = local::local_files(app, &folder)?;
    let plan = make_plan(on_share, here, library.sync_records()?, &library.sync_settings()?);
    Ok((share, folder, plan))
}

/// The sync status now.
pub fn status(app: &AppHandle) -> SyncStatus {
    let syncing = app.state::<Syncing>();
    let mut status = syncing.status.lock().unwrap().clone();
    if status.last.is_none() {
        status.last = app.state::<Library>().last_sync().ok().flatten();
    }
    status
}

fn set_status(app: &AppHandle, change: impl FnOnce(&mut SyncStatus)) {
    let syncing = app.state::<Syncing>();
    let mut status = syncing.status.lock().unwrap();
    change(&mut status);
    let _ = app.emit(STATUS_EVENT, &*status);
}

/// Whether the connection is metered (mobile data, a phone's hotspot). Desktop: no.
fn metered(app: &AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        app.melue_android().metered().unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        false
    }
}

/// Android's "Syncing music" notification: shown with progress while a sync copies, gone at the end.
fn notify(app: &AppHandle, progress: Option<&Progress>) {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        let android = app.melue_android();
        let _ = match progress {
            Some(p) => android.sync_notification(p.files_done.min(p.files_total), p.files_total, p.current.as_deref().unwrap_or("")),
            None => android.sync_notification_done(),
        };
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, progress);
}

/// Syncs once (one at a time): plans, copies (on a metered connection copies wait for Wi-Fi), keeps the
/// status, and then lets the library scan the synced folder.
pub fn sync_once(app: &AppHandle, trigger: Trigger, confirmed_removal: bool) -> std::result::Result<SyncResult, SyncFailure> {
    let syncing = app.state::<Syncing>();
    if syncing.running.swap(true, Ordering::SeqCst) {
        return Err(SyncFailure::Running);
    }
    syncing.cancel.store(false, Ordering::SeqCst);
    set_status(app, |s| s.syncing = Some(Progress::default()));
    let result: std::result::Result<(SyncResult, Box<dyn Share>), SyncFailure> = (|| {
        let (share, folder, plan) = plan_now(app).map_err(|error| SyncFailure::Setup { error })?;
        let target = local::target(app, &folder)?;
        let library = app.state::<Library>();
        let settings = library.sync_settings()?;
        let options = RunOptions {
            free_space: local::free_space(&folder, target.as_ref()),
            confirmed_removal,
            no_copies: metered(app) && !settings.copy_on_metered,
        };
        let mut last = Instant::now() - Duration::from_secs(1);
        let mut current = None;
        let mut report = |progress: &Progress| {
            // Every file, and at most a few times a second within one.
            if progress.current != current || last.elapsed() >= Duration::from_millis(250) {
                current = progress.current.clone();
                last = Instant::now();
                if progress.files_total > 0 && progress.current.is_some() {
                    notify(app, Some(progress));
                }
                set_status(app, |s| s.syncing = Some(progress.clone()));
            }
        };
        let done = run(&library, share.as_ref(), target.as_ref(), &plan, options, &syncing.cancel, &mut report).map_err(|e| match e {
            SyncError::NotEnoughSpace { needed, free } => SyncFailure::NotEnoughSpace { needed, free },
            SyncError::Other { detail } => SyncFailure::Other { detail },
        })?;
        Ok((done, share))
    })();
    notify(app, None);
    syncing.running.store(false, Ordering::SeqCst);
    let result = match result {
        Ok((done, share)) => {
            let last = LastSync { at_ms: now_ms(), trigger, result: done.clone() };
            let _ = app.state::<Library>().save_last_sync(&last);
            *syncing.retry.lock().unwrap() = None;
            let note = share.note();
            set_status(app, |s| {
                s.syncing = None;
                s.last = Some(last);
                s.problem = None;
                s.retry_at_ms = None;
                s.note = note;
            });
            // The share's layout, as the share says (another device may have changed it).
            if let Err(e) = super::layout::agree(&app.state::<Library>(), share.as_ref()) {
                eprintln!("melue: can't read the share's layout: {e}");
            }
            keep_watching(app, share);
            super::data::soon(app);
            Ok(done)
        }
        Err(failure) => {
            // Couldn't reach the share: try again in a while (a sync already running isn't a failure).
            let retry_at_ms = match &failure {
                SyncFailure::Running => status(app).retry_at_ms,
                f if f.unreachable() => {
                    let mut retry = syncing.retry.lock().unwrap();
                    let attempts = retry.map_or(0, |(n, _)| n) + 1;
                    let wait = retry_after_ms(attempts);
                    *retry = Some((attempts, Instant::now() + Duration::from_millis(wait as u64)));
                    Some(now_ms() + wait)
                }
                _ => {
                    *syncing.retry.lock().unwrap() = None;
                    None
                }
            };
            set_status(app, |s| {
                s.syncing = None;
                s.problem = Some(failure.clone());
                s.retry_at_ms = retry_at_ms;
            });
            Err(failure)
        }
    };
    // The library picks the songs up (and tells the UI what changed).
    scanner::scan_soon(app);
    result
}

/// Runs `work` as the one sync of the moment (moving songs to sync), its progress shown like a sync's (the
/// status, and Android's notification). Cancel stops it after the file it's on (see [`cancelled`]).
pub fn exclusive<T>(
    app: &AppHandle,
    work: impl FnOnce(&mut dyn FnMut(&Progress)) -> std::result::Result<T, SyncFailure>,
) -> std::result::Result<T, SyncFailure> {
    let syncing = app.state::<Syncing>();
    if syncing.running.swap(true, Ordering::SeqCst) {
        return Err(SyncFailure::Running);
    }
    syncing.cancel.store(false, Ordering::SeqCst);
    set_status(app, |s| s.syncing = Some(Progress::default()));
    let mut report = |progress: &Progress| {
        notify(app, Some(progress));
        set_status(app, |s| s.syncing = Some(progress.clone()));
    };
    let result = work(&mut report);
    notify(app, None);
    syncing.running.store(false, Ordering::SeqCst);
    set_status(app, |s| s.syncing = None);
    result
}

/// Whether Cancel was pressed during [`exclusive`] work.
pub fn cancelled(app: &AppHandle) -> bool {
    app.state::<Syncing>().cancel.load(Ordering::SeqCst)
}

/// Asks the share to say when its music changes (when that's on and it isn't being watched already). A
/// change in Melue's playback files is for synced playback (handoff.rs), not a sync.
fn keep_watching(app: &AppHandle, share: Box<dyn Share>) {
    let on_change = app.state::<Library>().sync_settings().map(|s| s.on_change).unwrap_or(false);
    let syncing = app.state::<Syncing>();
    let mut watching = syncing.watch.lock().unwrap();
    if !on_change {
        *watching = None;
        return;
    }
    if watching.as_ref().is_some_and(|(_, watch)| watch.alive()) {
        return;
    }
    let handle = app.clone();
    let on_change = move |path: &str| {
        if handoff::is_playback_file(path) {
            handoff::check(&handle);
        } else if super::data::is_library_file(path) {
            super::data::soon(&handle);
        } else if !path.split('/').any(crate::share::is_hidden) {
            trigger(&handle, Trigger::Changed);
        }
    };
    match share.watch(Arc::new(on_change)) {
        Ok(Some(watch)) => *watching = Some((share, watch)),
        _ => *watching = None,
    }
}

/// Stops watching the share (it was removed, or the setting switched off).
pub fn stop_watching(app: &AppHandle) {
    *app.state::<Syncing>().watch.lock().unwrap() = None;
}

/// The network changed (Android): tries the share again now if it couldn't be reached, and lets synced
/// playback open the share again.
pub fn network_changed(app: &AppHandle) {
    trigger(app, Trigger::Network);
    handoff::network_changed(app);
    super::data::soon(app);
}

/// Asks for a sync (the scheduler decides whether one runs, see schedule.rs).
pub fn trigger(app: &AppHandle, trigger: Trigger) {
    if let Some(triggers) = app.state::<Syncing>().triggers.lock().unwrap().as_ref() {
        let _ = triggers.send(trigger);
    }
}

/// Spike: shows the "Syncing music" notification for a slow pretend sync of 20 files (a second each), to look at
/// it (a real sync is often too quick). Android only.
pub fn simulate_notification(app: &AppHandle) {
    let app = app.clone();
    app.state::<Syncing>().cancel.store(false, Ordering::SeqCst);
    std::thread::spawn(move || {
        for done in 0..20 {
            // Cancel on the notification stops it, like a real sync.
            if app.state::<Syncing>().cancel.load(Ordering::SeqCst) {
                break;
            }
            let progress = Progress { files_done: done, files_total: 20, current: Some(format!("Pretend Album/{:02} Pretend song.mp3", done + 1)), ..Default::default() };
            notify(&app, Some(&progress));
            std::thread::sleep(Duration::from_secs(1));
        }
        notify(&app, None);
    });
}

/// Stops a running sync after the file it's copying.
pub fn cancel(app: &AppHandle) {
    app.state::<Syncing>().cancel.store(true, Ordering::SeqCst);
}

/// Starts the scheduler: a sync shortly after start-up (when that's on), then on the triggers it's sent.
pub fn start(app: &AppHandle) {
    let (sender, triggers) = mpsc::channel::<Trigger>();
    *app.state::<Syncing>().triggers.lock().unwrap() = Some(sender.clone());
    let app = app.clone();
    std::thread::Builder::new()
        .name("melue-sync".into())
        .spawn(move || {
            // After the library's start-up scan has had a moment.
            std::thread::sleep(Duration::from_secs(3));
            let _ = sender.send(Trigger::Start);
            loop {
                // Wakes up for the periodic check, or sooner when a retry is due.
                let retry_due = *app.state::<Syncing>().retry.lock().unwrap();
                let wait = retry_due.map_or(TICK, |(_, due)| due.saturating_duration_since(Instant::now()).min(TICK));
                let mut next = match triggers.recv_timeout(wait) {
                    Ok(trigger) => trigger,
                    Err(RecvTimeoutError::Timeout) if retry_due.is_some_and(|(_, due)| due <= Instant::now()) => Trigger::Retry,
                    Err(RecvTimeoutError::Timeout) => Trigger::Periodic,
                    Err(RecvTimeoutError::Disconnected) => return,
                };
                // Changes come in bursts (a copy of an album; Wi-Fi, then the VPN): wait until things are quiet.
                if matches!(next, Trigger::Changed | Trigger::Network) {
                    let settle = if next == Trigger::Changed { SETTLE_MS } else { NETWORK_SETTLE_MS };
                    while let Ok(more) = triggers.recv_timeout(Duration::from_millis(settle as u64)) {
                        if more != next {
                            next = more;
                            break;
                        }
                    }
                }
                // A network change only matters while the share couldn't be reached.
                if next == Trigger::Network && app.state::<Syncing>().retry.lock().unwrap().is_none() {
                    continue;
                }
                let library = app.state::<Library>();
                let Ok(settings) = library.sync_settings() else { continue };
                if !library.sync_available().unwrap_or(false) {
                    continue;
                }
                let last_ms = library.last_sync().ok().flatten().map(|l| l.at_ms);
                if should_sync(next, now_ms(), last_ms, &settings) {
                    eprintln!("melue: sync ({next:?}): starting");
                    match sync_once(&app, next, false) {
                        Ok(r) => eprintln!("melue: sync ({next:?}): {} copied, {} moved, {} waiting for Wi-Fi", r.copied, r.moved, r.waiting_for_wifi),
                        Err(e) => eprintln!("melue: sync ({next:?}): didn't sync: {e:?}"),
                    }
                } else if !matches!(next, Trigger::Periodic | Trigger::Network) {
                    eprintln!("melue: sync ({next:?}): not now (the settings, or the last sync is recent)");
                }
            }
        })
        .expect("the sync scheduler thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_unreachable_share_is_tried_again() {
        let share = |error| SyncFailure::Setup { error: SetupError::Share { error } };
        assert!(share(ShareError::Unreachable { detail: "timed out".into() }).unreachable());
        // A wrong password or a full disk doesn't fix itself by waiting.
        assert!(!share(ShareError::Login { detail: "no".into() }).unreachable());
        assert!(!SyncFailure::NotEnoughSpace { needed: 2, free: 1 }.unreachable());
        assert!(!SyncFailure::Running.unreachable());
    }
}
