//! The share (mod.rs): paths, the folder share, the test kit, and (opt-in) the real NAS.

use std::io::{self, Read};

use super::smb::{SmbConfig, SmbShare};
use super::testkit::{FlakyShare, TestShare};
use super::{clean_path, part_path, Share, ShareError};

#[test]
fn paths_stay_inside_the_music_folder() {
    assert_eq!(clean_path("ABBA/Waterloo/01.mp3").unwrap(), "ABBA/Waterloo/01.mp3");
    assert_eq!(clean_path("/ABBA//./Waterloo/").unwrap(), "ABBA/Waterloo");
    assert_eq!(clean_path("ABBA\\Waterloo").unwrap(), "ABBA/Waterloo");
    assert_eq!(clean_path("").unwrap(), "");
    for escaping in ["../x", "a/../../x", "C:/x", "c:", "\\\\server\\share", "//server/share"] {
        assert!(clean_path(escaping).is_err(), "{escaping}");
    }
    assert_eq!(part_path("ABBA/01.mp3"), "ABBA/.01.mp3.melue-part");
    assert_eq!(part_path("state.json"), ".state.json.melue-part");
}

#[test]
fn listing_skips_hidden_files_and_walking_finds_every_file() {
    let t = TestShare::new("list");
    t.add("ABBA/Waterloo/01.mp3", b"one");
    t.add("ABBA/Waterloo/02.mp3", b"two!");
    t.add("Queen/x.flac", b"q");
    t.add(".hidden/secret.mp3", b"no");
    t.add("ABBA/.DS_Store", b"no");
    let top = t.share.list("").unwrap();
    assert_eq!(top.iter().map(|e| (e.name.as_str(), e.is_dir)).collect::<Vec<_>>(), vec![("ABBA", true), ("Queen", true)]);
    let walked = t.share.walk("").unwrap();
    let paths: Vec<&str> = walked.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, vec!["ABBA/Waterloo/01.mp3", "ABBA/Waterloo/02.mp3", "Queen/x.flac"]);
    assert_eq!(walked[1].size, 4);
    assert!(walked.iter().all(|e| e.modified_ms > 0));
    assert_eq!(t.share.walk("ABBA").unwrap().len(), 2);

    let file = t.share.stat("ABBA/Waterloo/01.mp3").unwrap().unwrap();
    assert_eq!((file.name.as_str(), file.is_dir, file.size), ("01.mp3", false, 3));
    assert!(t.share.stat("ABBA").unwrap().unwrap().is_dir);
    assert_eq!(t.share.stat("nope.mp3").unwrap(), None);
}

#[test]
fn files_are_read_and_written_safely() {
    let t = TestShare::new("write");
    let song = vec![7u8; 600 * 1024]; // a few chunks
    let mut reported = Vec::new();
    let size = t.share.write_from("New/Album/01.mp3", &mut &song[..], &mut |n| reported.push(n)).unwrap();
    assert_eq!(size, song.len() as u64);
    assert_eq!(reported.last(), Some(&(song.len() as u64)));
    assert_eq!(t.share.read("New/Album/01.mp3").unwrap(), song);
    // Replacing a file; nothing temporary left behind.
    t.share.write("New/Album/01.mp3", b"replaced").unwrap();
    assert_eq!(t.share.read("New/Album/01.mp3").unwrap(), b"replaced");
    assert_eq!(t.all_files(), vec!["New/Album/01.mp3"]);

    let mut copy = Vec::new();
    let mut seen = 0;
    assert_eq!(t.share.read_to("New/Album/01.mp3", &mut copy, &mut |n| seen = n).unwrap(), 8);
    assert_eq!((copy.as_slice(), seen), (&b"replaced"[..], 8));
}

/// Gives some data, then fails: a file that can't be read to the end.
struct Breaks {
    left: usize,
}

impl Read for Breaks {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Err(io::Error::other("the disk went away"));
        }
        let n = buf.len().min(self.left);
        buf[..n].fill(1);
        self.left -= n;
        Ok(n)
    }
}

#[test]
fn a_write_that_fails_halfway_leaves_nothing_behind() {
    let t = TestShare::new("halfway");
    t.share.write("a.mp3", b"the old one").unwrap();
    assert!(t.share.write_from("a.mp3", &mut Breaks { left: 300 * 1024 }, &mut |_| {}).is_err());
    assert_eq!(t.share.read("a.mp3").unwrap(), b"the old one", "what was there is untouched");
    assert!(t.share.write_from("b/new.mp3", &mut Breaks { left: 10 }, &mut |_| {}).is_err());
    assert_eq!(t.all_files(), vec!["a.mp3"], "no half file, no temporary file");
}

