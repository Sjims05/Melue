//! The sync's commands for the UI. Share work runs on a thread meant for waiting.

use tauri::{AppHandle, Manager};

use super::plan::SyncPlan;
use super::run::SyncResult;
use super::schedule::Trigger;
use super::scheduler::{self, SyncFailure, SyncStatus};
use super::handoff::{self, PlaybackFile, PlaybackSettings};
use super::playlist_sync::{self, Choice, DeletedPlaylist, PlaylistConflict};
use crate::library::changes::{self, LibraryChange};
use super::moving::{self, Copies, CopyCheck, LocalSong, MoveResult, ToMove};
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
    super::moving::moving_new_songs(&library, settings.move_new_songs)?;
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

/// Spike: syncs the library data (likes, plays, playlists) with the share now.
#[tauri::command]
pub async fn library_sync_now(app: AppHandle) -> Result<(), String> {
    on_share_thread(move || super::data::run(&app)).await
}

/// Playlists changed on two devices in ways that don't merge, waiting for a choice.
#[tauri::command(async)]
pub fn playlist_conflicts(library: tauri::State<'_, Library>) -> Result<Vec<PlaylistConflict>, String> {
    library.playlist_conflicts()
}

/// A choice for a conflict: "mine", "theirs" or "both" (see playlist_sync.rs).
#[tauri::command]
pub async fn playlist_answer_conflict(app: AppHandle, sync_id: String, choice: Choice) -> Result<(), String> {
    on_share_thread(move || playlist_sync::answer(&app, &sync_id, choice)).await
}

/// Synced playlists deleted in the last 30 days, which can be restored.
#[tauri::command(async)]
pub fn deleted_playlists(library: tauri::State<'_, Library>) -> Result<Vec<DeletedPlaylist>, String> {
    library.deleted_playlists()
}

/// Restores a deleted playlist (everywhere, at the next sync). Returns its id.
#[tauri::command(async)]
pub fn restore_playlist(app: AppHandle, library: tauri::State<'_, Library>, sync_id: String) -> Result<i64, String> {
    let id = library.restore_playlist(&sync_id)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    Ok(id)
}

/// "Only on this device": the playlist isn't synced (the other devices keep theirs), or is again.
#[tauri::command(async)]
pub fn set_playlist_local_only(app: AppHandle, library: tauri::State<'_, Library>, id: i64, local_only: bool) -> Result<(), String> {
    library.set_playlist_local_only(id, local_only)?;
    changes::emit(&app, LibraryChange::playlist(id, Vec::new()));
    Ok(())
}

/// Local songs that aren't on the share (the "Not on your share" list).
#[tauri::command]
pub async fn sync_not_on_share(app: AppHandle) -> Result<Vec<LocalSong>, String> {
    on_share_thread(move || {
        let library = app.state::<Library>();
        let folder = library.synced_folder()?.ok_or("no share is set up")?;
        let songs = library.songs_here(&folder, &handoff::device_id(&library)?)?;
        library.not_on_share(&folder, &songs)
    })
    .await
}

/// Moves songs (or copies files) to sync: to the share and into the synced folder; a song's original goes
/// once its synced copy is in the library.
#[tauri::command]
pub async fn sync_move_to_sync(app: AppHandle, items: Vec<ToMove>) -> Result<MoveResult, SyncFailure> {
    on_share_thread(move || moving::move_to_sync(&app, items)).await
}

/// Hidden copies of synced songs waiting for a choice (also sent as `song-copies` events).
#[tauri::command(async)]
pub fn sync_copies(library: tauri::State<'_, Library>) -> Result<Copies, String> {
    library.copies_waiting()
}

/// Clear space: removes the hidden copies (Recycle Bin; Android's trash, after Android asks).
#[tauri::command]
pub async fn sync_clear_copies(app: AppHandle) -> Result<usize, String> {
    on_share_thread(move || moving::clear_copies(&app)).await
}

/// Keeps the hidden copies (not asked about again).
#[tauri::command(async)]
pub fn sync_keep_copies(app: AppHandle, library: tauri::State<'_, Library>) -> Result<(), String> {
    library.keep_copies()?;
    moving::emit_copies(&app);
    Ok(())
}

/// Spike: looks for copies of synced songs now, and says what it found (near misses too).
#[tauri::command]
pub async fn sync_check_copies(app: AppHandle) -> Result<CopyCheck, String> {
    on_share_thread(move || moving::check_copies(&app)).await
}

/// The share's layout, and how many songs aren't in it.
#[tauri::command]
pub async fn sync_layout_status(app: AppHandle) -> Result<super::layout::LayoutStatus, String> {
    on_share_thread(move || super::layout::status(&app)).await
}

/// Reorganise now: puts the share in the chosen layout (songs, playlists; the copies here move too).
#[tauri::command]
pub async fn sync_reorganise(app: AppHandle) -> Result<super::layout::ReorganiseResult, SyncFailure> {
    on_share_thread(move || super::layout::reorganise(&app, false)).await
}
