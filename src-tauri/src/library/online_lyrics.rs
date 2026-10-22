//! Lyrics from LRCLIB (lrclib.net: a free, open lyrics database, mostly timed lyrics) for songs that have none of their
//! own (no .lrc next to them, none in their tags). "Find lyrics online" (Playback settings, on by default) looks them up
//! on Wi-Fi only: when a song plays without lyrics, and in the background for every song (new ones first, after each
//! scan). What's found is kept (the online_lyrics table), so it works offline; "not found" too, asked again after a
//! month. LRCLIB is sent the song's artist, title, album and length, nothing else.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use super::{err, now_ms, Library, Result};

/// The library setting: "0" switches it off (on by default).
pub const SETTING: &str = "lyrics_online";
const GET: &str = "https://lrclib.net/api/get";
const SEARCH: &str = "https://lrclib.net/api/search";
/// A song not found is asked about again after this long (LRCLIB grows).
const ASK_AGAIN_MS: i64 = 30 * 86_400_000;
/// The background round: at most this often, and between songs a pause (LRCLIB is free: gently).
const ROUND: Duration = Duration::from_secs(30 * 60);
const BETWEEN: Duration = Duration::from_secs(1);
/// LRCLIB busy ("ServerOverloaded", 503: "retry in a moment"): tried again after these pauses.
const BUSY_RETRIES: [Duration; 3] = [Duration::from_secs(2), Duration::from_secs(4), Duration::from_secs(8)];

/// What LRCLIB has for a song.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// Its lyrics: timed (LRC) when there are, else plain.
    Lyrics(String),
    /// It has no words.
    Instrumental,
    NotFound,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Record {
    id: i64,
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

impl Record {
    fn synced(&self) -> Option<&str> {
        self.synced_lyrics.as_deref().filter(|t| !t.trim().is_empty())
    }

    fn found(self) -> Found {
        let text = |t: Option<String>| t.filter(|t| !t.trim().is_empty());
        match (text(self.synced_lyrics), text(self.plain_lyrics)) {
            (Some(lrc), _) | (None, Some(lrc)) => Found::Lyrics(lrc),
            _ if self.instrumental => Found::Instrumental,
            _ => Found::NotFound,
        }
    }
}

/// Whether timed lyrics can fit the song: no lines sung impossibly fast (more letters than can be sung before the next
/// line: two or more of those, a verse crammed into a moment, mean the timing is for another version), and none past the
/// song's end.
pub fn plausible(lrc: &str, duration_s: Option<f64>) -> bool {
    let Some(lyrics) = super::lyrics::parse(lrc, "online").filter(|l| l.synced) else { return true };
    let times: Vec<(i64, usize)> = lyrics.lines.iter().filter_map(|l| Some((l.at_ms?, l.text.chars().count()))).collect();
    let crammed = times
        .windows(2)
        .filter(|w| {
            let ((at, letters), (next, _)) = (w[0], w[1]);
            // About 25 letters a second is as fast as words are sung.
            letters >= 8 && next - at < (letters as i64 * 40).max(300)
        })
        .count();
    let past_end = duration_s.is_some_and(|d| times.last().is_some_and(|&(at, _)| at as f64 > d * 1000.0 + 2000.0));
    crammed < 2 && !past_end
}

/// A song as LRCLIB is asked about it.
pub struct Song {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
}

/// Asks LRCLIB at `url` with `query`, again after a pause while it says it's busy. Its status and body.
fn ask(agent: &ureq::Agent, url: &str, query: &[(&str, &str)]) -> std::result::Result<(u16, String), String> {
    let mut pauses = BUSY_RETRIES.iter();
    loop {
        let mut request = agent.get(url);
        for (key, value) in query {
            request = request.query(*key, *value);
        }
        let mut answer = request.call().map_err(|e| e.to_string())?;
        let status = answer.status().as_u16();
        if status == 503 {
            if let Some(pause) = pauses.next() {
                std::thread::sleep(*pause);
                continue;
            }
        }
        return Ok((status, answer.body_mut().read_to_string().map_err(|e| e.to_string())?));
    }
}

fn json<T: serde::de::DeserializeOwned>(body: &str) -> std::result::Result<T, String> {
    serde_json::from_str(body).map_err(|e| e.to_string())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(concat!("Melue/", env!("CARGO_PKG_VERSION"), " (a personal music player)"))
        .http_status_as_error(false)
        .build()
        .into()
}

/// LRCLIB's entries for a song, best first: the exact one (artist, title, album, length) and those a search by artist and
/// title finds of about the same length (within 8 s), ranked: timed lyrics whose timing fits the song, then other timed
/// ones, then plain ones; each group by how close its length is. Err: LRCLIB couldn't be reached (try again later).
fn entries(song: &Song) -> std::result::Result<Vec<Record>, String> {
    let agent = agent();
    let mut all: Vec<Record> = Vec::new();
    if let (Some(album), Some(duration)) = (&song.album, song.duration_s) {
        let duration = format!("{}", duration.round() as i64);
        let query = [("artist_name", song.artist.as_str()), ("track_name", &song.title), ("album_name", album), ("duration", &duration)];
        let (status, body) = ask(&agent, GET, &query)?;
        if status == 200 {
            all.push(json(&body)?);
        }
    }
    let (status, body) = ask(&agent, SEARCH, &[("artist_name", &song.artist), ("track_name", &song.title)])?;
    if status != 200 {
        return Err(format!("LRCLIB answered {status}"));
    }
    let found: Vec<Record> = json(&body)?;
    let off = |r: &Record| match (song.duration_s, r.duration) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => 0.0,
    };
    for record in found {
        if off(&record) <= 8.0 && !all.iter().any(|r| r.id == record.id) {
            all.push(record);
        }
    }
    let rank = |r: &Record| match r.synced() {
        Some(lrc) if plausible(lrc, song.duration_s) => 0,
        Some(_) => 1,
        None if r.plain_lyrics.as_deref().is_some_and(|t| !t.trim().is_empty()) => 2,
        None => 3,
    };
    // Stable: the exact entry stays ahead of its equals.
    all.sort_by(|a, b| rank(a).cmp(&rank(b)).then(off(a).total_cmp(&off(b))));
    Ok(all)
}

