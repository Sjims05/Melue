//! Songs on more than one device (to_share.rs): which are offered, and which device puts each on the share.

use super::data::DeviceData;
use super::to_share::{offer, Here, NotOnShare, SongName};

fn copy(title: &str, bitrate: i64, size: i64) -> NotOnShare {
    NotOnShare { name: SongName { artist: "artist".into(), title: title.into() }, duration_ms: Some(200_000), bitrate: Some(bitrate), size }
}

fn here(id: i64, copy: NotOnShare) -> Here {
    Here { id, title: copy.name.title.clone(), artist: Some("Artist".into()), copy }
}

fn device(id: &str, name: &str, songs: Vec<NotOnShare>, approved: Vec<SongName>) -> DeviceData {
    DeviceData { device_id: id.into(), device_name: name.into(), local_songs: songs, put_on_share: approved, ..Default::default() }
}

#[test]
fn a_song_another_device_has_too_is_offered_and_the_better_copy_goes() {
    let mine = vec![here(1, copy("the sign", 320, 9_000)), here(2, copy("only here", 320, 9_000)), here(3, copy("video", 128, 4_000))];
    let phone = device("phone", "Phone", vec![copy("the sign", 256, 8_000), copy("video", 320, 9_000)], vec![]);
    let offered = offer(&mine, &[phone], "pc", &[]);
    let summary: Vec<(i64, bool, Vec<String>)> = offered.iter().map(|o| (o.id, o.best_here, o.devices.clone())).collect();
    // Only on this device (not in the synced folder): not offered. The sign: the better copy is here; video: there.
    assert_eq!(summary, vec![(1, true, vec!["Phone".to_string()]), (3, false, vec!["Phone".to_string()])]);
}

#[test]
fn only_songs_all_the_other_devices_have_are_offered() {
    let mine = vec![here(1, copy("the sign", 320, 9_000)), here(2, copy("video", 320, 9_000))];
    let phone = device("phone", "Phone", vec![copy("the sign", 320, 1), copy("video", 320, 1)], vec![]);
    let laptop = device("laptop", "Laptop", vec![copy("the sign", 320, 1)], vec![]);
    let offered: Vec<i64> = offer(&mine, &[phone, laptop], "pc", &[]).iter().map(|o| o.id).collect();
    // The laptop hasn't got video: only the sign is on all of them.
    assert_eq!(offered, vec![1]);
    assert!(offer(&mine, &[], "pc", &[]).is_empty(), "no other devices: nothing to share between");
}

#[test]
fn a_yes_on_any_device_counts_and_equal_copies_go_from_one_device_only() {
    let name = SongName { artist: "artist".into(), title: "the sign".into() };
    let mine = vec![here(1, copy("the sign", 320, 9_000))];
    let phone = device("phone", "Phone", vec![copy("the sign", 320, 9_000)], vec![name]);
    let pc = offer(&mine, &[phone.clone()], "pc", &[]);
    assert!(pc[0].approved, "the phone said yes");
    // The same copy on both: exactly one of them puts it on the share.
    let on_phone = offer(&[here(7, copy("the sign", 320, 9_000))], &[device("pc", "PC", vec![copy("the sign", 320, 9_000)], vec![])], "phone", &[]);
    assert_ne!(pc[0].best_here, on_phone[0].best_here);
}
