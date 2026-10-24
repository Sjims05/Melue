//! Synced playback's rules (handoff.rs).

use std::collections::HashMap;

use super::handoff::*;
use crate::library::{Folder, FolderKind};

fn folder(kind: FolderKind, location: &str) -> Folder {
    Folder { id: 1, kind, location: location.into(), name: "Melue".into(), enabled: true, synced: true, added_at: 0, last_scan_at: None, network: false, tracks: 0 }
}

fn playback(queue: &[&str], index: usize) -> Playback {
    Playback { queue: queue.iter().map(|s| s.to_string()).collect(), index, repeat: "off".into(), ..Default::default() }
}

/// A device's file, written at `share_ms` on the share's clock (its own clock the same).
fn file(device: &str, share_ms: i64, queue: &[&str]) -> PlaybackFile {
    PlaybackFile {
        device_id: device.into(),
        device_name: device.into(),
        saved_at_ms: share_ms,
        share_ms,
        state: playback(queue, 0),
        title: None,
        artist: None,
    }
}

#[test]
fn playback_files_are_told_from_music() {
    assert!(is_playback_file(".melue/playback/abc.json"));
    assert!(!is_playback_file(".melue/playback/abc.json.melue-part") && !is_playback_file(".melue/playback"));
    assert!(!is_playback_file("Artist/Album/song.mp3") && !is_playback_file(".melue/other/abc.json"));
}

#[test]
fn songs_have_their_path_on_the_share() {
    let desktop = folder(FolderKind::Path, r"C:\Users\me\Music\Melue");
    assert_eq!(share_path(&desktop, r"Artist\Album\01 Song.mp3").as_deref(), Some("Artist/Album/01 Song.mp3"));
    // A chosen Android folder: content:// addresses, inside the folder's.
    let phone = folder(FolderKind::AndroidTree, "content://com.android.externalstorage.documents/tree/primary%3AMusic%2FMelue");
    let song = "content://com.android.externalstorage.documents/tree/primary%3AMusic%2FMelue/document/primary%3AMusic%2FMelue%2FArtist%2F01%20Song.mp3";
    assert_eq!(share_path(&phone, song).as_deref(), Some("Artist/01 Song.mp3"));
    let elsewhere = "content://com.android.externalstorage.documents/tree/primary%3AMusic/document/primary%3AMusic%2FOther%2Fa.mp3";
    assert_eq!(share_path(&phone, elsewhere), None);
    assert_eq!(share_path(&folder(FolderKind::AndroidMedia, ""), "content://media/1"), None);
}

#[test]
fn only_songs_on_the_share_are_saved() {
    let paths: HashMap<i64, String> = [(1, "a.mp3"), (3, "c.mp3"), (4, "d.mp3")].into_iter().map(|(i, p)| (i, p.to_string())).collect();
    let queue = [Some(1), Some(2), None, Some(3), Some(4)];
    // Song 2 and the item without an id aren't on the share: left out, and the place moves with it.
    assert_eq!(on_share(&queue, 3, &paths), Some((vec!["a.mp3".into(), "c.mp3".into(), "d.mp3".into()], 1)));
    assert_eq!(on_share(&queue, 0, &paths).map(|(_, i)| i), Some(0));
    // The song itself only here: nothing is saved.
    assert_eq!(on_share(&queue, 1, &paths), None);
    assert_eq!(on_share(&queue, 9, &paths), None);
}

#[test]
fn another_devices_queue_has_the_songs_here() {
    let songs: HashMap<String, i64> = [("a.mp3", 10), ("artist/c.mp3", 30)].into_iter().map(|(p, i)| (p.to_string(), i)).collect();
    // Paths compare without case; songs not here (not synced yet) are left out.
    assert_eq!(here(&playback(&["a.mp3", "b.mp3", "Artist/C.mp3"], 2), &songs), Some((vec![10, 30], 1)));
    assert_eq!(here(&playback(&["a.mp3", "b.mp3"], 1), &songs), None, "the song isn't here");
    assert_eq!(here(&playback(&[], 0), &songs), None);
}

