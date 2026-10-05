//! Doing a sync: carrying out a plan (plan.rs) from the share (`source`) into the synced folder (`target`),
//! safely: every file is written to a hidden temporary file first and put in place when complete (the
//! targets do that), and remembered right after, so an interrupted sync carries on where it stopped (a file
//! cut off halfway starts over).
//!
//! Before copying, the free space is checked (what's copied, plus [`KEEP_FREE`]). A file that fails is
//! skipped and reported (tried again next sync); losing the share stops the sync. Removing songs gone from
//! the share waits for the user's OK when the plan says so.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use serde::{Deserialize, Serialize};

use super::plan::{PlanFile, SyncPlan};
use crate::library::Library;
use crate::share::{Entry, Share, ShareError};

/// Left free on the device on top of what a sync copies.
pub const KEEP_FREE: u64 = 500 * 1024 * 1024;

/// How far a sync is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub files_done: u32,
    pub files_total: u32,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// The file being copied.
    pub current: Option<String>,
}

/// What a sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub copied: u32,
    pub bytes: u64,
    pub moved: u32,
    pub adopted: u32,
    pub removed: u32,
    /// Files that couldn't be copied (tried again next sync), with why.
    pub failed: Vec<(String, String)>,
    /// Songs gone from the share waiting for the user's OK to be removed.
    pub removals_waiting: u32,
    /// Files not copied because the connection is metered (they wait for Wi-Fi).
    pub waiting_for_wifi: u32,
    pub cancelled: bool,
    /// Why the sync stopped before the end (the share lost, the device full).
    pub stopped: Option<String>,
}

/// Why a sync couldn't start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SyncError {
    /// Not enough free space: `needed` (what's copied plus what's kept free), `free`.
    NotEnoughSpace { needed: u64, free: u64 },
    Other { detail: String },
}

impl From<String> for SyncError {
    fn from(detail: String) -> Self {
        Self::Other { detail }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let gb = |bytes: u64| format!("{:.1} GB", bytes as f64 / 1e9);
        match self {
            Self::NotEnoughSpace { needed, free } => write!(f, "not enough free space: needs {}, {} free", gb(*needed), gb(*free)),
            Self::Other { detail } => write!(f, "{detail}"),
        }
    }
}

/// Errors that mean the sync can't go on (the share gone, the device full), rather than one bad file.
fn stops_the_sync(e: &ShareError) -> bool {
    matches!(e, ShareError::Unreachable { .. } | ShareError::Login { .. } | ShareError::Full { .. })
}

/// Hands chunks from the reading thread to the writing one.
struct ChunksOut(mpsc::SyncSender<Vec<u8>>);

impl Write for ChunksOut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf.to_vec()).map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the writing side stopped"))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ChunksIn<'a> {
    from: mpsc::Receiver<Vec<u8>>,
    pending: Vec<u8>,
    at: usize,
    /// Set by the reading side when it failed: the end of the data is then an error, not the end of the file.
    failed: &'a AtomicBool,
}

impl Read for ChunksIn<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.at == self.pending.len() {
            match self.from.recv() {
                Ok(chunk) => {
                    self.pending = chunk;
                    self.at = 0;
                }
                Err(_) if self.failed.load(Ordering::SeqCst) => return Err(io::Error::other("reading from the share failed")),
                Err(_) => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len() - self.at);
        buf[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// Copies a file from `source` to `target`, streamed (the file is never all in memory). When reading fails
/// halfway, the write fails too: nothing is put in place, and what was there stays.
fn copy(source: &dyn Share, target: &dyn Share, path: &str, progress: &mut dyn FnMut(u64)) -> Result<u64, ShareError> {
    let (out, from) = mpsc::sync_channel(4);
    let failed = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let reading = scope.spawn(|| {
            let mut out = ChunksOut(out);
            let read = source.read_to(path, &mut out, &mut |_| {});
            if read.is_err() {
                failed.store(true, Ordering::SeqCst);
            }
            read // `out` is dropped after the flag is set: the writing side sees the end only then
        });
        let mut input = ChunksIn { from, pending: Vec::new(), at: 0, failed: &failed };
        let written = target.write_from(path, &mut input, progress);
        drop(input);
        let read = reading.join().unwrap_or_else(|_| Err(ShareError::Other { detail: "reading panicked".into() }));
        match read {
            Err(e) => Err(e),
            Ok(_) => written,
        }
    })
}

