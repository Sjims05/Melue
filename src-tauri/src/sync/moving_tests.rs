//! Moving songs to sync (moving.rs): where they go, and copies of synced songs.

use super::moving::*;
use crate::library::playback::PlaybackEvent;
use crate::library::sync::SHARE_SETTING;
use crate::library::testkit::{Tags, TestLibrary};

#[test]
fn names_that_work_everywhere() {
    assert_eq!(clean_name("AC/DC"), "AC_DC");
    assert_eq!(clean_name("What? Why: \"Me\" <3 *"), "What_ Why_ _Me_ _3 _");
    assert_eq!(clean_name("Greatest Hits..."), "Greatest Hits");
    assert_eq!(clean_name("  "), "_");
    assert_eq!(clean_name(&"x".repeat(300)).len(), 120);
}

#[test]
fn songs_go_into_artist_and_album_folders() {
    assert_eq!(upload_path(Some("ABBA"), Some("ABBA feat. X"), Some("Gold"), "01 Dancing Queen.mp3"), "ABBA/Gold/01 Dancing Queen.mp3");
    assert_eq!(upload_path(None, Some("Adele"), Some("25"), "a.mp3"), "Adele/25/a.mp3", "no album artist: the artist");
    assert_eq!(upload_path(None, Some("Adele"), None, "a.mp3"), "Adele/a.mp3");
    assert_eq!(upload_path(Some(" "), None, Some("25"), "a.mp3"), "Uploads/a.mp3", "no artist");
    assert_eq!(upload_path(Some("AC/DC"), None, Some("Back in Black"), "x.mp3"), "AC_DC/Back in Black/x.mp3");
}

#[test]
fn a_taken_name_gets_a_number() {
    let taken = ["A/01 Intro.mp3", "A/01 Intro (2).mp3", "A/notes"];
    let free = |p: &str| free_path(p, |q| taken.contains(&q));
    assert_eq!(free("A/02 Song.mp3"), "A/02 Song.mp3");
    assert_eq!(free("A/01 Intro.mp3"), "A/01 Intro (3).mp3");
    assert_eq!(free("A/notes"), "A/notes (2)");
}

/// A library with a synced folder ("Synced") and another music folder ("Other").
fn setup(name: &str) -> (TestLibrary, crate::library::Folder, crate::library::testkit::MusicFolder, i64) {
    let t = TestLibrary::new(name);
    let synced_music = t.music_folder("Synced");
    let other_music = t.music_folder("Other");
    let synced = t.add_folder(&synced_music);
    let other = t.add_folder(&other_music).id;
    let setting = format!(r#"{{"kind":"drive","path":"Z:\\","stream":true,"syncFolderId":{}}}"#, synced.id);
    t.library.set_setting(SHARE_SETTING, &setting).unwrap();
    synced_music.add("Band/A.wav", &Tags::song("A", "Band", "", 1));
    other_music.add("A.wav", &Tags::song("A", "Band", "", 1));
    other_music.add("B.wav", &Tags::song("B", "Band", "", 2));
    t.scan(synced.id, &synced_music);
    t.scan(other, &other_music);
    (t, synced, other_music, other)
}

fn track(t: &TestLibrary, folder: i64, title: &str) -> i64 {
    t.library
        .conn()
        .query_row("SELECT id FROM tracks WHERE folder_id = ?1 AND title = ?2", rusqlite::params![folder, title], |r| r.get(0))
        .unwrap()
}

#[test]
fn copies_of_synced_songs_are_hidden_with_their_likes_plays_and_playlists_carried_over() {
    let (t, synced, _, other) = setup("moving-copies");
    let (copy, original) = (track(&t, other, "A"), track(&t, synced.id, "A"));
    t.library.set_liked(copy, true).unwrap();
    for (kind, at) in [("start", 1_000), ("complete", 2_000)] {
        let event = PlaybackEvent { id: kind.into(), kind: kind.into(), item_id: copy, position_ms: 1_000, at_ms: at };
        t.library.record_playback(&event).unwrap();
    }
    let mix = t.library.create_playlist("Mix").unwrap().id;
    t.library.add_to_playlist(mix, &[copy], None).unwrap();
    let songs = t.library.songs_here(&synced, "me").unwrap();
    assert_eq!(t.library.not_on_share(&synced, &songs).unwrap().iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), vec!["B"]);

    let found = t.library.find_copies(&synced, &songs).unwrap();
    assert_eq!((found.hidden, found.remove.len()), (1, 0));
    // Shown once: the synced song, with the copy's like, play and playlist entry.
    let titles: Vec<String> = t.library.tracks(0, 10, Default::default()).unwrap().into_iter().map(|s| s.title).collect();
    assert_eq!(titles, vec!["A", "B"]);
    assert!(t.library.liked_ids().unwrap().contains(&original));
    let a = t.library.tracks(0, 10, Default::default()).unwrap().into_iter().find(|s| s.title == "A").unwrap();
    assert_eq!((a.id, a.play_count), (original, 1));
    assert_eq!(t.library.playlist_entries(mix).unwrap()[0].track.as_ref().unwrap().id, original);
    // Waiting for the user's choice; kept: not asked again.
    assert_eq!(t.library.copies_waiting().unwrap().songs, 1);
    t.library.keep_copies().unwrap();
    assert_eq!(t.library.copies_waiting().unwrap().songs, 0);
    assert_eq!(t.library.find_copies(&synced, &songs).unwrap().hidden, 0, "found once");
}

