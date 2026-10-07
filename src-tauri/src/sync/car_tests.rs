//! Synced playback in the car with Melue closed (car.rs): the phone's library and a share with another
//! device's playback on it.

use super::car::{Car, CarPlayback, ENTRY_FOLDER};
use super::handoff::{self, read_playback_files, write_playback_file, Playback, PlaybackFile};
use crate::library::sync::SHARE_SETTING;
use crate::library::testkit::{Tags, TestLibrary};
use crate::library::now_ms;
use crate::share::testkit::TestShare;

/// The phone: a library whose synced folder has A and B, with the share set up as a drive (TestShare).
fn phone(name: &str, share: &TestShare) -> (Car, crate::library::testkit::TestDir, Vec<i64>) {
    let t = TestLibrary::new(name);
    let music = t.music_folder("Synced");
    music.add("A.wav", &Tags::song("A", "Band", "", 1));
    music.add("B.wav", &Tags::song("B", "Band", "", 2));
    let folder = t.add_folder(&music);
    t.scan(folder.id, &music);
    let setting = serde_json::json!({ "kind": "drive", "path": share.dir.path.to_string_lossy(), "stream": false, "syncFolderId": folder.id });
    t.library.set_setting(SHARE_SETTING, &setting.to_string()).unwrap();
    let ids = ["A", "B"].iter().map(|s| t.library.conn().query_row("SELECT id FROM tracks WHERE title = ?1", [s], |r| r.get(0)).unwrap()).collect();
    let TestLibrary { library, dir } = t;
    (Car::new(&dir.path, library), dir, ids)
}

/// The desktop's playback on the share: B, 42 s in, saved now.
fn desktop_played(share: &TestShare) {
    let file = PlaybackFile {
        device_id: "desktop".into(),
        device_name: "TROLLGANG".into(),
        saved_at_ms: now_ms(),
        share_ms: 0,
        state: Playback { queue: vec!["A.wav".into(), "B.wav".into()], index: 1, position_s: 42.0, playing: false, shuffle: true, repeat: "all".into() },
        title: Some("B".into()),
        artist: Some("Band".into()),
    };
    write_playback_file(&share.share, &file).unwrap();
}

#[test]
fn the_car_continues_recent_playback_from_another_device() {
    let share = TestShare::new("car-start");
    let (mut car, _dir, ids) = phone("car-start", &share);
    desktop_played(&share);
    let queue = car.queue_at_start(None).unwrap().expect("the desktop's, recent");
    let titles: Vec<_> = queue.items.iter().map(|i| i.title.clone().unwrap()).collect();
    assert_eq!((titles, queue.index, queue.position_ms, queue.shuffle, queue.repeat.as_str()), (vec!["A".to_string(), "B".to_string()], 1, 42_000, true, "all"));
    assert_eq!(queue.items[1].id, Some(ids[1]));
    // Taken: not again, and no entry for it.
    assert_eq!(car.queue_at_start(None).unwrap(), None);
    assert!(car.entries(None, ENTRY_FOLDER).unwrap().is_empty());
}

#[test]
fn older_playback_is_an_entry_in_recently_played_instead() {
    let share = TestShare::new("car-entry");
    let (mut car, _dir, _) = phone("car-entry", &share);
    desktop_played(&share);
    // Always ask: not continued by itself, but offered as an entry.
    let settings = handoff::PlaybackSettings { continue_within_minutes: None, ..Default::default() };
    car.library().set_setting("playback_sync", &serde_json::to_string(&settings).unwrap()).unwrap();
    assert_eq!(car.queue_at_start(None).unwrap(), None);
    let entries = car.entries(None, ENTRY_FOLDER).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "Continue from TROLLGANG");
    assert_eq!(entries[0].subtitle.as_deref(), Some("B — Band · 0:42"));
    assert!(car.entries(None, "albums").unwrap().is_empty(), "only in Recently played");
    // Picked: its queue, and the entry goes.
    let queue = car.queue_for_entry(None, &entries[0].id).unwrap().unwrap();
    assert_eq!(queue.index, 1);
    assert!(car.entries(None, ENTRY_FOLDER).unwrap().is_empty());
}

#[test]
fn what_the_car_plays_goes_to_the_share() {
    let share = TestShare::new("car-save");
    let (mut car, _dir, ids) = phone("car-save", &share);
    let playing = |index: usize, playing: bool| CarPlayback {
        item_ids: ids.iter().map(|&i| Some(i)).collect(),
        index,
        position_ms: 5_000,
        playing,
        shuffle: false,
        repeat_mode: "off".into(),
        title: Some(["A", "B"][index].into()),
        artist: Some("Band".into()),
    };
    // The queue it started with isn't news; playing is.
    assert!(!car.playback(None, "OnePlus", &playing(0, false)).unwrap());
    assert!(car.playback(None, "OnePlus", &playing(0, true)).unwrap());
    assert!(!car.playback(None, "OnePlus", &playing(0, true)).unwrap(), "unchanged: not again (until 30 s)");
    assert!(car.playback(None, "OnePlus", &playing(1, true)).unwrap(), "the next song");
    let files = read_playback_files(&share.share).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!((files[0].device_name.as_str(), files[0].state.index, files[0].title.as_deref()), ("OnePlus", 1, Some("B")));
    assert_eq!(files[0].state.queue, vec!["A.wav", "B.wav"]);
}

#[test]
fn off_means_nothing_in_the_car() {
    let share = TestShare::new("car-off");
    let (mut car, _dir, _) = phone("car-off", &share);
    desktop_played(&share);
    let settings = handoff::PlaybackSettings { mode: handoff::Mode::Off, ..Default::default() };
    car.library().set_setting("playback_sync", &serde_json::to_string(&settings).unwrap()).unwrap();
    assert_eq!(car.queue_at_start(None).unwrap(), None);
    assert!(car.entries(None, ENTRY_FOLDER).unwrap().is_empty());
}
