//! Spotify playlists (spotify.rs): made from what Spotify shows (no network here), matched to the library.

use super::spotify::{SpotifyList, SpotifySong};
use super::testkit::{Tags, TestLibrary};

fn song(artist: &str, title: &str, seconds: i64) -> SpotifySong {
    SpotifySong { title: title.into(), artist: artist.into(), duration_ms: Some(seconds * 1000) }
}

/// A library with "Song One" by Band (1 s long, as the test kit's songs are).
fn library(name: &str) -> TestLibrary {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    m.add("One.wav", &Tags::song("Song One", "Band", "Album", 1));
    t.scan(f, &m);
    t
}

/// The playlist's songs by title, and whether each is here.
fn songs(t: &TestLibrary, id: i64) -> Vec<(String, bool)> {
    t.library
        .playlist_entries(id)
        .unwrap()
        .into_iter()
        .map(|e| match e.track {
            Some(track) => (track.title, true),
            None => (e.sync_title.unwrap_or_default(), false),
        })
        .collect()
}

#[test]
fn a_short_playlist_is_matched_and_follows_spotify() {
    let t = library("spotify-short");
    let index = t.library.track_index().unwrap();
    // A version note on Spotify, and another artist with it: still the song here.
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Band, Guest", "Song One - Remastered 2001", 1), song("Other", "Not Here", 200)], total: Some(2) };
    let id = t.library.add_spotify_list("abc", &list, &index).unwrap();
    assert_eq!(songs(&t, id), [("Song One".to_string(), true), ("Not Here".to_string(), false)]);
    // Changed on Spotify: the change comes in.
    let now = SpotifyList { songs: vec![song("Band", "Song One", 1)], total: Some(1), ..list };
    assert!(t.library.update_from_list(id, None, &now, &index).unwrap());
    assert_eq!(songs(&t, id), [("Song One".to_string(), true)]);
}

#[test]
fn a_long_playlist_says_it_changed_and_takes_its_list_from_a_csv() {
    let t = library("spotify-long");
    let index = t.library.track_index().unwrap();
    // 150 songs on Spotify: the first ones only.
    let list = SpotifyList { name: "Long".into(), songs: vec![song("Band", "Song One", 1)], total: Some(150) };
    let id = t.library.add_spotify_list("long", &list, &index).unwrap();
    let seen: Option<String> = t.library.conn().query_row("SELECT spotify_seen FROM playlists WHERE id = ?1", [id], |r| r.get(0)).unwrap();
    // The same on Spotify: nothing to do. Changed there: the "!".
    assert!(!t.library.update_from_list(id, seen.clone(), &list, &index).unwrap());
    let changed = SpotifyList { total: Some(151), ..list };
    assert!(t.library.update_from_list(id, seen, &changed, &index).unwrap());
    assert!(t.library.playlist(id).unwrap().spotify.unwrap().changed);
    // Its list from a CSV: all its songs (offline here: the "!" goes, the count is the CSV's).
    t.library.conn().execute("UPDATE playlists SET spotify_id = 'offline-test-id' WHERE id = ?1", [id]).unwrap();
    let csv = "Track Name,Artist Name(s),Duration (ms)\nSong One,Band,1000\nSecond,Other,200000\n";
    t.library.import_spotify_csv(id, csv, &index).unwrap();
    assert_eq!(songs(&t, id).len(), 2);
    assert!(!t.library.playlist(id).unwrap().spotify.unwrap().changed);
}

#[test]
fn a_missing_song_fills_in_when_it_comes_to_the_library() {
    let t = library("spotify-arrives");
    let index = t.library.track_index().unwrap();
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Band", "Song One", 1), song("Other", "Later", 1)], total: Some(2) };
    let id = t.library.add_spotify_list("arrives", &list, &index).unwrap();
    assert_eq!(songs(&t, id), [("Song One".to_string(), true), ("Later".to_string(), false)]);
    // Its file put in the music folder: the next scan matches it, with no update from Spotify.
    let m = t.music_folder("Music");
    let f = t.library.folders().unwrap()[0].id;
    m.add("Later.wav", &Tags::song("Later", "Other", "Album", 2));
    t.scan(f, &m);
    assert_eq!(songs(&t, id), [("Song One".to_string(), true), ("Later".to_string(), true)]);
}

#[test]
fn stopping_following_spotify_keeps_the_songs_and_stops_reading_it() {
    let t = library("spotify-stop");
    let index = t.library.track_index().unwrap();
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Band", "Song One", 1)], total: Some(1) };
    let id = t.library.add_spotify_list("stop", &list, &index).unwrap();
    t.library.unlink_playlist(id).unwrap();
    let p = t.library.playlist(id).unwrap();
    assert!(p.spotify.is_none());
    assert_eq!(songs(&t, id), [("Song One".to_string(), true)]);
    // Not among the playlists the background update reads.
    let left: i64 = t.library.conn().query_row("SELECT count(*) FROM playlists WHERE spotify_id IS NOT NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
}
