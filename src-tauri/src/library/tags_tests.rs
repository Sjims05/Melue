//! The second pass of a scan (tags.rs) and covers (covers.rs), on generated and sample files.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use super::covers::{self, LARGE, SMALL};
use super::scan::{cover_rank, find_folder_cover, list_folder, ScanSummary};
use super::tags::open_local;
use super::testkit::{MusicFolder, Tags, TestLibrary};
use super::{Folder, FolderKind, Track};

/// A whole scan of one folder, as the scanner does it: list, read tags, clean up covers.
fn scan(t: &TestLibrary, folder: &Folder, m: &MusicFolder) -> ScanSummary {
    let mut summary = t.library.apply_scan(folder.id, &list_folder(&m.root).unwrap()).unwrap();
    summary.add(t.library.read_tags(&open_local, &mut |_, _| {}).unwrap());
    t.library.clean_covers().unwrap();
    summary
}

fn folder(t: &TestLibrary, m: &MusicFolder) -> Folder {
    t.library.add_folder(FolderKind::Path, m.root.to_str().unwrap(), None).unwrap()
}

/// The track for a file in the folder.
fn track(t: &TestLibrary, path: &str) -> Track {
    let id: i64 = t.library.conn().query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| r.get(0)).unwrap();
    t.library.tracks(0, 10_000).unwrap().into_iter().find(|t| t.id == id).unwrap()
}

fn column<T: rusqlite::types::FromSql>(t: &TestLibrary, path: &str, column: &str) -> T {
    let sql = format!("SELECT {column} FROM tracks WHERE path = ?1");
    t.library.conn().query_row(&sql, [path], |r| r.get(0)).unwrap()
}

fn cover_files(t: &TestLibrary) -> usize {
    std::fs::read_dir(t.library.covers_dir()).map_or(0, |d| d.count())
}

fn covers(t: &TestLibrary) -> i64 {
    t.library.conn().query_row("SELECT count(*) FROM covers", [], |r| r.get(0)).unwrap()
}

#[test]
fn the_sample_files_tags_and_details_are_read() {
    let t = TestLibrary::new("tags-samples");
    let m = t.music_folder("Music");
    for format in ["mp3", "flac", "m4a"] {
        m.add_sample(&format!("sample.{format}"), &format!("song.{format}"));
    }
    let f = folder(&t, &m);
    let s = scan(&t, &f, &m);
    assert_eq!((s.added, s.tags_read, s.unreadable), (3, 3, 0));
    for format in ["mp3", "flac", "m4a"] {
        let song = track(&t, &format!("song.{format}"));
        assert_eq!(song.title, "Sample Song", "{format}");
        assert_eq!(song.artist.as_deref(), Some("Test Artist"), "{format}");
        assert_eq!(song.album.as_deref(), Some("Test Album"), "{format}");
        assert_eq!(song.album_artist.as_deref(), Some("Test Album Artist"), "{format}");
        assert_eq!((song.track_no, song.disc_no), (Some(3), Some(1)), "{format}");
        assert!(song.duration_ms.unwrap() >= 500, "{format}");
        assert_eq!(song.format.as_deref(), Some(format));
        assert!(song.sample_rate.is_some() && song.bitrate_kbps.is_some(), "{format}");
        assert!(song.cover.is_some(), "{format}: cover");
        assert!(song.tags_read && song.tags_error.is_none());
    }
}

#[test]
fn every_tag_is_kept_as_the_file_has_it() {
    let t = TestLibrary::new("tags-raw");
    let m = t.music_folder("Music");
    let tags = Tags {
        album_artist: Some("Various Artists".into()),
        compilation: true,
        artist_sort: Some("Beatles, The".into()),
        disc: Some(2),
        year: Some(1969),
        genre: Some("Rock".into()),
        ..Tags::song("Come Together", "The Beatles", "Hits", 1)
    };
    m.add("a.wav", &tags);
    let f = folder(&t, &m);
    scan(&t, &f, &m);
    let song = track(&t, "a.wav");
    assert_eq!(song.title, "Come Together");
    assert_eq!((song.year, song.genre.as_deref(), song.disc_no), (Some(1969), Some("Rock"), Some(2)));
    assert_eq!(song.format.as_deref(), Some("wav"));
    assert_eq!(column::<Option<String>>(&t, "a.wav", "tag_title").as_deref(), Some("Come Together"));
    assert_eq!(column::<Option<String>>(&t, "a.wav", "tag_album_artist").as_deref(), Some("Various Artists"));
    assert_eq!(column::<Option<String>>(&t, "a.wav", "tag_artist_sort").as_deref(), Some("Beatles, The"));
    assert!(column::<bool>(&t, "a.wav", "tag_compilation"));
}

