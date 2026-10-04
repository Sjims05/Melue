//! App playlists (playlists.rs).

use std::time::SystemTime;

use super::changes::{ChangeReason, LibraryChange};
use super::testkit::{MusicFolder, Tags, TestLibrary};

/// A library with songs "01" to "<n>" (in Album/, each with its own cover); their ids in order.
fn setup(name: &str, songs: u32) -> (TestLibrary, MusicFolder, i64, Vec<i64>) {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    for n in 1..=songs {
        let tags = Tags { cover_color: Some([n as u8 * 20, 0, 0]), ..Tags::song(&format!("{n:02}"), "Artist", "Album", n) };
        m.add(&format!("Album/{n:02}.wav"), &tags);
    }
    t.scan(f, &m);
    let ids = (1..=songs).map(|n| track(&t, &format!("Album/{n:02}.wav"))).collect();
    (t, m, f, ids)
}

fn track(t: &TestLibrary, path: &str) -> i64 {
    t.library.conn().query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| r.get(0)).unwrap()
}

/// The playlist's songs, by title, in order.
fn titles(t: &TestLibrary, playlist: i64) -> Vec<String> {
    t.library.playlist_entries(playlist).unwrap().into_iter().map(|e| e.track.unwrap().title).collect()
}

#[test]
fn making_renaming_and_deleting_playlists() {
    let (t, _m, _f, songs) = setup("playlists-basics", 1);
    let road = t.library.create_playlist("  Road trip ").unwrap();
    assert_eq!((road.name.as_str(), road.kind.as_str(), road.entries), ("Road trip", "app", 0));
    let chill = t.library.create_playlist("chill").unwrap();
    assert!(t.library.create_playlist("   ").is_err(), "a name is needed");
    let names: Vec<String> = t.library.playlists().unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(names, vec!["chill", "Road trip"], "by name, ignoring case");

    let renamed = t.library.rename_playlist(chill.id, "Chill").unwrap();
    assert_eq!(renamed.name, "Chill");
    assert!(renamed.updated_at >= chill.updated_at);

    t.library.add_to_playlist(road.id, &songs, None).unwrap();
    t.library.delete_playlist(road.id).unwrap();
    assert!(t.library.playlist(road.id).is_err());
    assert!(t.library.delete_playlist(road.id).is_err());
    assert_eq!(t.library.status().unwrap().tracks, 1, "its songs stay in the library");
}

#[test]
fn adding_moving_and_removing_songs() {
    let (t, _m, _f, s) = setup("playlists-edit", 5);
    let p = t.library.create_playlist("Mix").unwrap().id;
    t.library.add_to_playlist(p, &[s[0], s[1]], None).unwrap();
    t.library.add_to_playlist(p, &[s[2]], Some(0)).unwrap();
    t.library.add_to_playlist(p, &[s[3], s[4]], Some(2)).unwrap();
    t.library.add_to_playlist(p, &[s[0]], Some(99)).unwrap(); // past the end: at the end; twice is fine
    assert_eq!(titles(&t, p), vec!["03", "01", "04", "05", "02", "01"]);
    let entries = t.library.playlist_entries(p).unwrap();
    assert_eq!(entries.iter().map(|e| e.position).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5]);

    // Removing one copy of the song that's in twice leaves the other.
    let second_copy = entries[5].id;
    t.library.remove_from_playlist(p, &[second_copy]).unwrap();
    assert_eq!(titles(&t, p), vec!["03", "01", "04", "05", "02"]);

    let first = t.library.playlist_entries(p).unwrap()[0].id;
    t.library.move_in_playlist(p, first, 3).unwrap();
    assert_eq!(titles(&t, p), vec!["01", "04", "05", "03", "02"]);
    t.library.move_in_playlist(p, first, 100).unwrap();
    assert_eq!(titles(&t, p), vec!["01", "04", "05", "02", "03"]);
    let positions: Vec<i64> = t.library.playlist_entries(p).unwrap().iter().map(|e| e.position).collect();
    assert_eq!(positions, vec![0, 1, 2, 3, 4]);
}

