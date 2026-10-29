//! How Melue's library-wide work grows with a big library: 5,000 generated songs (250 artists, 500 albums), each
//! library-wide step timed and what the page receives sized. Nothing is asserted: it measures. Ignored by default;
//! run it optimised: `cargo test --release --lib big_library -- --ignored --nocapture`.

use std::time::Instant;

use super::handoff;
use crate::library::sync::SHARE_SETTING;
use crate::library::testkit::{Tags, TestLibrary};
use crate::library::{AlbumSort, ArtistSort, TrackSort};

const SONGS: usize = 5_000;

fn timed<T>(what: &str, work: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = work();
    println!("{what:<58} {:>9.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    out
}

fn mb(bytes: usize) -> String {
    format!("{:.2} MB", bytes as f64 / 1e6)
}

#[test]
#[ignore = "measures (slow): cargo test --release --lib big_library -- --ignored --nocapture"]
fn big_library() {
    let t = TestLibrary::new("big-library");
    let music = t.music_folder("Synced");
    timed("generate the files", || {
        for n in 0..SONGS {
            let (artist, album) = (format!("Artist {}", n / 20), format!("Album {}", n / 10));
            let track = (n % 10 + 1) as u32;
            music.add(&format!("{artist}/{album}/{track:02} Song {n}.wav"), &Tags::song(&format!("Song {n}"), &artist, &album, track));
        }
    });
    let folder = t.add_folder(&music);
    timed("first scan (5,000 new songs)", || t.scan(folder.id, &music));
    t.on_share(folder.id);
    let setting = format!(r#"{{"kind":"drive","path":"Z:\\","stream":true,"syncFolderId":{}}}"#, folder.id);
    t.library.set_setting(SHARE_SETTING, &setting).unwrap();
    let folder = t.library.folder(folder.id).unwrap().unwrap();

    println!("--- scanning (the folder watcher scans after every change)");
    timed("rescan, nothing changed", || t.scan(folder.id, &music));
    music.add("Artist new/Album new/01 New.wav", &Tags::song("New", "Artist new", "Album new", 1));
    timed("rescan, one song added", || t.scan(folder.id, &music));
    timed("regroup albums and artists (after any change)", || t.library.regroup().unwrap());

    println!("--- background work");
    let paths = timed("synced_songs (handoff: every 5 s)", || t.library.synced_songs(&folder).unwrap());
    let ids: Vec<Option<i64>> = paths.iter().take(1_000).map(|(id, _)| Some(*id)).collect();
    timed("playback_on_share, a 1,000-song queue (every 5 s)", || {
        handoff::playback_on_share(&t.library, &folder, &ids, 500, &[], 10.0, (true, false, "off")).unwrap()
    });
    let me = handoff::device_id(&t.library).unwrap();
    timed("songs_here (every playlist sync)", || t.library.songs_here(&folder, &me).unwrap());
    timed("track_index (Spotify updates, rematching)", || t.library.track_index().unwrap());

    println!("--- what the pages load (on every library change)");
    let tracks = timed("tracks(): the Songs tab", || t.library.tracks(0, 1_000_000, TrackSort::Title).unwrap());
    let json = timed("  to JSON", || serde_json::to_vec(&tracks).unwrap());
    println!("  {} songs: {}", tracks.len(), mb(json.len()));
    let albums = timed("albums(): the Albums tab", || t.library.albums(0, 1_000_000, AlbumSort::Title).unwrap());
    println!("  {} albums: {}", albums.len(), mb(serde_json::to_vec(&albums).unwrap().len()));
    let artists = timed("artists(): the Artists tab", || t.library.artists(0, 1_000_000, ArtistSort::Name).unwrap());
    println!("  {} artists: {}", artists.len(), mb(serde_json::to_vec(&artists).unwrap().len()));
    let playlist = t.library.create_playlist("Big").unwrap().id;
    let some: Vec<i64> = tracks.iter().take(1_000).map(|track| track.id).collect();
    t.library.add_to_playlist(playlist, &some, None).unwrap();
    let entries = timed("playlist_entries(): a 1,000-song playlist", || t.library.playlist_entries(playlist).unwrap());
    println!("  {} entries: {}", entries.len(), mb(serde_json::to_vec(&entries).unwrap().len()));
}