#[test]
fn songs_without_tags_are_called_after_their_files() {
    let t = TestLibrary::new("tags-names");
    let m = t.music_folder("Music");
    m.add_untagged("Old name.wav", 100);
    m.add("tagged.wav", &Tags::song("Real Title", "X", "Y", 1));
    let f = folder(&t, &m);
    scan(&t, &f, &m);
    assert_eq!(track(&t, "Old name.wav").title, "Old name");
    assert_eq!(track(&t, "tagged.wav").title, "Real Title");

    // Renamed: the untagged one takes its new name, the tagged one keeps its title. Neither is re-read.
    m.rename("Old name.wav", "New name.wav");
    m.rename("tagged.wav", "renamed.wav");
    let s = scan(&t, &f, &m);
    assert_eq!((s.moved, s.tags_read), (2, 0));
    assert_eq!(track(&t, "New name.wav").title, "New name");
    assert_eq!(track(&t, "renamed.wav").title, "Real Title");
}

#[test]
fn only_new_and_changed_songs_are_read_again() {
    let t = TestLibrary::new("tags-changes");
    let m = t.music_folder("Music");
    m.add("a.wav", &Tags::song("A", "X", "Y", 1));
    m.add("b.wav", &Tags::song("B", "X", "Y", 2));
    let f = folder(&t, &m);
    assert_eq!(scan(&t, &f, &m).tags_read, 2);
    assert_eq!(scan(&t, &f, &m), ScanSummary { unchanged: 2, ..Default::default() });

    m.retag("a.wav", &Tags::song("A, edited", "X", "Y", 1));
    let s = scan(&t, &f, &m);
    assert_eq!((s.changed, s.tags_read), (1, 1));
    assert_eq!(track(&t, "a.wav").title, "A, edited");

    // Its title tag removed: back to the file's name.
    m.retag("a.wav", &Tags::default());
    scan(&t, &f, &m);
    assert_eq!(track(&t, "a.wav").title, "a");
    assert_eq!(track(&t, "a.wav").artist, None);
}

#[test]
fn a_broken_file_doesnt_stop_the_scan() {
    let t = TestLibrary::new("tags-broken");
    let m = t.music_folder("Music");
    m.add_text("Broken.mp3", "this isn't music");
    m.add("good.wav", &Tags::song("Good", "X", "Y", 1));
    let f = folder(&t, &m);
    let s = scan(&t, &f, &m);
    assert_eq!((s.added, s.tags_read, s.unreadable), (2, 1, 1));
    let broken = track(&t, "Broken.mp3");
    assert_eq!(broken.title, "Broken");
    assert!(broken.tags_read && broken.tags_error.is_some());
    // Not tried again until the file changes.
    assert_eq!(scan(&t, &f, &m).unreadable, 0);
}

#[test]
fn a_file_that_cant_be_opened_is_tried_again_next_scan() {
    let t = TestLibrary::new("tags-unopenable");
    let m = t.music_folder("Music");
    m.add("a.wav", &Tags::song("A", "X", "Y", 1));
    let f = folder(&t, &m);
    t.library.apply_scan(f.id, &list_folder(&m.root).unwrap()).unwrap();
    // The card is taken out between the two passes.
    let gone = |_: &super::tags::TagJob, _: &str| -> super::Result<std::fs::File> { Err("unplugged".into()) };
    let s = t.library.read_tags(&gone, &mut |_, _| {}).unwrap();
    assert_eq!((s.tags_read, s.unreadable), (0, 1));
    assert!(!track(&t, "a.wav").tags_read);
    assert_eq!(t.library.read_tags(&open_local, &mut |_, _| {}).unwrap().tags_read, 1);
}

#[test]
fn an_album_shares_one_cover_in_two_sizes() {
    let t = TestLibrary::new("tags-covers");
    let m = t.music_folder("Music");
    for n in 1..=10 {
        m.add(&format!("Album/{n:02}.wav"), &Tags { cover_color: Some([200, 30, 30]), ..Tags::song("S", "X", "Album", n) });
    }
    m.add_image("Big/folder.png", [10, 10, 200], 1500, 1200);
    m.add_untagged("Big/song.wav", 100);
    let f = folder(&t, &m);
    scan(&t, &f, &m);
    assert_eq!(covers(&t), 2);
    assert_eq!(cover_files(&t), 4);
    let album = track(&t, "Album/01.wav").cover.unwrap();
    assert!((2..=10).all(|n| track(&t, &format!("Album/{n:02}.wav")).cover.as_deref() == Some(album.as_str())));

    // The 1500 x 1200 picture: 1000 x 800 and 300 x 240. The 400 px one is smaller than large: kept as is.
    let big = track(&t, "Big/song.wav").cover.unwrap();
    let size = |file: &str, px| image::image_dimensions(t.library.covers_dir().join(covers::file_name(file, px))).unwrap();
    assert_eq!((size(&big, LARGE), size(&big, SMALL)), ((1000, 800), (300, 240)));
    assert_eq!((size(&album, LARGE), size(&album, SMALL)), ((400, 400), (300, 300)));
}

