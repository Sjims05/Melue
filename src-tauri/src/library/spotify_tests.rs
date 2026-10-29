//! Spotify playlists (spotify.rs): made from what Spotify shows (no network here), matched to the library.

use super::spotify::{SpotifyList, SpotifySong};
use super::testkit::{Tags, TestLibrary};
use super::TrackSort;

fn song(artist: &str, title: &str, seconds: i64) -> SpotifySong {
    SpotifySong { title: title.into(), artist: artist.into(), duration_ms: Some(seconds * 1000), track: Some(format!("id{title}").replace(' ', "")) }
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
fn its_songs_ids_are_kept_for_links_and_go_with_them() {
    let t = library("spotify-ids");
    let index = t.library.track_index().unwrap();
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Band", "Song One", 1), song("Other", "Not Here", 200)], total: Some(2), cover: None };
    let id = t.library.add_spotify_list("abc", &list, &index).unwrap();
    // By the entries' lines: the missing song's id, to link to it.
    let missing = t.library.playlist_entries(id).unwrap().into_iter().find(|e| e.track.is_none()).unwrap();
    assert_eq!(t.library.spotify_tracks(id).unwrap().get(&missing.file_entry.unwrap()).map(String::as_str), Some("idNotHere"));
    // Gone from Spotify: its id goes too.
    let now = SpotifyList { songs: vec![song("Band", "Song One", 1)], total: Some(1), ..list };
    t.library.update_from_list(id, None, &now, &index).unwrap();
    assert_eq!(t.library.spotify_tracks(id).unwrap().into_values().collect::<Vec<_>>(), ["idSongOne"]);
}

#[test]
fn a_short_playlist_is_matched_and_follows_spotify() {
    let t = library("spotify-short");
    let index = t.library.track_index().unwrap();
    // A version note on Spotify, and another artist with it: still the song here.
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Band, Guest", "Song One - Remastered 2001", 1), song("Other", "Not Here", 200)], total: Some(2), cover: None };
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
    let list = SpotifyList { name: "Long".into(), songs: vec![song("Band", "Song One", 1)], total: Some(150), cover: None };
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
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Band", "Song One", 1), song("Other", "Later", 1)], total: Some(2), cover: None };
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
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Band", "Song One", 1)], total: Some(1), cover: None };
    let id = t.library.add_spotify_list("stop", &list, &index).unwrap();
    t.library.unlink_playlist(id).unwrap();
    let p = t.library.playlist(id).unwrap();
    assert!(p.spotify.is_none());
    assert_eq!(songs(&t, id), [("Song One".to_string(), true)]);
    // Not among the playlists the background update reads.
    let left: i64 = t.library.conn().query_row("SELECT count(*) FROM playlists WHERE spotify_id IS NOT NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
}

#[test]
fn downloads_with_other_separators_and_lengths_still_match() {
    // As a downloader writes them: several artists joined by "/", a band name's comma turned into one, and a few
    // seconds off Spotify's length.
    let t = TestLibrary::new("spotify-loose");
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    m.add("Bang Bang.wav", &Tags::song("Bang Bang", "K'NAAN/Adam Levine", "Album", 1));
    m.add("September.wav", &Tags::song("September", "Earth/ Wind & Fire", "Album", 1));
    m.add("Heaven.wav", &Tags::song("Heaven", "Belinda Carlisle", "Album", 1));
    // Two versions of one song here, both close enough: the one closest to Spotify's length is the one.
    m.add("Lovefool.wav", &Tags::song("Lovefool", "The Cardigans", "Album", 1));
    m.add("Lovefool (long).wav", &Tags::song("Lovefool", "The Cardigans", "Other album", 3));
    t.scan(f, &m);
    let index = t.library.track_index().unwrap();
    let list = SpotifyList {
        name: "Mix".into(),
        songs: vec![
            song("K'NAAN, Adam Levine", "Bang Bang", 5),
            song("Earth, Wind & Fire", "September", 1),
            // Another version here, two minutes longer: another recording, not this one.
            song("Belinda Carlisle", "Heaven", 121),
            song("The Cardigans", "Lovefool", 3),
            // Not here at all.
            song("Nena", "99 Luftballons", 230),
        ],
        total: Some(5),
        cover: None,
    };
    let id = t.library.add_spotify_list("loose", &list, &index).unwrap();
    let entries = t.library.playlist_entries(id).unwrap();
    let here: Vec<(String, bool)> = songs(&t, id);
    assert_eq!(
        here,
        [
            ("Bang Bang".to_string(), true),
            ("September".to_string(), true),
            ("Heaven".to_string(), false),
            ("Lovefool".to_string(), true),
            ("99 Luftballons".to_string(), false)
        ]
    );
    let lovefool = entries[3].track.as_ref().unwrap();
    assert!(lovefool.path.contains("long"), "the 3 s Lovefool, closest to Spotify's");
}

#[test]
fn a_song_chosen_by_hand_stays_and_goes_with_the_playlist() {
    // Spotify's song is four minutes; yours is a longer version: not matched by itself, so you choose it (Find song).
    let t = TestLibrary::new("spotify-chosen");
    let m = t.music_folder("Music");
    let folder = t.add_folder(&m);
    m.add("Heaven.wav", &Tags::song("Heaven", "Belinda Carlisle", "Album", 1));
    t.scan(folder.id, &m);
    let index = t.library.track_index().unwrap();
    let list = SpotifyList { name: "Mix".into(), songs: vec![song("Belinda Carlisle", "Heaven", 240)], total: Some(1), cover: None };
    let id = t.library.add_spotify_list("chosen", &list, &index).unwrap();
    assert_eq!(songs(&t, id), [("Heaven".to_string(), false)]);

    let entry = t.library.playlist_entries(id).unwrap()[0].id;
    let index = t.library.track_index().unwrap();
    let track = t.library.tracks(0, 10, TrackSort::Title).unwrap().into_iter().find(|s| s.title == "Heaven").unwrap().id;
    t.library.choose_playlist_song(id, entry, track).unwrap();
    assert_eq!(songs(&t, id), [("Heaven".to_string(), true)]);

    // Read from Spotify again: it stays (an entry keeps its song).
    t.library.update_from_list(id, None, &list, &index).unwrap();
    assert_eq!(songs(&t, id), [("Heaven".to_string(), true)]);

    // Synced: the other devices get which song was chosen.
    let here = t.library.songs_here(&folder, "me").unwrap();
    let data = t.library.playlist_data(id, &here).unwrap();
    assert_eq!(data.entries[0].chosen.as_deref(), Some(here.key_of(track).as_str()));
}
