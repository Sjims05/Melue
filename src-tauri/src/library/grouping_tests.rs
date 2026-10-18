//! The grouping rules (grouping.rs): the name rules on their own, then albums and artists built from
//! generated music folders.

use std::time::{Duration, Instant};

use super::grouping::{featured_in_title, file_name_of, fold, folder_of, parse_credit, parse_file_name, sort_key, Credit, NameParts};
use super::testkit::{MusicFolder, Tags, TestLibrary};
use super::{Album, Artist, Track};

// ---- names

#[test]
fn names_compare_without_case_accents_or_spaces() {
    assert_eq!(fold("  Beyoncé "), "beyonce");
    assert_eq!(fold("SIGUR  RÓS"), "sigur ros");
    assert_eq!(fold("Björk"), fold("bjork"));
}

#[test]
fn sorting_ignores_the_and_punctuation() {
    assert_eq!(sort_key("The Beatles"), "beatles");
    assert_eq!(sort_key("The The"), "the");
    assert_eq!(sort_key("Theory of a Deadman"), "theory of a deadman");
    assert_eq!(sort_key("A Tribe Called Quest"), "a tribe called quest");
    assert_eq!(sort_key("'Round Midnight"), "round midnight");
    assert_eq!(sort_key("Ólafur Arnalds"), "olafur arnalds");
    assert_eq!(sort_key("...And Justice for All"), "and justice for all");
    assert_eq!(sort_key("!!!"), "!!!");
}

fn credit(main: &[&str], featured: &[&str]) -> Credit {
    Credit { main: main.iter().map(|s| s.to_string()).collect(), featured: featured.iter().map(|s| s.to_string()).collect() }
}

#[test]
fn artist_credits_are_split_into_main_and_featured() {
    let cases = [
        ("Drake feat. Rihanna", credit(&["Drake"], &["Rihanna"])),
        ("Drake ft. Rihanna", credit(&["Drake"], &["Rihanna"])),
        ("Calvin Harris FEAT. Dua Lipa", credit(&["Calvin Harris"], &["Dua Lipa"])),
        ("Drake (feat. Rihanna & Future)", credit(&["Drake"], &["Rihanna", "Future"])),
        ("Daft Punk featuring Pharrell Williams, Nile Rodgers", credit(&["Daft Punk"], &["Pharrell Williams", "Nile Rodgers"])),
        ("Dan + Shay with Justin Bieber", credit(&["Dan + Shay"], &["Justin Bieber"])),
        ("Simon & Garfunkel", credit(&["Simon & Garfunkel"], &[])),
        ("Earth, Wind & Fire", credit(&["Earth, Wind & Fire"], &[])),
        ("Queen; David Bowie", credit(&["Queen", "David Bowie"], &[])),
        ("A; B ft. C", credit(&["A", "B"], &["C"])),
        ("A feat. a", credit(&["A"], &[])),
        ("  ", credit(&[], &[])),
    ];
    for (text, expected) in cases {
        assert_eq!(parse_credit(text), expected, "{text}");
    }
}

#[test]
fn guests_in_titles() {
    assert_eq!(featured_in_title("Song (feat. X)"), vec!["X"]);
    assert_eq!(featured_in_title("Song [ft. X & Y]"), vec!["X", "Y"]);
    assert_eq!(featured_in_title("Song (with X)"), vec!["X"]);
    assert_eq!(featured_in_title("Song feat. Z"), vec!["Z"]);
    assert!(featured_in_title("Dancing with Myself").is_empty());
    assert!(featured_in_title("Wonderwall").is_empty());
}

fn parts(disc: Option<u32>, track: Option<u32>, artist: Option<&str>, title: &str) -> NameParts {
    NameParts { disc, track, artist: artist.map(String::from), title: title.into() }
}

#[test]
fn file_names_of_untagged_songs() {
    let cases = [
        ("01 Song", parts(None, Some(1), None, "Song")),
        ("01 - Song", parts(None, Some(1), None, "Song")),
        ("01. Song", parts(None, Some(1), None, "Song")),
        ("7_Song", parts(None, Some(7), None, "Song")),
        ("12 Song", parts(None, Some(12), None, "Song")),
        ("1-03 Song", parts(Some(1), Some(3), None, "Song")),
        ("2-11 - Song", parts(Some(2), Some(11), None, "Song")),
        ("03 Artist - Title", parts(None, Some(3), Some("Artist"), "Title")),
        ("Artist - Title", parts(None, None, Some("Artist"), "Title")),
        ("Artist - Title - Live", parts(None, None, Some("Artist"), "Title - Live")),
        ("50 Cent - In da Club", parts(None, None, Some("50 Cent"), "In da Club")),
        ("2Pac - Changes", parts(None, None, Some("2Pac"), "Changes")),
        ("1 Thing", parts(None, None, None, "1 Thing")),
        ("1999", parts(None, None, None, "1999")),
        ("Song", parts(None, None, None, "Song")),
    ];
    for (name, expected) in cases {
        assert_eq!(parse_file_name(name), expected, "{name}");
    }
}

