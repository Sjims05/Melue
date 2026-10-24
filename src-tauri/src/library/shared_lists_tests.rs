//! Lists named for another device (shared_lists.rs): two libraries with the same songs, as two of your devices.

use super::shared_lists::SharedList;
use super::testkit::{Tags, TestLibrary};

/// A device's library: "Song 1" and "Song 2" by Artist on Album; `extra`: one more song of that album only there.
fn device(name: &str, extra: bool) -> TestLibrary {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    m.add("Album/1.wav", &Tags::song("Song 1", "Artist", "Album", 1));
    m.add("Album/2.wav", &Tags::song("Song 2", "Artist", "Album", 2));
    if extra {
        m.add("Album/3.wav", &Tags::song("Only here", "Artist", "Album", 3));
    }
    t.scan(f, &m);
    t
}

fn id_of(t: &TestLibrary, sql: &str) -> i64 {
    t.library.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn an_album_artist_and_synced_playlist_are_found_on_the_other_device() {
    let phone = device("shared-lists-phone", false);
    let pc = device("shared-lists-pc", true);

    // The album on the phone is the PC's own album there, with its song only on the PC.
    let album = phone.library.shared_list(&format!("album:{}", id_of(&phone, "SELECT id FROM albums"))).unwrap();
    let there = pc.library.shared_list_here(&album).unwrap().unwrap();
    let titles: Vec<String> = pc.library.queue_items(&there).unwrap().into_iter().filter_map(|i| i.title).collect();
    assert_eq!(titles, ["Song 1", "Song 2", "Only here"]);

    let artist = phone.library.shared_list(&format!("artist:{}", id_of(&phone, "SELECT id FROM artists"))).unwrap();
    assert_eq!(pc.library.shared_list_here(&artist).unwrap(), Some(format!("artist:{}", id_of(&pc, "SELECT id FROM artists"))));

    // A synced playlist: by its sync id. One that hasn't reached the PC yet isn't there (None).
    let mine = phone.library.create_playlist("Mix").unwrap().id;
    phone.library.conn().execute("UPDATE playlists SET sync_id = 'mix-1' WHERE id = ?1", [mine]).unwrap();
    let list = phone.library.shared_list(&format!("playlist:{mine}")).unwrap();
    assert_eq!(list, SharedList::Playlist { sync_id: "mix-1".into() });
    assert_eq!(pc.library.shared_list_here(&list).unwrap(), None);
    let theirs = pc.library.create_playlist("Mix").unwrap().id;
    pc.library.conn().execute("UPDATE playlists SET sync_id = 'mix-1' WHERE id = ?1", [theirs]).unwrap();
    assert_eq!(pc.library.shared_list_here(&list).unwrap(), Some(format!("playlist:{theirs}")));

    // Liked songs and the Top lists are each device's own.
    assert_eq!(pc.library.shared_list_here(&phone.library.shared_list("liked").unwrap()).unwrap().as_deref(), Some("liked"));
    assert_eq!(pc.library.shared_list_here(&phone.library.shared_list("top:month").unwrap()).unwrap().as_deref(), Some("top:month"));
}

#[test]
fn lists_only_on_this_device_arent_named_for_others() {
    let phone = device("shared-lists-only-here", false);
    let mine = phone.library.create_playlist("Just mine").unwrap().id;
    phone.library.conn().execute("UPDATE playlists SET sync_id = 'x', local_only = 1 WHERE id = ?1", [mine]).unwrap();
    assert!(phone.library.shared_list(&format!("playlist:{mine}")).unwrap_err().contains("only on this device"));
    let folder = id_of(&phone, "SELECT id FROM folders");
    assert!(phone.library.shared_list(&format!("folder:{folder}")).unwrap_err().contains("only on this device"));
}
