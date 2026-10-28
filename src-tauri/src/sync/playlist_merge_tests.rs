//! Merging playlists changed on two devices (playlist_merge.rs).

use super::playlist_merge::*;

fn keys(s: &str) -> Vec<&str> {
    s.split_whitespace().collect()
}

fn merged(base: &str, mine: &str, theirs: &str) -> Option<String> {
    merge_keys(&keys(base), &keys(mine), &keys(theirs)).map(|k| k.join(" "))
}

fn list(name: &str, songs: &str) -> PlaylistData {
    PlaylistData { name: name.into(), entries: keys(songs).into_iter().map(|k| SyncEntry { key: k.into(), title: None, artist: None, duration_ms: None, chosen: None }).collect(), spotify: None }
}

#[test]
fn one_side_changed_takes_that_side() {
    assert_eq!(merged("a b c", "a b c", "a c d").as_deref(), Some("a c d"));
    assert_eq!(merged("a b c", "c b a", "a b c").as_deref(), Some("c b a"));
    assert_eq!(merged("a b", "a b x", "a b x").as_deref(), Some("a b x"), "the same change on both");
}

#[test]
fn additions_and_removals_on_both_sides_are_kept() {
    // The example: song A added on the phone, song B removed on the desktop.
    assert_eq!(merged("b c d", "b c d a", "c d").as_deref(), Some("c d a"));
    // Additions go in after the song before them on their side.
    assert_eq!(merged("a b c", "a x b c", "a b y c").as_deref(), Some("a x b y c"));
    assert_eq!(merged("a b c", "x a b c", "a b c y").as_deref(), Some("x a b c y"));
    // Removed on both, or removed on one and kept on the other: removed.
    assert_eq!(merged("a b c d", "a c d", "a b d e").as_deref(), Some("a d e"));
    assert_eq!(merged("a b", "a", "").as_deref(), Some(""));
}

#[test]
fn the_same_song_twice_is_counted() {
    // One copy added on each side: both kept.
    assert_eq!(merged("a b", "a b a", "a a b").as_deref(), Some("a a b a"));
    // One of two copies removed here, a song added there: one copy left.
    assert_eq!(merged("a b a", "a b", "a b a c").as_deref(), Some("a b c"));
}

#[test]
fn the_order_comes_from_the_side_that_reordered() {
    // Reordered here, a song added there: the new order, with the addition after its neighbour.
    assert_eq!(merged("a b c", "c b a", "a b x c").as_deref(), Some("c b x a"));
    // Reordered there, a song removed here.
    assert_eq!(merged("a b c", "a c", "c b a").as_deref(), Some("c a"));
    // Both reordered the same way: fine.
    assert_eq!(merged("a b c", "c a b", "c a b x").as_deref(), Some("c a b x"));
    // Both reordered differently: a conflict.
    assert_eq!(merged("a b c", "c b a", "b a c"), None);
}

#[test]
fn names_merge_unless_both_renamed() {
    let base = list("Gym", "a");
    assert_eq!(merge(&base, &list("Gym", "a"), &list("Workout", "a")).unwrap().name, "Workout");
    assert_eq!(merge(&base, &list("Run", "a"), &list("Gym", "a b")).unwrap().name, "Run");
    assert_eq!(merge(&base, &list("Run", "a"), &list("Run", "a")).unwrap().name, "Run");
    assert_eq!(merge(&base, &list("Run", "a"), &list("Workout", "a")), None, "renamed differently on both");
}

#[test]
fn what_happens_to_a_playlist() {
    let (base, changed, other) = (list("Gym", "a b"), list("Gym", "a b c"), list("Gym", "b"));
    let live = |d: &PlaylistData| Theirs::Live(d.clone());
    let uses = |a: Action| match a {
        Action::Use { result, here, share } => Some((result.keys_joined(), here, share)),
        _ => None,
    };
    // Unchanged on both: nothing.
    assert_eq!(decide(Some(&base), Some(&base), &live(&base)), Action::Nothing);
    // Changed here: to the share. Changed there: here.
    assert_eq!(uses(decide(Some(&base), Some(&changed), &live(&base))), Some(("a b c".into(), false, true)));
    assert_eq!(uses(decide(Some(&base), Some(&base), &live(&other))), Some(("b".into(), true, false)));
    // Changed on both: merged, to both.
    assert_eq!(uses(decide(Some(&base), Some(&changed), &live(&other))), Some(("b c".into(), true, true)));
    // Made here: to the share. Made there: here.
    assert_eq!(uses(decide(None, Some(&changed), &Theirs::Missing)), Some(("a b c".into(), false, true)));
    assert_eq!(uses(decide(None, None, &live(&other))), Some(("b".into(), true, false)));
    // Made on both with the same name before syncing: both sides' songs.
    assert_eq!(uses(decide(None, Some(&list("Gym", "a b")), &live(&list("Gym", "c")))), Some(("a b c".into(), true, true)));
    // Restored here over a deletion: to the share again.
    assert_eq!(uses(decide(None, Some(&base), &Theirs::Deleted)), Some(("a b".into(), false, true)));
    // The share's file gone: put back.
    assert_eq!(uses(decide(Some(&base), Some(&base), &Theirs::Missing)), Some(("a b".into(), false, true)));
}

#[test]
fn deleting_and_conflicts() {
    let (base, changed) = (list("Gym", "a b"), list("Gym", "a b c"));
    // Deleted there, unchanged here: deleted here. Changed here: ask.
    assert_eq!(decide(Some(&base), Some(&base), &Theirs::Deleted), Action::DeleteHere);
    assert_eq!(decide(Some(&base), Some(&changed), &Theirs::Deleted), Action::Conflict(ConflictKind::DeletedThere));
    // Deleted here, unchanged there: deleted there. Changed there: ask.
    assert_eq!(decide(Some(&base), None, &Theirs::Live(base.clone())), Action::DeleteOnShare);
    assert_eq!(decide(Some(&base), None, &Theirs::Live(changed.clone())), Action::Conflict(ConflictKind::DeletedHere));
    // Gone on both: forgotten. Never here, deleted there: nothing.
    assert_eq!(decide(Some(&base), None, &Theirs::Deleted), Action::Forget);
    assert_eq!(decide(None, None, &Theirs::Deleted), Action::Nothing);
    // Both reordered differently: ask.
    let (mine, theirs) = (list("Gym", "b a c"), list("Gym", "c a b"));
    let base = list("Gym", "a b c");
    assert_eq!(decide(Some(&base), Some(&mine), &Theirs::Live(theirs)), Action::Conflict(ConflictKind::Changed));
}

trait Joined {
    fn keys_joined(&self) -> String;
}

impl Joined for PlaylistData {
    fn keys_joined(&self) -> String {
        self.entries.iter().map(|e| e.key.as_str()).collect::<Vec<_>>().join(" ")
    }
}