#[test]
fn file_names_and_folders_of_paths() {
    assert_eq!((file_name_of("Album/01 Song.mp3"), folder_of("Album/01 Song.mp3")), ("01 Song.mp3".into(), "Album".into()));
    assert_eq!((file_name_of("loose.mp3"), folder_of("loose.mp3")), ("loose.mp3".into(), String::new()));
    let android = "/storage/emulated/0/Music/A/b.mp3";
    assert_eq!((file_name_of(android), folder_of(android)), ("b.mp3".into(), "/storage/emulated/0/Music/A".into()));
    let tree = "content://com.android.externalstorage.documents/tree/primary%3AMusic/document/primary%3AMusic%2FAlbum%2F01%20Song.mp3";
    assert_eq!((file_name_of(tree), folder_of(tree)), ("01 Song.mp3".into(), "primary:Music/Album".into()));
}

// ---- albums and artists

fn albums(t: &TestLibrary) -> Vec<Album> {
    t.library.albums(0, 1000, Default::default()).unwrap()
}

fn artists(t: &TestLibrary) -> Vec<Artist> {
    t.library.artists(0, 1000, Default::default()).unwrap()
}

fn album_summary(t: &TestLibrary) -> Vec<(String, Option<String>, bool, i64)> {
    albums(t).into_iter().map(|a| (a.title, a.artist, a.compilation, a.tracks)).collect()
}

fn track(t: &TestLibrary, path: &str) -> Track {
    let id: i64 = t.library.conn().query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| r.get(0)).unwrap();
    t.library.tracks(0, 100_000, Default::default()).unwrap().into_iter().find(|t| t.id == id).unwrap()
}

fn song(title: &str, artist: &str, album: &str, n: u32) -> Tags {
    Tags::song(title, artist, album, n)
}

fn setup(name: &str) -> (TestLibrary, MusicFolder, i64) {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let id = t.add_folder(&m).id;
    (t, m, id)
}

#[test]
fn a_guest_on_one_song_doesnt_split_the_album() {
    let (t, m, f) = setup("group-guest");
    m.add("Album/01.wav", &song("One", "Drake", "Views", 1));
    m.add("Album/02.wav", &song("Two", "Drake feat. Rihanna", "Views", 2));
    m.add("Album/03.wav", &song("Three (feat. Future)", "Drake", "Views", 3));
    t.scan(f, &m);
    assert_eq!(album_summary(&t), vec![("Views".into(), Some("Drake".into()), false, 3)]);
    let found: Vec<(String, i64, i64, i64)> = artists(&t).into_iter().map(|a| (a.name, a.tracks, a.featured, a.albums)).collect();
    assert_eq!(found, vec![("Drake".into(), 3, 0, 1), ("Future".into(), 0, 1, 0), ("Rihanna".into(), 0, 1, 0)]);
    // The title stays as tagged.
    assert_eq!(track(&t, "Album/03.wav").title, "Three (feat. Future)");
}

#[test]
fn compilations() {
    let (t, m, f) = setup("group-compilations");
    // No album artist, different artists in one folder.
    m.add("Hits/01.wav", &song("A", "Artist A", "Summer Hits", 1));
    m.add("Hits/02.wav", &song("B", "Artist B", "Summer Hits", 2));
    // Marked as a compilation, and "Various Artists" as album artist.
    m.add("Now/01.wav", &Tags { compilation: true, ..song("C", "Artist C", "Now 1", 1) });
    m.add("Now/02.wav", &Tags { compilation: true, ..song("D", "Artist C", "Now 1", 2) });
    m.add("Va/01.wav", &Tags { album_artist: Some("Various Artists".into()), ..song("E", "Artist E", "Best Of", 1) });
    t.scan(f, &m);
    assert_eq!(
        album_summary(&t),
        vec![
            ("Best Of".into(), None, true, 1),
            ("Now 1".into(), None, true, 2),
            ("Summer Hits".into(), None, true, 2),
        ]
    );
}

