//! The share Melue syncs with (Phase 2): one interface, [`Share`], over the real SMB share ([`smb::SmbShare`])
//! and a plain folder ([`folder::FolderShare`]: the tests, and on desktop a network drive Windows already
//! connected). The sync engine, uploads and synced playback only ever use this interface.
//!
//! Paths are relative to the music folder on the share, with '/' ("ABBA/Waterloo/01.mp3"); paths that would
//! leave it ("../x") are refused. Files are always written safely: to a hidden temporary name first, put in
//! place only when complete, so nothing ever sees half a file. Failures are sorted into [`ShareError`]s the
//! UI can act on ("turn on the VPN?", "wrong password").

pub mod commands;
pub mod folder;
pub mod secrets;
pub mod setup;
#[cfg(test)]
mod setup_tests;
pub mod smb;
#[cfg(test)]
pub mod testkit;
#[cfg(test)]
mod tests;

use std::fmt;
use std::io::{Read, Write};

use serde::Serialize;

/// Why something on the share failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ShareError {
    /// The share can't be reached: the NAS is off, the VPN is off, no network, or it took too long.
    Unreachable { detail: String },
    /// The user name or password is wrong (or the share wants a login).
    Login { detail: String },
    /// The share, folder or file isn't there.
    NotFound { path: String },
    /// Logged in, but not allowed to do that.
    NoPermission { path: String },
    /// The share (or this device) is full.
    Full { detail: String },
    /// Anything else, with the details.
    Other { detail: String },
}

impl fmt::Display for ShareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable { detail } => write!(f, "can't reach the share (is it on, and the VPN connected?): {detail}"),
            Self::Login { detail } => write!(f, "the share didn't accept the user name or password: {detail}"),
            Self::NotFound { path } => write!(f, "not found on the share: {path}"),
            Self::NoPermission { path } => write!(f, "no permission on the share for: {path}"),
            Self::Full { detail } => write!(f, "out of space: {detail}"),
            Self::Other { detail } => write!(f, "share error: {detail}"),
        }
    }
}

impl From<ShareError> for String {
    fn from(e: ShareError) -> Self {
        e.to_string()
    }
}

pub type ShareResult<T> = Result<T, ShareError>;

/// A file or folder on the share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Relative to the music folder, with '/'.
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    /// Bytes (0 for folders).
    pub size: u64,
    /// Milliseconds since 1970.
    pub modified_ms: i64,
}

/// The share, wherever it is. (Steps 3 to 8 of Phase 2 use what the spike doesn't yet.)
#[cfg_attr(not(test), allow(dead_code))]
pub trait Share: Send + Sync {
    /// A folder's files and folders ("" is the music folder itself), hidden ones (starting with '.') left out.
    fn list(&self, dir: &str) -> ShareResult<Vec<Entry>>;

    /// A file or folder, or None when it isn't there.
    fn stat(&self, path: &str) -> ShareResult<Option<Entry>>;

    /// Reads a file into `out`, calling `progress` with the bytes so far. Returns its size.
    fn read_to(&self, path: &str, out: &mut dyn Write, progress: &mut dyn FnMut(u64)) -> ShareResult<u64>;

    /// Writes a file from `input` safely (a temporary file first, put in place when complete, replacing what
    /// was there), creating its folders. Returns its size.
    fn write_from(&self, path: &str, input: &mut dyn Read, progress: &mut dyn FnMut(u64)) -> ShareResult<u64>;

    /// Moves a file (replacing what's there).
    fn rename(&self, from: &str, to: &str) -> ShareResult<()>;

    /// Deletes a file (not there: fine).
    fn delete(&self, path: &str) -> ShareResult<()>;

    /// Creates a folder and those above it (already there: fine).
    fn create_dir_all(&self, dir: &str) -> ShareResult<()>;

    /// Deletes an empty folder (not there: fine).
    fn delete_empty_dir(&self, dir: &str) -> ShareResult<()>;

    /// Free space on the share, when it says.
    fn free_space(&self) -> ShareResult<Option<u64>>;

    /// A whole small file (state files, playlists).
    fn read(&self, path: &str) -> ShareResult<Vec<u8>> {
        let mut bytes = Vec::new();
        self.read_to(path, &mut bytes, &mut |_| {})?;
        Ok(bytes)
    }

    /// Writes a whole small file, safely.
    fn write(&self, path: &str, bytes: &[u8]) -> ShareResult<()> {
        self.write_from(path, &mut &bytes[..], &mut |_| {}).map(|_| ())
    }

    /// Every file under a folder (not the folders themselves), hidden ones left out, by path.
    fn walk(&self, dir: &str) -> ShareResult<Vec<Entry>> {
        let mut files = Vec::new();
        let mut stack = vec![clean_path(dir)?];
        while let Some(dir) = stack.pop() {
            for entry in self.list(&dir)? {
                if entry.is_dir {
                    stack.push(entry.path);
                } else {
                    files.push(entry);
                }
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }
}

/// A path made safe for the share: '/' separators, no leading or trailing '/', no "." parts; refused when it
/// would leave the music folder ("..") or is absolute ("C:", "\\server").
pub fn clean_path(path: &str) -> ShareResult<String> {
    let unified = path.replace('\\', "/");
    if unified.starts_with("//") || unified.split('/').next().is_some_and(|first| first.ends_with(':')) {
        return Err(ShareError::Other { detail: format!("not a path inside the music folder: {path}") });
    }
    let mut parts = Vec::new();
    for part in unified.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(ShareError::Other { detail: format!("not a path inside the music folder: {path}") }),
            part => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

/// Joins a folder and a name into a share path.
pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

/// The hidden name a file is written to before it's put in place ("ABBA/.01.mp3.melue-part").
pub fn part_path(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, name)) => format!("{dir}/.{name}.melue-part"),
        None => format!(".{path}.melue-part"),
    }
}

/// Hidden names (dot-files, our temporary files) aren't listed.
pub(crate) fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

/// The size of the chunks files are read and written in.
pub(crate) const CHUNK: usize = 256 * 1024;