#[test]
fn a_moved_songs_original_goes_once_its_synced_copy_is_in() {
    let (t, synced, _, other) = setup("moving-moved");
    let b = track(&t, other, "B");
    // Moved to sync: on the share as Band/B.wav, not in the library yet.
    t.library.conn().execute("INSERT INTO moves (track_id, share_path, moved_at) VALUES (?1, 'Band/B.wav', 0)", [b]).unwrap();
    let songs = t.library.songs_here(&synced, "me").unwrap();
    assert!(t.library.find_copies(&synced, &songs).unwrap().remove.is_empty(), "the synced copy isn't in yet");

    // The scan finds it in the synced folder.
    let synced_music = crate::library::testkit::MusicFolder { root: std::path::PathBuf::from(&synced.location) };
    synced_music.add("Band/B.wav", &Tags::song("B", "Band", "", 2));
    t.scan(synced.id, &synced_music);
    let songs = t.library.songs_here(&synced, "me").unwrap();
    let found = t.library.find_copies(&synced, &songs).unwrap();
    assert_eq!(found.remove, vec![b], "the original goes");
    assert_eq!(t.library.copies_waiting().unwrap().songs, 1, "only A's copy asks: B's original is going anyway");
}

#[test]
fn a_copy_is_shown_again_when_its_synced_song_is_gone() {
    let (t, synced, _, other) = setup("moving-unhide");
    let songs = t.library.songs_here(&synced, "me").unwrap();
    t.library.find_copies(&synced, &songs).unwrap();
    t.library.conn().execute("UPDATE tracks SET missing_since = 1 WHERE folder_id = ?1", [synced.id]).unwrap();
    let songs = t.library.songs_here(&synced, "me").unwrap();
    t.library.find_copies(&synced, &songs).unwrap();
    let titles: Vec<String> = t.library.tracks(0, 10, Default::default()).unwrap().into_iter().map(|s| s.title).collect();
    assert_eq!(titles, vec!["A", "B"]);
    assert_eq!(t.library.tracks(0, 10, Default::default()).unwrap()[0].id, track(&t, other, "A"), "the copy");
}

#[test]
fn on_the_same_drive_a_song_is_moved_into_the_synced_folder() {
    let (t, synced, other_music, _) = setup("moving-real-move");
    let original = other_music.path("B.wav");
    assert!(move_into(&t.library, &original, "Band/B.wav"));
    assert!(!original.exists(), "moved, not copied");
    assert!(std::path::Path::new(&synced.location).join("Band/B.wav").exists());
}

#[test]
fn linked_playlists_follow_their_songs_to_the_synced_copy() {
    let (t, synced, other_music, other) = setup("moving-linked");
    other_music.add_text("Mix.m3u", "A.wav\nB.wav\n");
    t.scan(other, &other_music);
    let mix = t.library.playlists().unwrap().into_iter().find(|p| p.name == "Mix").unwrap().id;
    let tracks = |t: &TestLibrary| -> Vec<i64> { t.library.playlist_entries(mix).unwrap().into_iter().filter_map(|e| e.track.map(|s| s.id)).collect() };
    assert_eq!(tracks(&t)[0], track(&t, other, "A"), "the file's A: the copy");

    let songs = t.library.songs_here(&synced, "me").unwrap();
    t.library.find_copies(&synced, &songs).unwrap();
    assert_eq!(tracks(&t), vec![track(&t, synced.id, "A"), track(&t, other, "B")], "A is the synced song now");
    let entry = t.library.playlist_entries(mix).unwrap().remove(0);
    assert_eq!(entry.file_entry.as_deref(), Some("A.wav"), "the file's line is kept");
    // A rescan of the file matches it to the synced song too.
    other_music.add_text("Mix.m3u", "B.wav\nA.wav\n");
    t.scan(other, &other_music);
    assert_eq!(tracks(&t), vec![track(&t, other, "B"), track(&t, synced.id, "A")]);
}
