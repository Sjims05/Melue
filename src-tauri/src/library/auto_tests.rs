//! The Android Auto library (auto.rs).

use tauri_plugin_native_audio::{LibraryChild, LibraryFolder};

use super::auto::RECENT_LISTS;
use super::testkit::{Tags, TestLibrary};

fn folders(children: &[LibraryChild]) -> Vec<&LibraryFolder> {
    children
        .iter()
        .map(|c| match c {
            LibraryChild::Folder(f) => f,
            LibraryChild::Item(id) => panic!("an item ({id}) where folders were expected"),
        })
        .collect()
}

fn items(children: &[LibraryChild]) -> Vec<i64> {
    children
        .iter()
        .map(|c| match c {
            LibraryChild::Item(id) => *id,
            LibraryChild::Folder(f) => panic!("a folder ({}) where songs were expected", f.id),
        })
        .collect()
}

fn track(t: &TestLibrary, path: &str) -> i64 {
    t.library.conn().query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| r.get(0)).unwrap()
}

#[test]
fn an_empty_library_says_where_the_music_comes_from() {
    let t = TestLibrary::new("auto-empty");
    let library = t.library.auto_library().unwrap();
    assert!(library.root.is_empty() && library.items.is_empty());
    assert!(library.empty_message.is_some());
}

#[test]
fn four_tabs_built_from_the_library() {
    let t = TestLibrary::new("auto-tabs");
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    m.add("Wall/02.wav", &Tags { cover: true, year: Some(1979), ..Tags::song("Two", "Pink Floyd", "The Wall", 2) });
    m.add("Wall/01.wav", &Tags { cover: true, year: Some(1979), ..Tags::song("One", "Pink Floyd", "The Wall", 1) });
    m.add("Abba/01.wav", &Tags::song("Waterloo", "ABBA", "Waterloo", 1));
    m.add("Guest/01.wav", &Tags::song("Duet", "Ólafur Arnalds feat. ABBA", "Some Kind", 1));
    m.add("Gone/01.wav", &Tags::song("Gone", "Ghost", "Gone", 1));
    t.scan(f, &m);
    m.remove("Gone/01.wav");
    t.scan(f, &m);
    let (one, two, waterloo, duet) = (track(&t, "Wall/01.wav"), track(&t, "Wall/02.wav"), track(&t, "Abba/01.wav"), track(&t, "Guest/01.wav"));
    t.library.set_liked(waterloo, true).unwrap();
    let mix = t.library.create_playlist("Mix").unwrap().id;
    t.library.add_to_playlist(mix, &[two, waterloo], None).unwrap();
    t.library.create_playlist("Empty").unwrap();

    let library = t.library.auto_library().unwrap();
    let tabs: Vec<&str> = library.root.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(tabs, vec!["recent", "playlists", "albums", "artists"]);
    assert_eq!(library.root[0].tracked_list.as_deref(), Some(RECENT_LISTS));
    assert_eq!(library.search, Some(true));

    // Only songs that can be played; no section headers anywhere.
    let mut ids: Vec<i64> = library.items.iter().map(|i| i.id).collect();
    ids.sort_unstable();
    let mut expected = vec![one, two, waterloo, duet];
    expected.sort_unstable();
    assert_eq!(ids, expected);
    assert!(library.items.iter().all(|i| i.group.is_none()));

    // Playlists: the auto playlists first (no plays yet: no Top songs; the one folder's too), empty ones left out.
    let playlists = folders(&library.root[1].children);
    assert_eq!(
        playlists.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        vec!["songs".to_string(), "liked".to_string(), format!("folder:{f}"), format!("playlist:{mix}")]
    );
    assert_eq!(items(&playlists[3].children), vec![two, waterloo]);
    assert!(playlists.iter().all(|p| p.playable && p.open_on_tap), "opened by a tap, played from a song");
    assert_eq!(playlists[3].subtitle.as_deref().map(|s| s.starts_with("2 songs")), Some(true));

    // Albums by title ("The" ignored), with covers and their songs in order.
    let albums = folders(&library.root[2].children);
    let titles: Vec<&str> = albums.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(titles, vec!["Some Kind", "The Wall", "Waterloo"]);
    assert!(albums.iter().all(|a| a.group.is_none()));
    let wall = albums[1];
    assert!(wall.id.starts_with("album:") && wall.playable);
    assert_eq!(items(&wall.children), vec![one, two]);
    assert_eq!(wall.subtitle.as_deref(), Some("Pink Floyd · 1979"));
    assert!(wall.artwork_url.as_deref().is_some_and(|a| a.ends_with("-1000.jpg")));

    // Artists: one list of their songs each (featured ones too), sorted without accents.
    let artists = folders(&library.root[3].children);
    let names: Vec<&str> = artists.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(names, vec!["ABBA", "Ólafur Arnalds", "Pink Floyd"]);
    assert!(artists.iter().all(|a| a.group.is_none()));
    // Neither album has a year: by title, so "Some Kind" (the duet) before "Waterloo".
    assert_eq!(items(&artists[0].children), vec![duet, waterloo]);
    assert!(artists[0].id.starts_with("artist:") && artists[0].playable);
}
