//! The sync's commands for the UI. Share work runs on a thread meant for waiting.

use tauri::AppHandle;

use super::plan::SyncPlan;
use super::run::SyncResult;
use super::schedule::Trigger;
use super::scheduler::{self, SyncFailure, SyncStatus};
use super::handoff::{self, PlaybackFile, PlaybackSettings};
use super::SyncSettings;
use crate::library::Library;
use crate::share::commands::on_share_thread;
use crate::share::setup::SetupError;

/// What a sync would do now (the dry run): nothing is changed.
#[tauri::command]
pub async fn sync_plan(app: AppHandle) -> Result<SyncPlan, SetupError> {
    on_share_thread(move || scheduler::plan_now(&app).map(|(_, _, plan)| plan)).await
}

/// Syncs now (the progress comes as `sync-status` events); then the library scans the synced folder.
/// `confirmed_removal`: yes to removing more songs gone from the share than the setting allows.
#[tauri::command]
pub async fn sync_now(app: AppHandle, confirmed_removal: Option<bool>) -> Result<SyncResult, SyncFailure> {
    on_share_thread(move || scheduler::sync_once(&app, Trigger::Button, confirmed_removal.unwrap_or(false))).await
}

/// Stops a running sync after the file it's copying (what's done stays done).
#[tauri::command]
pub fn sync_cancel(app: AppHandle) {
    scheduler::cancel(&app);
}

/// Spike: shows the "Syncing music" notification for a slow pretend sync (Android).
#[tauri::command]
pub fn sync_simulate_notification(app: AppHandle) -> Result<(), String> {
    if !cfg!(target_os = "android") {
        return Err("the sync notification is only on Android".into());
    }
    scheduler::simulate_notification(&app);
    Ok(())
}

/// Whether a sync is running (and how far), the last sync, and the last problem.
#[tauri::command]
pub fn sync_status(app: AppHandle) -> SyncStatus {
    scheduler::status(&app)
}

#[tauri::command(async)]
pub fn sync_settings(library: tauri::State<'_, Library>) -> Result<SyncSettings, String> {
    library.sync_settings()
}

#[tauri::command(async)]
pub fn sync_set_settings(app: AppHandle, library: tauri::State<'_, Library>, settings: SyncSettings) -> Result<SyncSettings, String> {
    library.set_sync_settings(&settings)?;
    if !settings.on_change {
        scheduler::stop_watching(&app);
    }
    library.sync_settings()
}

/// Synced playback's settings: ask, follow or off, and this device's name.
#[tauri::command(async)]
pub fn playback_sync_settings(app: AppHandle) -> Result<PlaybackSettings, String> {
    handoff::settings(&app)
}

#[tauri::command(async)]
pub fn playback_sync_set_settings(app: AppHandle, settings: PlaybackSettings) -> Result<PlaybackSettings, String> {
    handoff::set_settings(&app, &settings)?;
    handoff::settings(&app)
}

/// Another device's playback waiting for an answer (also sent as `playback-offer` events), if any.
#[tauri::command]
pub fn playback_offer(app: AppHandle) -> Option<PlaybackFile> {
    handoff::offer(&app)
}

/// Continue where the other device left off: its queue, loaded paused at its position.
#[tauri::command]
pub async fn playback_continue(app: AppHandle) -> Result<(), String> {
    on_share_thread(move || handoff::answer(&app, true)).await
}

/// Not now: the offer goes (until that device plays something else).
#[tauri::command]
pub async fn playback_not_now(app: AppHandle) -> Result<(), String> {
    on_share_thread(move || handoff::answer(&app, false)).await
}

/// Spike: looks at the other devices' playback now.
#[tauri::command]
pub fn playback_check(app: AppHandle) {
    handoff::check(&app);
}
