//! Android: the synced folder (a folder chosen with the picker) as a [`Share`], so the sync writes into it the
//! same way as into a folder on disk: files go to a hidden temporary file first and are renamed when complete;
//! moving into another folder keeps the file (and its time: the library sees a move, not a new song).

use std::io::{Read, Write};

use tauri::AppHandle;
use tauri_plugin_android_fs::{AndroidFsExt, Entry as FsEntry, FsUri};
use tauri_plugin_melue_android::MelueAndroidExt;

use crate::library::android::top_tree;
use crate::share::folder::copy;
use crate::share::{clean_path, is_hidden, join, part_path, Entry, Share, ShareError, ShareResult};

pub struct TreeShare {
    app: AppHandle,
    root: FsUri,
}

fn error(e: impl std::fmt::Display, path: &str) -> ShareError {
    let detail = e.to_string();
    if detail.to_lowercase().contains("not found") || detail.contains("FileNotFoundException") {
        ShareError::NotFound { path: path.into() }
    } else if detail.to_lowercase().contains("no space") {
        ShareError::Full { detail }
    } else {
        ShareError::Other { detail: format!("{path}: {detail}") }
    }
}

fn modified_ms(time: std::time::SystemTime) -> i64 {
    time.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn parent_and_name(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

impl TreeShare {
    pub fn new(app: &AppHandle, location: &str) -> Self {
        Self { app: app.clone(), root: FsUri { uri: location.to_string(), document_top_tree_uri: Some(top_tree(location)) } }
    }

    fn file(&self, path: &str) -> Option<FsUri> {
        self.app.android_fs().resolve_file_uri(&self.root, path).ok()
    }

    fn dir(&self, path: &str) -> Option<FsUri> {
        if path.is_empty() {
            return Some(self.root.clone());
        }
        self.app.android_fs().resolve_dir_uri(&self.root, path).ok()
    }
}

impl Share for TreeShare {
    fn list(&self, dir: &str) -> ShareResult<Vec<Entry>> {
        let dir = clean_path(dir)?;
        let uri = self.dir(&dir).ok_or_else(|| ShareError::NotFound { path: dir.clone() })?;
        let entries = self.app.android_fs().read_dir(&uri).map_err(|e| error(e, &dir))?;
        let mut found: Vec<Entry> = entries
            .into_iter()
            .filter_map(|entry| match entry {
                FsEntry::Dir { name, last_modified, .. } if !is_hidden(&name) => {
                    Some(Entry { path: join(&dir, &name), name, is_dir: true, size: 0, modified_ms: modified_ms(last_modified) })
                }
                FsEntry::File { name, len, last_modified, .. } if !is_hidden(&name) => {
                    Some(Entry { path: join(&dir, &name), name, is_dir: false, size: len, modified_ms: modified_ms(last_modified) })
                }
                _ => None,
            })
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(found)
    }

    fn stat(&self, path: &str) -> ShareResult<Option<Entry>> {
        let path = clean_path(path)?;
        let name = parent_and_name(&path).1.to_string();
        if let Some(file) = self.file(&path) {
            let meta = self.app.android_fs().get_metadata(&file).map_err(|e| error(e, &path))?;
            let time = meta.modified().map(modified_ms).unwrap_or(0);
            return Ok(Some(Entry { path, name, is_dir: false, size: meta.len(), modified_ms: time }));
        }
        Ok(self.dir(&path).map(|_| Entry { path: path.clone(), name, is_dir: true, size: 0, modified_ms: 0 }))
    }

    fn read_to(&self, path: &str, out: &mut dyn Write, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        let path = clean_path(path)?;
        let file = self.file(&path).ok_or_else(|| ShareError::NotFound { path: path.clone() })?;
        let mut input = self.app.android_fs().open_file_readable(&file).map_err(|e| error(e, &path))?;
        copy(&mut input, out, progress).map_err(|e| error(e, &path))
    }

    fn write_from(&self, path: &str, input: &mut dyn Read, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        let path = clean_path(path)?;
        let part = part_path(&path);
        let fs = self.app.android_fs();
        if let Some(old) = self.file(&part) {
            let _ = fs.remove_file(&old);
        }
        let created = fs.create_new_file(&self.root, &part, None).map_err(|e| error(e, &path))?;
        let written = (|| -> ShareResult<u64> {
            let mut file = fs.open_file_writable(&created).map_err(|e| error(e, &path))?;
            let n = copy(input, &mut file, progress).map_err(|e| error(e, &path))?;
            file.sync_all().map_err(|e| error(e, &path))?;
            Ok(n)
        })();
        let n = match written {
            Ok(n) => n,
            Err(e) => {
                let _ = fs.remove_file(&created);
                return Err(e);
            }
        };
        // Put in place: what was there goes first (the provider would pick another name otherwise).
        if let Some(existing) = self.file(&path) {
            fs.remove_file(&existing).map_err(|e| error(e, &path))?;
        }
        fs.rename(&created, parent_and_name(&path).1).map_err(|e| error(e, &path))?;
        Ok(n)
    }

    fn rename(&self, from: &str, to: &str) -> ShareResult<()> {
        let (from, to) = (clean_path(from)?, clean_path(to)?);
        let fs = self.app.android_fs();
        let file = self.file(&from).ok_or_else(|| ShareError::NotFound { path: from.clone() })?;
        if let Some(existing) = self.file(&to) {
            fs.remove_file(&existing).map_err(|e| error(e, &to))?;
        }
        let ((from_dir, _), (to_dir, to_name)) = (parent_and_name(&from), parent_and_name(&to));
        let moved = if from_dir == to_dir {
            file
        } else {
            // Another folder: Android moves the document itself (it keeps its time).
            let source_dir = self.dir(from_dir).ok_or_else(|| ShareError::NotFound { path: from_dir.into() })?;
            let target_dir = if to_dir.is_empty() {
                self.root.clone()
            } else {
                fs.create_dir_all(&self.root, to_dir).map_err(|e| error(e, to_dir))?
            };
            let uri = self.app.melue_android().move_document(&file.uri, &source_dir.uri, &target_dir.uri).map_err(|e| error(e, &from))?;
            FsUri { uri, document_top_tree_uri: self.root.document_top_tree_uri.clone() }
        };
        if parent_and_name(&from).1 != to_name {
            fs.rename(&moved, to_name).map_err(|e| error(e, &to))?;
        }
        Ok(())
    }

    fn delete(&self, path: &str) -> ShareResult<()> {
        let path = clean_path(path)?;
        match self.file(&path) {
            Some(file) => self.app.android_fs().remove_file(&file).map_err(|e| error(e, &path)),
            None => Ok(()),
        }
    }

    fn create_dir_all(&self, dir: &str) -> ShareResult<()> {
        let dir = clean_path(dir)?;
        if dir.is_empty() {
            return Ok(());
        }
        self.app.android_fs().create_dir_all(&self.root, &dir).map(|_| ()).map_err(|e| error(e, &dir))
    }

    fn delete_empty_dir(&self, dir: &str) -> ShareResult<()> {
        let dir = clean_path(dir)?;
        match self.dir(&dir).filter(|_| !dir.is_empty()) {
            Some(uri) => self.app.android_fs().remove_dir(&uri).map_err(|e| error(e, &dir)),
            None => Ok(()),
        }
    }

    fn free_space(&self) -> ShareResult<Option<u64>> {
        // The volume the folder is on ("primary:Music/Melue" is on /storage/emulated/0).
        let document = crate::library::linked::document_path(&self.root.uri);
        let volume = crate::library::folders::android_path(document.split(':').next().map(|v| format!("{v}:")).as_deref().unwrap_or(""));
        Ok(fs4::available_space(volume.trim_end_matches('/')).ok())
    }
}
