//! Playing from the library (playback.rs) and the queue following it (follow.rs).

use super::changes::{ChangeReason, LibraryChange};
use super::follow::{affects, QueueFollow};
use super::playback::{outcome, Outcome, PlaybackEvent, HISTORY_LIMIT};
use super::testkit::{MusicFolder, Tags, TestLibrary};

fn setup(name: &str) -> (TestLibrary, MusicFolder, i64) {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    (t, m, f)
}

fn id(t: &TestLibrary, sql: &str, value: &str) -> i64 {
    t.library.conn().query_row(sql, [value], |r| r.get(0)).unwrap()
}

fn track(t: &TestLibrary, path: &str) -> i64 {
    id(t, "SELECT id FROM tracks WHERE path = ?1", path)
}

/// The queue's songs, by title.
fn titles(t: &TestLibrary, source: &str) -> Vec<String> {
    t.library.queue_items(source).unwrap().into_iter().map(|i| i.title.unwrap()).collect()
}

fn song(title: &str, artist: &str, album: &str, n: u32) -> Tags {
    Tags::song(title, artist, album, n)
}

#[test]
fn queues_for_each_kind_of_list() {
    let (t, m, f) = setup("play-queues");
    m.add("Old/02.wav", &Tags { year: Some(1990), ..song("Old two", "Band", "Old", 2) });
    m.add("Old/01.wav", &Tags { year: Some(1990), ..song("Old one", "Band", "Old", 1) });
    m.add("New/CD2/01.wav", &Tags { year: Some(2020), disc: Some(2), ..song("New disc 2", "Band", "New", 1) });
    m.add("New/CD1/01.wav", &Tags { year: Some(2020), disc: Some(1), ..song("New disc 1", "Band", "New", 1) });
    m.add("loose.wav", &song("A single", "Band", "", 1));
    m.add("guest.wav", &song("With a guest", "Other feat. Band", "", 1));
    t.scan(f, &m);

    let new = id(&t, "SELECT id FROM albums WHERE title = ?1", "New");
    assert_eq!(titles(&t, &format!("album:{new}")), vec!["New disc 1", "New disc 2"]);
    // An artist: albums oldest first, then the songs not on an album (featured ones too).
    let band = id(&t, "SELECT id FROM artists WHERE name = ?1", "Band");
    assert_eq!(titles(&t, &format!("artist:{band}")), vec!["Old one", "Old two", "New disc 1", "New disc 2", "A single", "With a guest"]);
    assert_eq!(titles(&t, "songs").first().map(String::as_str), Some("A single"));

    let p = t.library.create_playlist("Mix").unwrap().id;
    let (loose, old_one) = (track(&t, "loose.wav"), track(&t, "Old/01.wav"));
    t.library.add_to_playlist(p, &[loose, old_one, loose], None).unwrap();
    assert_eq!(titles(&t, &format!("playlist:{p}")), vec!["A single", "Old one", "A single"]);

    t.library.set_liked(old_one, true).unwrap();
    t.library.set_liked(loose, true).unwrap();
    assert_eq!(titles(&t, "liked").len(), 2);

    for bad in ["album:9999", "playlist:9999", "artist:x", "nothing"] {
        assert!(t.library.queue_items(bad).is_err(), "{bad}");
    }
}

