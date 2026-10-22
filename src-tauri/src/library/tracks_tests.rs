//! The Library's lists and pages (tracks.rs): sorting, an album's, an artist's and a folder's songs, and what each
//! song carries (its album, its artists with their ids).

use super::testkit::{MusicFolder, Tags, TestLibrary};
use super::{AlbumSort, ArtistSort, Track, TrackSort};

fn setup(name: &str) -> (TestLibrary, MusicFolder, i64) {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    (t, m, f)
}

fn song(title: &str, artist: &str, album: &str, n: u32) -> Tags {
    Tags::song(title, artist, album, n)
}

fn titles(tracks: &[Track]) -> Vec<&str> {
    tracks.iter().map(|t| t.title.as_str()).collect()
}

/// A band with two albums (the newer one on two discs), a song not on an album, and a guest spot on someone else's.
fn library(name: &str) -> TestLibrary {
    let (t, m, f) = setup(name);
    m.add("Old/02.wav", &Tags { year: Some(1990), ..song("Old two", "Band", "Old", 2) });
    m.add("Old/01.wav", &Tags { year: Some(1990), ..song("Old one", "Band", "Old", 1) });
    m.add("New/CD2/01.wav", &Tags { year: Some(2020), disc: Some(2), ..song("Disc two", "Band", "New", 1) });
    m.add("New/CD1/01.wav", &Tags { year: Some(2020), disc: Some(1), ..song("Disc one", "Band", "New", 1) });
    m.add("loose.wav", &song("A single", "Band", "", 1));
    m.add("Other/01.wav", &song("Duet", "Singer feat. Band", "Other", 1));
    t.scan(f, &m);
    t
}

#[test]
fn songs_carry_their_album_and_artists() {
    let t = library("pages-carry");
    let all = t.library.tracks(0, 100, TrackSort::Title).unwrap();
    let duet = all.iter().find(|s| s.title == "Duet").unwrap();
    let names: Vec<&str> = duet.artists.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["Singer", "Band"], "main first, then featured");
    assert!(duet.album_id.is_some());
    assert_eq!(duet.path, "Other/01.wav");
    let single = all.iter().find(|s| s.title == "A single").unwrap();
    assert_eq!(single.album_id, None);
}

#[test]
fn songs_sort_by_each_choice() {
    let t = library("pages-sort");
    let sorted = |sort| t.library.tracks(0, 100, sort).unwrap();
    assert_eq!(titles(&sorted(TrackSort::Title)), ["A single", "Disc one", "Disc two", "Duet", "Old one", "Old two"]);
    // By album (A–Z: New, Old, Other), disc and track; not on an album last.
    assert_eq!(titles(&sorted(TrackSort::Album)), ["Disc one", "Disc two", "Old one", "Old two", "Duet", "A single"]);
    // By the first main artist: Band's songs, then Singer's.
    assert_eq!(*titles(&sorted(TrackSort::Artist)).last().unwrap(), "Duet");
    assert_eq!(sorted(TrackSort::Added).len(), 6);
    assert_eq!(sorted(TrackSort::Plays).len(), 6);
}

#[test]
fn albums_and_artists_sort() {
    let t = library("pages-sort-lists");
    let albums = |sort| t.library.albums(0, 100, sort).unwrap().into_iter().map(|a| a.title).collect::<Vec<_>>();
    assert_eq!(albums(AlbumSort::Title), ["New", "Old", "Other"]);
    assert_eq!(albums(AlbumSort::Year)[0], "New", "newest first");
    assert_eq!(albums(AlbumSort::Artist), ["Old", "New", "Other"], "Band's (oldest first), then Singer's");
    let artists = |sort| t.library.artists(0, 100, sort).unwrap().into_iter().map(|a| a.name).collect::<Vec<_>>();
    assert_eq!(artists(ArtistSort::Name), ["Band", "Singer"]);
    assert_eq!(artists(ArtistSort::Songs), ["Band", "Singer"], "most songs first");
}