#[test]
fn albums_with_the_same_title_by_different_artists_stay_apart() {
    let (t, m, f) = setup("group-same-title");
    m.add("Queen/01.wav", &Tags { album_artist: Some("Queen".into()), ..song("A", "Queen", "Greatest Hits", 1) });
    m.add("ABBA/01.wav", &Tags { album_artist: Some("ABBA".into()), ..song("B", "ABBA", "Greatest Hits", 1) });
    // Without album artists, in separate folders.
    m.add("Eagles/01.wav", &song("C", "Eagles", "Their Greatest Hits", 1));
    m.add("Other/01.wav", &song("D", "Someone Else", "Their Greatest Hits", 1));
    t.scan(f, &m);
    assert_eq!(
        album_summary(&t),
        vec![
            ("Greatest Hits".into(), Some("ABBA".into()), false, 1),
            ("Greatest Hits".into(), Some("Queen".into()), false, 1),
            ("Their Greatest Hits".into(), Some("Eagles".into()), false, 1),
            ("Their Greatest Hits".into(), Some("Someone Else".into()), false, 1),
        ]
    );
}

#[test]
fn an_album_in_disc_folders_is_one_album() {
    let (t, m, f) = setup("group-discs");
    m.add("Album/CD1/01.wav", &Tags { disc: Some(1), ..song("A", "Pink Floyd", "The Wall", 1) });
    m.add("Album/CD2/01.wav", &Tags { disc: Some(2), ..song("B", "Pink Floyd", "the wall", 1) });
    t.scan(f, &m);
    assert_eq!(album_summary(&t), vec![("The Wall".into(), Some("Pink Floyd".into()), false, 2)]);
}

#[test]
fn an_albums_year_and_cover_are_what_most_songs_have() {
    let (t, m, f) = setup("group-year-cover");
    let red = Some([200, 0, 0]);
    m.add("A/01.wav", &Tags { year: Some(1999), cover_color: red, ..song("A", "X", "Album", 1) });
    m.add("A/02.wav", &Tags { year: Some(1999), cover_color: red, ..song("B", "X", "Album", 2) });
    m.add("A/03.wav", &Tags { year: Some(2005), cover_color: Some([0, 0, 200]), ..song("C", "X", "Album", 3) });
    t.scan(f, &m);
    let album = &albums(&t)[0];
    assert_eq!(album.year, Some(1999));
    assert_eq!(album.cover, track(&t, "A/01.wav").cover);
}

#[test]
fn untagged_songs_are_named_after_their_files() {
    let (t, m, f) = setup("group-untagged");
    m.add_untagged("Downloads/03 Daft Punk - Get Lucky.wav", 100);
    m.add_untagged("Downloads/mystery.wav", 100);
    m.add("Downloads/04 Tagged.wav", &Tags { title: Some("Real".into()), ..Tags::default() });
    t.scan(f, &m);
    let lucky = track(&t, "Downloads/03 Daft Punk - Get Lucky.wav");
    assert_eq!((lucky.title.as_str(), lucky.artist.as_deref(), lucky.track_no), ("Get Lucky", Some("Daft Punk"), Some(3)));
    // No album tag: no album (not "Downloads").
    assert!(albums(&t).is_empty());
    let names: Vec<String> = artists(&t).into_iter().map(|a| a.name).collect();
    assert_eq!(names, vec!["Daft Punk", "Unknown artist"]);
    // A title tag but no artist: the file name only gives the track number.
    let tagged = track(&t, "Downloads/04 Tagged.wav");
    assert_eq!((tagged.title.as_str(), tagged.artist.as_deref(), tagged.track_no), ("Real", None, Some(4)));
}

#[test]
fn artists_sort_without_the_and_accents_and_a_sort_tag_wins() {
    let (t, m, f) = setup("group-sorting");
    for (n, artist) in ["Ólafur Arnalds", "Coldplay", "The Beatles", "ABBA", "Nirvana"].iter().enumerate() {
        m.add(&format!("{n}.wav"), &song("Song", artist, "", 1));
    }
    m.add("z.wav", &Tags { artist_sort: Some("Zzz".into()), ..song("Song", "Aardvark", "", 1) });
    m.add("b.wav", &song("Song", "beatles, the", "", 1)); // a differently spelled Beatles is another artist
    t.scan(f, &m);
    let names: Vec<String> = artists(&t).into_iter().map(|a| a.name).collect();
    assert_eq!(names, vec!["ABBA", "The Beatles", "beatles, the", "Coldplay", "Nirvana", "Ólafur Arnalds", "Aardvark"]);
}