#[test]
fn renaming_deleting_and_folders() {
    let t = TestShare::new("edit");
    t.add("a.mp3", b"a");
    t.add("b.mp3", b"b");
    t.share.rename("a.mp3", "Moved/b.mp3").unwrap();
    t.share.rename("b.mp3", "Moved/b.mp3").unwrap(); // replaces
    assert_eq!(t.share.read("Moved/b.mp3").unwrap(), b"b");
    assert!(!t.exists("a.mp3") && !t.exists("b.mp3"));
    t.share.delete("Moved/b.mp3").unwrap();
    t.share.delete("Moved/b.mp3").unwrap(); // not there: fine
    t.share.create_dir_all("x/y/z").unwrap();
    t.share.create_dir_all("x/y/z").unwrap();
    assert!(t.path("x/y/z").is_dir());
    t.share.delete_empty_dir("x/y/z").unwrap();
    t.share.delete_empty_dir("x/y/z").unwrap();
    assert!(!t.path("x/y/z").exists());
}

#[test]
fn errors_say_what_went_wrong() {
    let t = TestShare::new("errors");
    assert!(matches!(t.share.read("missing.mp3"), Err(ShareError::NotFound { .. })));
    assert!(matches!(t.share.list("missing"), Err(ShareError::NotFound { .. })));
    assert!(t.share.read("../outside.txt").is_err(), "can't leave the music folder");
    // A disconnected network drive: unreachable, not "empty".
    let gone = super::folder::FolderShare::new(t.path("not-plugged-in"));
    assert!(matches!(gone.list(""), Err(ShareError::Unreachable { .. })));
    // The messages are for people.
    let message = ShareError::Login { detail: "x".into() }.to_string();
    assert!(message.contains("user name or password"), "{message}");
}

#[test]
fn the_flaky_share_fails_when_told_to() {
    let t = TestShare::new("flaky");
    t.add("a.mp3", b"a");
    let flaky = FlakyShare::new(t.share);
    flaky.fail_next(2, ShareError::Unreachable { detail: "Wi-Fi off".into() });
    assert!(matches!(flaky.read("a.mp3"), Err(ShareError::Unreachable { .. })));
    assert!(flaky.walk("").is_err());
    assert_eq!(flaky.read("a.mp3").unwrap(), b"a", "then it works again");
    assert_eq!(flaky.calls(), 3);
}

// ---- the real NAS (opt-in)

/// The share the development app is connected to: its library's share setting (read only), and the password from
/// the system's secure store, where the app keeps it. Err: why it can't be used (no development app here, no SMB
/// share set up, its password not kept). The test works in a folder of its own on it, removed afterwards.
fn nas() -> Result<(SmbConfig, String), String> {
    use super::secrets::{PasswordStore, SERVICE};
    use super::setup::{ShareSettings, ShareSetup, PASSWORD_KEY};
    let appdata = std::env::var_os("APPDATA").ok_or("no APPDATA folder")?;
    let library = std::path::PathBuf::from(appdata).join(crate::IDENTITY).join("library.db");
    let conn = rusqlite::Connection::open_with_flags(&library, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("the development app's library ({}): {e}", library.display()))?;
    let json: String = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [crate::library::sync::SHARE_SETTING], |r| r.get(0))
        .map_err(|e| format!("no share set up in the development app: {e}"))?;
    let settings: ShareSettings = serde_json::from_str(&json).map_err(|e| format!("its share setting: {e}"))?;
    let ShareSetup::Smb { host, share, folder, user, password_store, home_host } = settings.setup else {
        return Err("its share isn't an SMB share".into());
    };
    if password_store != PasswordStore::Secure {
        return Err("its share's password isn't kept (asked for each time)".into());
    }
    let password = keyring::Entry::new(SERVICE, PASSWORD_KEY).and_then(|entry| entry.get_password()).map_err(|e| format!("its share's password: {e}"))?;
    let config = SmbConfig { host, home_host, share, folder, user, password, ..Default::default() };
    let test_folder = format!("melue-test-{}-{}", std::process::id(), crate::library::testkit::unique());
    Ok((config, test_folder))
}

