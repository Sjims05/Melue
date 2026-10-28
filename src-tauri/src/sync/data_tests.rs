//! Library data's rules (data.rs).

use super::data::*;

#[test]
fn the_newest_like_or_unlike_wins() {
    // Liked here at 10, unliked on another device at 20: unliked.
    assert_eq!(newer_like(Some((true, 10)), &[(false, 20)]), Some((false, 20)));
    // Older changes elsewhere don't count.
    assert_eq!(newer_like(Some((true, 30)), &[(false, 20)]), None);
    // The same as here: nothing to do.
    assert_eq!(newer_like(Some((true, 10)), &[(true, 20)]), None);
    // The newest of several devices.
    assert_eq!(newer_like(Some((false, 5)), &[(true, 10), (false, 8)]), Some((true, 10)));
    // Never touched here: a like is taken, an unlike is how it is already.
    assert_eq!(newer_like(None, &[(true, 10)]), Some((true, 10)));
    assert_eq!(newer_like(None, &[(false, 10)]), None);
    assert_eq!(newer_like(None, &[]), None);
}

#[test]
fn a_devices_file_is_written_only_when_it_says_something_new() {
    let data = DeviceData {
        device_id: "d".into(),
        device_name: "Desktop".into(),
        saved_at_ms: 1,
        likes: vec![LikeRecord { path: "a.mp3".into(), liked: true, at_ms: 5 }],
        ..Default::default()
    };
    assert!(same_data(&data, &DeviceData { saved_at_ms: 99, share_ms: 7, ..data.clone() }), "only when it was written");
    assert!(!same_data(&data, &DeviceData { likes: Vec::new(), ..data.clone() }));
    assert!(!same_data(&data, &DeviceData { device_name: "Renamed".into(), ..data.clone() }));
}

#[test]
fn the_shares_clock_from_this_devices_own_file() {
    let file = |id: &str, saved, share| DeviceData { device_id: id.into(), saved_at_ms: saved, share_ms: share, ..Default::default() };
    let files = vec![file("me", 1_000, 1_250), file("other", 0, 9_999)];
    assert_eq!(share_clock_ahead(&files, "me"), 250);
    assert_eq!(share_clock_ahead(&files, "new"), 0);
}

#[test]
fn library_files_are_told_apart() {
    assert!(is_library_file(".melue/library/abc.json") && is_library_file(".melue/playlists/abc.json"));
    assert!(is_library_file(".melue/connections.json") && is_library_file(".melue/settings.json"));
    assert!(!is_library_file(".melue/playback/abc.json") && !is_library_file("Artist/song.mp3"));
}
