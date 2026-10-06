//! What a change reports (changes.rs): the songs that changed and the lists whose songs changed.

use std::time::SystemTime;

use super::changes::{ChangeReason, LibraryChange};
use super::testkit::{MusicFolder, Tags, TestLibrary};

fn setup(name: &str) -> (TestLibrary, MusicFolder, i64) {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let id = t.add_folder(&m).id;
    (t, m, id)
}

/// What a whole scan changed.
fn scan(t: &TestLibrary, folder: i64, m: &MusicFolder) -> Option<LibraryChange> {
    tracked(t, || {
        t.scan(folder, m);
    })
}

/// What `change` changed.
fn tracked(t: &TestLibrary, change: impl FnOnce()) -> Option<LibraryChange> {
    let before = t.library.snapshot().unwrap();
    change();
    before.changes(&t.library.snapshot().unwrap(), ChangeReason::Scan)
}

fn id(t: &TestLibrary, sql: &str, value: &str) -> i64 {
    t.library.conn().query_row(sql, [value], |r| r.get(0)).unwrap()
}

fn track(t: &TestLibrary, path: &str) -> i64 {
    id(t, "SELECT id FROM tracks WHERE path = ?1", path)
}

fn album(t: &TestLibrary, title: &str) -> String {
    format!("album:{}", id(t, "SELECT id FROM albums WHERE title = ?1", title))
}

fn artist(t: &TestLibrary, name: &str) -> String {
    format!("artist:{}", id(t, "SELECT id FROM artists WHERE name = ?1", name))
}

fn song(title: &str, artist: &str, album: &str, n: u32) -> Tags {
    Tags::song(title, artist, album, n)
}

#[test]
fn the_first_scan_reports_everything_and_a_quiet_one_nothing() {
    let (t, m, f) = setup("changes-first");
    m.add("A/01.wav", &song("One", "Adele", "25", 1));
    m.add("A/02.wav", &song("Two", "Adele", "25", 2));
    let change = scan(&t, f, &m).unwrap();
    assert_eq!(change.tracks, vec![track(&t, "A/01.wav"), track(&t, "A/02.wav")]);
    assert_eq!(change.sources, vec!["songs".to_string(), album(&t, "25"), artist(&t, "Adele"), format!("folder:{f}")]);
    assert_eq!(scan(&t, f, &m), None);
}

#[test]
fn a_new_song_reports_itself_and_its_album_and_artist_only() {
    let (t, m, f) = setup("changes-new");
    m.add("A/01.wav", &song("One", "Adele", "25", 1));
    m.add("B/01.wav", &song("Other", "Queen", "Jazz", 1));
    scan(&t, f, &m);
    m.add("A/02.wav", &song("Two", "Adele", "25", 2));
    let change = scan(&t, f, &m).unwrap();
    assert_eq!(change.tracks, vec![track(&t, "A/02.wav")]);
    assert_eq!(change.sources, vec!["songs".to_string(), album(&t, "25"), artist(&t, "Adele"), format!("folder:{f}")]);
}

#[test]
fn a_song_moving_to_another_album_reports_both() {
    let (t, m, f) = setup("changes-retag");
    m.add("A/01.wav", &song("One", "Adele", "19", 1));
    m.add("A/02.wav", &song("Two", "Adele", "19", 2));
    scan(&t, f, &m);
    let old_album = album(&t, "19");
    m.retag("A/02.wav", &song("Two", "Adele", "21", 2));
    let change = scan(&t, f, &m).unwrap();
    assert_eq!(change.tracks, vec![track(&t, "A/02.wav")]);
    assert!(change.sources.contains(&old_album) && change.sources.contains(&album(&t, "21")), "{:?}", change.sources);
}

#[test]
fn a_new_guest_is_reported() {
    let (t, m, f) = setup("changes-guest");
    m.add("A/01.wav", &song("One", "Drake", "Views", 1));
    scan(&t, f, &m);
    m.retag("A/01.wav", &song("One", "Drake feat. Rihanna", "Views", 1));
    let change = scan(&t, f, &m).unwrap();
    assert!(change.sources.contains(&artist(&t, "Rihanna")) && change.sources.contains(&artist(&t, "Drake")));
}

#[test]
fn missing_back_and_moved_songs_are_reported() {
    let (t, m, f) = setup("changes-missing");
    m.add("A/01.wav", &song("One", "Adele", "25", 1));
    m.add("A/02.wav", &song("Two", "Adele", "25", 2));
    scan(&t, f, &m);
    let one = track(&t, "A/01.wav");
    let backup = std::fs::read(m.path("A/01.wav")).unwrap();
    let time: SystemTime = std::fs::metadata(m.path("A/01.wav")).unwrap().modified().unwrap();

    m.remove("A/01.wav");
    let gone = scan(&t, f, &m).unwrap();
    assert_eq!(gone.tracks, vec![one]);
    assert!(gone.sources.contains(&album(&t, "25")));

    std::fs::write(m.path("A/01.wav"), backup).unwrap();
    m.touch("A/01.wav", time);
    assert_eq!(scan(&t, f, &m).unwrap().tracks, vec![one]);

    m.rename("A/01.wav", "B/01 renamed.wav");
    assert_eq!(scan(&t, f, &m).unwrap().tracks, vec![one]);
}

#[test]
fn a_folder_switched_off_or_removed_reports_its_songs() {
    let (t, m, f) = setup("changes-folders");
    m.add("A/01.wav", &song("One", "Adele", "25", 1));
    let other = t.music_folder("Other");
    let g = t.add_folder(&other).id;
    other.add("01.wav", &song("Elsewhere", "Queen", "Jazz", 1));
    scan(&t, f, &m);
    scan(&t, g, &other);
    let one = track(&t, "A/01.wav");

    let off = tracked(&t, || {
        t.library.set_folder_enabled(f, false).unwrap();
    })
    .unwrap();
    assert_eq!(off.tracks, vec![one]);
    let on = tracked(&t, || {
        t.library.set_folder_enabled(f, true).unwrap();
    })
    .unwrap();
    assert_eq!(on.tracks, vec![one]);

    let removed = tracked(&t, || t.library.remove_folder(f).unwrap()).unwrap();
    assert_eq!(removed.tracks, vec![one]);
    assert!(removed.sources.contains(&"songs".to_string()));
    // Nothing else changed: renaming a folder changes no song.
    assert_eq!(tracked(&t, || drop(t.library.rename_folder(g, "Renamed").unwrap())), None);
}
