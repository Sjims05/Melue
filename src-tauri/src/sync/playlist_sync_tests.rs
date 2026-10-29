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
        t.on_share(folder.id);
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
    phone.t.on_share(phone.folder.id);
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
    desktop.lib().set_playlist_local_only(private, true, false).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Private"), None);
}

#[test]
fn only_on_this_device_can_take_it_off_the_others_and_syncing_again_brings_it_back() {
    let share = TestShare::new("devices-unshare");
    let (desktop, phone) = (Device::new("desktop", &["A"]), Device::new("phone", &["A"]));
    let mine = desktop.make_playlist("Mine", &["A"]);
    desktop.sync(&share);
    phone.sync(&share);
    assert!(phone.playlist("Mine").is_some());
    // Kept only on the desktop and taken off the others: gone on the phone, not in its Recently deleted.
    desktop.lib().set_playlist_local_only(mine, true, true).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Mine"), None);
    assert!(phone.lib().deleted_playlists().unwrap().is_empty());
    assert_eq!(desktop.playlist("Mine").unwrap(), vec!["A"]);
    // Synced again: back on the phone.
    desktop.lib().set_playlist_local_only(mine, false, false).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Mine").unwrap(), vec!["A"]);
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
        self.t.on_share(folder.id);
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

/// The covers' sync, as data.rs runs it after the playlists'.
fn sync_covers(device: &Device, share: &TestShare) -> Vec<i64> {
    let (_, ahead) = sync_device_data(device.lib(), &share.share, &device.folder, &device.id, &device.name).unwrap();
    super::cover_sync::sync_covers(device.lib(), &share.share, &device.folder, &device.id, ahead).unwrap()
}

fn custom_cover(device: &Device, playlist: &str) -> Option<String> {
    device.lib().playlist(device.playlist_id(playlist)).unwrap().custom_cover
}

#[test]
fn a_synced_playlists_cover_goes_to_the_other_devices_and_so_does_reverting_it() {
    let share = TestShare::new("devices-covers");
    let desktop = Device::new("desktop", &["A"]);
    let phone = Device::new("phone", &["A"]);
    let mix = desktop.make_playlist("Mix", &["A"]);
    desktop.sync(&share);
    phone.sync(&share);
    // Only on this device: its cover stays here.
    let mine = desktop.make_playlist("Mine", &["A"]);
    desktop.lib().set_playlist_local_only(mine, true, false).unwrap();

    let picture = std::fs::read(crate::library::testkit::sample("cover.png")).unwrap();
    desktop.lib().set_playlist_cover(mix, &picture).unwrap();
    desktop.lib().set_playlist_cover(mine, &picture).unwrap();
    assert!(sync_covers(&desktop, &share).is_empty());
    assert_eq!(sync_covers(&phone, &share), [phone.playlist_id("Mix")]);
    assert!(custom_cover(&phone, "Mix").is_some(), "the phone has the cover");
    assert!(phone.playlist("Mine").is_none());
    // Nothing more to do on either side.
    assert!(sync_covers(&phone, &share).is_empty());
    assert!(sync_covers(&desktop, &share).is_empty());

    // Reverted on the phone, later: the desktop goes back to the default too.
    std::thread::sleep(std::time::Duration::from_millis(5));
    phone.lib().clear_playlist_cover(phone.playlist_id("Mix")).unwrap();
    sync_covers(&phone, &share);
    assert_eq!(sync_covers(&desktop, &share), [mix]);
    assert!(custom_cover(&desktop, "Mix").is_none());
    assert!(custom_cover(&desktop, "Mine").is_some(), "the one only here keeps its own");
}

#[test]
fn a_linked_playlist_in_the_synced_folder_shares_its_cover_one_elsewhere_keeps_it() {
    let share = TestShare::new("devices-linked-covers");
    let desktop = Device::new("desktop", &["A"]);
    let phone = Device::new("phone", &["A"]);
    // The same playlist file in both synced folders (the share brought it), and one only on the desktop.
    for device in [&desktop, &phone] {
        // Marked as the synced folder, as setting up the share does.
        device.lib().set_folder_synced(device.folder.id, true).unwrap();
        device.music.add_text("Road.m3u", "A.wav\n");
        device.t.scan(device.folder.id, &device.music);
        device.t.on_share(device.folder.id);
    }
    let elsewhere = desktop.t.music_folder("Local");
    elsewhere.add_text("Here.m3u", "");
    let local = desktop.t.add_folder(&elsewhere);
    desktop.t.scan(local.id, &elsewhere);
    assert!(desktop.lib().playlist(desktop.playlist_id("Road")).unwrap().file_synced);
    assert!(!desktop.lib().playlist(desktop.playlist_id("Here")).unwrap().file_synced);

    let picture = std::fs::read(crate::library::testkit::sample("cover.png")).unwrap();
    desktop.lib().set_playlist_cover(desktop.playlist_id("Road"), &picture).unwrap();
    desktop.lib().set_playlist_cover(desktop.playlist_id("Here"), &picture).unwrap();
    sync_covers(&desktop, &share);
    assert_eq!(sync_covers(&phone, &share), [phone.playlist_id("Road")]);
    assert!(custom_cover(&phone, "Road").is_some());
    assert_eq!(share.all_files().iter().filter(|f| f.starts_with(".melue/covers/") && f.ends_with(".json")).count(), 1);
}

#[test]
fn an_empty_playlist_deleted_isnt_kept_for_restoring() {
    let share = TestShare::new("devices-empty-deleted");
    let desktop = Device::new("desktop", &["A"]);
    let empty = desktop.lib().create_playlist("Empty").unwrap().id;
    let full = desktop.make_playlist("Full", &["A"]);
    desktop.sync(&share);
    desktop.lib().delete_playlist(empty).unwrap();
    desktop.lib().delete_playlist(full).unwrap();
    let names: Vec<String> = desktop.lib().deleted_playlists().unwrap().into_iter().map(|d| d.name).collect();
    assert_eq!(names, ["Full"]);
}

#[test]
fn settings_go_across_devices_the_newest_change_winning_and_a_new_device_doesnt_push_its_defaults() {
    use serde_json::json;
    use std::collections::BTreeMap;
    let share = TestShare::new("devices-settings");
    let desktop = Device::new("desktop", &["A"]);
    let phone = Device::new("phone", &["A"]);
    let set = |d: &Device, id: &str, value: serde_json::Value, first: bool| {
        d.lib().record_settings(BTreeMap::from([(id.to_string(), value)]), first).unwrap();
    };
    let exchange = |d: &Device| super::settings_sync::sync_settings(d.lib(), &share.share).unwrap();

    set(&desktop, "display.upNextPrimary", json!(false), false);
    assert!(exchange(&desktop).is_empty());
    // The phone's first snapshot (its defaults) loses to the desktop's choice.
    set(&phone, "display.upNextPrimary", json!(true), true);
    assert_eq!(exchange(&phone), BTreeMap::from([("display.upNextPrimary".to_string(), json!(false))]));

    // Changed on the phone later: the desktop takes it; then nothing more to do.
    std::thread::sleep(std::time::Duration::from_millis(5));
    set(&phone, "display.upNextPrimary", json!(true), false);
    assert!(exchange(&phone).is_empty());
    assert_eq!(exchange(&desktop), BTreeMap::from([("display.upNextPrimary".to_string(), json!(true))]));
    assert!(exchange(&desktop).is_empty());
    assert!(exchange(&phone).is_empty());
    assert_eq!(desktop.lib().shared_settings_snapshot().unwrap()["display.upNextPrimary"], json!(true));
}

// ---- Spotify playlists: synced like Melue's own, with Spotify's list as their reference.

fn spotify(songs: &[&str]) -> crate::library::spotify::SpotifyList {
    let songs = songs.iter().map(|s| crate::library::spotify::SpotifySong { title: s.to_string(), artist: "Band".into(), duration_ms: Some(1000), track: Some(format!("id{s}")) });
    crate::library::spotify::SpotifyList { name: "Mix".into(), songs: songs.collect(), total: None, cover: None }
}

impl Device {
    fn add_spotify(&self, songs: &[&str]) -> i64 {
        let index = self.lib().track_index().unwrap();
        let mut list = spotify(songs);
        list.total = Some(songs.len());
        self.lib().add_spotify_list("mix", &list, &index).unwrap()
    }

    /// Spotify playlist "Mix" read again (as it is now there).
    fn read_spotify(&self, songs: &[&str]) {
        let index = self.lib().track_index().unwrap();
        let mut list = spotify(songs);
        list.total = Some(songs.len());
        self.lib().update_from_list(self.playlist_id("Mix"), None, &list, &index).unwrap();
    }

    /// A playlist's songs and their states ("A", or "A: removed_here"; not here: "(A)").
    fn states(&self, name: &str) -> Vec<String> {
        let entries = self.lib().playlist_entries(self.playlist_id(name)).unwrap();
        entries
            .into_iter()
            .map(|e| {
                let title = match e.track {
                    Some(t) => t.title,
                    None => format!("({})", e.sync_title.unwrap_or_default()),
                };
                if e.state == "both" { title } else { format!("{title}: {}", e.state) }
            })
            .collect()
    }

    fn follows_spotify(&self, name: &str) -> bool {
        self.lib().playlist(self.playlist_id(name)).unwrap().spotify.is_some()
    }
}

#[test]
fn a_spotify_playlist_syncs_with_its_reference_and_your_changes() {
    let share = TestShare::new("devices-spotify");
    let (desktop, phone) = (Device::new("desktop", &["A", "B"]), Device::new("phone", &["A", "B"]));
    desktop.add_spotify(&["A", "B", "Gone"]);
    desktop.sync(&share);
    phone.sync(&share);
    // On the phone: following Spotify, matched to the phone's songs (their ids on Spotify too, for links).
    assert!(phone.follows_spotify("Mix"));
    assert_eq!(phone.states("Mix"), ["A", "B", "(Gone)"]);
    let mut ids: Vec<String> = phone.lib().spotify_tracks(phone.playlist_id("Mix")).unwrap().into_values().collect();
    ids.sort();
    assert_eq!(ids, ["idA", "idB", "idGone"]);
    // Removed on the phone: greyed on the desktop (Restore), still followed.
    phone.lib().remove_from_playlist(phone.playlist_id("Mix"), &[phone.lib().playlist_entries(phone.playlist_id("Mix")).unwrap()[0].id]).unwrap();
    phone.sync(&share);
    desktop.sync(&share);
    assert_eq!(desktop.states("Mix"), ["A: removed_here", "B", "(Gone)"]);
    // A song added on Spotify, read by the desktop only: the phone takes it from the share, once.
    desktop.read_spotify(&["A", "B", "Gone", "C"]);
    desktop.sync(&share);
    phone.sync(&share);
    phone.read_spotify(&["A", "B", "Gone", "C"]);
    phone.sync(&share);
    assert_eq!(phone.states("Mix"), ["A: removed_here", "B", "(Gone)", "(C)"]);
}

#[test]
fn a_spotify_change_read_on_two_devices_comes_in_once() {
    let share = TestShare::new("devices-spotify-both");
    let (desktop, phone) = (Device::new("desktop", &["A", "B"]), Device::new("phone", &["A", "B"]));
    desktop.add_spotify(&["A"]);
    desktop.sync(&share);
    phone.sync(&share);
    // Both read the new song before either synced.
    desktop.read_spotify(&["A", "B"]);
    phone.read_spotify(&["A", "B"]);
    desktop.sync(&share);
    phone.sync(&share);
    desktop.sync(&share);
    assert_eq!(phone.states("Mix"), ["A", "B"]);
    assert_eq!(desktop.states("Mix"), ["A", "B"]);
}

#[test]
fn stopping_following_spotify_stops_it_everywhere_and_keeps_the_songs() {
    let share = TestShare::new("devices-spotify-stop");
    let (desktop, phone) = (Device::new("desktop", &["A"]), Device::new("phone", &["A"]));
    desktop.add_spotify(&["A", "Gone"]);
    desktop.sync(&share);
    phone.sync(&share);
    desktop.lib().unlink_playlist(desktop.playlist_id("Mix")).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert!(!phone.follows_spotify("Mix"));
    assert_eq!(phone.states("Mix"), ["A", "(Gone)"]);
    assert_eq!(desktop.states("Mix"), ["A", "(Gone)"]);
}

#[test]
fn a_spotify_playlist_can_be_kept_only_on_one_device() {
    let share = TestShare::new("devices-spotify-local");
    let (desktop, phone) = (Device::new("desktop", &["A"]), Device::new("phone", &["A"]));
    let mix = desktop.add_spotify(&["A"]);
    desktop.lib().set_playlist_local_only(mix, true, false).unwrap();
    desktop.sync(&share);
    phone.sync(&share);
    assert_eq!(phone.playlist("Mix"), None);
}

#[test]
fn songs_that_stay_keep_their_entries_when_another_device_changes_the_playlist() {
    let share = TestShare::new("devices-keep-ids");
    let (desktop, phone) = (Device::new("desktop", &["A", "B", "C"]), Device::new("phone", &["A", "B", "C"]));
    desktop.make_playlist("Gym", &["A", "B"]);
    desktop.sync(&share);
    phone.sync(&share);
    let ids = |d: &Device| d.lib().playlist_entries(d.playlist_id("Gym")).unwrap().into_iter().map(|e| e.id).collect::<Vec<_>>();
    let before = ids(&desktop);
    // Added and removed on the phone: on the desktop, A's entry stays, B's goes, C's is new.
    phone.lib().add_to_playlist(phone.playlist_id("Gym"), &[phone.track("C")], None).unwrap();
    phone.remove_from("Gym", "B");
    phone.sync(&share);
    desktop.sync(&share);
    assert_eq!(desktop.playlist("Gym").unwrap(), vec!["A", "C"]);
    let after = ids(&desktop);
    assert_eq!(after[0], before[0]);
    assert!(!before.contains(&after[1]));
}