/// How a sync runs.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    /// Free bytes on this device, when known.
    pub free_space: Option<u64>,
    /// The user said yes to removing more songs than the setting allows.
    pub confirmed_removal: bool,
    /// Copies wait for Wi-Fi (a metered connection): everything else still happens.
    pub no_copies: bool,
}

/// Carries out `plan`. `cancel` stops it after the current file.
pub fn run(
    library: &Library,
    source: &dyn Share,
    target: &dyn Share,
    plan: &SyncPlan,
    options: RunOptions,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&Progress),
) -> Result<SyncResult, SyncError> {
    let RunOptions { free_space, confirmed_removal, no_copies } = options;
    if let Some(free) = free_space.filter(|_| !no_copies) {
        let needed = plan.bytes + KEEP_FREE;
        if free < needed {
            return Err(SyncError::NotEnoughSpace { needed, free });
        }
    }
    let mut result = SyncResult::default();
    let remember = |path: &str, share: Option<Entry>| -> Result<(), String> {
        let share = match share {
            Some(entry) => entry,
            None => source.stat(path)?.ok_or_else(|| format!("{path} isn't on the share"))?,
        };
        let local = target.stat(path)?.ok_or_else(|| format!("{path} isn't here"))?;
        library.record_synced(&share, &local)
    };

    // Already here: only remembered.
    for path in &plan.adopt {
        match remember(path, None) {
            Ok(()) => result.adopted += 1,
            Err(e) => result.failed.push((path.clone(), e)),
        }
    }
    // Moved on the share: the copy here follows.
    for moved in &plan.moves {
        let done = target.rename(&moved.from, &moved.to).map_err(String::from).and_then(|_| {
            library.forget_synced(&moved.from)?;
            remember(&moved.to, None)
        });
        match done {
            Ok(()) => result.moved += 1,
            Err(e) => result.failed.push((moved.to.clone(), e)),
        }
    }
    for path in &plan.forget {
        library.forget_synced(path)?;
    }

    // Copies (on a metered connection they wait for Wi-Fi).
    let files: Vec<&PlanFile> = if no_copies {
        result.waiting_for_wifi = (plan.download.len() + plan.update.len()) as u32;
        Vec::new()
    } else {
        plan.download.iter().chain(&plan.update).collect()
    };
    let bytes_total = files.iter().map(|f| f.size).sum();
    let mut state = Progress { files_total: files.len() as u32, bytes_total, ..Default::default() };
    for file in files {
        if cancel.load(Ordering::SeqCst) {
            result.cancelled = true;
            break;
        }
        state.current = Some(file.path.clone());
        progress(&state);
        let before = state.bytes_done;
        let copied = copy(source, target, &file.path, &mut |n| {
            state.bytes_done = before + n;
            progress(&state);
        });
        match copied {
            Ok(size) => {
                let share = Entry { path: file.path.clone(), name: String::new(), is_dir: false, size: file.size, modified_ms: file.modified_ms };
                match remember(&file.path, Some(share)) {
                    Ok(()) => {
                        result.copied += 1;
                        result.bytes += size;
                    }
                    Err(e) => result.failed.push((file.path.clone(), e)),
                }
            }
            Err(e) if stops_the_sync(&e) => {
                result.stopped = Some(e.to_string());
                break;
            }
            Err(e) => result.failed.push((file.path.clone(), e.to_string())),
        }
        state.files_done += 1;
        state.bytes_done = before + file.size;
        progress(&state);
    }

    // Songs gone from the share: removed with the setting on, and the user's OK when it's many.
    if plan.remove_gone && !plan.gone.is_empty() && result.stopped.is_none() && !result.cancelled {
        if plan.needs_confirmation && !confirmed_removal {
            result.removals_waiting = plan.gone.len() as u32;
        } else {
            for path in &plan.gone {
                match target.delete(path).map_err(String::from).and_then(|_| library.forget_synced(path)) {
                    Ok(()) => result.removed += 1,
                    Err(e) => result.failed.push((path.clone(), e)),
                }
            }
        }
    }
    state.current = None;
    progress(&state);
    Ok(result)
}
