//! The share's layout (layout.rs): where songs go, and reorganising.

use std::collections::HashMap;

use super::layout::*;
use crate::library::sync::SHARE_SETTING;
use crate::library::testkit::{Tags, TestLibrary};
use crate::share::folder::FolderShare;
use crate::share::testkit::TestShare;
use crate::share::Share;

#[test]
fn where_songs_go_in_each_layout() {
    let place = |layout| layout_path(layout, None, Some("Big Mountain"), Some("Unriddle Me This"), "Baby, I Love Your Way.mp3");
    assert_eq!(place(Layout::ArtistAlbum), "Big Mountain/Unriddle Me This/Baby, I Love Your Way.mp3");
    assert_eq!(place(Layout::OneFolder), "Baby, I Love Your Way.mp3");
    assert_eq!(layout_path(Layout::OneFolder, None, None, None, "What?.mp3"), "What_.mp3");
}

#[test]
fn a_song_with_a_number_added_is_in_place() {
    assert!(in_place("Song.mp3", "song.mp3"), "case");
    assert!(in_place("A/01 Intro (2).mp3", "A/01 Intro.mp3"), "a number Melue added");
    assert!(!in_place("B/01 Intro (2).mp3", "A/01 Intro.mp3"), "another folder");
    assert!(!in_place("A/01 Intro (live).mp3", "A/01 Intro.mp3"));
    assert!(!in_place("A/01 Intro.flac", "A/01 Intro.mp3"));
}

#[test]
fn playlist_files_follow_the_moves() {
    let moved: HashMap<String, String> =
        [("artist/album/a.mp3", "a.mp3"), ("b.mp3", "Band/Album/b.mp3")].into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    // A playlist at the top: paths relative to it.
    let text = "#EXTM3U\r\n#EXTINF:1,A\r\nArtist/Album/a.mp3\r\nb.mp3\r\nc.mp3\r\n";
    assert_eq!(update_playlist_text(text, "Mix.m3u", &moved).unwrap(), "#EXTM3U\r\n#EXTINF:1,A\r\na.mp3\r\nBand/Album/b.mp3\r\nc.mp3\r\n");
    // In a folder, with Windows paths: "..", and backslashes kept.
    let text = "..\\Artist\\Album\\a.mp3\n";
    assert_eq!(update_playlist_text(text, "Lists/Mix.m3u", &moved).unwrap(), "..\\a.mp3\n");
    // Nothing moved in it: left alone.
    assert_eq!(update_playlist_text("c.mp3\nhttp://radio\nC:\\x.mp3\n", "Mix.m3u", &moved), None);
}