#[test]
fn queue_items_are_ready_to_play() {
    let (t, m, f) = setup("play-items");
    m.add("Album/01.wav", &Tags { cover: true, ..song("One", "Artist", "Album", 1) });
    m.add("Album/02.wav", &song("Two", "Artist", "Album", 2));
    t.scan(f, &m);
    let items = t.library.queue_items("songs").unwrap();
    let one = &items[0];
    assert_eq!(one.id, Some(track(&t, "Album/01.wav")));
    assert_eq!((one.artist.as_deref(), one.album.as_deref()), (Some("Artist"), Some("Album")));
    assert!(std::path::Path::new(&one.src).is_file(), "{}", one.src);
    let artwork = one.artwork_url.as_deref().unwrap();
    assert!(artwork.ends_with("-1000.jpg") && std::path::Path::new(artwork).is_file(), "{artwork}");
    assert_eq!(items[1].artwork_url, None);

    // Missing songs and switched-off folders aren't played.
    m.remove("Album/02.wav");
    t.scan(f, &m);
    assert_eq!(titles(&t, "songs"), vec!["One"]);
    t.library.set_folder_enabled(f, false).unwrap();
    assert!(titles(&t, "songs").is_empty());
}

#[test]
fn what_leaving_a_song_counts_as() {
    let min = 60_000;
    // A 4-minute song: a play from 2:00, a skip before 0:30.
    assert_eq!(outcome(2 * min, Some(4 * min)), Outcome::Play);
    assert_eq!(outcome(29_000, Some(4 * min)), Outcome::Skip);
    assert_eq!(outcome(min, Some(4 * min)), Outcome::Neither);
    // A 20-minute song: a play from 4:00.
    assert_eq!(outcome(4 * min, Some(20 * min)), Outcome::Play);
    assert_eq!(outcome(3 * min, Some(20 * min)), Outcome::Neither);
    // A 40-second song: half is 0:20, before the skip limit: from 0:20 it's a play.
    assert_eq!(outcome(20_000, Some(40_000)), Outcome::Play);
    assert_eq!(outcome(10_000, Some(40_000)), Outcome::Skip);
    // Unknown length: 4 minutes.
    assert_eq!(outcome(4 * min, None), Outcome::Play);
}

fn event(id: &str, kind: &str, item: i64, position_s: i64, at_s: i64) -> PlaybackEvent {
    PlaybackEvent { id: id.into(), kind: kind.into(), item_id: item, position_ms: position_s * 1000, at_ms: at_s * 1000 }
}