/// What LRCLIB has for a song (its best entry) and that entry's id.
pub fn look_up(song: &Song) -> std::result::Result<(Found, Option<i64>), String> {
    Ok(match entries(song)?.into_iter().next() {
        Some(best) => {
            let id = best.id;
            (best.found(), Some(id))
        }
        None => (Found::NotFound, None),
    })
}

/// The next of LRCLIB's entries with lyrics after `current` (round again after the last). None: there's no other.
pub fn other_version(song: &Song, current: Option<i64>) -> std::result::Result<Option<(Found, i64)>, String> {
    let has_lyrics = |r: &Record| r.synced().is_some() || r.plain_lyrics.as_deref().is_some_and(|t| !t.trim().is_empty());
    let with_lyrics: Vec<Record> = entries(song)?.into_iter().filter(has_lyrics).collect();
    if with_lyrics.len() < 2 && with_lyrics.first().is_some_and(|r| Some(r.id) == current) {
        return Ok(None);
    }
    let at = current.and_then(|id| with_lyrics.iter().position(|r| r.id == id));
    let next = match at {
        Some(i) => (i + 1) % with_lyrics.len(),
        None => 0,
    };
    Ok(with_lyrics.into_iter().nth(next).map(|r| {
        let id = r.id;
        (r.found(), id)
    }))
}

impl Library {
    pub fn online_lyrics_on(&self) -> Result<bool> {
        Ok(self.setting(SETTING)?.as_deref() != Some("0"))
    }

