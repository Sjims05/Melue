//! Setting up the share (setup.rs), with the test kit's folder share standing in for the NAS.

use super::folder::FolderShare;
use super::secrets::{MemorySecrets, PasswordStore, Secrets};
use super::setup::{self, PasswordStores, SetupError, ShareSetup, SyncFolderChoice, PASSWORD_KEY};
use super::testkit::TestShare;
use super::{Share, ShareError};
use crate::library::sync::SHARE_SETTING;
use crate::library::testkit::{Tags, TestLibrary};
use crate::library::FolderKind;

const RIGHT: &str = "hunter2";

/// A secure store that isn't there (a system without a keyring).
struct NoKeyring;

impl Secrets for NoKeyring {
    fn get(&self, _: &str) -> Result<Option<String>, String> {
        Err("no secret service".into())
    }
    fn set(&self, _: &str, _: &str) -> Result<(), String> {
        Err("no secret service".into())
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        Err("no secret service".into())
    }
}

struct Stores {
    secure: MemorySecrets,
    memory: MemorySecrets,
    file: MemorySecrets,
}

impl Stores {
    fn new() -> Self {
        Self { secure: MemorySecrets::default(), memory: MemorySecrets::default(), file: MemorySecrets::default() }
    }

    fn stores(&self) -> PasswordStores<'_> {
        PasswordStores { secure: &self.secure, memory: &self.memory, file: &self.file }
    }
}

/// Opens the test share for an SMB setup, when the password is right.
fn opener<'a>(share: &'a TestShare) -> impl Fn(&ShareSetup, Option<&str>) -> Result<Box<dyn Share>, SetupError> + 'a {
    move |setup: &ShareSetup, password: Option<&str>| match setup {
        ShareSetup::Smb { .. } if password != Some(RIGHT) => {
            Err(ShareError::Login { detail: "wrong password".into() }.into())
        }
        ShareSetup::Smb { .. } => Ok(Box::new(FolderShare::new(&share.dir.path)) as Box<dyn Share>),
        ShareSetup::Drive { path, .. } => Ok(Box::new(FolderShare::new(path)) as Box<dyn Share>),
    }
}

fn smb(store: PasswordStore) -> ShareSetup {
    ShareSetup::Smb { host: "nas".into(), share: "music".into(), folder: "".into(), user: "user1".into(), password_store: store, home_host: None }
}

fn sync_into(t: &TestLibrary, name: &str) -> SyncFolderChoice {
    let folder = t.music_folder(name);
    SyncFolderChoice { kind: FolderKind::Path, location: folder.root.to_string_lossy().into_owned() }
}

#[test]
fn setting_up_an_smb_share() {
    let t = TestLibrary::new("setup-smb");
    let share = TestShare::new("setup-smb");
    let s = Stores::new();
    let open = opener(&share);

    assert!(!t.library.sync_available().unwrap());
    let wrong = setup::set_up(&t.library, &s.stores(), &open, smb(PasswordStore::Secure), Some("nope"), Some(sync_into(&t, "Synced")));
    assert!(matches!(wrong, Err(SetupError::Share { error: ShareError::Login { .. } })), "{wrong:?}");
    assert!(!t.library.sync_available().unwrap(), "nothing kept after a failed setup");

    let settings = setup::set_up(&t.library, &s.stores(), &open, smb(PasswordStore::Secure), Some(RIGHT), Some(sync_into(&t, "Synced2"))).unwrap();
    assert!(t.library.sync_available().unwrap());
    // The password is in the secure store, and nowhere in the database.
    assert_eq!(s.secure.get(PASSWORD_KEY).unwrap().as_deref(), Some(RIGHT));
    let saved = t.library.setting(SHARE_SETTING).unwrap().unwrap();
    assert!(!saved.contains(RIGHT) && saved.contains("\"kind\":\"smb\""), "{saved}");
    // The sync folder is in the library, marked as the synced one.
    let folder = t.library.folder(settings.sync_folder_id).unwrap().unwrap();
    assert!(folder.synced && folder.name == "Synced music");
    // Ready to use.
    let (_, ready) = setup::current(&t.library, &s.stores(), &open).unwrap();
    assert!(ready.list("").is_ok());
    assert!(!setup::needs_password(&t.library, &s.stores()).unwrap());
}

