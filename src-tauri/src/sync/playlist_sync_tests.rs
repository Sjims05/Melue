//! Library data across devices, end to end: two devices (each its own library and synced folder) syncing
//! likes, plays and playlists through one share.

use super::data::sync_device_data;
use super::handoff;
use super::playlist_merge::ConflictKind;
use super::playlist_sync::{answer_conflict, sync_playlists, Choice};
use crate::library::playback::PlaybackEvent;
use crate::library::sync::SHARE_SETTING;
use crate::library::testkit::{MusicFolder, Tags, TestLibrary};
use crate::library::{Folder, Library};
use crate::share::testkit::TestShare;

struct Device {
    t: TestLibrary,
    music: MusicFolder,
    folder: Folder,
    id: String,
    name: String,
}

impl Device {
    /// A device whose synced folder has these songs (file names; the title is the name).
    fn new(name: &str, songs: &[&str]) -> Self {
        let t = TestLibrary::new(&format!("devices-{name}"));
        let music = t.music_folder("Synced");
        for (n, song) in songs.iter().enumerate() {
            music.add(&format!("{song}.wav"), &Tags::song(song, "Band", "", n as u32 + 1));
        }
        let folder = t.add_folder(&music);
        t.scan(folder.id, &music);
        let setting = format!(r#"{{"kind":"drive","path":"Z:\\","stream":true,"syncFolderId":{}}}"#, folder.id);
        t.library.set_setting(SHARE_SETTING, &setting).unwrap();
        let id = handoff::device_id(&t.library).unwrap();
        Self { t, music, folder, id, name: name.into() }
    }

    fn lib(&self) -> &Library {
        &self.t.library
    }

    /// A sync of the library data and the playlists, as data.rs runs it.
    fn sync(&self, share: &TestShare) -> super::playlist_sync::PlaylistResult {
        let (_, ahead) = sync_device_data(self.lib(), &share.share, &self.folder, &self.id, &self.name).unwrap();
        sync_playlists(self.lib(), &share.share, &self.folder, &self.id, &self.name, ahead).unwrap()
    }

    fn track(&self, song: &str) -> i64 {
        self.lib().conn().query_row("SELECT id FROM tracks WHERE title = ?1", [song], |r| r.get(0)).unwrap()
    }

    fn liked(&self, song: &str) -> bool {
        self.lib().liked_ids().unwrap().contains(&self.track(song))
    }

    /// A playlist here by name: its songs (titles; greyed ones as "(title)").
    fn playlist(&self, name: &str) -> Option<Vec<String>> {
        let p = self.lib().playlists().unwrap().into_iter().find(|p| p.name == name)?;
        let entries = self.lib().playlist_entries(p.id).unwrap();
        Some(
            entries
                .into_iter()
                .map(|e| match e.track {
                    Some(t) => t.title,
                    None => format!("({})", e.sync_title.unwrap_or_default()),
                })
                .collect(),
        )
    }

    fn playlist_id(&self, name: &str) -> i64 {
        self.lib().playlists().unwrap().into_iter().find(|p| p.name == name).unwrap().id
    }

    fn make_playlist(&self, name: &str, songs: &[&str]) -> i64 {
        let id = self.lib().create_playlist(name).unwrap().id;
        let tracks: Vec<i64> = songs.iter().map(|s| self.track(s)).collect();
        self.lib().add_to_playlist(id, &tracks, None).unwrap();
        id
    }

    fn remove_from(&self, playlist: &str, song: &str) {
        let id = self.playlist_id(playlist);
        let entry = self.lib().playlist_entries(id).unwrap().into_iter().find(|e| e.track.as_ref().is_some_and(|t| t.title == song)).unwrap();
        self.lib().remove_from_playlist(id, &[entry.id]).unwrap();
    }

    fn play(&self, song: &str, n: u32) {
        let track = self.track(song);
        for i in 0..n {
            let at_ms = crate::library::now_ms() - 3_600_000 + i64::from(i) * 60_000;
            let event = |kind: &str, at_ms| PlaybackEvent { id: format!("{}-{song}-{i}-{kind}", self.name), kind: kind.into(), item_id: track, position_ms: 1_000, at_ms };
            self.lib().record_playback(&event("start", at_ms)).unwrap();
            self.lib().record_playback(&event("complete", at_ms + 1_000)).unwrap();
        }
    }
}

fn titles(lib: &Library, source: &str) -> Vec<String> {
    lib.queue_items(source).unwrap().into_iter().map(|i| i.title.unwrap()).collect()
}

#[test]
fn likes_go_both_ways_and_the_newest_change_wins() {
    let share = TestShare::new("devices-likes");
    let (desktop, phone) = (Device::new("desktop", &["One", "Two"]), Device::new("phone", &["One", "Two"]));
    desktop.lib().set_liked(desktop.track("One"), true).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert!(phone.liked("One") && !phone.liked("Two"), "liked on the desktop");

    // Unliked on the phone later: unliked on the desktop too.
    std::thread::sleep(std::time::Duration::from_millis(5));
    phone.lib().set_liked(phone.track("One"), false).unwrap();
    phone.sync(&share);
    desktop.sync(&share);
    assert!(!desktop.liked("One"));
    // And it stays that way (the desktop's older like doesn't come back).
    phone.sync(&share);
    assert!(!phone.liked("One"));
}

#[test]
fn plays_on_every_device_count_without_counting_twice() {
    let share = TestShare::new("devices-plays");
    let (desktop, phone) = (Device::new("desktop", &["One", "Two"]), Device::new("phone", &["One", "Two"]));
    desktop.play("One", 2);
    phone.play("Two", 3);
    phone.play("One", 2);
    for _ in 0..2 {
        desktop.sync(&share);
        phone.sync(&share);
    }
    // All devices together: Two 3 plays, One 4.
    for device in [&desktop, &phone] {
        assert_eq!(titles(device.lib(), "top:all-time"), vec!["One", "Two"], "{}", device.name);
        assert_eq!(titles(device.lib(), "top:month"), vec!["One", "Two"]);
        let one = device.lib().tracks(0, 10, Default::default()).unwrap().into_iter().find(|t| t.title == "One").unwrap();
        assert_eq!(one.play_count, 4, "{}", device.name);
        assert_eq!(device.lib().recently_played(10).unwrap().len(), 2, "the shared history");
    }
}

#[test]
fn playlists_changed_on_both_devices_are_merged() {
    let share = TestShare::new("devices-merge");
    let songs = ["A", "B", "C", "D"];
    let (desktop, phone) = (Device::new("desktop", &songs), Device::new("phone", &songs));
    desktop.make_playlist("Gym", &["B", "C"]);
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Gym").unwrap(), vec!["B", "C"], "made on the desktop");

    // While they can't sync: A added on the phone, B removed on the desktop.
    phone.lib().add_to_playlist(phone.playlist_id("Gym"), &[phone.track("A")], None).unwrap();
    desktop.remove_from("Gym", "B");
    desktop.sync(&share);
    phone.sync(&share);
    desktop.sync(&share);
    assert_eq!(phone.playlist("Gym").unwrap(), vec!["C", "A"]);
    assert_eq!(desktop.playlist("Gym").unwrap(), vec!["C", "A"]);
    assert!(phone.lib().playlist_conflicts().unwrap().is_empty());
}

#[test]
fn songs_a_device_doesnt_have_yet_wait_greyed() {
    let share = TestShare::new("devices-greyed");
    let desktop = Device::new("desktop", &["A", "New"]);
    let phone = Device::new("phone", &["A"]);
    desktop.make_playlist("Mix", &["A", "New"]);
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Mix").unwrap(), vec!["A", "(New)"], "greyed, at its place");
    assert_eq!(phone.lib().playlist(phone.playlist_id("Mix")).unwrap().not_here, 1);
    assert_eq!(titles(phone.lib(), &format!("playlist:{}", phone.playlist_id("Mix"))), vec!["A"], "only what's here plays");
    // Nothing changes back on the share: the desktop keeps both.
    desktop.sync(&share);
    assert_eq!(desktop.playlist("Mix").unwrap(), vec!["A", "New"]);

    // The song arrives on the phone: it fills in.
    phone.music.add("New.wav", &Tags::song("New", "Band", "", 2));
    phone.t.scan(phone.folder.id, &phone.music);
    phone.sync(&share);
    assert_eq!(phone.playlist("Mix").unwrap(), vec!["A", "New"]);
}

#[test]
fn playlists_made_on_both_devices_with_the_same_name_become_one() {
    let share = TestShare::new("devices-same-name");
    let songs = ["A", "B", "C"];
    let (desktop, phone) = (Device::new("desktop", &songs), Device::new("phone", &songs));
    desktop.make_playlist("Gym", &["A", "B"]);
    phone.make_playlist("Gym", &["B", "C"]);
    desktop.sync(&share);
    phone.sync(&share);
    desktop.sync(&share);
    for device in [&desktop, &phone] {
        assert_eq!(device.lib().playlists().unwrap().len(), 1, "{}", device.name);
    }
    assert_eq!(phone.playlist("Gym").unwrap(), vec!["B", "C", "A"]);
    assert_eq!(desktop.playlist("Gym").unwrap(), vec!["B", "C", "A"]);
}

#[test]
fn deleting_deletes_everywhere_and_can_be_restored() {
    let share = TestShare::new("devices-delete");
    let (desktop, phone) = (Device::new("desktop", &["A"]), Device::new("phone", &["A"]));
    desktop.make_playlist("Old", &["A"]);
    desktop.sync(&share);
    phone.sync(&share);
    desktop.lib().delete_playlist(desktop.playlist_id("Old")).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Old"), None, "deleted on the phone too");
    let deleted = phone.lib().deleted_playlists().unwrap();
    assert_eq!((deleted.len(), deleted[0].name.as_str(), deleted[0].songs), (1, "Old", 1));

    // Restored on the phone: back everywhere.
    phone.lib().restore_playlist(&deleted[0].sync_id).unwrap();
    phone.sync(&share);
    desktop.sync(&share);
    assert_eq!(desktop.playlist("Old").unwrap(), vec!["A"]);
    assert!(phone.lib().deleted_playlists().unwrap().is_empty());
}

#[test]
fn only_on_this_device_isnt_synced() {
    let share = TestShare::new("devices-local");
    let (desktop, phone) = (Device::new("desktop", &["A"]), Device::new("phone", &["A"]));
    let private = desktop.make_playlist("Private", &["A"]);
    desktop.lib().set_playlist_local_only(private, true).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Private"), None);
}

#[test]
fn changes_that_dont_merge_wait_for_a_choice() {
    let share = TestShare::new("devices-conflict");
    let (desktop, phone) = (Device::new("desktop", &["A", "B"]), Device::new("phone", &["A", "B"]));
    desktop.make_playlist("Gym", &["A", "B"]);
    desktop.sync(&share);
    phone.sync(&share);
    // Renamed differently on both.
    desktop.lib().rename_playlist(desktop.playlist_id("Gym"), "Workout").unwrap();
    phone.lib().rename_playlist(phone.playlist_id("Gym"), "Run").unwrap();
    phone.lib().add_to_playlist(phone.playlist_id("Run"), &[phone.track("A")], None).unwrap();
    desktop.sync(&share);
    let result = phone.sync(&share);
    assert!(result.conflicts_changed);
    let conflicts = phone.lib().playlist_conflicts().unwrap();
    assert_eq!((conflicts.len(), conflicts[0].kind, conflicts[0].device_name.as_str()), (1, ConflictKind::Changed, "desktop"));
    assert_eq!(conflicts[0].name, "Run");
    // Meanwhile it's left as it is here, and not overwritten there.
    phone.sync(&share);
    assert_eq!(phone.playlist("Run").unwrap(), vec!["A", "B", "A"]);

    // Keep both: the phone's becomes a copy, the desktop's version is the playlist.
    answer_conflict(phone.lib(), &share.share, &phone.folder, &phone.id, &phone.name, &conflicts[0].sync_id, Choice::Both).unwrap();
    assert_eq!(phone.playlist("Workout").unwrap(), vec!["A", "B"]);
    assert_eq!(phone.playlist("Run (phone)").unwrap(), vec!["A", "B", "A"]);
    assert!(phone.lib().playlist_conflicts().unwrap().is_empty());
    phone.sync(&share);
    desktop.sync(&share);
    assert_eq!(desktop.playlist("Run (phone)").unwrap(), vec!["A", "B", "A"], "the copy is synced too");
}

#[test]
fn deleted_on_one_device_while_changed_on_the_other_asks() {
    let share = TestShare::new("devices-deleted-changed");
    let (desktop, phone) = (Device::new("desktop", &["A", "B"]), Device::new("phone", &["A", "B"]));
    desktop.make_playlist("Mix", &["A"]);
    desktop.sync(&share);
    phone.sync(&share);
    desktop.lib().delete_playlist(desktop.playlist_id("Mix")).unwrap();
    phone.lib().add_to_playlist(phone.playlist_id("Mix"), &[phone.track("B")], None).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    let conflicts = phone.lib().playlist_conflicts().unwrap();
    assert_eq!(conflicts[0].kind, ConflictKind::DeletedThere);
    // Keep it, with the phone's changes: back on the desktop.
    answer_conflict(phone.lib(), &share.share, &phone.folder, &phone.id, &phone.name, &conflicts[0].sync_id, Choice::Mine).unwrap();
    desktop.sync(&share);
    assert_eq!(desktop.playlist("Mix").unwrap(), vec!["A", "B"]);
}

impl Device {
    /// Another music folder (not synced) with these songs: copies of the share's, as before the share was set up.
    fn other_folder(&self, songs: &[&str]) -> i64 {
        let music = self.t.music_folder("Other");
        for (n, song) in songs.iter().enumerate() {
            music.add(&format!("{song}.wav"), &Tags::song(song, "Band", "", n as u32 + 1));
        }
        let folder = self.t.add_folder(&music);
        self.t.scan(folder.id, &music);
        folder.id
    }

