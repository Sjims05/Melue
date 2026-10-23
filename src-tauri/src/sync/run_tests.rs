//! Doing a sync (run.rs), with the test kit's share and a local folder as the synced folder.

use std::sync::atomic::AtomicBool;

use super::plan::{make_plan, SyncPlan};
use super::run::{run, Progress, RunOptions, SyncError, SyncResult, KEEP_FREE};
use super::SyncSettings;
use crate::library::testkit::{TestDir, TestLibrary};
use crate::share::folder::FolderShare;
use crate::share::testkit::{FlakyShare, TestShare};
use crate::share::{Share, ShareError};

struct Setup {
    share: TestShare,
    here_dir: TestDir,
    here: FolderShare,
    t: TestLibrary,
}

impl Setup {
    fn new(name: &str) -> Self {
        let here_dir = TestDir::new(&format!("run-here-{name}"));
        let here = FolderShare::new(&here_dir.path);
        Self { share: TestShare::new(&format!("run-{name}")), here_dir, here, t: TestLibrary::new(&format!("run-{name}")) }
    }

    fn plan(&self, settings: &SyncSettings) -> SyncPlan {
        make_plan(self.share.share.walk("").unwrap(), self.here.walk("").unwrap(), self.t.library.sync_records().unwrap(), settings)
    }

    fn sync_from(&self, source: &dyn Share, settings: &SyncSettings, confirmed: bool) -> SyncResult {
        let plan = self.plan(settings);
        let options = RunOptions { confirmed_removal: confirmed, ..Default::default() };
        run(&self.t.library, source, &self.here, &plan, options, &AtomicBool::new(false), &mut |_| {}).unwrap()
    }

    fn sync(&self) -> SyncResult {
        self.sync_from(&self.share.share, &SyncSettings::default(), false)
    }