#[test]
fn without_a_secure_store_the_password_is_asked_each_time_or_kept_in_a_file() {
    let t = TestLibrary::new("setup-no-keyring");
    let share = TestShare::new("setup-no-keyring");
    let (memory, file) = (MemorySecrets::default(), MemorySecrets::default());
    let stores = PasswordStores { secure: &NoKeyring, memory: &memory, file: &file };
    let open = opener(&share);

    let refused = setup::set_up(&t.library, &stores, &open, smb(PasswordStore::Secure), Some(RIGHT), Some(sync_into(&t, "A")));
    assert!(matches!(refused, Err(SetupError::NoSecureStore { .. })), "{refused:?}");

    // Asked each time: kept in memory only.
    setup::set_up(&t.library, &stores, &open, smb(PasswordStore::Ask), Some(RIGHT), Some(sync_into(&t, "B"))).unwrap();
    assert!(setup::current(&t.library, &stores, &open).is_ok());
    // After a restart (memory empty): the password is needed again.
    let restarted = MemorySecrets::default();
    let after = PasswordStores { secure: &NoKeyring, memory: &restarted, file: &file };
    assert!(setup::needs_password(&t.library, &after).unwrap());
    assert!(matches!(setup::current(&t.library, &after, &open), Err(SetupError::NeedPassword)));
    assert!(matches!(setup::set_password(&t.library, &after, &open, "nope", None), Err(SetupError::Share { .. })));
    setup::set_password(&t.library, &after, &open, RIGHT, None).unwrap();
    assert!(setup::current(&t.library, &after, &open).is_ok());

    // Kept in a plain file instead, chosen knowingly: then it's there after a restart.
    setup::set_password(&t.library, &after, &open, RIGHT, Some(PasswordStore::File)).unwrap();
    assert_eq!(file.get(PASSWORD_KEY).unwrap().as_deref(), Some(RIGHT));
    assert_eq!(restarted.get(PASSWORD_KEY).unwrap(), None, "kept in one place only");
    let again = PasswordStores { secure: &NoKeyring, memory: &MemorySecrets::default(), file: &file };
    assert!(setup::current(&t.library, &again, &open).is_ok());
}

#[test]
fn a_network_drive_copied_or_played_straight_from() {
    let t = TestLibrary::new("setup-drive");
    let drive = t.music_folder("Z");
    drive.add("ABBA/01.wav", &Tags::song("Waterloo", "ABBA", "Waterloo", 1));
    let share = TestShare::new("setup-drive");
    let s = Stores::new();
    let open = opener(&share);
    let path = drive.root.to_string_lossy().into_owned();

    // Copied: like the SMB share, into a sync folder; no password anywhere.
    let copied = setup::set_up(&t.library, &s.stores(), &open, ShareSetup::Drive { path: path.clone(), stream: false }, None, Some(sync_into(&t, "Copies"))).unwrap();
    assert_ne!(t.library.folder(copied.sync_folder_id).unwrap().unwrap().location, path);
    assert_eq!(s.secure.get(PASSWORD_KEY).unwrap(), None);

    // Played straight from: the drive is the synced library folder; the copies folder is a normal one now.
    let streamed = setup::set_up(&t.library, &s.stores(), &open, ShareSetup::Drive { path: path.clone(), stream: true }, None, None).unwrap();
    let folder = t.library.folder(streamed.sync_folder_id).unwrap().unwrap();
    assert!(folder.synced && folder.name == "Share");
    assert!(!t.library.folder(copied.sync_folder_id).unwrap().unwrap().synced);
    t.scan(folder.id, &drive);
    assert_eq!(t.library.status().unwrap().tracks, 1, "its songs are in the library, no copies made");
}

#[test]
fn a_sync_folder_already_in_the_library_is_marked_not_added_twice() {
    let t = TestLibrary::new("setup-existing");
    let share = TestShare::new("setup-existing");
    let s = Stores::new();
    let choice = sync_into(&t, "Melue");
    let existing = t.library.add_folder(FolderKind::Path, &choice.location, None).unwrap();
    let settings = setup::set_up(&t.library, &s.stores(), &opener(&share), smb(PasswordStore::Secure), Some(RIGHT), Some(choice)).unwrap();
    assert_eq!(settings.sync_folder_id, existing.id);
    assert_eq!(t.library.folders().unwrap().len(), 1);
}

