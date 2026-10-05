//! The sync's commands for the UI. Share work runs on a thread meant for waiting.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::plan::{make_plan, SyncPlan};
use super::run::{run, Progress, SyncError, SyncResult};
use super::{local, SyncSettings};
use crate::library::{scanner, Folder, Library};
use crate::share::commands::{on_share_thread, with_stores};
use crate::share::setup::{self, SetupError, ShareSetup};
use crate::share::Share;

/// The progress of a running sync, for the UI.
pub const SYNC_EVENT: &str = "sync-progress";

/// One sync at a time; Cancel stops the running one.
#[derive(Default)]
pub struct SyncState {
    running: AtomicBool,
    cancel: AtomicBool,
}

/// The share, the synced folder, and the plan for them.
fn plan_now(app: &AppHandle) -> Result<(Box<dyn Share>, Folder, SyncPlan), SetupError> {
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

/// What a sync would do now (the dry run): nothing is changed.
#[tauri::command]
pub async fn sync_plan(app: AppHandle) -> Result<SyncPlan, SetupError> {
    on_share_thread(move || plan_now(&app).map(|(_, _, plan)| plan)).await
}

/// Why Sync now didn't sync.
#[derive(Debug, Serialize)]
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

/// Syncs now: carries out the plan, reporting progress as `sync-progress` events; then the library scans the
/// synced folder. `confirmed_removal`: yes to removing more songs gone from the share than the setting allows.
#[tauri::command]
pub async fn sync_now(app: AppHandle, confirmed_removal: Option<bool>) -> Result<SyncResult, SyncFailure> {
    on_share_thread(move || {
        let state = app.state::<SyncState>();
        if state.running.swap(true, Ordering::SeqCst) {
            return Err(SyncFailure::Running);
        }
        state.cancel.store(false, Ordering::SeqCst);
        let result = (|| {
            let (share, folder, plan) = plan_now(&app).map_err(|error| SyncFailure::Setup { error })?;
            let target = local::target(&app, &folder)?;
            let free = local::free_space(&folder, target.as_ref());
            let library = app.state::<Library>();
            let mut last = Instant::now() - Duration::from_secs(1);
            let mut current = None;
            let mut report = |progress: &Progress| {
                // Every file, and at most a few times a second within one.
                if progress.current != current || last.elapsed() >= Duration::from_millis(250) {
                    current = progress.current.clone();
                    last = Instant::now();
                    let _ = app.emit(SYNC_EVENT, progress);
                }
            };
            let confirmed = confirmed_removal.unwrap_or(false);
            run(&library, share.as_ref(), target.as_ref(), &plan, free, confirmed, &state.cancel, &mut report).map_err(|e| match e {
                SyncError::NotEnoughSpace { needed, free } => SyncFailure::NotEnoughSpace { needed, free },
                SyncError::Other { detail } => SyncFailure::Other { detail },
            })
        })();
        state.running.store(false, Ordering::SeqCst);
        // The library picks the songs up (and tells the UI what changed).
        scanner::scan_soon(&app);
        result
    })
    .await
}

/// Stops a running sync after the file it's copying (what's done stays done).
#[tauri::command]
pub fn sync_cancel(state: tauri::State<'_, SyncState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command(async)]
pub fn sync_settings(library: tauri::State<'_, Library>) -> Result<SyncSettings, String> {
    library.sync_settings()
}

#[tauri::command(async)]
pub fn sync_set_settings(library: tauri::State<'_, Library>, settings: SyncSettings) -> Result<SyncSettings, String> {
    library.set_sync_settings(&settings)?;
    library.sync_settings()
}
