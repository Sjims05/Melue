//! What a sync would do (plan.rs), with the test kit's share and a local folder.

use std::time::{Duration, SystemTime};

use super::plan::{make_plan, PlanMove, SyncPlan, Why};
use super::{kind_of, FileKind, SyncSettings};
use crate::library::testkit::{TestDir, TestLibrary};
use crate::share::folder::FolderShare;
use crate::share::testkit::TestShare;
use crate::share::{Entry, Share};

/// The share, the synced folder here, and the library that remembers what was synced.
struct Setup {
    share: TestShare,
    here_dir: TestDir,
    here: FolderShare,
    t: TestLibrary,
}

impl Setup {
    fn new(name: &str) -> Self {
        let here_dir = TestDir::new(&format!("plan-here-{name}"));
        let here = FolderShare::new(&here_dir.path);
        Self { share: TestShare::new(&format!("plan-{name}")), here_dir, here, t: TestLibrary::new(&format!("plan-{name}")) }
    }

    fn plan_with(&self, settings: &SyncSettings) -> SyncPlan {
        make_plan(self.share.share.walk("").unwrap(), self.here.walk("").unwrap(), self.t.library.sync_records().unwrap(), settings)
    }

    fn plan(&self) -> SyncPlan {
        self.plan_with(&SyncSettings::default())
    }

    /// Copies a file from the share to here and remembers it, as the copying will (step 4).
    fn sync(&self, path: &str) {
        let bytes = self.share.share.read(path).unwrap();
        self.here.write(path, &bytes).unwrap();
        let (there, local) = (self.share.share.stat(path).unwrap().unwrap(), self.here.stat(path).unwrap().unwrap());
        self.t.library.record_synced(&there, &local).unwrap();
    }

    /// Changes a file's contents and moves its time on, as an edit would.
    fn edit(folder: &FolderShare, root: &std::path::Path, path: &str, bytes: &[u8]) {
        let before = std::fs::metadata(root.join(path)).unwrap().modified().unwrap();
        folder.write(path, bytes).unwrap();
        let file = std::fs::File::options().write(true).open(root.join(path)).unwrap();
        file.set_modified(before + Duration::from_secs(5)).unwrap();
    }
}

fn paths(files: &[super::plan::PlanFile]) -> Vec<(&str, Why)> {
    files.iter().map(|f| (f.path.as_str(), f.why)).collect()
}

#[test]
fn only_what_melue_uses_is_synced() {
    for (name, kind) in [("a.mp3", Some(FileKind::Song)), ("b.FLAC", Some(FileKind::Song)), ("mix.m3u8", Some(FileKind::Playlist)), ("folder.jpg", Some(FileKind::Cover)), ("notes.txt", None), ("album.nfo", None), ("song.mp4", Some(FileKind::Song)), ("back.jpg", None)] {
        assert_eq!(kind_of(name), kind, "{name}");
    }
    let s = Setup::new("kinds");
    s.share.add("ABBA/01.mp3", b"song");
    s.share.add("ABBA/cover.jpg", b"img");
    s.share.add("ABBA/info.txt", b"no");
    s.share.add("Lists/Mix.m3u", b"x");
    s.share.add(".melue/state.json", b"melue's own data: not part of the music sync");
    let plan = s.plan();
    let files: Vec<&str> = plan.download.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, vec!["ABBA/01.mp3", "ABBA/cover.jpg", "Lists/Mix.m3u"]);
    assert_eq!(plan.bytes, 4 + 3 + 1);
    assert!(!plan.is_empty());
}

#[test]
fn new_unchanged_changed_there_and_changed_here() {
    let s = Setup::new("changes");
    for name in ["a", "b", "c", "d"] {
        s.share.add(&format!("{name}.mp3"), name.as_bytes());
        s.sync(&format!("{name}.mp3"));
    }
    s.share.add("new.mp3", b"new");
    assert_eq!(s.plan().unchanged, 4);

    Setup::edit(&s.share.share, &s.share.dir.path, "b.mp3", b"b, edited on the share");
    Setup::edit(&s.here, &s.here_dir.path, "c.mp3", b"c, edited here");
    s.here.delete("d.mp3").unwrap();
    let plan = s.plan();
    assert_eq!(paths(&plan.download), vec![("d.mp3", Why::DeletedHere), ("new.mp3", Why::New)]);
    assert_eq!(paths(&plan.update), vec![("b.mp3", Why::ChangedThere), ("c.mp3", Why::ChangedHere)]);
    assert_eq!(plan.unchanged, 1);
}

#[test]
fn files_already_here_are_adopted_without_downloading() {
    let s = Setup::new("adopt");
    s.share.add("a.mp3", b"same");
    s.share.add("b.mp3", b"on the share");
    s.here.write("a.mp3", b"same").unwrap(); // e.g. copied by hand before the first sync
    s.here.write("b.mp3", b"different").unwrap();
    let plan = s.plan();
    assert_eq!(plan.adopt, vec!["a.mp3"]);
    assert_eq!(paths(&plan.update), vec![("b.mp3", Why::ChangedThere)]);
    assert!(plan.download.is_empty());
}