fn stats(t: &TestLibrary, track: i64) -> (i64, i64) {
    t.library
        .conn()
        .query_row("SELECT play_count, skip_count FROM track_stats WHERE track_id = ?1", [track], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap_or((0, 0))
}

#[test]
fn playback_events_count_plays_and_skips_once() {
    let (t, m, f) = setup("play-events");
    m.add("a.wav", &song("A", "X", "Y", 1)); // a quarter of a second long: anything past half is a play
    m.add("b.wav", &song("B", "X", "Y", 2));
    t.scan(f, &m);
    let (a, b) = (track(&t, "a.wav"), track(&t, "b.wav"));
    let record = |e: PlaybackEvent| t.library.record_playback(&e).unwrap();

    assert_eq!(record(event("1", "start", a, 0, 100)), None);
    assert_eq!(record(event("2", "complete", a, 0, 101)), Some(Outcome::Play));
    assert_eq!(record(event("2", "complete", a, 0, 101)), None, "the same event again");
    assert_eq!(record(event("3", "start", b, 0, 200)), None);
    assert_eq!(record(event("4", "skip", b, 0, 200)), Some(Outcome::Skip));
    assert_eq!(record(event("5", "skip", 9999, 0, 300)), None, "not in the library");
    assert_eq!((stats(&t, a), stats(&t, b)), ((1, 0), (0, 1)));

    // The history: each start once, with how it ended.
    let history: Vec<(i64, bool, Option<i64>)> = {
        let conn = t.library.conn();
        let mut stmt = conn.prepare("SELECT track_id, completed, position_ms FROM plays ORDER BY id").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    };
    assert_eq!(history, vec![(a, true, Some(0)), (b, false, Some(0))]);
    // An end whose start was missed still counts, with a history entry of its own.
    record(event("6", "complete", b, 0, 400));
    assert_eq!(stats(&t, b), (1, 1));

    let recent: Vec<String> = t.library.recently_played(10).unwrap().into_iter().map(|t| t.title).collect();
    assert_eq!(recent, vec!["B", "A"]);
    let most = t.library.most_played(10).unwrap();
    assert_eq!(most.iter().map(|t| (t.title.as_str(), t.play_count)).collect::<Vec<_>>(), vec![("B", 1), ("A", 1)]);
}

#[test]
fn the_history_is_trimmed_and_the_counts_stay() {
    let (t, m, f) = setup("play-trim");
    m.add("a.wav", &song("A", "X", "Y", 1));
    t.scan(f, &m);
    let a = track(&t, "a.wav");
    t.library.record_playback(&event("first", "complete", a, 0, 1)).unwrap();
    {
        let mut conn = t.library.conn();
        let tx = conn.transaction().unwrap();
        for n in 0..HISTORY_LIMIT {
            tx.execute("INSERT INTO plays (track_id, started_at, completed) VALUES (?1, ?2, 1)", [a, 10 + n]).unwrap();
        }
        tx.commit().unwrap();
    }
    t.library.record_playback(&event("last", "start", a, 0, 1_000_000)).unwrap();
    let kept: i64 = t.library.conn().query_row("SELECT count(*) FROM plays", [], |r| r.get(0)).unwrap();
    assert_eq!(kept, HISTORY_LIMIT);
    assert_eq!(stats(&t, a).0, 1, "the count doesn't depend on the history");
}

#[test]
fn likes() {
    let (t, m, f) = setup("play-likes");
    m.add("a.wav", &song("A", "X", "Y", 1));
    t.scan(f, &m);
    let a = track(&t, "a.wav");
    t.library.set_liked(a, true).unwrap();
    t.library.set_liked(a, true).unwrap(); // twice is fine
    assert_eq!(t.library.liked_ids().unwrap(), vec![a]);
    assert!(t.library.tracks(0, 10).unwrap()[0].liked);
    t.library.set_liked(a, false).unwrap();
    assert!(t.library.liked_ids().unwrap().is_empty());
    assert!(t.library.set_liked(9999, true).is_err());
}

#[test]
fn queue_follow_settings_and_which_changes_touch_a_queue() {
    let t = TestLibrary::new("play-follow");
    assert_eq!(t.library.queue_follow().unwrap(), QueueFollow::default());
    assert!(QueueFollow::default().enabled);
    let mine = QueueFollow { new_items: "end".into(), removed_current: "skip".into(), ..Default::default() };
    t.library.set_queue_follow(&mine).unwrap();
    assert_eq!(t.reopen().library.queue_follow().unwrap(), mine);
    let t = TestLibrary::new("play-follow-bad");
    assert!(t.library.set_queue_follow(&QueueFollow { order: "random".into(), ..Default::default() }).is_err());

    let change = LibraryChange { reason: ChangeReason::Scan, tracks: vec![1], sources: vec!["songs".into(), "album:3".into()] };
    assert!(affects(&change, Some("album:3")));
    assert!(!affects(&change, Some("album:4")));
    assert!(!affects(&change, None), "a queue not started from a list");
    assert!(affects(&LibraryChange::liked(5), Some("liked")));
}

#[test]
fn queue_items_for_songs_keep_their_order() {
    let (t, m, f) = setup("play-items-for");
    m.add("a.wav", &song("A", "Band", "", 1));
    m.add("b.wav", &song("B", "Band", "", 2));
    t.scan(f, &m);
    let (a, b) = (track(&t, "a.wav"), track(&t, "b.wav"));
    let titles = |ids: &[i64]| -> Vec<String> { t.library.queue_items_for(ids).unwrap().into_iter().map(|i| i.title.unwrap()).collect() };
    assert_eq!(titles(&[b, a, b]), vec!["B", "A", "B"]);
    assert_eq!(titles(&[a, 9999]), vec!["A"], "songs not there are left out");
    assert!(titles(&[]).is_empty());
}