    /// Every file here on disk, hidden ones too (to see nothing temporary was left behind).
    fn files_here(&self) -> Vec<String> {
        let mut all = Vec::new();
        let mut stack = vec![self.here_dir.path.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                if entry.file_type().unwrap().is_dir() {
                    stack.push(entry.path());
                } else {
                    all.push(entry.path().strip_prefix(&self.here_dir.path).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        all.sort();
        all
    }
}

#[test]
fn a_sync_copies_everything_and_then_has_nothing_to_do() {
    let s = Setup::new("full");
    let song = vec![9u8; 700 * 1024];
    s.share.add("ABBA/Waterloo/01.mp3", &song);
    s.share.add("ABBA/Waterloo/cover.jpg", b"img");
    let playlist = b"#EXTM3U\n../ABBA/Waterloo/01.mp3\n";
    s.share.add("Lists/Mix.m3u", playlist);
    s.share.add("ABBA/notes.txt", b"not synced");
    let mut seen = Vec::new();
    let plan = s.plan(&SyncSettings::default());
    let result = run(&s.t.library, &s.share.share, &s.here, &plan, RunOptions::default(), &AtomicBool::new(false), &mut |p: &Progress| seen.push(p.clone())).unwrap();
    assert_eq!((result.copied, result.bytes), (3, (song.len() + 3 + playlist.len()) as u64));
    assert!(result.failed.is_empty() && result.stopped.is_none());
    assert_eq!(s.files_here(), vec!["ABBA/Waterloo/01.mp3", "ABBA/Waterloo/cover.jpg", "Lists/Mix.m3u"], "no temporary files left");
    assert_eq!(std::fs::read(s.here_dir.path.join("ABBA/Waterloo/01.mp3")).unwrap(), song);
    let last = seen.last().unwrap();
    assert_eq!((last.files_done, last.files_total, last.bytes_done, last.current.clone()), (3, 3, plan.bytes, None));
    assert!(s.plan(&SyncSettings::default()).is_empty(), "a second plan has nothing to do");
}

#[test]
fn changes_on_either_side_are_brought_back_to_the_share() {
    let s = Setup::new("changes");
    s.share.add("a.mp3", b"a");
    s.share.add("b.mp3", b"b");
    s.share.add("c.mp3", b"c");
    s.sync();
    // Changed on the share, changed here, deleted here.
    s.share.add("a.mp3", b"a, new on the share");
    s.here.write("b.mp3", b"b, edited here").unwrap();
    s.here.delete("c.mp3").unwrap();
    let result = s.sync();
    assert_eq!(result.copied, 3);
    for (name, bytes) in [("a.mp3", &b"a, new on the share"[..]), ("b.mp3", b"b"), ("c.mp3", b"c")] {
        assert_eq!(s.here.read(name).unwrap(), bytes, "{name}");
    }
    assert!(s.plan(&SyncSettings::default()).is_empty());
}

#[test]
fn moves_and_adopted_files_need_no_copying() {
    let s = Setup::new("moves");
    s.share.add("Old/song.mp3", b"song");
    s.share.add("same.mp3", b"same");
    s.sync();
    s.here.write("adopt.mp3", b"here already").unwrap();
    s.share.add("adopt.mp3", b"here already");
    let from = s.share.path("Old/song.mp3");
    std::fs::create_dir_all(s.share.path("New")).unwrap();
    std::fs::rename(from, s.share.path("New/renamed.mp3")).unwrap();
    let result = s.sync();
    assert_eq!((result.copied, result.moved, result.adopted), (0, 1, 1));
    assert!(s.here.stat("New/renamed.mp3").unwrap().is_some() && s.here.stat("Old/song.mp3").unwrap().is_none());
    assert!(s.plan(&SyncSettings::default()).is_empty());
}

#[test]
fn songs_gone_from_the_share_are_kept_or_removed_with_an_ok() {
    let s = Setup::new("removing");
    for n in 0..7 {
        s.share.add(&format!("{n}.mp3"), format!("{n}").as_bytes());
    }
    s.sync();
    for n in 0..6 {
        std::fs::remove_file(s.share.path(&format!("{n}.mp3"))).unwrap();
    }
    // Kept (the default).
    assert_eq!(s.sync().removed, 0);
    assert_eq!(s.files_here().len(), 7);
    // Removing 6 with "ask before removing more than 5": waits for the OK.
    let removing = SyncSettings { remove_gone: true, ask_before_removing: 5, ..Default::default() };
    let waiting = s.sync_from(&s.share.share, &removing, false);
    assert_eq!((waiting.removed, waiting.removals_waiting), (0, 6));
    assert_eq!(s.files_here().len(), 7);
    let confirmed = s.sync_from(&s.share.share, &removing, true);
    assert_eq!(confirmed.removed, 6);
    assert_eq!(s.files_here(), vec!["6.mp3"]);
    assert!(s.plan(&removing).is_empty());
}

#[test]
fn folders_a_removed_or_moved_song_leaves_empty_go_too() {
    let s = Setup::new("tidy");
    s.share.add("Artist/Album/1.mp3", b"one");
    s.share.add("Artist/Album/2.mp3", b"two");
    s.share.add("Other/3.mp3", b"three");
    s.sync();
    // 3 moves to the top on the share; 1 and 2 are gone.
    std::fs::rename(s.share.path("Other/3.mp3"), s.share.path("3.mp3")).unwrap();
    std::fs::remove_dir(s.share.path("Other")).unwrap();
    std::fs::remove_file(s.share.path("Artist/Album/1.mp3")).unwrap();
    std::fs::remove_file(s.share.path("Artist/Album/2.mp3")).unwrap();
    let removing = SyncSettings { remove_gone: true, ask_before_removing: 0, ..Default::default() };
    s.sync_from(&s.share.share, &removing, true);
    assert_eq!(s.files_here(), vec!["3.mp3"]);
    for gone in ["Other", "Artist/Album", "Artist"] {
        assert!(!s.here_dir.path.join(gone).exists(), "{gone} left empty: gone");
    }
}

#[test]
fn an_interrupted_sync_is_finished_by_the_next_one() {
    let s = Setup::new("interrupted");
    for n in 0..5 {
        s.share.add(&format!("{n}.mp3"), format!("song {n}").as_bytes());
    }
    let plan = s.plan(&SyncSettings::default());
    // Two files copy, then the share is lost halfway through the third.
    let losing = FailAfter { inner: &s.share.share, reads_left: std::cell::Cell::new(2).into() };
    let result = run(&s.t.library, &losing, &s.here, &plan, RunOptions::default(), &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert_eq!(result.copied, 2);
    assert!(result.stopped.is_some(), "losing the share stops the sync");
    assert_eq!(s.files_here().len(), 2, "no half files, no temporary files");
    let rest = s.sync();
    assert_eq!(rest.copied, 3);
    assert!(s.plan(&SyncSettings::default()).is_empty());
}

/// A share whose reads work a few times, then it's unreachable.
struct FailAfter<'a, S: Share> {
    inner: &'a S,
    reads_left: std::sync::Mutex<std::cell::Cell<u32>>,
}

impl<S: Share> Share for FailAfter<'_, S> {
    fn list(&self, dir: &str) -> crate::share::ShareResult<Vec<crate::share::Entry>> {
        self.inner.list(dir)
    }
    fn stat(&self, path: &str) -> crate::share::ShareResult<Option<crate::share::Entry>> {
        self.inner.stat(path)
    }
    fn read_to(&self, path: &str, out: &mut dyn std::io::Write, progress: &mut dyn FnMut(u64)) -> crate::share::ShareResult<u64> {
        let left = self.reads_left.lock().unwrap();
        if left.get() == 0 {
            // Half a file, then the connection's gone.
            let _ = out.write_all(b"half");
            return Err(ShareError::Unreachable { detail: "the VPN dropped".into() });
        }
        left.set(left.get() - 1);
        drop(left);
        self.inner.read_to(path, out, progress)
    }
    fn write_from(&self, path: &str, input: &mut dyn std::io::Read, progress: &mut dyn FnMut(u64)) -> crate::share::ShareResult<u64> {
        self.inner.write_from(path, input, progress)
    }
    fn rename(&self, from: &str, to: &str) -> crate::share::ShareResult<()> {
        self.inner.rename(from, to)
    }
    fn delete(&self, path: &str) -> crate::share::ShareResult<()> {
        self.inner.delete(path)
    }
    fn create_dir_all(&self, dir: &str) -> crate::share::ShareResult<()> {
        self.inner.create_dir_all(dir)
    }
    fn delete_empty_dir(&self, dir: &str) -> crate::share::ShareResult<()> {
        self.inner.delete_empty_dir(dir)
    }
    fn free_space(&self) -> crate::share::ShareResult<Option<u64>> {
        self.inner.free_space()
    }
}

#[test]
fn a_failed_update_keeps_the_old_copy() {
    let s = Setup::new("failed-update");
    s.share.add("a.mp3", b"the good one");
    s.sync();
    s.share.add("a.mp3", b"a new version on the share");
    let broken = FailAfter { inner: &s.share.share, reads_left: std::cell::Cell::new(0).into() };
    let result = s.sync_from(&broken, &SyncSettings::default(), false);
    assert!(result.stopped.is_some());
    assert_eq!(s.here.read("a.mp3").unwrap(), b"the good one", "untouched, not half a file");
    assert_eq!(s.files_here(), vec!["a.mp3"]);
}

#[test]
fn a_bad_file_is_skipped_and_the_rest_copied() {
    let s = Setup::new("bad-file");
    s.share.add("a.mp3", b"a");
    s.share.add("b.mp3", b"b");
    s.share.add("c.mp3", b"c");
    let flaky = FlakyShare::new(FolderShare::new(&s.share.dir.path));
    // The first read fails for this file only (a damaged file on the share).
    flaky.fail_next(1, ShareError::Other { detail: "damaged".into() });
    let result = s.sync_from(&flaky, &SyncSettings::default(), false);
    assert_eq!(result.copied, 2);
    assert_eq!(result.failed.len(), 1);
    assert!(result.stopped.is_none());
    assert_eq!(s.sync().copied, 1, "tried again next sync");
}

#[test]
fn not_enough_space_copies_nothing() {
    let s = Setup::new("space");
    s.share.add("big.mp3", &vec![1u8; 2048]);
    let plan = s.plan(&SyncSettings::default());
    let refused = run(&s.t.library, &s.share.share, &s.here, &plan, RunOptions { free_space: Some(KEEP_FREE + 1000), ..Default::default() }, &AtomicBool::new(false), &mut |_| {});
    assert_eq!(refused, Err(SyncError::NotEnoughSpace { needed: 2048 + KEEP_FREE, free: KEEP_FREE + 1000 }));
    assert!(s.files_here().is_empty());
    assert!(run(&s.t.library, &s.share.share, &s.here, &plan, RunOptions { free_space: Some(KEEP_FREE + 4096), ..Default::default() }, &AtomicBool::new(false), &mut |_| {}).is_ok());
}

#[test]
fn cancelling_stops_after_the_current_file() {
    let s = Setup::new("cancel");
    for n in 0..4 {
        s.share.add(&format!("{n}.mp3"), b"x");
    }
    let plan = s.plan(&SyncSettings::default());
    let cancel = AtomicBool::new(false);
    let result = run(&s.t.library, &s.share.share, &s.here, &plan, RunOptions::default(), &cancel, &mut |p| {
        if p.files_done == 2 {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    })
    .unwrap();
    assert!(result.cancelled);
    assert_eq!(result.copied, 2);
    assert_eq!(s.sync().copied, 2, "the rest next time");
}

#[test]
fn a_copy_budget_copies_what_fits_and_the_rest_waits() {
    let s = Setup::new("budget");
    s.share.add("a.mp3", b"aaaa");
    s.share.add("b.mp3", b"bbbb");
    s.share.add("c.mp3", b"cccc");
    let plan = s.plan(&SyncSettings::default());
    let options = RunOptions { copy_budget: Some(9), ..Default::default() };
    let result = run(&s.t.library, &s.share.share, &s.here, &plan, options, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert_eq!((result.copied, result.waiting_for_wifi, result.bytes), (2, 1, 8), "two fit in 9 bytes");
    assert_eq!(s.sync().copied, 1, "the last one on Wi-Fi");
}

#[test]
fn on_a_metered_connection_copies_wait_and_the_rest_happens() {
    let s = Setup::new("metered");
    s.share.add("Old/a.mp3", b"a");
    s.sync();
    std::fs::create_dir_all(s.share.path("New")).unwrap();
    std::fs::rename(s.share.path("Old/a.mp3"), s.share.path("New/a.mp3")).unwrap();
    s.share.add("b.mp3", b"new on the share");
    let plan = s.plan(&SyncSettings::default());
    let options = RunOptions { copy_budget: Some(0), free_space: Some(0), ..Default::default() };
    let result = run(&s.t.library, &s.share.share, &s.here, &plan, options, &AtomicBool::new(false), &mut |_| {}).unwrap();
    assert_eq!((result.moved, result.copied, result.waiting_for_wifi), (1, 0, 1), "no space check either: nothing is copied");
    assert_eq!(s.files_here(), vec!["New/a.mp3"]);
    assert_eq!(s.sync().copied, 1, "copied once on Wi-Fi");
}