    fn track_in(&self, folder: i64, song: &str) -> i64 {
        self.lib()
            .conn()
            .query_row("SELECT id FROM tracks WHERE title = ?1 AND folder_id = ?2", rusqlite::params![song, folder], |r| r.get(0))
            .unwrap()
    }
}

#[test]
fn copies_of_the_shares_songs_count_as_the_same_song() {
    let share = TestShare::new("devices-copies");
    let desktop = Device::new("desktop", &["A", "B"]);
    let phone = Device::new("phone", &["A", "B"]);
    // The phone's playlist was made from its other copies (from before the share was set up).
    let other = phone.other_folder(&["A", "B"]);
    let mix = phone.lib().create_playlist("Mix").unwrap().id;
    phone.lib().add_to_playlist(mix, &[phone.track_in(other, "B"), phone.track_in(other, "A")], None).unwrap();
    phone.sync(&share);
    desktop.sync(&share);
    assert_eq!(desktop.playlist("Mix").unwrap(), vec!["B", "A"], "the desktop plays its own (the share's) songs");
    assert_eq!(desktop.lib().playlist(desktop.playlist_id("Mix")).unwrap().not_here, 0);
}

#[test]
fn a_device_plays_its_own_copy_of_a_song_it_doesnt_have_from_the_share() {
    let share = TestShare::new("devices-own-copy");
    let desktop = Device::new("desktop", &["A", "New"]);
    // The phone has "New" only in another folder, not (yet) from the share.
    let phone = Device::new("phone", &["A"]);
    let other = phone.other_folder(&["New"]);
    desktop.make_playlist("Mix", &["A", "New"]);
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Mix").unwrap(), vec!["A", "New"]);
    let entries = phone.lib().playlist_entries(phone.playlist_id("Mix")).unwrap();
    assert_eq!(entries[1].track.as_ref().unwrap().id, phone.track_in(other, "New"), "its own copy");
    // The devices don't keep "correcting" each other: nothing changes on later syncs.
    for _ in 0..2 {
        assert_eq!(phone.sync(&share).uploaded, 0);
        assert_eq!(desktop.sync(&share).uploaded, 0);
    }
    assert_eq!(desktop.playlist("Mix").unwrap(), vec!["A", "New"]);
}
