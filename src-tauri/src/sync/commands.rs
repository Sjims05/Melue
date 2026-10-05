//! The sync's commands for the UI. Share work runs on a thread meant for waiting.

use tauri::{AppHandle, Manager};

use super::plan::{make_plan, SyncPlan};
use super::{local, SyncSettings};
use crate::library::Library;
use crate::share::commands::{on_share_thread, with_stores};
use crate::share::setup::{self, SetupError, ShareSetup};

/// What a sync would do now (the dry run): nothing is changed.
#[tauri::command]
pub async fn sync_plan(app: AppHandle) -> Result<SyncPlan, SetupError> {
    on_share_thread(move || {
        let (settings, share) = with_stores(&app, |library, stores| setup::current(library, stores, &setup::open))??;
        if matches!(settings.setup, ShareSetup::Drive { stream: true, .. }) {
            return Err("played straight from the share: there's nothing to copy".to_string().into());
        }
        let library = app.state::<Library>();
        let folder = library.folder(settings.sync_folder_id)?.ok_or_else(|| "the synced folder isn't in the library any more".to_string())?;
        let on_share = share.walk("")?;
        let here = local::local_files(&app, &folder)?;
        Ok(make_plan(on_share, here, library.sync_records()?, &library.sync_settings()?))
    })
    .await
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
