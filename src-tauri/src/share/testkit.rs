//! The share's test kit: a share in a temporary folder (the NAS stand-in, for testing everything built on
//! the share without one), and a share that fails on purpose (what the sync has to survive).

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use super::folder::FolderShare;
use super::{Entry, Share, ShareError, ShareResult};
use crate::library::testkit::TestDir;

/// A share in a temporary folder, removed when dropped.
pub struct TestShare {
    pub share: FolderShare,
    pub dir: TestDir,
}

impl TestShare {
    pub fn new(name: &str) -> Self {
        let dir = TestDir::new(&format!("share-{name}"));
        Self { share: FolderShare::new(&dir.path), dir }
    }

    /// The file at `path` on the share, on disk.
    pub fn path(&self, path: &str) -> PathBuf {
        self.dir.path.join(path)
    }

    /// Puts a file on the share as someone else would (directly, not through the share).
    pub fn add(&self, path: &str, bytes: &[u8]) {
        let full = self.path(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, bytes).unwrap();
    }

    pub fn exists(&self, path: &str) -> bool {
        self.path(path).exists()
    }

    /// Every file on the share's disk, hidden ones too (to check nothing was left behind), by path.
    pub fn all_files(&self) -> Vec<String> {
        let mut files = Vec::new();
        let mut stack = vec![self.dir.path.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                if entry.file_type().unwrap().is_dir() {
                    stack.push(entry.path());
                } else {
                    let relative = entry.path().strip_prefix(&self.dir.path).unwrap().to_string_lossy().replace('\\', "/");
                    files.push(relative);
                }
            }
        }
        files.sort();
        files
    }
}

/// A share that fails on purpose: the next calls fail with the errors given, then it works again.
pub struct FlakyShare<S: Share> {
    pub inner: S,
    failures: Mutex<VecDeque<ShareError>>,
    calls: AtomicU32,
}

impl<S: Share> FlakyShare<S> {
    pub fn new(inner: S) -> Self {
        Self { inner, failures: Mutex::new(VecDeque::new()), calls: AtomicU32::new(0) }
    }

    /// The next `times` calls fail with `error`.
    pub fn fail_next(&self, times: usize, error: ShareError) {
        let mut failures = self.failures.lock().unwrap();
        for _ in 0..times {
            failures.push_back(error.clone());
        }
    }

    /// How many calls were made.
    pub fn calls(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }

    fn call(&self) -> ShareResult<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.failures.lock().unwrap().pop_front() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl<S: Share> Share for FlakyShare<S> {
    fn list(&self, dir: &str) -> ShareResult<Vec<Entry>> {
        self.call()?;
        self.inner.list(dir)
    }

    fn stat(&self, path: &str) -> ShareResult<Option<Entry>> {
        self.call()?;
        self.inner.stat(path)
    }

    fn read_to(&self, path: &str, out: &mut dyn Write, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        self.call()?;
        self.inner.read_to(path, out, progress)
    }

    fn write_from(&self, path: &str, input: &mut dyn Read, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        self.call()?;
        self.inner.write_from(path, input, progress)
    }

    fn rename(&self, from: &str, to: &str) -> ShareResult<()> {
        self.call()?;
        self.inner.rename(from, to)
    }

    fn delete(&self, path: &str) -> ShareResult<()> {
        self.call()?;
        self.inner.delete(path)
    }

    fn create_dir_all(&self, dir: &str) -> ShareResult<()> {
        self.call()?;
        self.inner.create_dir_all(dir)
    }

    fn delete_empty_dir(&self, dir: &str) -> ShareResult<()> {
        self.call()?;
        self.inner.delete_empty_dir(dir)
    }

    fn free_space(&self) -> ShareResult<Option<u64>> {
        self.call()?;
        self.inner.free_space()
    }
}
