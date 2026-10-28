//! Merging a playlist changed on two devices: from the version both started from (the last sync, "base"),
//! this device's ("mine") and the share's ("theirs"). What only one side changed is taken from it; when both
//! changed it:
//! - songs added on either side are kept, songs removed on either side are removed (the same song can be in
//!   a playlist more than once: counted);
//! - the order is the side's that reordered (both reordered differently: a conflict);
//! - the name is the side's that renamed (both renamed differently: a conflict).
//!
//! Songs are compared by their key: their path on the share, or for a song only on one device
//! "local:<device>/<id>", or a Spotify playlist's song by its line from Spotify ("spotify:…"). A Spotify song
//! added on both sides is the same change (both read it from Spotify): it's in once. A Spotify playlist's
//! reference (Spotify's list as last read) is the side's that changed it; both: the newest read, and a side that
//! stopped following Spotify wins.
//!
//! [`decide`] adds playlists made, deleted or missing on either side.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::library::spotify::SpotifyRef;

/// A song in a synced playlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEntry {
    pub key: String,
    /// To show it on a device that doesn't have it.
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    /// Its length, to recognise the same song elsewhere on a device (see same_song.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// A Spotify playlist's song chosen by hand (Find song): that song's key, used for it on every device that has
    /// it (the automatic match didn't take it: another version, say).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chosen: Option<String>,
}

/// A synced playlist's name and songs, and a Spotify playlist's reference.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistData {
    pub name: String,
    pub entries: Vec<SyncEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spotify: Option<SpotifyRef>,
}

impl PlaylistData {
    fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    /// The same name and songs in the same order (titles aside: they can differ between devices), and the same
    /// Spotify reference.
    pub fn same(&self, other: &PlaylistData) -> bool {
        self.name == other.name && self.keys() == other.keys() && self.spotify == other.spotify
    }
}

/// The share's version of a playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Theirs {
    /// No file (never uploaded, or the share's files were cleaned up).
    Missing,
    /// Deleted on a device.
    Deleted,
    Live(PlaylistData),
}

/// What to do with a playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Nothing,
    /// The result goes here and/or to the share (where it differs), and becomes the base.
    Use { result: PlaylistData, here: bool, share: bool },
    /// Deleted on another device, unchanged here: deleted here too (restorable).
    DeleteHere,
    /// Deleted here, unchanged on the share: deleted there too.
    DeleteOnShare,
    /// Gone on both sides: the base is forgotten.
    Forget,
    Conflict(ConflictKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    /// Both changed it in ways that don't merge (both reordered, or renamed it differently).
    Changed,
    /// Deleted on another device, changed here.
    DeletedThere,
    /// Deleted here, changed on another device.
    DeletedHere,
}

impl ConflictKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::DeletedThere => "deleted_there",
            Self::DeletedHere => "deleted_here",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Changed, Self::DeletedThere, Self::DeletedHere].into_iter().find(|k| k.as_str() == value)
    }
}

/// What to do with a playlist, from its base (None: never synced here), this device's version (None: not
/// here) and the share's.
pub fn decide(base: Option<&PlaylistData>, mine: Option<&PlaylistData>, theirs: &Theirs) -> Action {
    let using = |result: PlaylistData, mine: Option<&PlaylistData>, theirs: Option<&PlaylistData>| {
        let here = !mine.is_some_and(|m| m.same(&result));
        let share = !theirs.is_some_and(|t| t.same(&result));
        if here || share || base.is_none_or(|b| !b.same(&result)) {
            Action::Use { result, here, share }
        } else {
            Action::Nothing
        }
    };
    match (base, mine, theirs) {
        // Never synced here.
        (None, None, Theirs::Live(t)) => using(t.clone(), None, Some(t)),
        (None, None, _) => Action::Nothing,
        // Made here (or restored here over a deletion): it goes to the share.
        (None, Some(m), Theirs::Missing | Theirs::Deleted) => using(m.clone(), Some(m), None),
        // Made on two devices with the same name before syncing: merged, songs of both kept.
        (None, Some(m), Theirs::Live(t)) => match merge(&PlaylistData::default(), m, t) {
            Some(result) => using(result, Some(m), Some(t)),
            None => Action::Conflict(ConflictKind::Changed),
        },
        (Some(b), Some(m), Theirs::Live(t)) => match merge(b, m, t) {
            Some(result) => using(result, Some(m), Some(t)),
            None => Action::Conflict(ConflictKind::Changed),
        },
        (Some(b), Some(m), Theirs::Deleted) => {
            if m.same(b) {
                Action::DeleteHere
            } else {
                Action::Conflict(ConflictKind::DeletedThere)
            }
        }
        (Some(_), Some(m), Theirs::Missing) => using(m.clone(), Some(m), None),
        // Deleted here.
        (Some(b), None, Theirs::Live(t)) => {
            if t.same(b) {
                Action::DeleteOnShare
            } else {
                Action::Conflict(ConflictKind::DeletedHere)
            }
        }
        (Some(_), None, _) => Action::Forget,
    }
}