#[test]
fn an_albums_page_has_its_songs_by_disc_and_track() {
    let t = library("pages-album");
    let new = t.library.albums(0, 100, AlbumSort::Title).unwrap().into_iter().find(|a| a.title == "New").unwrap();
    let page = t.library.album_page(new.id).unwrap().unwrap();
    assert_eq!(page.album.artist.as_deref(), Some("Band"));
    assert!(page.album.artist_id.is_some());
    assert_eq!(titles(&page.tracks), ["Disc one", "Disc two"]);
    assert!(t.library.album_page(9999).unwrap().is_none());
}

#[test]
fn an_artists_page_has_albums_songs_and_featured() {
    let t = library("pages-artist");
    let band = t.library.artists(0, 100, ArtistSort::Name).unwrap().into_iter().find(|a| a.name == "Band").unwrap();
    let page = t.library.artist_page(band.id).unwrap().unwrap();
    let albums: Vec<&str> = page.albums.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(albums, ["Old", "New"], "oldest first");
    assert_eq!(titles(&page.tracks), ["Old one", "Old two", "Disc one", "Disc two", "A single"]);
    assert_eq!(titles(&page.featured), ["Duet"]);
    assert!(t.library.artist_page(9999).unwrap().is_none());
}

#[test]
fn made_for_you_lists_have_their_songs_in_order() {
    let t = library("pages-auto");
    let all = t.library.auto_playlist_tracks("songs").unwrap();
    assert_eq!(all.len(), 6);
    let id = |title: &str| all.iter().find(|s| s.title == title).unwrap().id;
    t.library.set_liked(id("Duet"), true).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    t.library.set_liked(id("Old one"), true).unwrap();
    assert_eq!(titles(&t.library.auto_playlist_tracks("liked").unwrap()), ["Old one", "Duet"], "newest liked first");
    assert!(t.library.auto_playlist_tracks("top:month").unwrap().is_empty(), "nothing played yet");
    assert!(t.library.auto_playlist_tracks("album:1").is_err());
}

#[test]
fn search_finds_names_anywhere_and_an_artists_songs_and_albums() {
    let t = library("pages-search");
    let found = t.library.search("disc", 10).unwrap();
    assert_eq!(titles(&found.tracks), ["Disc one", "Disc two"]);
    // An artist: them, their songs (Singer's duet) and their albums.
    let found = t.library.search("singer", 10).unwrap();
    assert_eq!(found.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Singer"]);
    assert_eq!(titles(&found.tracks), ["Duet"]);
    assert_eq!(found.albums.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), ["Other"]);
    // Anywhere in a name, case ignored; songs found by title come before those found by artist or album.
    let found = t.library.search("ONE", 1).unwrap();
    assert_eq!((found.track_count, found.tracks.len()), (2, 1), "Disc one, Old one; the first only");
    let found = t.library.search("band", 10).unwrap();
    assert_eq!(found.track_count, 6, "every song by Band, or featuring it");
    assert!(t.library.search("  ", 10).unwrap().tracks.is_empty());
}

#[test]
fn a_folders_songs_go_by_where_they_are() {
    let (t, m, f) = setup("pages-folder");
    m.add("B/01.wav", &song("Two", "Band", "B", 1));
    m.add("A/01.wav", &song("One", "Band", "A", 1));
    m.add("top.wav", &song("Three", "Band", "", 1));
    t.scan(f, &m);
    let tracks = t.library.folder_tracks(f).unwrap();
    assert_eq!(titles(&tracks), ["One", "Two", "Three"]);
    let file = t.library.track_file(tracks[0].id).unwrap().unwrap();
    assert!(file.ends_with("A/01.wav"), "{}", file.display());
}

#[test]
fn songs_that_cant_be_found_are_listed_with_their_folders_name() {
    // A folder without a name of its own (its name is the path's, made when it's read).
    let (t, m, f) = setup("missing-list");
    m.add("gone.wav", &song("Gone", "Band", "", 1));
    m.add("here.wav", &song("Here", "Band", "", 1));
    t.scan(f, &m);
    std::fs::remove_file(m.path("gone.wav")).unwrap();
    t.scan(f, &m);
    let missing = t.library.missing_songs().unwrap();
    assert_eq!(missing.iter().map(|s| (s.title.as_str(), s.folder.as_str())).collect::<Vec<_>>(), vec![("Gone", "Music")]);
}