/// Removes the test folder afterwards, also when the test fails.
struct CleanUp<'a> {
    share: &'a SmbShare,
    folder: &'a str,
}

impl Drop for CleanUp<'_> {
    fn drop(&mut self) {
        if let Ok(files) = self.share.walk(self.folder) {
            for file in files {
                let _ = self.share.delete(&file.path);
            }
        }
        for dir in [format!("{}/Album", self.folder), self.folder.to_string()] {
            let _ = self.share.delete_empty_dir(&dir);
        }
    }
}

#[test]
#[ignore = "uses the share the development app is connected to (run: cargo test real_nas -- --ignored); works only in its own test folder"]
fn the_real_nas_does_everything() {
    let (config, folder) = nas().unwrap_or_else(|why| panic!("the share the development app is connected to: {why}"));
    let share = SmbShare::new(config.clone()).unwrap();
    share.check().unwrap();
    let clean_up = CleanUp { share: &share, folder: &folder };
    assert_eq!(share.stat(&folder).unwrap(), None, "a fresh test folder");

    let song: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    let path = format!("{folder}/Album/01.mp3");
    assert_eq!(share.write_from(&path, &mut &song[..], &mut |_| {}).unwrap(), song.len() as u64);
    assert_eq!(share.read(&path).unwrap(), song);
    share.write(&path, b"replaced").unwrap();
    assert_eq!(share.read(&path).unwrap(), b"replaced");

    share.rename(&path, &format!("{folder}/Album/02.mp3")).unwrap();
    let files: Vec<String> = share.walk(&folder).unwrap().into_iter().map(|e| e.path).collect();
    assert_eq!(files, vec![format!("{folder}/Album/02.mp3")], "no temporary files left");
    let entry = share.stat(&format!("{folder}/Album/02.mp3")).unwrap().unwrap();
    assert!(entry.size == 8 && entry.modified_ms > 0);
    assert!(share.free_space().unwrap().is_some_and(|free| free > 0));
    assert!(matches!(share.read(&format!("{folder}/nope.mp3")), Err(ShareError::NotFound { .. })));

    // A wrong password, and a share that isn't there.
    let wrong = SmbShare::new(SmbConfig { password: "not the password".into(), ..config.clone() }).unwrap();
    assert!(matches!(wrong.check(), Err(ShareError::Login { .. })), "{:?}", wrong.check());
    let missing = SmbShare::new(SmbConfig { share: "melue-no-such-share".into(), ..config }).unwrap();
    assert!(matches!(missing.check(), Err(ShareError::NotFound { .. } | ShareError::NoPermission { .. })), "{:?}", missing.check());

    // From a thread of another async runtime (Tauri runs commands that way): reading and writing still work.
    let inside = tokio::runtime::Builder::new_multi_thread().worker_threads(1).build().unwrap();
    let read_inside = inside.block_on(async { tokio::spawn(async { "spawned" }).await.unwrap(); share.read(&format!("{folder}/Album/02.mp3")) });
    assert_eq!(read_inside.unwrap(), b"replaced");
    inside.block_on(async { share.write(&format!("{folder}/Album/03.mp3"), b"three") }).unwrap();
    inside.shutdown_background();

    // Nothing of the test is left on the share.
    drop(clean_up);
    assert_eq!(share.stat(&folder).unwrap(), None);
}

#[test]
#[ignore = "uses the share the development app is connected to (run: cargo test real_nas -- --ignored); works only in its own test folder"]
fn the_real_nas_says_when_its_music_changes() {
    let (config, folder) = nas().unwrap_or_else(|why| panic!("the share the development app is connected to: {why}"));
    let share = SmbShare::new(config).unwrap();
    let _clean_up = CleanUp { share: &share, folder: &folder };
    let (changed, noticed) = std::sync::mpsc::channel();
    let watch = share.watch(std::sync::Arc::new(move |path: &str| {
        let _ = changed.send(path.to_string());
    }));
    let watch = watch.unwrap().expect("SMB shares can be watched");
    assert!(watch.alive());
    share.write(&format!("{folder}/Album/new.mp3"), b"a new song").unwrap();
    let path = noticed.recv_timeout(std::time::Duration::from_secs(10)).expect("the NAS said something changed");
    assert!(path.starts_with(&folder), "{path}");
    drop(watch);
}