#[test]
fn a_song_that_isnt_there_adds_nothing() {
    let (t, _m, _f, s) = setup("playlists-unknown", 1);
    let p = t.library.create_playlist("Mix").unwrap().id;
    assert!(t.library.add_to_playlist(p, &[s[0], 9999], None).is_err());
    assert!(titles(&t, p).is_empty(), "all or nothing");
    assert!(t.library.add_to_playlist(4242, &s, None).is_err(), "no such playlist");
    let entry = t.library.add_to_playlist(p, &s, None).unwrap()[0];
    assert!(t.library.move_in_playlist(p, entry + 1, 0).is_err(), "no such entry");
}

#[test]
fn what_a_playlist_says_about_itself() {
    let (t, _m, _f, s) = setup("playlists-summary", 6);
    let p = t.library.create_playlist("Mix").unwrap().id;
    t.library.add_to_playlist(p, &[s[1], s[1], s[0], s[2], s[3], s[4], s[5]], None).unwrap();
    let mix = t.library.playlist(p).unwrap();
    assert_eq!((mix.entries, mix.available), (7, 7));
    assert!(mix.duration_ms >= 7 * 240, "{}", mix.duration_ms);
    // The first four different covers, in playlist order.
    let cover = |n: usize| t.library.playlist_entries(p).unwrap()[n].track.clone().unwrap().cover.unwrap();
    assert_eq!(mix.covers, vec![cover(0), cover(2), cover(3), cover(4)]);
}

#[test]
fn missing_songs_stay_and_removed_folders_leave() {
    let (t, m, f, s) = setup("playlists-missing", 2);
    let p = t.library.create_playlist("Mix").unwrap().id;
    t.library.add_to_playlist(p, &s, None).unwrap();
    let backup = std::fs::read(m.path("Album/01.wav")).unwrap();
    let time: SystemTime = std::fs::metadata(m.path("Album/01.wav")).unwrap().modified().unwrap();

    m.remove("Album/01.wav");
    t.scan(f, &m);
    let entries = t.library.playlist_entries(p).unwrap();
    assert_eq!(entries.iter().map(|e| e.track.as_ref().unwrap().available).collect::<Vec<_>>(), vec![false, true]);
    let mix = t.library.playlist(p).unwrap();
    assert_eq!((mix.entries, mix.available), (2, 1));

    std::fs::write(m.path("Album/01.wav"), backup).unwrap();
    m.touch("Album/01.wav", time);
    t.scan(f, &m);
    assert!(t.library.playlist_entries(p).unwrap()[0].track.as_ref().unwrap().available, "back where it was");

    // Switched off: unavailable; removed from the library: gone from the playlist.
    t.library.set_folder_enabled(f, false).unwrap();
    assert_eq!(t.library.playlist(p).unwrap().available, 0);
    t.library.remove_folder(f).unwrap();
    assert_eq!(t.library.playlist(p).unwrap().entries, 0);
}

#[test]
fn changes_to_a_playlists_songs_name_the_playlist() {
    let (t, m, f, s) = setup("playlists-changes", 2);
    let p = t.library.create_playlist("Mix").unwrap().id;
    t.library.add_to_playlist(p, &[s[0]], None).unwrap();

    // A song in it edited: the playlist is one of the lists that changed; a song not in it isn't.
    let before = t.library.snapshot().unwrap();
    m.retag("Album/01.wav", &Tags::song("01 (edit)", "Artist", "Album", 1));
    t.scan(f, &m);
    let change = before.changes(&t.library.snapshot().unwrap(), ChangeReason::Scan).unwrap();
    assert!(change.sources.contains(&format!("playlist:{p}")), "{:?}", change.sources);

    let before = t.library.snapshot().unwrap();
    m.retag("Album/02.wav", &Tags::song("02 (edit)", "Artist", "Album", 2));
    t.scan(f, &m);
    let change = before.changes(&t.library.snapshot().unwrap(), ChangeReason::Scan).unwrap();
    assert!(!change.sources.contains(&format!("playlist:{p}")), "{:?}", change.sources);

    // Edits to the playlist itself: its songs, once each.
    let edit = LibraryChange::playlist(p, vec![s[1], s[0], s[1]]);
    assert_eq!((edit.reason, edit.tracks, edit.sources), (ChangeReason::Playlists, vec![s[0], s[1]], vec![format!("playlist:{p}")]));
}