#[test]
fn what_gets_saved() {
    let paused = Playback { position_s: 10.0, ..playback(&["a", "b"], 0) };
    let last = Some((paused.clone(), 1_000));
    assert!(needs_saving(None, &paused, 0), "nothing saved yet");
    assert!(!needs_saving(last.as_ref(), &paused, 100_000), "paused, unchanged");
    assert!(!needs_saving(last.as_ref(), &Playback { position_s: 11.5, ..paused.clone() }, 2_000), "a small move");
    assert!(needs_saving(last.as_ref(), &Playback { position_s: 40.0, ..paused.clone() }, 2_000), "moved while paused");
    assert!(needs_saving(last.as_ref(), &Playback { index: 1, ..paused.clone() }, 2_000), "another song");
    assert!(needs_saving(last.as_ref(), &Playback { shuffle: true, ..paused.clone() }, 2_000));
    assert!(needs_saving(last.as_ref(), &Playback { repeat: "one".into(), ..paused.clone() }, 2_000));
    // Playing: on play, then every 30 seconds.
    let playing = Playback { playing: true, ..paused.clone() };
    assert!(needs_saving(last.as_ref(), &playing, 2_000), "play");
    let last = Some((playing.clone(), 1_000));
    assert!(!needs_saving(last.as_ref(), &Playback { position_s: 25.0, ..playing.clone() }, 1_000 + SAVE_PLAYING_MS - 1));
    assert!(needs_saving(last.as_ref(), &Playback { position_s: 40.0, ..playing.clone() }, 1_000 + SAVE_PLAYING_MS));
    assert!(needs_saving(last.as_ref(), &paused, 2_000), "pause");
}

#[test]
fn the_newest_other_device_is_offered() {
    let files = || vec![file("me", 400, &["a"]), file("phone", 500, &["a"]), file("laptop", 700, &["b"]), file("empty", 800, &[])];
    // Mine and empty ones don't count.
    assert_eq!(newest_other(files(), "me", 0).map(|f| f.device_id), Some("laptop".into()));
    // Only when newer than the last one answered...
    assert_eq!(newest_other(files(), "me", 600).map(|f| f.device_id), Some("laptop".into()));
    assert_eq!(newest_other(files(), "me", 700), None);
    // ...and than what was played here.
    let mut played_here = files();
    played_here[0].share_ms = 750;
    assert_eq!(newest_other(played_here, "me", 0), None);
    assert_eq!(newest_other(Vec::new(), "me", 0), None);
}

#[test]
fn the_shares_clock_decides_not_the_devices() {
    // The desktop's clock is 40 s ahead: its playback looks newer by its own clock, but the share wrote the
    // phone's later.
    let desktop = PlaybackFile { saved_at_ms: 1_040_000, share_ms: 1_000_000, ..file("desktop", 0, &["a"]) };
    let phone = PlaybackFile { saved_at_ms: 1_010_000, share_ms: 1_010_000, ..file("phone", 0, &["b"]) };
    assert_eq!(newest_other(vec![desktop.clone(), phone.clone()], "desktop", 0).map(|f| f.device_id), Some("phone".into()));
    assert_eq!(newest_other(vec![desktop.clone(), phone.clone()], "phone", 0), None, "the phone's is newer");
    // The desktop's "now" on the share's clock: its own clock minus 40 s.
    let files = vec![desktop, phone];
    assert_eq!(share_clock_ahead(&files, "desktop"), -40_000);
    assert_eq!(share_clock_ahead(&files, "phone"), 0);
    assert_eq!(share_clock_ahead(&files, "new device"), 0, "nothing saved yet");
}

#[test]
fn playback_files_are_kept_as_json() {
    let f = PlaybackFile { title: Some("Song".into()), artist: Some("Band".into()), ..file("phone", 5, &["Band/Song.mp3"]) };
    let json = serde_json::to_value(&f).unwrap();
    assert_eq!(json["deviceName"], "phone");
    assert_eq!(json["queue"][0], "Band/Song.mp3", "the playback's fields are at the top");
    assert!(json.get("shareMs").is_none(), "the share's time comes from the share, not the file");
    assert_eq!(serde_json::from_value::<PlaybackFile>(json).unwrap(), PlaybackFile { share_ms: 0, ..f });
}