#[test]
fn reorganising_moves_songs_on_the_share_and_here_with_their_playlists() {
    let t = TestLibrary::new("layout-reorganise");
    let music = t.music_folder("Synced");
    let share = TestShare::new("layout-reorganise");
    // The same songs on the share and in the synced folder here.
    for (path, tags) in [
        ("Big Mountain/Unriddle/Baby.wav", Tags::song("Baby", "Big Mountain", "Unriddle", 1)),
        ("Flat.wav", Tags::song("Flat", "Band", "", 1)),
    ] {
        music.add(path, &tags);
        share.add(path, &std::fs::read(music.path(path)).unwrap());
    }
    share.add("Mix.m3u", b"Big Mountain/Unriddle/Baby.wav\nFlat.wav\n");
    share.add(".melue/playlists/p1.json", br#"{"id":"p1","name":"Mix","entries":[{"key":"Big Mountain/Unriddle/Baby.wav"}]}"#);
    // One more only here (not on the share): the layout leaves it alone.
    music.add("Only here.wav", &Tags::song("Only here", "Someone", "Mine", 1));
    let folder = t.add_folder(&music);
    t.scan(folder.id, &music);
    t.library.set_setting(SHARE_SETTING, &format!(r#"{{"kind":"drive","path":"Z:","stream":false,"syncFolderId":{}}}"#, folder.id)).unwrap();
    // The sync knows the shared ones (they came from the share).
    let here = FolderShare::new(&music.root);
    for path in ["Big Mountain/Unriddle/Baby.wav", "Flat.wav"] {
        t.library.record_synced(&share.share.stat(path).unwrap().unwrap(), &here.stat(path).unwrap().unwrap()).unwrap();
    }

    let moves = t.library.layout_moves(&folder, Layout::OneFolder).unwrap();
    assert_eq!(moves.iter().map(|m| (m.from.as_str(), m.to.as_str())).collect::<Vec<_>>(), vec![("Big Mountain/Unriddle/Baby.wav", "Baby.wav")]);
    assert_eq!(t.library.layout_moves(&folder, Layout::ArtistAlbum).unwrap().len(), 1, "Flat.wav goes to Band/; Only here stays");

    let result = reorganise_files(&t.library, &share.share, Some(&here as &dyn Share), &moves, &mut |_| {}, &|| false).unwrap();
    assert_eq!((result.moved, result.playlists_updated, result.failed.len()), (1, 2, 0));
    assert!(share.exists("Baby.wav") && music.path("Baby.wav").exists(), "moved on the share and here");
    assert!(!share.exists("Big Mountain") && !music.path("Big Mountain").exists(), "empty folders gone");
    assert_eq!(std::fs::read_to_string(share.path("Mix.m3u")).unwrap(), "Baby.wav\nFlat.wav\n");
    assert!(std::fs::read_to_string(share.path(".melue/playlists/p1.json")).unwrap().contains(r#""key":"Baby.wav""#));
    // The sync knows it's here: nothing to download.
    let records: Vec<String> = t.library.sync_records().unwrap().into_iter().map(|r| r.path).collect();
    assert!(records.contains(&"Baby.wav".to_string()) && !records.iter().any(|p| p.starts_with("Big Mountain")));
}

#[test]
fn a_song_gone_from_the_share_is_left_for_the_sync_not_failed() {
    let t = TestLibrary::new("layout-gone");
    let music = t.music_folder("Synced");
    let share = TestShare::new("layout-gone");
    // Synced from the share, then taken off it by another device; the copy here stays.
    music.add("Melue test/Test song.wav", &Tags::song("Test song", "", "", 1));
    let folder = t.add_folder(&music);
    t.scan(folder.id, &music);
    t.library.set_setting(SHARE_SETTING, &format!(r#"{{"kind":"drive","path":"Z:","stream":false,"syncFolderId":{}}}"#, folder.id)).unwrap();
    // The sync knew it (it came from the share).
    let here = FolderShare::new(&music.root);
    let entry = here.stat("Melue test/Test song.wav").unwrap().unwrap();
    t.library.record_synced(&entry, &entry).unwrap();
    let moves = t.library.layout_moves(&folder, Layout::OneFolder).unwrap();
    assert_eq!(moves.len(), 1);
    let result = reorganise_files(&t.library, &share.share, Some(&here as &dyn Share), &moves, &mut |_| {}, &|| false).unwrap();
    assert_eq!((result.moved, result.failed.len(), result.gone.as_slice()), (0, 0, ["Melue test/Test song.wav".to_string()].as_slice()));
    assert!(music.path("Melue test/Test song.wav").exists(), "the copy here is the sync's to remove (or keep)");
}

#[test]
fn every_device_uses_the_layout_on_the_share() {
    let share = TestShare::new("layout-shared");
    let (desktop, phone) = (TestLibrary::new("layout-shared-desktop"), TestLibrary::new("layout-shared-phone"));
    // The phone never chose: its default isn't written to the share.
    assert_eq!(agree(&phone.library, &share.share).unwrap().layout, Layout::ArtistAlbum);
    assert!(!share.exists(SHARE_FILE));
    // The desktop chose one folder: it goes to the share, and the phone takes it.
    let mut settings = desktop.library.sync_settings().unwrap();
    settings.share_layout = Layout::OneFolder;
    desktop.library.set_sync_settings(&settings).unwrap();
    desktop.library.layout_changed_here().unwrap();
    assert_eq!(agree(&desktop.library, &share.share).unwrap(), ShareFile { layout: Layout::OneFolder, applied: None });
    assert_eq!(agree(&phone.library, &share.share).unwrap().layout, Layout::OneFolder);
    assert_eq!(phone.library.sync_settings().unwrap().share_layout, Layout::OneFolder, "kept on the phone too");
    // Changed on the phone later: the desktop follows.
    let mut settings = phone.library.sync_settings().unwrap();
    settings.share_layout = Layout::ArtistAlbum;
    phone.library.set_sync_settings(&settings).unwrap();
    phone.library.layout_changed_here().unwrap();
    agree(&phone.library, &share.share).unwrap();
    assert_eq!(agree(&desktop.library, &share.share).unwrap().layout, Layout::ArtistAlbum);
}
