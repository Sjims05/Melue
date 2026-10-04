//! The library's commands for the UI. They run off the main thread (`async`), so a slow query never
//! freezes the window.

use serde::Serialize;
use tauri::State;

use super::{Library, Result};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub schema_version: i64,
}

#[tauri::command(async)]
pub fn library_status(library: State<'_, Library>) -> Result<LibraryStatus> {
    Ok(LibraryStatus { schema_version: library.schema_version()? })
}

/// An app setting saved in the library (null when it was never set).
#[tauri::command(async)]
pub fn library_setting(library: State<'_, Library>, key: String) -> Result<Option<String>> {
    library.setting(&key)
}

#[tauri::command(async)]
pub fn library_set_setting(library: State<'_, Library>, key: String, value: String) -> Result<()> {
    library.set_setting(&key, &value)
}
