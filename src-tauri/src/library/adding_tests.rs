//! Adding songs from anywhere into a music folder (adding.rs): copied into `Artist/Album/`, the originals kept, and
//! into a playlist once a scan has found them. And the Music folder a fresh install starts with (folders.rs).

use super::testkit::{Tags, TestLibrary};

#[test]
fn dropped_songs_are_copied_into_artist_album_and_go_into_the_playlist_after_the_scan() {
    let t = TestLibrary::new("adding-copy");
    let music = t.music_folder("Music");
    let folder = t.add_folder(&music).id;
    let elsewhere = t.music_folder("Downloads");
    let one = elsewhere.add("one.wav", &Tags::song("One", "Band", "Album", 1));
    let two = elsewhere.add("two.wav", &Tags::song("Two", "Band", "Album", 2));
    // The same name again: it gets a number, not the first one's place.
    let again = elsewhere.add("again/one.wav", &Tags::song("One again", "Band", "Album", 3));
    let not_there = elsewhere.path("gone.wav");
    let files: Vec<String> = [&one, &two, &again, &not_there].iter().map(|p| p.to_string_lossy().into_owned()).collect();

    let result = t.library.copy_songs_into(folder, &files).unwrap();
    assert_eq!(result.added, 3);
    assert_eq!(result.placed, ["Band/Album/one.wav", "Band/Album/two.wav", "Band/Album/one (2).wav"]);
    assert_eq!(result.failed.len(), 1);
    assert!(one.exists() && two.exists(), "the files dropped stay where they are");

    let playlist = t.library.create_playlist("Mix").unwrap().id;
    t.library.wait_for_songs(folder, &result.placed, playlist).unwrap();
    // Before the scan: nothing to add yet.
    assert!(t.library.add_waiting_songs().unwrap().is_empty());
    t.scan(folder, &music);
    assert_eq!(t.library.add_waiting_songs().unwrap(), [playlist]);
    let titles: Vec<String> = t.library.playlist_entries(playlist).unwrap().into_iter().map(|e| e.track.unwrap().title).collect();
    assert_eq!(titles, ["One", "Two", "One again"]);
    // Done: not added twice.
    assert!(t.library.add_waiting_songs().unwrap().is_empty());
}

#[test]
fn songs_that_never_come_stop_waiting_after_a_few_scans() {
    let t = TestLibrary::new("adding-give-up");
    let music = t.music_folder("Music");
    let folder = t.add_folder(&music).id;
    let playlist = t.library.create_playlist("Mix").unwrap().id;
    t.library.wait_for_songs(folder, &["Band/Album/never.wav".to_string()], playlist).unwrap();
    for _ in 0..3 {
        t.library.add_waiting_songs().unwrap();
    }
    // Comes now, too late: it's no longer waited for.
    music.add("Band/Album/never.wav", &Tags::song("Never", "Band", "Album", 1));
    t.scan(folder, &music);
    assert!(t.library.add_waiting_songs().unwrap().is_empty());
}

#[test]
fn a_fresh_install_starts_with_the_music_folder_once() {
    let t = TestLibrary::new("adding-music-folder");
    let music = t.music_folder("Music");
    t.library.add_music_folder_once(Some(&music.root)).unwrap();
    assert_eq!(t.library.folders().unwrap().len(), 1);
    // Removed by the user: it doesn't come back.
    let id = t.library.folders().unwrap()[0].id;
    t.library.remove_folder(id).unwrap();
    t.library.add_music_folder_once(Some(&music.root)).unwrap();
    assert!(t.library.folders().unwrap().is_empty());

    // A library that has folders already isn't given one.
    let t = TestLibrary::new("adding-music-folder-not-new");
    let mine = t.music_folder("Mine");
    t.add_folder(&mine);
    t.library.add_music_folder_once(Some(&t.music_folder("Music").root)).unwrap();
    assert_eq!(t.library.folders().unwrap().len(), 1);
}