#[test]
fn names_spelled_differently_are_one_artist() {
    let (t, m, f) = setup("group-spellings");
    m.add("1.wav", &song("A", "Beyoncé", "", 1));
    m.add("2.wav", &song("B", "Beyoncé", "", 2));
    m.add("3.wav", &song("C", "beyonce", "", 3));
    t.scan(f, &m);
    let found: Vec<(String, i64)> = artists(&t).into_iter().map(|a| (a.name, a.tracks)).collect();
    assert_eq!(found, vec![("Beyoncé".into(), 3)]);
}

#[test]
fn albums_and_artists_keep_their_ids() {
    let (t, m, f) = setup("group-ids");
    m.add("A/01.wav", &song("One", "Artist", "Album", 1));
    m.add("A/02.wav", &song("Two", "Artist", "Album", 2));
    m.add("B/01.wav", &song("Other", "Someone", "Other Album", 1));
    t.scan(f, &m);
    let album = albums(&t)[0].id;
    let artist = artists(&t)[0].id;

    // A song retagged, and a rescan: the same ids, and nothing written when nothing changed.
    m.retag("A/02.wav", &song("Two (edit)", "Artist", "Album", 2));
    t.scan(f, &m);
    assert_eq!((albums(&t)[0].id, artists(&t)[0].id), (album, artist));
    assert_eq!(t.library.regroup().unwrap().written, 0);

    // Songs gone: their album and artist stay while they're missing (they may come back)...
    m.remove("B/01.wav");
    t.scan(f, &m);
    assert_eq!(albums(&t).len(), 1, "a missing song's album isn't listed");
    let rows: i64 = t.library.conn().query_row("SELECT count(*) FROM albums", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 2);
    // ...and go when the songs leave the library (their folder removed; the next scan regroups).
    t.library.remove_folder(f).unwrap();
    assert!(t.library.regroup_after(&Default::default()).unwrap().is_some());
    assert!(t.library.regroup_after(&Default::default()).unwrap().is_none(), "only once");
    let rows: (i64, i64) = t
        .library
        .conn()
        .query_row("SELECT (SELECT count(*) FROM albums), (SELECT count(*) FROM artists)", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!(rows, (0, 0));
}

#[test]
fn the_library_status_counts_what_is_there() {
    let (t, m, f) = setup("group-status");
    m.add("A/01.wav", &song("One", "Drake feat. Rihanna", "Views", 1));
    m.add("B/01.wav", &song("Two", "Adele", "25", 1));
    t.scan(f, &m);
    let status = t.library.status().unwrap();
    assert_eq!((status.tracks, status.albums, status.artists), (2, 2, 3));
    t.library.set_folder_enabled(f, false).unwrap();
    let status = t.library.status().unwrap();
    assert_eq!((status.tracks, status.albums, status.artists), (0, 0, 0));
}

#[test]
fn grouping_ten_thousand_songs() {
    let (t, m, f) = setup("group-big");
    {
        let mut conn = t.library.conn();
        let tx = conn.transaction().unwrap();
        for n in 0..10_000 {
            let artist = format!("Artist {}", n % 300);
            let credit = if n % 10 == 0 { format!("{artist} feat. Guest {}", n % 50) } else { artist };
            tx.execute(
                "INSERT INTO tracks (folder_id, path, size, modified_at, title, sort_title, artist_credit, tag_album, added_at, tags_read_at)
                 VALUES (?1, ?2, 1, 1, ?3, ?3, ?4, ?5, 0, 0)",
                rusqlite::params![f, format!("{}/{n}.mp3", n / 12), format!("Song {n}"), credit, format!("Album {}", n / 12)],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    let start = Instant::now();
    let first = t.library.regroup().unwrap();
    let took = start.elapsed();
    let start = Instant::now();
    let again = t.library.regroup().unwrap();
    let again_took = start.elapsed();
    println!("10000 songs: grouped in {took:?}, again (nothing changed) in {again_took:?}");
    assert_eq!((first.albums, first.artists), (834, 305)); // 300 artists, 5 guests
    assert_eq!(again.written, 0);
    assert!(took < Duration::from_secs(10) && again_took < Duration::from_secs(5), "{took:?} / {again_took:?}");

    // What every scan costs to find what changed (changes.rs): two snapshots and a comparison.
    let start = Instant::now();
    let before = t.library.snapshot().unwrap();
    assert_eq!(before.changes(&t.library.snapshot().unwrap(), super::changes::ChangeReason::Scan), None);
    println!("10000 songs: change check in {:?}", start.elapsed());
    let _ = m;
}
