//! A plain folder as the share: the tests' stand-in for the NAS, and on desktop a network drive Windows has
//! already connected (`Z:`, `\\nas\music`: Windows does the SMB and the login).
// Used by the tests now, and by the share setup's network drives (Phase 2 step 2).
#![cfg_attr(not(test), allow(dead_code))]

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::{clean_path, is_hidden, join, part_path, Entry, Share, ShareError, ShareResult, CHUNK};

pub struct FolderShare {
    root: PathBuf,
}

impl FolderShare {
    /// The music folder at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn full(&self, path: &str) -> ShareResult<PathBuf> {
        self.reachable()?;
        let path = clean_path(path)?;
        Ok(if path.is_empty() { self.root.clone() } else { self.root.join(path) })
    }

    /// The folder itself has to be there: a disconnected network drive is "unreachable", not "empty".
    fn reachable(&self) -> ShareResult<()> {
        if self.root.is_dir() {
            Ok(())
        } else {
            Err(ShareError::Unreachable { detail: format!("{} isn't there (a disconnected network drive?)", self.root.display()) })
        }
    }
}

fn error(e: io::Error, path: &Path) -> ShareError {
    let shown = path.display().to_string();
    match e.kind() {
        io::ErrorKind::NotFound => ShareError::NotFound { path: shown },
        io::ErrorKind::PermissionDenied => ShareError::NoPermission { path: shown },
        io::ErrorKind::StorageFull => ShareError::Full { detail: shown },
        io::ErrorKind::TimedOut | io::ErrorKind::NotConnected | io::ErrorKind::ConnectionReset | io::ErrorKind::HostUnreachable => {
            ShareError::Unreachable { detail: format!("{shown}: {e}") }
        }
        _ => ShareError::Other { detail: format!("{shown}: {e}") },
    }
}

fn modified_ms(meta: &fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64)
}

fn entry(path: String, name: String, meta: &fs::Metadata) -> Entry {
    Entry { path, name, is_dir: meta.is_dir(), size: if meta.is_dir() { 0 } else { meta.len() }, modified_ms: modified_ms(meta) }
}

/// Copies `input` to `out` in chunks, reporting the bytes so far.
pub(crate) fn copy(input: &mut dyn Read, out: &mut dyn Write, progress: &mut dyn FnMut(u64)) -> io::Result<u64> {
    let mut buffer = vec![0; CHUNK];
    let mut total = 0;
    loop {
        let n = match input.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        out.write_all(&buffer[..n])?;
        total += n as u64;
        progress(total);
    }
}

impl Share for FolderShare {
    fn list(&self, dir: &str) -> ShareResult<Vec<Entry>> {
        let dir = clean_path(dir)?;
        let full = self.full(&dir)?;
        let mut entries = Vec::new();
        for item in fs::read_dir(&full).map_err(|e| error(e, &full))? {
            let item = item.map_err(|e| error(e, &full))?;
            let name = item.file_name().to_string_lossy().into_owned();
            if is_hidden(&name) {
                continue;
            }
            let Ok(meta) = item.metadata() else { continue };
            entries.push(entry(join(&dir, &name), name, &meta));
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn stat(&self, path: &str) -> ShareResult<Option<Entry>> {
        let path = clean_path(path)?;
        let full = self.full(&path)?;
        match fs::metadata(&full) {
            Ok(meta) => {
                let name = path.rsplit('/').next().unwrap_or_default().to_string();
                Ok(Some(entry(path, name, &meta)))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(error(e, &full)),
        }
    }

    fn read_to(&self, path: &str, out: &mut dyn Write, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        let full = self.full(path)?;
        let mut file = fs::File::open(&full).map_err(|e| error(e, &full))?;
        copy(&mut file, out, progress).map_err(|e| error(e, &full))
    }

    fn write_from(&self, path: &str, input: &mut dyn Read, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        let path = clean_path(path)?;
        let target = self.full(&path)?;
        let part = self.full(&part_path(&path))?;
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir).map_err(|e| error(e, dir))?;
        }
        let written = (|| -> io::Result<u64> {
            let mut file = fs::File::create(&part)?;
            let n = copy(input, &mut file, progress)?;
            file.sync_all()?;
            Ok(n)
        })();
        let result = written.and_then(|n| fs::rename(&part, &target).map(|_| n));
        if result.is_err() {
            let _ = fs::remove_file(&part);
        }
        result.map_err(|e| error(e, &target))
    }

    fn rename(&self, from: &str, to: &str) -> ShareResult<()> {
        let (from, to) = (self.full(from)?, self.full(to)?);
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir).map_err(|e| error(e, dir))?;
        }
        fs::rename(&from, &to).map_err(|e| error(e, &from))
    }

    fn delete(&self, path: &str) -> ShareResult<()> {
        let full = self.full(path)?;
        match fs::remove_file(&full) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(error(e, &full)),
            _ => Ok(()),
        }
    }

    fn create_dir_all(&self, dir: &str) -> ShareResult<()> {
        let full = self.full(dir)?;
        fs::create_dir_all(&full).map_err(|e| error(e, &full))
    }

    fn delete_empty_dir(&self, dir: &str) -> ShareResult<()> {
        let full = self.full(dir)?;
        match fs::remove_dir(&full) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(error(e, &full)),
            _ => Ok(()),
        }
    }

    fn free_space(&self) -> ShareResult<Option<u64>> {
        self.reachable()?;
        // std has no free-space call: the sync engine checks this device's space itself; for a folder share
        // it's the same disk or Windows' network drive.
        Ok(None)
    }
}
