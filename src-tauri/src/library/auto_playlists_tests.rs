//! Auto playlists (auto_playlists.rs) and the plays per day they count from.

use super::auto_playlists::{AutoKind, TopPeriod, DEFAULT_SIZE};
use super::playback::{PlaybackEvent, DAY_MS};
use super::testkit::{Tags, TestLibrary};

/// Plays `track` to the end `times` times on `day` (days since 1970).
fn play(t: &TestLibrary, track: i64, day: i64, times: u32) {
    for n in 0..times {
        let at_ms = day * DAY_MS + 3_600_000 + i64::from(n) * 600_000;
        let id = format!("{track}-{day}-{n}");
        let event = |kind: &str, position_ms, at_ms| PlaybackEvent { id: format!("{id}-{kind}"), kind: kind.into(), item_id: track, position_ms, at_ms };
        t.library.record_playback(&event("start", 0, at_ms)).unwrap();
        t.library.record_playback(&event("complete", 1_000, at_ms + 1_000)).unwrap();
    }
}

fn track(t: &TestLibrary, path: &str) -> i64 {
    t.library.conn().query_row("SELECT id FROM tracks WHERE path = ?1", [path], |r| r.get(0)).unwrap()
}

const TODAY: i64 = 20_000;

#[test]
fn top_songs_count_the_plays_in_their_period() {
    let t = TestLibrary::new("auto-top");
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    for name in ["old", "recent", "new"] {
        m.add(&format!("{name}.wav"), &Tags::song(name, "Band", "", 1));
    }
    t.scan(f, &m);
    let (old, recent, new) = (track(&t, "old.wav"), track(&t, "recent.wav"), track(&t, "new.wav"));
    play(&t, old, TODAY - 300, 5); // most played, but most of a year ago
    play(&t, recent, TODAY - 100, 3); // a few months ago
    play(&t, new, TODAY - 1, 2); // yesterday
    play(&t, recent, TODAY, 1); // and today

    let top = |period| t.library.top_songs_on(period, 50, TODAY).unwrap();
    assert_eq!(top(TopPeriod::Month), vec![new, recent]);
    assert_eq!(top(TopPeriod::HalfYear), vec![recent, new]);
    assert_eq!(top(TopPeriod::Year), vec![old, recent, new]);
    assert_eq!(top(TopPeriod::AllTime), vec![old, recent, new]);
    // The size: the most played first.
    assert_eq!(t.library.top_songs_on(TopPeriod::Year, 1, TODAY).unwrap(), vec![old]);
    // A window's first day: 30 days back is outside the month.
    play(&t, old, TODAY - 30, 9);
    assert_eq!(top(TopPeriod::Month), vec![new, recent]);
    // A skip isn't a play.
    let skip = PlaybackEvent { id: "skip".into(), kind: "skip".into(), item_id: new, position_ms: 0, at_ms: TODAY * DAY_MS };
    t.library.record_playback(&skip).unwrap();
    assert_eq!(t.library.conn().query_row("SELECT sum(plays) FROM play_days WHERE track_id = ?1", [new], |r| r.get::<_, i64>(0)).unwrap(), 2);
}

#[test]
fn the_history_fills_the_plays_per_day_when_the_library_is_updated() {
    // The migration counts what the history has: played to the end, or left after half the song.
    let t = TestLibrary::new("auto-migrate");
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    m.add("a.wav", &Tags::song("A", "Band", "", 1));
    t.scan(f, &m);
    let a = track(&t, "a.wav");
    {
        let conn = t.library.conn();
        conn.execute("UPDATE tracks SET duration_ms = 200000 WHERE id = ?1", [a]).unwrap();
        for (started, completed, position) in [(1, 1, Some(200_000)), (2, 0, Some(150_000)), (3, 0, Some(10_000)), (4, 0, None)] {
            conn.execute(
                "INSERT INTO plays (track_id, started_at, completed, position_ms) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![a, started * DAY_MS, completed, position],
            )
            .unwrap();
        }
        conn.execute("DROP TABLE play_days", []).unwrap();
        conn.pragma_update(None, "user_version", 8).unwrap();
    }
    let t = t.reopen();
    let days: Vec<(i64, i64)> = {
        let conn = t.library.conn();
        let mut stmt = conn.prepare("SELECT day, plays FROM play_days ORDER BY day").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
    };
    assert_eq!(days, vec![(1, 1), (2, 1)]);
}

#[test]
fn the_lists_in_order_with_their_settings() {
    let t = TestLibrary::new("auto-lists");
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    m.add("a.wav", &Tags::song("A", "Band", "", 1));
    m.add("b.wav", &Tags::song("B", "Band", "", 2));
    t.scan(f, &m);
    t.library.set_liked(track(&t, "a.wav"), true).unwrap();

    let lists = t.library.auto_playlists().unwrap();
    let sources: Vec<&str> = lists.iter().map(|l| l.source.as_str()).collect();
    // One folder: no folder lists (All songs is the same).
    assert_eq!(sources, vec!["songs", "liked", "top:month", "top:half-year", "top:year", "top:all-time"]);
    assert_eq!((lists[0].songs, lists[1].songs, lists[2].songs), (2, 1, 0));
    assert_eq!((lists[2].kind, lists[2].size), (AutoKind::Top, Some(DEFAULT_SIZE)));
    assert_eq!(lists[0].size, None);

    // A second folder: a list for each.
    let other = t.music_folder("Other");
    other.add("c.wav", &Tags::song("C", "Band", "", 1));
    let g = t.add_folder(&other).id;
    t.scan(g, &other);
    let lists = t.library.auto_playlists().unwrap();
    let folders: Vec<(&str, i64)> = lists.iter().filter(|l| l.kind == AutoKind::Folder).map(|l| (l.name.as_str(), l.songs)).collect();
    assert_eq!(folders, vec![("Music", 2), ("Other", 1)]);
    let titles = |source: &str| -> Vec<String> { t.library.queue_items(source).unwrap().into_iter().map(|i| i.title.unwrap()).collect() };
    assert_eq!(titles(&format!("folder:{f}")), vec!["A", "B"]);
    assert!(t.library.queue_items("folder:9999").is_err() && t.library.queue_items("top:week").is_err());

    // Hidden, and shown again; sizes only for Top songs, and only the offered ones.
    t.library.set_auto_playlist_hidden("liked", true).unwrap();
    assert!(t.library.auto_playlists().unwrap().iter().any(|l| l.source == "liked" && l.hidden), "still listed, marked");
    t.library.set_auto_playlist_hidden("liked", false).unwrap();
    assert!(t.library.auto_playlists().unwrap().iter().all(|l| !l.hidden));
    t.library.set_auto_playlist_size("top:year", 100).unwrap();
    assert!(t.library.set_auto_playlist_size("top:year", 7).is_err());
    assert!(t.library.set_auto_playlist_size("liked", 100).is_err());
    let t = t.reopen();
    assert_eq!(t.library.auto_playlists().unwrap().iter().find(|l| l.source == "top:year").unwrap().size, Some(100), "kept");
}
