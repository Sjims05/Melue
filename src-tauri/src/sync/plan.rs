//! What a sync would do (the dry run): the share's music folder compared with the synced folder here and
//! with what was synced before (the records). Nothing is changed.
//!
//! | On the share                    | Here                         | Plan                                    |
//! |---------------------------------|------------------------------|-----------------------------------------|
//! | there                           | not there, never synced      | download (new)                          |
//! | there                           | deleted here (synced before) | download (restore)                      |
//! | changed since the last sync     | there                        | update (the share wins)                 |
//! | unchanged                       | changed here                 | update (undone: the share wins)         |
//! | there, same size as the file here, never synced | there        | adopt (no download: e.g. a first sync)  |
//! | moved or renamed (same size and time as one gone) | the old one | move here (no download, nothing twice) |
//! | gone                            | synced before                | gone: kept (or removed, a setting)      |
//! | gone                            | gone too                     | forgotten                               |
//! | —                               | never synced (put by hand)   | only here: stays                        |

use std::collections::HashMap;

use serde::Serialize;

use super::{kind_of, FileKind, Record, SyncSettings};
use crate::share::Entry;

/// A file to copy from the share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFile {
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub why: Why,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    /// New on the share.
    New,
    /// Synced before, deleted here.
    DeletedHere,
    /// Changed on the share.
    ChangedThere,
    /// Changed here (undone).
    ChangedHere,
}

/// A file moved or renamed on the share: the copy here follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMove {
    pub from: String,
    pub to: String,
}

/// What a sync would do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    /// Copied from the share (not here yet).
    pub download: Vec<PlanFile>,
    /// Copied from the share over the copy here.
    pub update: Vec<PlanFile>,
    /// Here already, the same as on the share, but never synced: only remembered.
    pub adopt: Vec<String>,
    pub moves: Vec<PlanMove>,
    /// Synced before, gone from the share: kept here, unless `remove_gone`.
    pub gone: Vec<String>,
    /// Put here by hand (never synced): stays.
    pub only_here: Vec<String>,
    /// Synced before, gone from the share and from here: only forgotten.
    pub forget: Vec<String>,
    /// Whether `gone` is removed (the setting).
    pub remove_gone: bool,
    /// Removing that many needs the user's OK first (more than the setting's number).
    pub needs_confirmation: bool,
    pub unchanged: u32,
    /// How much `download` and `update` copy.
    pub bytes: u64,
}

impl SyncPlan {
    /// Whether the sync has nothing to do.
    #[cfg_attr(not(test), allow(dead_code))] // the sync uses it (step 4)
    pub fn is_empty(&self) -> bool {
        self.download.is_empty()
            && self.update.is_empty()
            && self.adopt.is_empty()
            && self.moves.is_empty()
            && self.forget.is_empty()
            // Songs gone from the share are only something to do when they're removed.
            && (!self.remove_gone || self.gone.is_empty())
    }
}

/// Paths compare without case (Windows and SMB don't tell "a.mp3" from "A.mp3").
fn key(path: &str) -> String {
    path.to_lowercase()
}

/// The plan, from the share's files, the synced folder's files and the records. Files Melue doesn't use are
/// left out on both sides.
pub fn make_plan(share: Vec<Entry>, local: Vec<Entry>, records: Vec<Record>, settings: &SyncSettings) -> SyncPlan {
    let mut plan = SyncPlan { remove_gone: settings.remove_gone, ..Default::default() };
    let mut local: HashMap<String, Entry> =
        local.into_iter().filter(|e| !e.is_dir && kind_of(&e.name).is_some()).map(|e| (key(&e.path), e)).collect();
    let mut records: HashMap<String, Record> = records.into_iter().map(|r| (key(&r.path), r)).collect();
    let changed_there = |s: &Entry, r: &Record| s.size != r.share_size || s.modified_ms != r.share_modified_ms;
    let changed_here = |l: &Entry, r: &Record| l.size != r.local_size || l.modified_ms != r.local_modified_ms;

    let mut new = Vec::new();
    for file in share.into_iter().filter(|e| !e.is_dir) {
        let Some(kind) = kind_of(&file.name) else { continue };
        let k = key(&file.path);
        let here = local.remove(&k);
        let record = records.remove(&k);
        let copy = |why| PlanFile { path: file.path.clone(), kind, size: file.size, why };
        match (here, record) {
            (None, None) => new.push((file, kind)),
            (None, Some(r)) => plan.download.push(copy(if changed_there(&file, &r) { Why::ChangedThere } else { Why::DeletedHere })),
            (Some(l), None) if l.size == file.size => plan.adopt.push(file.path),
            (Some(_), None) => plan.update.push(copy(Why::ChangedThere)),
            (Some(_), Some(r)) if changed_there(&file, &r) => plan.update.push(copy(Why::ChangedThere)),
            (Some(l), Some(r)) if changed_here(&l, &r) => plan.update.push(copy(Why::ChangedHere)),
            (Some(_), Some(_)) => plan.unchanged += 1,
        }
    }

    // What's left of the records is gone from the share. Those whose copy is still here, unchanged, can be
    // the source of a move: a new file with exactly their size and time on the share, when that matches one
    // way only.
    let mut gone_here: Vec<(Record, Entry)> = Vec::new();
    for (k, record) in records {
        match local.remove(&k) {
            Some(here) => gone_here.push((record, here)),
            None => plan.forget.push(record.path),
        }
    }
    let identity = |size: u64, time: i64| (size, time);
    let mut new_count: HashMap<(u64, i64), u32> = HashMap::new();
    for (file, _) in &new {
        *new_count.entry(identity(file.size, file.modified_ms)).or_default() += 1;
    }
    let mut gone_count: HashMap<(u64, i64), u32> = HashMap::new();
    for (record, _) in &gone_here {
        *gone_count.entry(identity(record.share_size, record.share_modified_ms)).or_default() += 1;
    }
    for (file, kind) in new {
        let id = identity(file.size, file.modified_ms);
        let unique = new_count.get(&id) == Some(&1) && gone_count.get(&id) == Some(&1);
        let source = unique
            .then(|| gone_here.iter().position(|(r, here)| identity(r.share_size, r.share_modified_ms) == id && !changed_here(here, r)))
            .flatten();
        match source {
            Some(at) => {
                let (record, _) = gone_here.remove(at);
                plan.moves.push(PlanMove { from: record.path, to: file.path });
            }
            None => plan.download.push(PlanFile { path: file.path, kind, size: file.size, why: Why::New }),
        }
    }
    plan.gone = gone_here.into_iter().map(|(record, _)| record.path).collect();
    plan.only_here = local.into_values().map(|e| e.path).collect();

    for list in [&mut plan.adopt, &mut plan.gone, &mut plan.only_here, &mut plan.forget] {
        list.sort();
    }
    plan.download.sort_by(|a, b| a.path.cmp(&b.path));
    plan.update.sort_by(|a, b| a.path.cmp(&b.path));
    plan.moves.sort_by(|a, b| a.to.cmp(&b.to));
    plan.bytes = plan.download.iter().chain(&plan.update).map(|f| f.size).sum();
    // Removing more songs than the setting allows (0: any) needs the user's OK.
    let removing = plan.gone.iter().filter(|p| kind_of(p) == Some(FileKind::Song)).count() as u32;
    plan.needs_confirmation =
        plan.remove_gone && removing > 0 && (settings.ask_before_removing == 0 || removing > settings.ask_before_removing);
    plan
}
