//! The same song on two devices. A song on the share is known by its path there. A copy of it elsewhere (in
//! another music folder, like the phone's music from before the share was set up) is recognised by its artist,
//! title and length (within 2 seconds): playlists then send it as the share's song, and a device that has the
//! song only elsewhere plays its own copy.

use std::collections::HashMap;

use super::handoff::{key, share_path};
use super::playlist_merge::SyncEntry;
use crate::library::{err, Folder, Library, Result};

/// Lengths closer than this are the same song.
const SAME_LENGTH_MS: i64 = 2_000;

/// A name for comparing: lower case, spaces tidied.
fn plain(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Words that mark a version of the same recording, not another song: "(Remastered 2009)", "- Radio Edit".
/// ("Live", "Acoustic" or "Remix" are left: those are other recordings; the length tells versions apart too.)
const VERSION_WORDS: [&str; 9] = ["remaster", "radio edit", "single version", "album version", "mono", "stereo", "explicit", "clean", "deluxe"];

/// Featured artists: "feat. X", "ft. X", "featuring X".
const FEATURING: [&str; 4] = ["feat.", "feat ", "ft.", "featuring "];

fn is_version_note(note: &str) -> bool {
    let lower = note.to_lowercase();
    if FEATURING.iter().any(|f| lower.trim_start().starts_with(f)) {
        return true;
    }
    // Whole words ("mono", not "Monologue"); "remaster" also as "remastered".
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let spaced = format!(" {} ", words.join(" "));
    VERSION_WORDS.iter().any(|w| if *w == "remaster" { words.iter().any(|x| x.starts_with(w)) } else { spaced.contains(&format!(" {w} ")) })
}

/// A title for comparing: without version notes ("Karma Chameleon - Remastered 2002" and "Karma Chameleon
/// (Radio Edit)" are "karma chameleon") and featured artists.
pub fn plain_title(title: &str) -> String {
    let mut t = title.trim().to_string();
    // Notes in brackets: "(Remastered 2009)", "[Radio Edit]", "(feat. X)".
    loop {
        let found = t.char_indices().filter(|(_, c)| matches!(c, '(' | '[')).find_map(|(open, c)| {
            let close = if c == '(' { ')' } else { ']' };
            let end = open + t[open..].find(close)?;
            is_version_note(&t[open + 1..end]).then_some((open, end + 1))
        });
        match found {
            Some((from, to)) => t.replace_range(from..to, " "),
            None => break,
        }
    }
    // A note after a dash: "- Remastered 2002", "- Single Version".
    if let Some(dash) = t.rfind(" - ") {
        if is_version_note(&t[dash + 3..]) {
            t.truncate(dash);
        }
    }
    // A featured artist in the title: "Song feat. X".
    let lower = t.to_lowercase();
    if let Some(at) = FEATURING.iter().filter_map(|f| lower.find(&format!(" {f}"))).min() {
        t.truncate(at);
    }
    plain(&t)
}

/// An artist for comparing: the main artist ("Band feat. X" is "band").
pub fn plain_artist(artist: &str) -> String {
    let lower = artist.to_lowercase();
    let main = FEATURING.iter().filter_map(|f| lower.find(&format!(" {f}"))).min().map_or(artist, |at| &artist[..at]);
    plain(main)
}

/// Whether two songs are the same, by artist, title and length (an unknown length doesn't count against it).
pub fn same_song(a: (&str, &str, Option<i64>), b: (&str, &str, Option<i64>)) -> bool {
    let lengths_match = match (a.2, b.2) {
        (Some(x), Some(y)) => (x - y).abs() <= SAME_LENGTH_MS,
        _ => true,
    };
    !plain_title(a.1).is_empty() && plain_artist(a.0) == plain_artist(b.0) && plain_title(a.1) == plain_title(b.1) && lengths_match
}

struct Song {
    artist: String,
    title: String,
    duration_ms: Option<i64>,
    /// In the synced folder: its path on the share.
    share_path: Option<String>,
}

/// The songs here, for matching playlist songs.
pub struct Songs {
    me: String,
    songs: HashMap<i64, Song>,
    /// The synced folder's songs by path on the share (compared without case).
    by_path: HashMap<String, i64>,
    /// Every song by (artist, title), plain.
    by_name: HashMap<(String, String), Vec<i64>>,
}

impl Songs {
    /// The key a song goes to the share with: its path there, or the same song's in the synced folder, or (only
    /// on this device) "local:<device>/<id>".
    pub fn key_of(&self, track: i64) -> String {
        let Some(song) = self.songs.get(&track) else { return format!("local:{}/{track}", self.me) };
        if let Some(path) = &song.share_path {
            return path.clone();
        }
        self.same_as(&song.artist, &song.title, song.duration_ms)
            .find_map(|other| self.songs[&other].share_path.clone())
            .unwrap_or_else(|| format!("local:{}/{track}", self.me))
    }

    /// The synced folder's song a song is a copy of (None: it's the synced song itself, or there's none).
    pub fn synced_copy(&self, track: i64) -> Option<i64> {
        let song = self.songs.get(&track)?;
        if song.share_path.is_some() {
            return None;
        }
        self.same_as(&song.artist, &song.title, song.duration_ms).filter(|id| self.songs[id].share_path.is_some()).min()
    }

    /// Local songs that are almost a synced song (the same title, but another artist or length), described:
    /// why a copy wasn't recognised.
    pub fn near_misses(&self) -> Vec<String> {
        let mut synced_by_title: HashMap<String, Vec<&Song>> = HashMap::new();
        for song in self.songs.values().filter(|s| s.share_path.is_some()) {
            synced_by_title.entry(plain_title(&song.title)).or_default().push(song);
        }
        let seconds = |ms: Option<i64>| ms.map_or("?".to_string(), |ms| format!("{:.1} s", ms as f64 / 1000.0));
        let mut misses = Vec::new();
        for (&id, song) in self.songs.iter().filter(|(_, s)| s.share_path.is_none()) {
            if self.synced_copy(id).is_some() {
                continue;
            }
            for synced in synced_by_title.get(&plain_title(&song.title)).into_iter().flatten() {
                misses.push(format!(
                    "\"{}\": here by \"{}\", {}; synced by \"{}\", {}",
                    song.title,
                    song.artist,
                    seconds(song.duration_ms),
                    synced.artist,
                    seconds(synced.duration_ms)
                ));
            }
        }
        misses.sort();
        misses
    }

    /// How many songs there are: (in the synced folder, elsewhere).
    pub fn counts(&self) -> (usize, usize) {
        let synced = self.songs.values().filter(|s| s.share_path.is_some()).count();
        (synced, self.songs.len() - synced)
    }

    /// The synced folder's song at a path on the share.
    pub fn at_path(&self, path: &str) -> Option<i64> {
        self.by_path.get(&key(path)).copied()
    }

    /// The song here for a playlist song: by its path on the share, as this device's own, or the same song
    /// (by artist, title and length; the synced folder's first). Also whether it's the song itself (not a copy).
    pub fn find(&self, entry: &SyncEntry) -> Option<(i64, bool)> {
        if let Some(&id) = self.by_path.get(&key(&entry.key)) {
            return Some((id, true));
        }
        if let Some(own) = entry.key.strip_prefix(&format!("local:{}/", self.me)) {
            if let Some(id) = own.parse().ok().filter(|id| self.songs.contains_key(id)) {
                return Some((id, true));
            }
        }
        let (artist, title) = (entry.artist.as_deref()?, entry.title.as_deref()?);
        let mut found: Vec<i64> = self.same_as(artist, title, entry.duration_ms).collect();
        found.sort_by_key(|id| (self.songs[id].share_path.is_none(), *id));
        found.first().map(|&id| (id, false))
    }

    /// Songs with this artist, title and length.
    fn same_as<'a>(&'a self, artist: &str, title: &str, duration_ms: Option<i64>) -> impl Iterator<Item = i64> + 'a {
        let wanted = (artist.to_string(), title.to_string(), duration_ms);
        self.by_name.get(&(plain_artist(artist), plain_title(title))).into_iter().flatten().copied().filter(move |id| {
            let s = &self.songs[id];
            same_song((&wanted.0, &wanted.1, wanted.2), (&s.artist, &s.title, s.duration_ms))
        })
    }
}

