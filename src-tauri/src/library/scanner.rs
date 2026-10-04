//! Running scans: every switched-on folder, one scan at a time, at start-up, when asked (the refresh
//! button), and on Android when the music index changes or the app comes back to the foreground.
//! A scan lists every folder first (songs show up with their file names), then reads the tags and covers
//! of new and changed songs (tags.rs), then groups them into albums and artists (grouping.rs). Progress
//! goes to the UI as `library-scan` events, and what it changed as one `library-changed` event (changes.rs).

use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::changes::{tracking, ChangeReason};
use super::scan::{self, ScanSummary};
use super::tags::{self, TagJob};
use super::{android, FolderKind, Library, Result};

pub const SCAN_EVENT: &str = "library-scan";

/// How long things have to be quiet after a change (Android sends one per song while copying) before
/// the scan starts.
const SETTLE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum ScanEvent {
    Started,
    /// One folder done: what changed, or why it couldn't be scanned (its songs stay as they were).
    #[serde(rename_all = "camelCase")]
    Folder { folder_id: i64, name: String, summary: Option<ScanSummary>, error: Option<String> },
    /// Reading tags: `done` of `total` songs.
    Tags { done: u32, total: u32 },
    /// `changed`: whether anything in the library changed (the UI only needs to reload then).
    #[serde(rename_all = "camelCase")]
    Finished { summary: ScanSummary, changed: bool },
}

#[derive(Default)]
pub struct Scanner {
    running: AtomicBool,
    /// Asked for while running: go once more at the end.
    again: AtomicBool,
    /// Wakes the background thread to scan once things settle.
    soon: Mutex<Option<Sender<()>>>,
}

/// Starts the background scanning: a scan now (start-up), then one after each burst of changes
/// (Android's music index, the app coming back to the foreground).
pub fn start(app: &AppHandle) {
    let (soon, wake) = mpsc::channel::<()>();
    *app.state::<Scanner>().soon.lock().unwrap() = Some(soon);
    let handle = app.clone();
    std::thread::Builder::new()
        .name("library-scanner".into())
        .spawn(move || {
            let _ = scan_all(&handle);
            while wake.recv().is_ok() {
                // Wait for a quiet moment.
                loop {
                    match wake.recv_timeout(SETTLE) {
                        Ok(()) => continue,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                let _ = scan_all(&handle);
            }
        })
        .expect("the library scanner thread");
    let handle = app.clone();
    android::on_music_changes(app, move || scan_soon(&handle));
}

/// Scans once things have been quiet for a moment.
pub fn scan_soon(app: &AppHandle) {
    if let Some(soon) = app.state::<Scanner>().soon.lock().unwrap().as_ref() {
        let _ = soon.send(());
    }
}

/// Scans every switched-on folder. With a scan already running, it's asked to go once more instead and
/// this returns None.
pub fn scan_all(app: &AppHandle) -> Result<Option<ScanSummary>> {
    let scanner = app.state::<Scanner>();
    if scanner.running.swap(true, Ordering::SeqCst) {
        scanner.again.store(true, Ordering::SeqCst);
        return Ok(None);
    }
    let mut total = ScanSummary::default();
    let result = loop {
        scanner.again.store(false, Ordering::SeqCst);
        match scan_folders(app) {
            Ok(summary) => total.add(summary),
            Err(e) => break Err(e),
        }
        if !scanner.again.load(Ordering::SeqCst) {
            break Ok(Some(total));
        }
    };
    scanner.running.store(false, Ordering::SeqCst);
    // Asked for between the last check and now: run once more.
    if scanner.again.swap(false, Ordering::SeqCst) {
        scan_soon(app);
    }
    result
}

fn scan_folders(app: &AppHandle) -> Result<ScanSummary> {
    tracking(app, ChangeReason::Scan, false, || scan_folders_now(app))
}

fn scan_folders_now(app: &AppHandle) -> Result<ScanSummary> {
    let library = app.state::<Library>();
    let _ = app.emit(SCAN_EVENT, ScanEvent::Started);
    let mut total = ScanSummary::default();
    let folder_covers = library.setting(android::FOLDER_COVERS_SETTING)?.as_deref() == Some("1");
    for folder in library.folders()?.into_iter().filter(|f| f.enabled) {
        let listed = match folder.kind {
            FolderKind::Path => scan::list_folder(Path::new(&folder.location)),
            FolderKind::AndroidMedia => android::list_music(app, folder_covers),
            FolderKind::AndroidTree => android::list_tree(app, &folder.location),
        };
        // A folder that can't be listed (unplugged, offline, no permission) is left as it was: its songs
        // aren't marked missing just because the drive is out.
        let result = listed.and_then(|files| library.apply_scan(folder.id, &files));
        let event = match result {
            Ok(summary) => {
                total.add(summary);
                ScanEvent::Folder { folder_id: folder.id, name: folder.name, summary: Some(summary), error: None }
            }
            Err(e) => ScanEvent::Folder { folder_id: folder.id, name: folder.name, summary: None, error: Some(e) },
        };
        let _ = app.emit(SCAN_EVENT, event);
    }

    // The second pass: tags and covers of new and changed songs.
    let open = |job: &TagJob, path: &str| open_file(app, job, path);
    let mut progress = |done, total| {
        let _ = app.emit(SCAN_EVENT, ScanEvent::Tags { done, total });
    };
    total.add(library.read_tags(&open, &mut progress)?);
    // Albums and artists from the tags (only what changed is written), then the covers nothing uses.
    library.regroup_after(&total)?;
    if let Err(e) = library.clean_covers() {
        eprintln!("melue: can't clean up the covers: {e}");
    }

    let _ = app.emit(SCAN_EVENT, ScanEvent::Finished { summary: total, changed: total.changed_anything() });
    Ok(total)
}

/// Opens a song's file (or a folder cover) wherever its folder is.
fn open_file(app: &AppHandle, job: &TagJob, path: &str) -> Result<File> {
    match job.kind {
        FolderKind::AndroidTree => android::open_tree_file(app, &job.location, path),
        _ => tags::open_local(job, path),
    }
}