#[test]
fn a_folder_cover_is_used_when_the_song_has_none() {
    let t = TestLibrary::new("tags-folder-cover");
    let m = t.music_folder("Music");
    m.add_image("Album/folder.png", [0, 0, 255], 500, 500);
    m.add_image("Album/back.png", [0, 255, 0], 500, 500); // not a cover name
    m.add_untagged("Album/plain.wav", 100);
    m.add("Album/own.wav", &Tags { cover_color: Some([255, 0, 0]), ..Tags::default() });
    let f = folder(&t, &m);
    scan(&t, &f, &m);
    let plain = track(&t, "Album/plain.wav").cover.unwrap();
    let own = track(&t, "Album/own.wav").cover.unwrap();
    assert_ne!(plain, own, "the song's own cover comes first");

    // The image is replaced: the songs using it take the new one, and the old one is cleaned up.
    m.add_image("Album/folder.png", [255, 255, 0], 500, 500);
    m.touch("Album/folder.png", SystemTime::now() + Duration::from_secs(5));
    let s = scan(&t, &f, &m);
    assert_eq!(s.changed, 2, "both songs' folder cover changed");
    let replaced = track(&t, "Album/plain.wav").cover.unwrap();
    assert_ne!(replaced, plain);
    assert_eq!(track(&t, "Album/own.wav").cover.unwrap(), own);
    assert_eq!((covers(&t), cover_files(&t)), (2, 4));

    // And removed: no cover.
    m.remove("Album/folder.png");
    scan(&t, &f, &m);
    assert_eq!(track(&t, "Album/plain.wav").cover, None);
    assert_eq!((covers(&t), cover_files(&t)), (1, 2));
}

#[test]
fn cover_names_and_where_they_are_found() {
    for (name, rank) in [("cover.jpg", Some(0)), ("Folder.JPG", Some(1)), ("front.png", Some(2)), ("album.webp", Some(3))] {
        assert_eq!(cover_rank(name), rank, "{name}");
    }
    assert_eq!(cover_rank("AlbumArt_{1234}_Large.jpg"), Some(4));
    assert_eq!(cover_rank("AlbumArtSmall.jpg"), Some(5));
    for name in ["back.jpg", "cover.txt", "cover", "song.mp3"] {
        assert_eq!(cover_rank(name), None, "{name}");
    }

    let t = TestLibrary::new("tags-find-cover");
    let m = t.music_folder("Music");
    m.add_image("A/front.jpg", [1, 2, 3], 10, 10);
    m.add_image("A/cover.png", [1, 2, 3], 10, 10);
    let found = find_folder_cover(&m.root.join("A")).unwrap();
    assert!(Path::new(&found.path).is_absolute() && found.path.ends_with("cover.png"));
    assert!(find_folder_cover(&m.root.join("missing")).is_none());
}

#[test]
fn an_interrupted_second_pass_carries_on_after_a_restart() {
    let t = TestLibrary::new("tags-resume");
    let m = t.music_folder("Music");
    for n in 1..=10 {
        m.add(&format!("{n:02}.wav"), &Tags::song(&format!("Song {n}"), "X", "Y", n));
    }
    let f = folder(&t, &m);
    t.library.apply_scan(f.id, &list_folder(&m.root).unwrap()).unwrap();
    let (first, _) = t.library.read_tag_batch(&open_local, 0, 4).unwrap();
    assert_eq!(first.tags_read, 4);

    let t = t.reopen();
    assert_eq!(t.library.tags_to_read().unwrap(), 6);
    let s = scan(&t, &f, &m);
    assert_eq!((s.unchanged, s.tags_read), (10, 6));
    assert_eq!(track(&t, "10.wav").title, "Song 10");
}

#[test]
fn only_cover_files_are_served() {
    let t = TestLibrary::new("tags-serve");
    let m = t.music_folder("Music");
    m.add("a.wav", &Tags { cover: true, ..Tags::default() });
    let f = folder(&t, &m);
    scan(&t, &f, &m);
    let cover = track(&t, "a.wav").cover.unwrap();
    let dir = t.library.covers_dir();
    let ok = covers::serve(dir, &format!("/{}", covers::file_name(&cover, SMALL)));
    assert_eq!((ok.status().as_u16(), ok.headers()["Content-Type"].to_str().unwrap()), (200, "image/jpeg"));
    assert!(!ok.body().is_empty());
    for bad in ["/../library.db", "/library.db", &format!("/{cover}-50.jpg"), "/"] {
        assert_eq!(covers::serve(dir, bad).status().as_u16(), 404, "{bad}");
    }
}

#[test]
fn reading_a_thousand_songs_tags() {
    let t = TestLibrary::new("tags-big");
    let m = t.music_folder("Music");
    for album in 0..40 {
        for n in 1..=25 {
            let tags = Tags { cover_color: Some([album as u8 * 6, 0, 0]), ..Tags::song("Song", "Artist", &format!("Album {album}"), n) };
            m.add(&format!("Album {album}/{n:02}.wav"), &tags);
        }
    }
    let f = folder(&t, &m);
    t.library.apply_scan(f.id, &list_folder(&m.root).unwrap()).unwrap();
    let start = Instant::now();
    let mut updates = 0;
    let s = t.library.read_tags(&open_local, &mut |_, _| updates += 1).unwrap();
    let took = start.elapsed();
    println!("1000 songs, 40 covers: tags read in {took:?}");
    assert_eq!((s.tags_read, covers(&t)), (1000, 40));
    assert_eq!(updates, 20, "progress after every batch of 50");
    assert!(took < Duration::from_secs(60), "took {took:?}");
}