#[test]
fn removing_the_share_keeps_the_folder_and_its_songs() {
    let t = TestLibrary::new("setup-remove");
    let share = TestShare::new("setup-remove");
    let s = Stores::new();
    let music = t.music_folder("Synced");
    music.add("a.wav", &Tags::song("A", "X", "Y", 1));
    let choice = SyncFolderChoice { kind: FolderKind::Path, location: music.root.to_string_lossy().into_owned() };
    let settings = setup::set_up(&t.library, &s.stores(), &opener(&share), smb(PasswordStore::Secure), Some(RIGHT), Some(choice)).unwrap();
    t.scan(settings.sync_folder_id, &music);

    setup::remove(&t.library, &s.stores()).unwrap();
    assert!(!t.library.sync_available().unwrap());
    assert_eq!(t.library.share_settings().unwrap(), None);
    assert_eq!(s.secure.get(PASSWORD_KEY).unwrap(), None);
    let folder = t.library.folder(settings.sync_folder_id).unwrap().unwrap();
    assert!(!folder.synced, "a normal folder now");
    assert_eq!(t.library.status().unwrap().tracks, 1, "its songs stay");
    assert!(matches!(setup::current(&t.library, &s.stores(), &opener(&share)), Err(SetupError::NotSetUp)));
}

#[test]
fn a_synced_folder_inside_a_library_folder_is_skipped_by_the_outer_one() {
    let t = TestLibrary::new("setup-nested");
    let music = t.music_folder("Music");
    music.add("Mine/01.wav", &Tags::song("Mine", "X", "Y", 1));
    music.add("Melue/ABBA/01.wav", &Tags::song("Synced", "ABBA", "Waterloo", 1));
    let outer = t.library.add_folder(FolderKind::Path, music.root.to_str().unwrap(), None).unwrap();
    let inner_path = music.path("Melue");
    // A normal folder inside another is still refused; the synced one is allowed.
    assert!(t.library.add_folder(FolderKind::Path, inner_path.to_str().unwrap(), None).is_err());
    let inner = t.library.add_synced_folder(FolderKind::Path, inner_path.to_str().unwrap(), None).unwrap();

    t.scan(outer.id, &music);
    let synced = crate::library::testkit::MusicFolder { root: inner_path };
    t.scan(inner.id, &synced);
    let counts: Vec<(String, i64)> = t.library.folders().unwrap().into_iter().map(|f| (f.name, f.tracks)).collect();
    assert_eq!(counts, vec![("Music".to_string(), 1), ("Melue".to_string(), 1)], "each song once");
}

#[test]
fn all_music_on_the_phone_skips_the_synced_folder() {
    use crate::library::scan::{FoundFile, Listing};
    let t = TestLibrary::new("setup-android");
    let media = t.library.add_folder(FolderKind::AndroidMedia, "", None).unwrap();
    // The synced folder is a chosen folder: allowed next to all music on the phone.
    let tree = "content://com.android.externalstorage.documents/tree/primary%3AMusic%2FMelue";
    t.library.add_synced_folder(FolderKind::AndroidTree, tree, None).unwrap();
    let file = |path: &str| FoundFile { path: path.into(), name: "x.mp3".into(), size: 1, modified_at: 1, folder_cover: None };
    let listing = Listing {
        songs: vec![file("/storage/emulated/0/Music/Mine/x.mp3"), file("/storage/emulated/0/Music/Melue/ABBA/x.mp3"), file("/storage/emulated/0/Music/MelueOther/x.mp3")],
        playlists: vec![],
    };
    let kept: Vec<String> = t.library.skip_nested(&media, listing).unwrap().songs.into_iter().map(|f| f.path).collect();
    assert_eq!(kept, vec!["/storage/emulated/0/Music/Mine/x.mp3", "/storage/emulated/0/Music/MelueOther/x.mp3"]);
    assert_eq!(crate::library::folders::android_path("1234-ABCD:Music"), "/storage/1234-ABCD/Music");
}