    /// What was found online for a song: None when it wasn't looked up (or "not found" is old enough to ask again).
    pub fn online_lyrics(&self, track_id: i64) -> Result<Option<Found>> {
        let row: Option<(Option<String>, bool, i64)> = self
            .conn()
            .query_row("SELECT lrc, instrumental, looked_at FROM online_lyrics WHERE track_id = ?1", [track_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
            .map_err(err)?;
        Ok(row.and_then(|(lrc, instrumental, at)| match lrc {
            Some(lrc) => Some(Found::Lyrics(lrc)),
            None if instrumental => Some(Found::Instrumental),
            None => (now_ms() - at < ASK_AGAIN_MS).then_some(Found::NotFound),
        }))
    }

    /// Which LRCLIB entry a song's lyrics came from.
    pub fn online_lyrics_entry(&self, track_id: i64) -> Result<Option<i64>> {
        Ok(self
            .conn()
            .query_row("SELECT lrclib_id FROM online_lyrics WHERE track_id = ?1", [track_id], |r| r.get(0))
            .optional()
            .map_err(err)?
            .flatten())
    }

    pub fn keep_online_lyrics(&self, track_id: i64, found: &Found, entry: Option<i64>) -> Result<()> {
        let (lrc, instrumental) = match found {
            Found::Lyrics(lrc) => (Some(lrc.as_str()), false),
            Found::Instrumental => (None, true),
            Found::NotFound => (None, false),
        };
        self.conn()
            .execute(
                "INSERT INTO online_lyrics (track_id, lrc, instrumental, looked_at, lrclib_id) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (track_id) DO UPDATE SET lrc = ?2, instrumental = ?3, looked_at = ?4, lrclib_id = ?5",
                params![track_id, lrc, instrumental, now_ms(), entry],
            )
            .map(|_| ())
            .map_err(err)
    }

    /// A song as LRCLIB is asked about it (None: no artist or title to ask with).
    pub fn lyrics_song(&self, track_id: i64) -> Result<Option<Song>> {
        let row: Option<(Option<String>, String, Option<String>, Option<i64>)> = self
            .conn()
            .query_row("SELECT artist_credit, title, tag_album, duration_ms FROM tracks WHERE id = ?1", [track_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .optional()
            .map_err(err)?;
        Ok(row.and_then(|(artist, title, album, ms)| {
            let artist = artist.filter(|a| !a.trim().is_empty())?;
            Some(Song { artist, title, album: album.filter(|a| !a.trim().is_empty()), duration_s: ms.map(|ms| ms as f64 / 1000.0) })
        }))
    }

    /// The songs to look up in the background: playable, not looked up yet (or not found a while ago), newest first.
    fn songs_to_look_up(&self) -> Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT t.id FROM tracks t JOIN folders f ON f.id = t.folder_id
                 LEFT JOIN online_lyrics o ON o.track_id = t.id
                 WHERE t.missing_since IS NULL AND t.copy_of IS NULL AND f.enabled AND t.artist_credit IS NOT NULL
                   AND (o.track_id IS NULL OR (o.lrc IS NULL AND NOT o.instrumental AND o.looked_at < ?1))
                 ORDER BY t.added_at DESC",
            )
            .map_err(err)?;
        let rows = stmt.query_map([now_ms() - ASK_AGAIN_MS], |r| r.get(0)).map_err(err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(err)
    }
}

/// Whether Melue may look lyrics up now: the setting on, and not on mobile data (Wi-Fi only).
pub fn may_look_up(app: &AppHandle) -> bool {
    let library = app.state::<Library>();
    library.online_lyrics_on().unwrap_or(false) && !crate::sync::mobile::metered(app)
}

/// Looks a song up now and keeps what's found (a song that plays without lyrics). Err: LRCLIB couldn't be reached.
pub fn look_up_now(app: &AppHandle, track_id: i64) -> std::result::Result<Option<Found>, String> {
    let library = app.state::<Library>();
    let Some(song) = library.lyrics_song(track_id)? else { return Ok(None) };
    let (found, entry) = look_up(&song)?;
    library.keep_online_lyrics(track_id, &found, entry)?;
    Ok(Some(found))
}

/// "Try another version": the song's lyrics from LRCLIB's next entry (its timing reset: it was for the other one).
/// Ok(false): LRCLIB has no other.
pub fn try_other_version(app: &AppHandle, track_id: i64) -> std::result::Result<bool, String> {
    let library = app.state::<Library>();
    let Some(song) = library.lyrics_song(track_id)? else { return Ok(false) };
    let Some((found, entry)) = other_version(&song, library.online_lyrics_entry(track_id)?)? else { return Ok(false) };
    library.keep_online_lyrics(track_id, &found, Some(entry))?;
    library.set_lyrics_timing(track_id, &super::lyrics::Timing::default(), now_ms())?;
    Ok(true)
}

#[derive(Default)]
pub struct LyricsLookup {
    soon: Mutex<Option<Sender<()>>>,
}

/// New songs may be there (a scan ended, the network changed): a round soon.
pub fn soon(app: &AppHandle) {
    if let Some(lookup) = app.try_state::<LyricsLookup>() {
        if let Some(soon) = lookup.soon.lock().unwrap().as_ref() {
            let _ = soon.send(());
        }
    }
}

/// The background rounds: after start-up, then after scans and every half hour.
pub fn start(app: &AppHandle) {
    let (sender, wake) = mpsc::channel::<()>();
    *app.state::<LyricsLookup>().soon.lock().unwrap() = Some(sender);
    let app = app.clone();
    std::thread::Builder::new()
        .name("melue-lyrics".into())
        .spawn(move || {
            crate::startup::wait_for_ui(crate::startup::UI_AT_MOST);
            loop {
                round(&app);
                match wake.recv_timeout(ROUND) {
                    Ok(()) => while wake.try_recv().is_ok() {},
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        })
        .expect("the lyrics lookup thread");
}

/// Looks up the songs without lyrics, one by one, while it may (Wi-Fi, the setting on). Stops at the first network
/// failure (offline: the next round tries again).
fn round(app: &AppHandle) {
    let library = app.state::<Library>();
    let Ok(songs) = library.songs_to_look_up() else { return };
    let (mut found, mut asked) = (0, 0);
    for id in songs {
        if !may_look_up(app) {
            break;
        }
        // Its own lyrics (a .lrc, its tags) come first: nothing to look up (kept as looked at, so its file isn't
        // opened again every round; checked again in a month).
        if library.own_lyrics(id, &|job, path| super::scanner::open_file(app, job, path)).ok().flatten().is_some() {
            let _ = library.keep_online_lyrics(id, &Found::NotFound, None);
            continue;
        }
        match look_up_now(app, id) {
            Ok(Some(Found::Lyrics(_))) => found += 1,
            Ok(_) => {}
            Err(e) => {
                eprintln!("melue: lyrics: LRCLIB can't be reached now: {e}");
                break;
            }
        }
        asked += 1;
        std::thread::sleep(BETWEEN);
    }
    if asked > 0 {
        eprintln!("melue: lyrics: looked up {asked} songs online, found {found}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LRCLIB itself (needs the internet; run with --ignored). Says only what kind of answer came, not the lyrics.
    #[test]
    fn timing_that_cant_be_sung_is_another_versions() {
        let fine = "[00:10.00]A line of some length here
[00:14.00]Another line of some length
[00:18.00]End";
        assert!(plausible(fine, Some(200.0)));
        let crammed = "[00:10.00]A line of some length here
[00:10.20]Another line of some length
[00:10.40]And a third one sung at once
[00:10.60]x";
        assert!(!plausible(crammed, Some(200.0)), "a verse in a moment");
        assert!(!plausible(fine, Some(12.0)), "past the song's end");
        assert!(plausible("just plain words", Some(10.0)), "untimed lyrics aren't judged");
    }

    #[test]
    #[ignore]
    fn lrclib_answers() {
        let song = Song { artist: "Daft Punk".into(), title: "Get Lucky".into(), album: None, duration_s: Some(248.0) };
        match look_up(&song).unwrap() {
            (Found::Lyrics(text), id) => println!("found: {} lines, timed: {}, entry {id:?}", text.lines().count(), text.contains("[0")),
            other => println!("answer: {other:?}"),
        }
        let nothing = Song { artist: "Nobody at all 7f3k".into(), title: "No such song 9q2x".into(), album: None, duration_s: None };
        assert_eq!(look_up(&nothing).unwrap(), (Found::NotFound, None));
    }
}