#[test]
fn the_default_settings_ask() {
    let s: PlaybackSettings = serde_json::from_str("{}").unwrap();
    assert_eq!((s.mode, s.device_name), (Mode::Ask, None));
    let s: PlaybackSettings = serde_json::from_str(r#"{"mode":"follow","deviceName":"Kitchen"}"#).unwrap();
    assert_eq!((s.mode, s.device_name.as_deref()), (Mode::Follow, Some("Kitchen")));
}

#[test]
fn recent_playback_continues_without_asking() {
    const MINUTE: i64 = 60_000;
    let ask = PlaybackSettings::default();
    assert_eq!(ask.continue_within_minutes, Some(10), "the default");
    // Ask: within 10 minutes it continues by itself (a switch between devices), older ones are asked about.
    assert!(continues_by_itself(&ask, 0, 10 * MINUTE));
    assert!(!continues_by_itself(&ask, 0, 10 * MINUTE + 1));
    let always_ask = PlaybackSettings { continue_within_minutes: None, ..ask.clone() };
    assert!(!continues_by_itself(&always_ask, 0, 0));
    // Follow: always; off: never.
    assert!(continues_by_itself(&PlaybackSettings { mode: Mode::Follow, ..always_ask.clone() }, 0, 1000 * MINUTE));
    assert!(!continues_by_itself(&PlaybackSettings { mode: Mode::Off, ..ask }, 0, 0));
}

#[test]
fn another_devices_song_keeps_its_place_when_one_before_it_cant_play_here() {
    use crate::library::testkit::{Tags, TestLibrary};
    let t = TestLibrary::new("handoff-place");
    let music = t.music_folder("Synced");
    for name in ["A", "B", "C"] {
        music.add(&format!("{name}.wav"), &Tags::song(name, "X", "Y", 1));
    }
    let synced = t.library.add_synced_folder(FolderKind::Path, music.root.to_str().unwrap(), None).unwrap();
    t.scan(synced.id, &music);
    t.on_share(synced.id);
    // B is here, but hidden as a copy: it can't be in the queue.
    t.library.conn().execute("UPDATE tracks SET copy_of = (SELECT id FROM tracks WHERE path = 'A.wav') WHERE path = 'B.wav'", []).unwrap();
    let (items, index) = queue_here(&t.library, &synced, &playback(&["A.wav", "B.wav", "C.wav"], 2)).unwrap().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!((index, items[index].title.as_deref()), (1, Some("C")), "C, the song the other device plays");
}

#[test]
fn a_song_put_in_the_synced_folder_by_hand_isnt_on_the_share() {
    use crate::library::testkit::{Tags, TestLibrary};
    let t = TestLibrary::new("handoff-by-hand");
    let music = t.music_folder("Synced");
    music.add("Synced.wav", &Tags::song("Synced", "X", "Y", 1));
    let synced = t.library.add_synced_folder(FolderKind::Path, music.root.to_str().unwrap(), None).unwrap();
    t.scan(synced.id, &music);
    t.on_share(synced.id);
    // Copied in by hand afterwards: no sync brought it, so the other devices haven't got it.
    music.add("By hand.wav", &Tags::song("By hand", "X", "Y", 2));
    t.scan(synced.id, &music);
    let paths: Vec<String> = t.library.synced_songs(&synced).unwrap().into_iter().map(|(_, p)| p).collect();
    assert_eq!(paths, ["Synced.wav"]);
}

#[test]
fn a_song_only_on_that_device_goes_out_as_the_one_after_it() {
    let paths: HashMap<i64, String> = [(1, "a.mp3"), (3, "c.mp3"), (4, "d.mp3")].into_iter().map(|(id, p)| (id, p.to_string())).collect();
    let queue = [Some(1), Some(2), Some(3), Some(4)];
    // Song 2 isn't on the share: the queue's songs that are, from c (the one after it).
    assert_eq!(next_on_share(&queue, 1, &paths), Some((vec!["a.mp3".into(), "c.mp3".into(), "d.mp3".into()], 1)));
    // Nothing on the share after it: nothing to continue.
    assert_eq!(next_on_share(&[Some(1), Some(2)], 1, &paths), None);
}
