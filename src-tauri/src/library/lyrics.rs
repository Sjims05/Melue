//! A song's lyrics, for Now playing: from a `.lrc` file next to it (same name), or else from its tags (USLT / LYRICS),
//! or else found online (online_lyrics.rs: LRCLIB).
//! Timed lyrics (LRC: `[01:02.50]a line`) follow the song line by line; enhanced LRC (`<01:02.50>a <01:03.10>word`)
//! word by word. A timed line without word times gets them estimated (spread over how long it's sung, by each word's
//! length), once, when the lyrics are read: nothing is written back. Lyrics without times are shown as they are.

use std::io::Read;

use lofty::prelude::*;
use serde::Serialize;

use super::folders::FolderKind;
use super::tags::{Opener, TagJob};
use super::{err, Library, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// Its lines have times (they follow the song).
    pub synced: bool,
    pub lines: Vec<Line>,
    /// Where they're from: "file" (the .lrc), "tags" or "online" (LRCLIB).
    pub source: &'static str,
    /// The song has no words (LRCLIB says so): no lines.
    pub instrumental: bool,
    /// Moved by hand for this song (ms; positive: later). The times above don't include it.
    pub offset_ms: i64,
    /// Lines tapped as they started being sung (the times above don't include them: the app applies them).
    pub anchors: Vec<Anchor>,
    /// The times are a rough guess (lyrics without any: [`estimate_untimed`]).
    pub estimated: bool,
}

/// A line tapped as it started being sung ("Sync by tapping"): it, and the lines after it, follow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    pub line: usize,
    pub at_ms: i64,
}

