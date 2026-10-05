//! The synced folder, wherever it is: a folder on disk, or on Android a folder chosen with the picker. Paths
//! are relative to the folder, with '/', like the share's.

use tauri::AppHandle;

use crate::library::{Folder, FolderKind, Result};
use crate::share::folder::FolderShare;
use crate::share::{Entry, Share};

/// Every file in the synced folder (hidden ones left out).
pub fn local_files(app: &AppHandle, folder: &Folder) -> Result<Vec<Entry>> {
    Ok(target(app, folder)?.walk("")?)
}

/// The synced folder, to write into.
pub fn target(app: &AppHandle, folder: &Folder) -> Result<Box<dyn Share>> {
    match folder.kind {
        FolderKind::Path => {
            let _ = app;
            Ok(Box::new(FolderShare::new(&folder.location)))
        }
        #[cfg(target_os = "android")]
        FolderKind::AndroidTree => Ok(Box::new(super::tree::TreeShare::new(app, &folder.location))),
        #[cfg(not(target_os = "android"))]
        FolderKind::AndroidTree => Err("chosen Android folders are only for Android".into()),
        FolderKind::AndroidMedia => Err("all music on the phone isn't a synced folder".into()),
    }
}

/// Free space where the synced folder is, when it can be told.
pub fn free_space(folder: &Folder, target: &dyn Share) -> Option<u64> {
    match folder.kind {
        FolderKind::Path => fs4::available_space(&folder.location).ok(),
        _ => target.free_space().ok().flatten(),
    }
}