/// The merged playlist; None when both sides' changes don't go together.
pub fn merge(base: &PlaylistData, mine: &PlaylistData, theirs: &PlaylistData) -> Option<PlaylistData> {
    let name = if mine.name == base.name || mine.name == theirs.name {
        theirs.name.clone()
    } else if theirs.name == base.name {
        mine.name.clone()
    } else {
        return None;
    };
    let keys = merge_keys(&base.keys(), &mine.keys(), &theirs.keys())?;
    // Titles from whichever side has the song.
    let known: HashMap<&str, &SyncEntry> =
        base.entries.iter().chain(&theirs.entries).chain(&mine.entries).map(|e| (e.key.as_str(), e)).collect();
    let entries = keys.into_iter().map(|k| known[k].clone()).collect();
    let spotify = if mine.spotify == base.spotify {
        theirs.spotify.clone()
    } else if theirs.spotify == base.spotify {
        mine.spotify.clone()
    } else {
        match (&mine.spotify, &theirs.spotify) {
            (Some(m), Some(t)) => Some(if t.read_at > m.read_at { t } else { m }.clone()),
            _ => None,
        }
    };
    Some(PlaylistData { name, entries, spotify })
}

fn counts<'a>(keys: &[&'a str]) -> HashMap<&'a str, usize> {
    let mut counts = HashMap::new();
    for k in keys {
        *counts.entry(*k).or_insert(0) += 1;
    }
    counts
}

/// `a`'s songs that `b` has too (as many times as both have them), in `a`'s order.
fn common_order<'a>(a: &[&'a str], b: &[&str]) -> Vec<&'a str> {
    let mut left = counts(b);
    a.iter()
        .copied()
        .filter(|k| match left.get_mut(k) {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        })
        .collect()
}

/// Whether `side` put songs it shares with `base` in another order.
fn reordered(base: &[&str], side: &[&str]) -> bool {
    common_order(side, base) != common_order(base, side)
}

/// The merged songs (keys); None when both sides reordered them differently.
pub fn merge_keys<'a>(base: &[&'a str], mine: &[&'a str], theirs: &[&'a str]) -> Option<Vec<&'a str>> {
    if mine == base {
        return Some(theirs.to_vec());
    }
    if theirs == base || mine == theirs {
        return Some(mine.to_vec());
    }
    // Made on both devices (nothing to start from): this device's songs, then the other's that aren't in it.
    if base.is_empty() {
        let mut left = counts(mine);
        let mut result = mine.to_vec();
        for k in theirs {
            match left.get_mut(k) {
                Some(n) if *n > 0 => *n -= 1,
                _ => result.push(k),
            }
        }
        return Some(result);
    }
    let (mine_moved, theirs_moved) = (reordered(base, mine), reordered(base, theirs));
    if mine_moved && theirs_moved && common_order(mine, theirs) != common_order(theirs, mine) {
        return None;
    }
    // The order comes from the side that reordered (or this device's); the other side's additions go in after
    // the song before them on that side.
    let (order, other) = if theirs_moved { (theirs, mine) } else { (mine, theirs) };
    let (b, m, t) = (counts(base), counts(mine), counts(theirs));
    let count = |c: &HashMap<&str, usize>, k: &str| c.get(k).copied().unwrap_or(0);
    // How many times each song ends up in it: added on either side, removed on either side. A Spotify song added
    // on both: read from Spotify by both, once.
    let target = |k: &str| {
        let (b, m, t) = (count(&b, k), count(&m, k), count(&t, k));
        if k.starts_with("spotify:") && m > b && t > b {
            m.max(t)
        } else {
            (m + t).saturating_sub(b)
        }
    };

    let mut result: Vec<&str> = Vec::new();
    let mut have: HashMap<&str, usize> = HashMap::new();
    for k in order {
        let n = have.entry(k).or_insert(0);
        if *n < target(k) {
            *n += 1;
            result.push(k);
        }
    }
    // The other side's songs, in its order: the ones it had before (from the base) mark where its additions go
    // in, after them.
    let mut from_base = b.clone();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut after: Option<usize> = None;
    for k in other {
        let kept = match from_base.get_mut(k) {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        };
        if kept {
            let nth = seen.entry(k).or_insert(0);
            *nth += 1;
            if let Some(i) = result.iter().enumerate().filter(|(_, r)| **r == *k).map(|(i, _)| i).nth(*nth - 1) {
                after = Some(i);
            }
        } else if have.get(k).copied().unwrap_or(0) < target(k) {
            let at = after.map_or(0, |i| i + 1);
            result.insert(at, k);
            *have.entry(k).or_insert(0) += 1;
            after = Some(at);
        }
    }
    Some(result)
}