/// A song's lyrics timing set by hand: moved earlier or later, and lines tapped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Timing {
    pub offset_ms: i64,
    pub anchors: Vec<Anchor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// When it starts (ms), for timed lyrics.
    pub at_ms: Option<i64>,
    pub text: String,
    /// Its words with their times (enhanced LRC, or estimated: see [`estimate_words`]); empty for plain lyrics.
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub at_ms: i64,
    /// The word with the space after it, as written.
    pub text: String,
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss:xx` (ms).
fn time(text: &str) -> Option<i64> {
    let (minutes, rest) = text.split_once(':')?;
    let (seconds, fraction) = match rest.split_once(['.', ':']) {
        Some((s, f)) => (s, f),
        None => (rest, ""),
    };
    let minutes: i64 = minutes.trim().parse().ok()?;
    let seconds: i64 = seconds.trim().parse().ok()?;
    let fraction: String = fraction.chars().take_while(char::is_ascii_digit).collect();
    let ms = match fraction.len() {
        0 => 0,
        n => fraction.parse::<i64>().ok()? * 1000 / 10_i64.pow(n as u32),
    };
    (seconds < 60).then_some(minutes * 60_000 + seconds * 1000 + ms)
}

/// A line's words, from enhanced LRC's `<mm:ss.xx>` marks (empty without them).
fn words(text: &str, offset: i64) -> (String, Vec<Word>) {
    if !text.contains('<') {
        return (text.trim().to_string(), Vec::new());
    }
    let mut words = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    let mut at = None;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('<') {
            if let Some((stamp, tail)) = after.split_once('>') {
                if let Some(t) = time(stamp) {
                    at = Some(t + offset);
                    rest = tail;
                    continue;
                }
            }
        }
        // Up to the next mark (past this piece's first character, which may be a '<' that wasn't a time).
        let end = rest.char_indices().skip(1).find(|&(_, c)| c == '<').map_or(rest.len(), |(i, _)| i);
        let piece = &rest[..end];
        plain.push_str(piece);
        if let Some(at) = at.take() {
            if !piece.trim().is_empty() {
                words.push(Word { at_ms: at, text: piece.to_string() });
            }
        } else if let Some(last) = words.last_mut() {
            last.text.push_str(piece);
        }
        rest = &rest[end..];
    }
    (plain.trim().to_string(), words)
}

/// Lyrics from a text: LRC (timed, when its lines have times), or plain lines.
pub fn parse(text: &str, source: &'static str) -> Option<Lyrics> {
    let text = text.trim_start_matches('\u{feff}');
    let mut offset = 0;
    let mut timed = Vec::new();
    let mut plain = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(after) = rest.strip_prefix('[') {
            let Some((tag, tail)) = after.split_once(']') else { break };
            if let Some(t) = time(tag) {
                times.push(t);
            } else if let Some(value) = tag.strip_prefix("offset:") {
                // A positive offset shows the lyrics sooner.
                offset = -value.trim().parse::<i64>().unwrap_or(0);
            }
            // Other tags ([ar:...], [ti:...]) say who and what: not lyrics.
            rest = tail.trim_start();
        }
        if times.is_empty() {
            if !raw.trim_start().starts_with('[') {
                plain.push(raw.trim().to_string());
            }
            continue;
        }
        for t in times {
            let (text, words) = words(rest, offset);
            timed.push(Line { at_ms: Some((t + offset).max(0)), text, words });
        }
    }
    if !timed.is_empty() {
        timed.sort_by_key(|l| l.at_ms);
        estimate_words(&mut timed);
        return Some(Lyrics { synced: true, lines: timed, source, instrumental: false, offset_ms: 0, anchors: Vec::new(), estimated: false });
    }
    // Plain lyrics: without the blank lines at the ends.
    while plain.first().is_some_and(String::is_empty) {
        plain.remove(0);
    }
    while plain.last().is_some_and(String::is_empty) {
        plain.pop();
    }
    (!plain.is_empty()).then(|| Lyrics {
        synced: false,
        lines: plain.into_iter().map(|text| Line { at_ms: None, text, words: Vec::new() }).collect(),
        source,
        instrumental: false,
        offset_ms: 0,
        anchors: Vec::new(),
        estimated: false,
    })
}

/// How long a line is sung: about this per syllable, plus a little, and never less than the floor.
const MS_PER_SYLLABLE: i64 = 280;
const LINE_EXTRA_MS: i64 = 300;
const LINE_FLOOR_MS: i64 = 1000;

/// Short words that carry little and are sung quickly (English and Danish).
const LIGHT: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "to", "of", "in", "on", "at", "for", "with", "by", "from", "is", "am", "are",
    "was", "be", "it", "its", "it's", "i", "i'm", "you", "me", "my", "your", "we", "he", "she", "they", "that", "this",
    "as", "so", "if", "do", "no", "not", "oh", "og", "i", "på", "at", "det", "er", "en", "et", "til", "med", "af",
    "den", "de", "som", "jeg", "du", "vi", "han", "hun", "har", "mig", "dig", "for", "så", "men",
];

/// A word's syllables, roughly: its groups of vowels (a silent final "e" in a short word, "time", "love", doesn't count;
/// in longer ones it often does: Danish "brænde"), at least one.
fn syllables(word: &str) -> i64 {
    let letters: Vec<char> = word.to_lowercase().chars().filter(|c| c.is_alphabetic()).collect();
    let vowel = |c: char| "aeiouyæøåäöüéèêáàíóú".contains(c);
    let mut groups = 0;
    let mut before = false;
    for &c in &letters {
        let v = vowel(c);
        if v && !before {
            groups += 1;
        }
        before = v;
    }
    if groups > 1 && letters.len() <= 5 && letters.last() == Some(&'e') && !vowel(letters[letters.len() - 2]) {
        groups -= 1;
    }
    groups.max(1)
}

/// How much of a line's time a word gets: its syllables, less for a light word, more held before a comma, and the
/// line's last word held longest (singers hold the ends of lines).
fn weight(word: &str, last: bool) -> f64 {
    let bare: String = word.trim().trim_matches(|c: char| !c.is_alphanumeric() && c != '\'').to_lowercase();
    let mut w = syllables(&bare) as f64;
    if LIGHT.contains(&bare.as_str()) {
        w *= 0.6;
    }
    if word.trim_end().ends_with([',', ';', ':']) {
        w *= 1.3;
    }
    if last {
        w *= 1.6;
    }
    w
}

/// Word times for timed lines without them: the line's time (until the next line, or as long as such a line is
/// sung, whichever is shorter) shared out by [`weight`].
pub fn estimate_words(lines: &mut [Line]) {
    for i in 0..lines.len() {
        if !lines[i].words.is_empty() || lines[i].text.is_empty() {
            continue;
        }
        let Some(start) = lines[i].at_ms else { continue };
        let next = lines[i + 1..].iter().find_map(|l| l.at_ms).unwrap_or(i64::MAX);
        // Each word with the space after it.
        let pieces: Vec<String> = lines[i].text.split_inclusive(' ').map(str::to_string).collect();
        let syllables_in_line: i64 = pieces.iter().map(|p| syllables(p)).sum();
        let sung = (syllables_in_line * MS_PER_SYLLABLE + LINE_EXTRA_MS).max(LINE_FLOOR_MS);
        let length = (next - start).min(sung).max(1) as f64;
        let weights: Vec<f64> = pieces.iter().enumerate().map(|(j, p)| weight(p, j + 1 == pieces.len())).collect();
        let total: f64 = weights.iter().sum();
        let mut done = 0.0;
        lines[i].words = pieces
            .into_iter()
            .zip(&weights)
            .map(|(text, w)| {
                let at = start + (length * done / total) as i64;
                done += w;
                Word { at_ms: at, text }
            })
            .collect();
    }
}

/// The part of a song left out of rough timing: an intro (8%, at most 15 s) and an outro (10%).
const INTRO_SHARE: f64 = 0.08;
const INTRO_MAX_MS: f64 = 15_000.0;
const OUTRO_SHARE: f64 = 0.10;
/// A blank line between verses counts as this many syllables: an instrumental break.
const BREAK_SYLLABLES: i64 = 8;

/// Rough times for lyrics without any, from the song's length (ms): its lines spread over the song by their syllables,
/// after an intro and before an outro, a blank line counting as a break. Words get times too ([`estimate_words`]).
/// False: no length to spread them over (they stay untimed).
pub fn estimate_untimed(lines: &mut [Line], duration_ms: Option<i64>) -> bool {
    let Some(duration) = duration_ms.filter(|&d| d > 10_000).map(|d| d as f64) else { return false };
    if lines.iter().all(|l| l.text.is_empty()) {
        return false;
    }
    let intro = (duration * INTRO_SHARE).min(INTRO_MAX_MS);
    let span = duration - intro - duration * OUTRO_SHARE;
    let weights: Vec<i64> = lines.iter().map(|l| if l.text.is_empty() { BREAK_SYLLABLES } else { syllables_in(&l.text) }).collect();
    let total = weights.iter().sum::<i64>().max(1) as f64;
    let mut done = 0;
    for (line, w) in lines.iter_mut().zip(&weights) {
        line.at_ms = Some((intro + span * done as f64 / total) as i64);
        done += w;
    }
    estimate_words(lines);
    true
}

fn syllables_in(text: &str) -> i64 {
    text.split_whitespace().map(syllables).sum::<i64>().max(1)
}

/// A text file's contents: UTF-8, or else Latin-1 (older .lrc files).
fn read_text(mut file: std::fs::File) -> Option<String> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8(bytes).unwrap_or_else(|e| e.into_bytes().iter().map(|&b| b as char).collect()))
}

impl Library {
    /// A song's lyrics: its own, or those found online before (`open` opens its files: see scanner.rs's open_file),
    /// with the timing set by hand. Lyrics without times get rough ones ([`estimate_untimed`]). None: none (yet).
    pub fn lyrics(&self, track_id: i64, open: &Opener) -> Result<Option<Lyrics>> {
        let Some(mut found) = self.found_lyrics(track_id, open)? else { return Ok(None) };
        if !found.synced && !found.instrumental {
            let duration: Option<i64> =
                self.conn().query_row("SELECT duration_ms FROM tracks WHERE id = ?1", [track_id], |r| r.get(0)).map_err(err)?;
            if estimate_untimed(&mut found.lines, duration) {
                found.synced = true;
                found.estimated = true;
            }
        }
        let timing = self.lyrics_timing(track_id)?.map(|(t, _)| t).unwrap_or_default();
        Ok(Some(Lyrics { offset_ms: timing.offset_ms, anchors: timing.anchors, ..found }))
    }

    /// A song's lyrics timing set by hand, and when it was changed (None: never).
    pub fn lyrics_timing(&self, track_id: i64) -> Result<Option<(Timing, i64)>> {
        use rusqlite::OptionalExtension;
        let row: Option<(i64, Option<String>, i64)> = self
            .conn()
            .query_row("SELECT offset_ms, anchors, changed_at FROM lyrics_offsets WHERE track_id = ?1", [track_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
            .map_err(err)?;
        Ok(row.map(|(offset_ms, anchors, at)| {
            let anchors = anchors.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default();
            (Timing { offset_ms, anchors }, at)
        }))
    }

    /// Sets a song's lyrics timing (all 0 and none tapped: back as they are, kept as a change), `at` when (ms; now, or
    /// another device's change: sync/data.rs).
    pub fn set_lyrics_timing(&self, track_id: i64, timing: &Timing, at: i64) -> Result<()> {
        let anchors = (!timing.anchors.is_empty()).then(|| serde_json::to_string(&timing.anchors)).transpose().map_err(err)?;
        self.conn()
            .execute(
                "INSERT INTO lyrics_offsets (track_id, offset_ms, anchors, changed_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (track_id) DO UPDATE SET offset_ms = ?2, anchors = ?3, changed_at = ?4",
                rusqlite::params![track_id, timing.offset_ms, anchors, at],
            )
            .map(|_| ())
            .map_err(err)
    }

    fn found_lyrics(&self, track_id: i64, open: &Opener) -> Result<Option<Lyrics>> {
        if let Some(own) = self.own_lyrics(track_id, open)? {
            return Ok(Some(own));
        }
        Ok(match self.online_lyrics(track_id)? {
            Some(super::online_lyrics::Found::Lyrics(text)) => parse(&text, "online"),
            Some(super::online_lyrics::Found::Instrumental) => {
                Some(Lyrics { synced: false, lines: Vec::new(), source: "online", instrumental: true, offset_ms: 0, anchors: Vec::new(), estimated: false })
            }
            _ => None,
        })
    }

    /// A song's own lyrics: a .lrc next to it, or its tags.
    pub fn own_lyrics(&self, track_id: i64, open: &Opener) -> Result<Option<Lyrics>> {
        let (kind, location, path): (String, String, String) = self
            .conn()
            .query_row(
                "SELECT f.kind, f.location, t.path FROM tracks t JOIN folders f ON f.id = t.folder_id WHERE t.id = ?1",
                [track_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(err)?;
        let kind = FolderKind::parse(&kind).unwrap_or(FolderKind::Path);
        let job = TagJob { track_id, kind, location, path: path.clone(), folder_cover: None };
        // A .lrc next to it (same name), first: it's usually the timed one.
        if let Some(dot) = path.rfind('.').filter(|&d| !path[d..].contains(['/', '\\'])) {
            for ext in ["lrc", "LRC"] {
                let lrc = format!("{}.{ext}", &path[..dot]);
                if let Some(lyrics) = open(&job, &lrc).ok().and_then(read_text).and_then(|t| parse(&t, "file")) {
                    return Ok(Some(lyrics));
                }
            }
        }
        // Its tags.
        let mut file = open(&job, &path)?;
        let tagged = lofty::read_from(&mut file).map_err(err)?;
        Ok(tagged.tags().iter().find_map(|tag| tag.get_string(ItemKey::Lyrics)).and_then(|text| parse(text, "tags")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_lines_follow_the_song() {
        let lyrics = parse("[ar:Someone]\n[00:12.50]First line\n[00:15.00][01:15.00]Chorus\n\n[00:20]Last", "file").unwrap();
        assert!(lyrics.synced);
        let lines: Vec<(Option<i64>, &str)> = lyrics.lines.iter().map(|l| (l.at_ms, l.text.as_str())).collect();
        assert_eq!(lines, vec![(Some(12_500), "First line"), (Some(15_000), "Chorus"), (Some(20_000), "Last"), (Some(75_000), "Chorus")]);
    }

    #[test]
    fn enhanced_lrc_has_word_times() {
        let lyrics = parse("[00:10.00]<00:10.00>Don't <00:10.40>need <00:10.90>no", "file").unwrap();
        let line = &lyrics.lines[0];
        assert_eq!(line.text, "Don't need no");
        let words: Vec<(i64, &str)> = line.words.iter().map(|w| (w.at_ms, w.text.as_str())).collect();
        assert_eq!(words, vec![(10_000, "Don't "), (10_400, "need "), (10_900, "no")]);
        let danish = parse("[00:01.00]<00:01.00>Æble <00:01.50>ø", "file").unwrap();
        assert_eq!(danish.lines[0].text, "Æble ø");
    }

    #[test]
    fn word_times_are_estimated_for_lines_without_them() {
        let lyrics = parse("[00:10.00]the morning light
[00:13.00]next", "online").unwrap();
        let words: Vec<(i64, &str)> = lyrics.lines[0].words.iter().map(|w| (w.at_ms, w.text.as_str())).collect();
        // 4 syllables: 4 × 280 + 300 = 1420 ms. Weights: "the" light 0.6, "morning" 2, "light" last 1 × 1.6.
        assert_eq!(words, vec![(10_000, "the "), (10_202, "morning "), (10_879, "light")]);
        // Until the next line at most.
        let quick = parse("[00:10.00]the morning light
[00:10.50]next", "online").unwrap();
        assert!(quick.lines[0].words[2].at_ms < 10_500);
    }

    #[test]
    fn syllables_roughly() {
        assert_eq!(syllables("morning"), 2);
        assert_eq!(syllables("time"), 1, "a silent e");
        assert_eq!(syllables("lyset"), 2);
        assert_eq!(syllables("brænde"), 2);
        assert_eq!(syllables("rhythm"), 1);
    }

    #[test]
    fn lyrics_without_times_get_rough_ones() {
        let mut lines = parse("one two three four\n\nfive six seven eight", "tags").unwrap().lines;
        assert!(estimate_untimed(&mut lines, Some(100_000)));
        let times: Vec<i64> = lines.iter().map(|l| l.at_ms.unwrap()).collect();
        // Intro 8 s; 82 s spread over 4 + 8 (the break) + 5 syllables ("seven" has two).
        assert_eq!(times, vec![8_000, 27_294, 65_882]);
        assert!(!lines[0].words.is_empty(), "words too");
        let mut plain = parse("words", "tags").unwrap().lines;
        assert!(!estimate_untimed(&mut plain, None), "no length: untimed");
    }

    #[test]
    fn an_offset_moves_the_times() {
        let lyrics = parse("[offset:+500]\n[00:10.00]Sooner", "file").unwrap();
        assert_eq!(lyrics.lines[0].at_ms, Some(9_500));
    }

    #[test]
    fn plain_lyrics_are_lines_without_times() {
        let lyrics = parse("\nFirst\n\nSecond\n\n", "tags").unwrap();
        assert!(!lyrics.synced);
        assert_eq!(lyrics.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), vec!["First", "", "Second"]);
        assert_eq!(parse("  \n\n", "tags"), None);
    }

    #[test]
    fn times_in_their_forms() {
        assert_eq!(time("01:02.5"), Some(62_500));
        assert_eq!(time("01:02.05"), Some(62_050));
        assert_eq!(time("01:02:345"), Some(62_345));
        assert_eq!(time("1:02"), Some(62_000));
        assert_eq!(time("ar:Someone"), None);
    }
}