impl Library {
    /// The songs here (there, in switched-on folders), with the synced folder's paths on the share.
    pub fn songs_here(&self, folder: &Folder, me: &str) -> Result<Songs> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.folder_id, t.path, t.title, coalesce(t.artist_credit, ''), t.duration_ms
                 FROM tracks t JOIN folders f ON f.id = t.folder_id WHERE t.missing_since IS NULL AND f.enabled",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })
            .map_err(err)?;
        let mut songs = Songs { me: me.to_string(), songs: HashMap::new(), by_path: HashMap::new(), by_name: HashMap::new() };
        for row in rows {
            let (id, folder_id, path, title, artist, duration_ms): (i64, i64, String, String, String, Option<i64>) = row.map_err(err)?;
            let share_path = if folder_id == folder.id { share_path(folder, &path) } else { None };
            if let Some(p) = &share_path {
                songs.by_path.insert(key(p), id);
            }
            songs.by_name.entry((plain_artist(&artist), plain_title(&title))).or_default().push(id);
            songs.songs.insert(id, Song { artist, title, duration_ms, share_path });
        }
        Ok(songs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_song_by_artist_title_and_length() {
        let song = ("ABBA", "Waterloo", Some(165_000));
        assert!(same_song(song, ("abba", "  waterloo ", Some(166_500))), "case, spaces and a second apart");
        assert!(!same_song(song, ("ABBA", "Waterloo", Some(170_000))), "another length: another recording");
        assert!(!same_song(song, ("ABBA", "Mamma Mia", Some(165_000))));
        assert!(!same_song(song, ("Other", "Waterloo", Some(165_000))));
        assert!(same_song(song, ("ABBA", "Waterloo", None)), "a length not known");
        assert!(!same_song(("ABBA", "", None), ("ABBA", "", None)), "no title: nothing to go by");
    }

    #[test]
    fn version_notes_and_featured_artists_dont_count() {
        let same = |a: (&str, &str), b: (&str, &str)| same_song((a.0, a.1, Some(200_000)), (b.0, b.1, Some(200_000)));
        assert!(same(("Culture Club", "Karma Chameleon - Remastered 2002"), ("Culture Club", "Karma Chameleon")));
        assert!(same(("Culture Club", "Karma Chameleon (2002 Remaster)"), ("Culture Club", "Karma Chameleon [Radio Edit]")));
        assert!(same(("Calvin Harris feat. Rihanna", "This Is What You Came For"), ("Calvin Harris", "This Is What You Came For (feat. Rihanna)")));
        assert!(same(("Band", "Song ft. Someone"), ("Band", "Song")));
        // Other recordings stay apart (and the length does its part).
        assert!(!same(("Band", "Song (Live)"), ("Band", "Song")));
        assert!(!same(("Band", "Song - Acoustic"), ("Band", "Song")));
        assert!(!same(("Band", "Song (Remix)"), ("Band", "Song")));
        // Brackets that are part of the title stay.
        assert!(!same(("Elton John", "Rocket Man (I Think It's Going To Be A Long, Long Time)"), ("Elton John", "Rocket Man")));
        assert_eq!(plain_title("Rocket Man (I Think It's Going To Be A Long, Long Time)"), "rocket man (i think it's going to be a long, long time)");
        assert_eq!(plain_title("Should I Stay or Should I Go - Remastered"), "should i stay or should i go");
        assert_eq!(plain_title("Intro (Monologue)"), "intro (monologue)", "whole words only");
    }
}
