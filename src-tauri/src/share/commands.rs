//! The share setup's commands for the UI. Network work runs on a thread meant for waiting (not one of
//! Tauri's async threads, see the playbook's lessons).

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::secrets::{FileSecrets, MemorySecrets, PasswordStore, SystemSecrets};
use super::setup::{self, PasswordStores, SetupError, ShareSettings, ShareSetup, SyncFolderChoice};
use crate::library::changes::{self, tracking, ChangeReason, LibraryChange};
use crate::library::{scanner, Folder, FolderKind, Library};

/// Passwords asked for since Melue started ("ask each time").
#[derive(Default)]
pub struct SessionPasswords(MemorySecrets);

/// Runs `work` on a thread meant for waiting.
pub(crate) async fn on_share_thread<T: Send + 'static, E: Send + 'static + From<String>>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|e| E::from(e.to_string()))?
}

/// The password stores, for `work`.
pub(crate) fn with_stores<T>(app: &AppHandle, work: impl FnOnce(&Library, &PasswordStores) -> T) -> Result<T, SetupError> {
    let secure = SystemSecrets::new(app);
    let file = FileSecrets::new(app.path().app_data_dir().map_err(|e| e.to_string())?.join("secrets.json"));
    let session = app.state::<SessionPasswords>();
    let stores = PasswordStores { secure: &secure, memory: &session.0, file: &file };
    Ok(work(&app.state::<Library>(), &stores))
}

/// The share as set up, for the UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInfo {
    /// None: no share set up.
    pub settings: Option<ShareSettings>,
    /// The synced library folder.
    pub sync_folder: Option<Folder>,
    /// The password has to be given before the share can be used ("ask each time", after a start).
    pub needs_password: bool,
    /// Desktop: where the synced music goes unless another folder is chosen (Music/Melue).
    pub default_sync_folder: Option<String>,
}

fn info(app: &AppHandle) -> Result<ShareInfo, SetupError> {
    with_stores(app, |library, stores| -> Result<ShareInfo, SetupError> {
        let settings = library.share_settings()?;
        let sync_folder = match &settings {
            Some(s) => library.folder(s.sync_folder_id)?,
            None => None,
        };
        Ok(ShareInfo { settings, sync_folder, needs_password: setup::needs_password(library, stores)?, default_sync_folder: default_folder(app) })
    })?
}

/// Desktop: Music/Melue (not created until it's used).
fn default_folder(app: &AppHandle) -> Option<String> {
    if cfg!(target_os = "android") {
        return None;
    }
    app.path().audio_dir().ok().map(|music| music.join("Melue").to_string_lossy().into_owned())
}

/// Tells the UI (and Android Auto, and the queue) that the share or its folder changed, and scans the folder.
fn changed(app: &AppHandle) {
    changes::emit(app, LibraryChange::sync());
    scanner::scan_soon(app);
}

#[tauri::command]
pub async fn share_info(app: AppHandle) -> Result<ShareInfo, SetupError> {
    on_share_thread(move || info(&app)).await
}

/// Tries a share without setting it up (the "Test connection" button).
#[tauri::command]
pub async fn share_test(setup: ShareSetup, password: Option<String>) -> Result<(), SetupError> {
    on_share_thread(move || setup::test(&setup::open, &setup, password.as_deref())).await
}

/// Sets up the share. `sync_folder`: where the synced music goes (desktop: Music/Melue when left out; not
/// needed for a drive played straight from). Android: a folder chosen with the picker (its access is kept).
#[tauri::command]
pub async fn share_set_up(
    app: AppHandle,
    setup: ShareSetup,
    password: Option<String>,
    sync_folder: Option<SyncFolderChoice>,
) -> Result<ShareInfo, SetupError> {
    on_share_thread(move || {
        let streamed = matches!(setup, ShareSetup::Drive { stream: true, .. });
        let sync_folder = match sync_folder {
            Some(choice) => Some(choice),
            None if streamed => None,
            None => {
                let music = app.path().audio_dir().map_err(|e| e.to_string())?;
                let location = setup::default_sync_folder(&music)?;
                Some(SyncFolderChoice { kind: FolderKind::Path, location })
            }
        };
        if let Some(SyncFolderChoice { kind: FolderKind::AndroidTree, location }) = &sync_folder {
            crate::library::android::keep_folder_access(&app, location)?;
        }
        tracking(&app, ChangeReason::Folders, true, || {
            with_stores(&app, |library, stores| setup::set_up(library, stores, &setup::open, setup, password.as_deref(), sync_folder))
                .and_then(|r| r)
                .map_err(|e| e.to_string())
        })
        .map_err(|e| SetupError::Other { detail: e })?;
        changed(&app);
        info(&app)
    })
    .await
}

/// A new password (changed, or "ask each time" after a start), kept in `store` (or where it was kept).
#[tauri::command]
pub async fn share_set_password(app: AppHandle, password: String, store: Option<PasswordStore>) -> Result<ShareInfo, SetupError> {
    on_share_thread(move || {
        with_stores(&app, |library, stores| setup::set_password(library, stores, &setup::open, &password, store))??;
        changed(&app);
        info(&app)
    })
    .await
}

/// Removes the share (the synced folder stays as a normal folder, with its songs).
#[tauri::command]
pub async fn share_remove(app: AppHandle) -> Result<ShareInfo, SetupError> {
    on_share_thread(move || {
        with_stores(&app, setup::remove)??;
        crate::sync::scheduler::stop_watching(&app);
        changes::emit(&app, LibraryChange::sync());
        info(&app)
    })
    .await
}

/// Android: shows the synced music in other apps, or hides it from them (a `.nomedia` file).
#[tauri::command]
pub async fn share_set_shown_in_other_apps(app: AppHandle, shown: bool) -> Result<(), SetupError> {
    on_share_thread(move || {
        let folder = info(&app)?.sync_folder.ok_or(SetupError::NotSetUp)?;
        if folder.kind != FolderKind::AndroidTree {
            return Err("only for a synced folder on Android".to_string().into());
        }
        crate::library::android::set_hidden_from_other_apps(&app, &folder.location, !shown)?;
        Ok(())
    })
    .await
}

/// The share's home address (an address or a name; empty: none): tried first at home.
#[tauri::command]
pub async fn share_set_home_host(app: AppHandle, home_host: Option<String>) -> Result<ShareInfo, SetupError> {
    on_share_thread(move || {
        setup::set_home_host(&app.state::<Library>(), home_host.as_deref())?;
        // Opened again with it.
        crate::sync::shared::reset(&app);
        info(&app)
    })
    .await
}