#[test]
fn a_move_or_rename_on_the_share_moves_the_copy_here() {
    let s = Setup::new("moves");
    s.share.add("ABBA/Waterloo/01.mp3", b"waterloo");
    s.sync("ABBA/Waterloo/01.mp3");
    s.share.rename("ABBA/Waterloo/01.mp3", "ABBA/Waterloo (1974)/01 Waterloo.mp3");
    let plan = s.plan();
    assert_eq!(plan.moves, vec![PlanMove { from: "ABBA/Waterloo/01.mp3".into(), to: "ABBA/Waterloo (1974)/01 Waterloo.mp3".into() }]);
    assert!(plan.download.is_empty() && plan.gone.is_empty(), "no download, nothing twice");
    assert_eq!(plan.bytes, 0);
}

impl TestShare {
    /// Renames on the share's disk, keeping the file's time (as moving a file does).
    fn rename(&self, from: &str, to: &str) {
        let target = self.path(to);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::rename(self.path(from), target).unwrap();
    }
}

#[test]
fn identical_looking_files_are_not_taken_for_a_move() {
    let s = Setup::new("no-move");
    let time = SystemTime::now() - Duration::from_secs(3600);
    for name in ["a.mp3", "b.mp3"] {
        s.share.add(name, b"x");
        std::fs::File::options().write(true).open(s.share.path(name)).unwrap().set_modified(time).unwrap();
        s.sync(name);
    }
    // Both gone, two new ones exactly like them: which went where? Nothing is assumed.
    s.share.rename("a.mp3", "c.mp3");
    s.share.rename("b.mp3", "d.mp3");
    let plan = s.plan();
    assert!(plan.moves.is_empty());
    assert_eq!(plan.download.len(), 2);
    assert_eq!(plan.gone, vec!["a.mp3", "b.mp3"]);
}

#[test]
fn gone_from_the_share_is_kept_unless_the_setting_removes_it() {
    let s = Setup::new("gone");
    for n in 0..8 {
        s.share.add(&format!("{n}.mp3"), format!("song {n}").as_bytes());
        s.sync(&format!("{n}.mp3"));
    }
    s.here.write("by hand.mp3", b"mine").unwrap();
    s.share.add("gone too.mp3", b"x");
    s.sync("gone too.mp3");
    for n in 0..6 {
        std::fs::remove_file(s.share.path(&format!("{n}.mp3"))).unwrap();
    }
    std::fs::remove_file(s.share.path("gone too.mp3")).unwrap();
    s.here.delete("gone too.mp3").unwrap();

    let kept = s.plan();
    assert_eq!(kept.gone.len(), 6);
    assert_eq!(kept.only_here, vec!["by hand.mp3"], "put here by hand: stays");
    assert_eq!(kept.forget, vec!["gone too.mp3"], "gone on both sides: only forgotten");
    assert!(!kept.remove_gone && !kept.needs_confirmation);

    // Removing: more than the setting's 5 asks first; 6 or more allowed doesn't; 0 always asks.
    let removing = |ask| s.plan_with(&SyncSettings { remove_gone: true, ask_before_removing: ask, ..Default::default() });
    assert!(removing(5).needs_confirmation);
    assert!(!removing(6).needs_confirmation);
    assert!(removing(0).needs_confirmation);
}

#[test]
fn paths_compare_without_case() {
    let s = Setup::new("case");
    s.share.add("ABBA/Waterloo.mp3", b"w");
    s.sync("ABBA/Waterloo.mp3");
    let entries: Vec<Entry> = s.share.share.walk("").unwrap();
    let mut shouting = entries.clone();
    shouting[0].path = "abba/WATERLOO.mp3".into();
    let plan = make_plan(shouting, s.here.walk("").unwrap(), s.t.library.sync_records().unwrap(), &SyncSettings::default());
    assert_eq!(plan.unchanged, 1);
    assert!(plan.download.is_empty() && plan.gone.is_empty());
}

#[test]
fn a_new_share_forgets_what_the_old_one_synced() {
    use crate::share::secrets::MemorySecrets;
    use crate::share::setup::{self, PasswordStores, ShareSetup, SyncFolderChoice};
    let s = Setup::new("forget");
    s.share.add("a.mp3", b"a");
    s.sync("a.mp3");
    assert_eq!(s.t.library.sync_records().unwrap().len(), 1);
    let (secure, memory, file) = (MemorySecrets::default(), MemorySecrets::default(), MemorySecrets::default());
    let stores = PasswordStores { secure: &secure, memory: &memory, file: &file };
    let drive = ShareSetup::Drive { path: s.share.dir.path.to_string_lossy().into_owned(), stream: false };
    let into = SyncFolderChoice { kind: crate::library::FolderKind::Path, location: s.here_dir.path.to_string_lossy().into_owned() };
    setup::set_up(&s.t.library, &stores, &setup::open, drive.clone(), None, Some(into.clone())).unwrap();
    assert!(s.t.library.sync_records().unwrap().is_empty(), "set up anew: nothing known");
    s.sync("a.mp3");
    setup::set_up(&s.t.library, &stores, &setup::open, drive, None, Some(into)).unwrap();
    assert_eq!(s.t.library.sync_records().unwrap().len(), 1, "the same share again: kept");
    setup::remove(&s.t.library, &stores).unwrap();
    assert!(s.t.library.sync_records().unwrap().is_empty());
}
