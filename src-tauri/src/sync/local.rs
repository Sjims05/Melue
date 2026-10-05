//! The synced folder's files, wherever it is: a folder on disk, or on Android a folder chosen with the picker.
//! Paths are relative to the folder, with '/', like the share's.

use tauri::AppHandle;

use crate::library::{Folder, FolderKind, Result};
use crate::share::folder::FolderShare;
use crate::share::{Entry, Share};

/// Every file in the synced folder (hidden ones left out).
pub fn local_files(app: &AppHandle, folder: &Folder) -> Result<Vec<Entry>> {
    match folder.kind {
        FolderKind::Path => Ok(FolderShare::new(&folder.location).walk("")?),
        FolderKind::AndroidTree => crate::library::android::walk_tree(app, &folder.location),
        FolderKind::AndroidMedia => Err("all music on the phone isn't a synced folder".into()),
    }
}
