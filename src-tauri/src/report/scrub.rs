//! What's personal in a report is replaced by placeholders before anyone sees it, as
//! reports are public issues on GitHub. Song titles, artists, albums, playlists and folders (from the library), the
//! share's address and login name, device and user names, paths, network addresses, emails and links become
//! "[song 1]", "[path 2].flac", "[address 1]"... (the same thing gets the same placeholder throughout, so the report
//! still reads), and the review lists what each stands for (shown only on this device). Plus the log's last lines.

use std::collections::HashMap;
use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use regex::Regex;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::library::Library;

/// How much of the log a report offers (its last lines).
const LOG_LINES: usize = 150;
/// Shorter names than this aren't looked for (a song called "Go" would take the word everywhere); the share's and
/// the person's names are, from 2 letters (a login "me").
const SHORTEST: usize = 3;
const SHORTEST_LOGIN: usize = 2;
/// Words of Melue's own that a song or playlist may also be called: left as they are.
const MELUE_WORDS: &[&str] = &[
    "home", "library", "playlists", "playlist", "now playing", "settings", "search", "play", "pause", "shuffle",
    "repeat", "queue", "up next", "liked", "lyrics", "albums", "artists", "songs", "folders", "sync", "share", "music",
    "error", "none", "true", "false", "null", "android", "desktop", "windows", "melue",
];
/// Links that say nothing about the person: kept.
const PUBLIC_HOSTS: &[&str] = &["github.com", "api.github.com", "lrclib.net"];

/// The report's text with its placeholders, and what each stands for.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Scrubbed {
    pub text: String,
    pub replaced: Vec<Replaced>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Replaced {
    /// "[song 1]".
    pub placeholder: String,
    pub original: String,
}

/// Something personal to look for: its kind ("song") and its text.
pub type Known = (String, String);

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\b(?:https?|smb|content|file)://[^\s"'<>)\]]+"#).unwrap());
static EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+").unwrap());
// A path: a drive (C:\...), a network path (\\host\...), or one of the system's folders (/storage/..., /home/...). Its
// parts may have spaces; it ends at what a file name can't hold (a colon, a quote, the line's end).
static PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:\b[a-z]:[\\/]|\\\\[^\s\\/]+[\\/]|/(?:storage|sdcard|data|home|users|mnt|media|volumes|tmp|private|var)/)[^\r\n"'<>|:*?]*"#,
    )
    .unwrap()
});
static ADDRESS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{1,3}(?:\.\d{1,3}){3}\b").unwrap());
static EXTENSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.[A-Za-z0-9]{2,4}$").unwrap());

/// Replaces what's personal in `text`: the patterns (links, emails, paths, addresses) and the `known` names.
pub fn scrub(text: &str, known: &[Known]) -> Scrubbed {
    // Every match: where, its kind (None: a link that stays).
    let mut found: Vec<(usize, usize, String)> = Vec::new();
    for m in LINK.find_iter(text) {
        if !public_link(m.as_str()) {
            found.push((m.start(), m.end(), "link".into()));
        }
    }
    for (pattern, kind) in [(&*EMAIL, "email"), (&*PATH, "path"), (&*ADDRESS, "address")] {
        for m in pattern.find_iter(text) {
            // A path's trailing spaces and dots stay outside it.
            let end = m.start() + m.as_str().trim_end_matches([' ', '.', ',', ')']).len();
            found.push((m.start(), end, kind.into()));
        }
    }
    let (names, kinds) = lookups(known);
    if let Some(matcher) = names {
        for m in matcher.find_iter(text) {
            if whole_word(text, m.start(), m.end()) {
                found.push((m.start(), m.end(), kinds[m.pattern().as_usize()].clone()));
            }
        }
    }
    // The first, then the longest; overlapping ones are left out.
    found.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    let mut namer = Namer::default();
    for (start, end, kind) in found {
        if start < at {
            continue;
        }
        out.push_str(&text[at..start]);
        out.push_str(&namer.name(&kind, &text[start..end]));
        at = end;
    }
    out.push_str(&text[at..]);
    Scrubbed { text: out, replaced: namer.replaced }
}

/// Numbers placeholders by kind, the same one for the same text (case aside).
#[derive(Default)]
struct Namer {
    seen: HashMap<(String, String), String>,
    counts: HashMap<String, usize>,
    replaced: Vec<Replaced>,
}

impl Namer {
    fn name(&mut self, kind: &str, original: &str) -> String {
        let key = (kind.to_string(), original.to_lowercase());
        if let Some(name) = self.seen.get(&key) {
            return name.clone();
        }
        let n = self.counts.entry(kind.to_string()).and_modify(|n| *n += 1).or_insert(1);
        let mut name = format!("[{kind} {n}]");
        // A path keeps its file's kind (".flac"): it often matters.
        if kind == "path" {
            if let Some(ext) = EXTENSION.find(original) {
                name.push_str(ext.as_str());
            }
        }
        self.seen.insert(key, name.clone());
        self.replaced.push(Replaced { placeholder: name.clone(), original: original.to_string() });
        name
    }
}

/// The names to look for, longest first among equals, and each one's kind.
fn lookups(known: &[Known]) -> (Option<AhoCorasick>, Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    let mut words = Vec::new();
    let mut kinds = Vec::new();
    for (kind, text) in known {
        let text = text.trim();
        let lower = text.to_lowercase();
        let shortest = if matches!(kind.as_str(), "share" | "name" | "device") { SHORTEST_LOGIN } else { SHORTEST };
        if text.chars().count() < shortest || text.chars().all(|c| c.is_ascii_digit()) || MELUE_WORDS.contains(&lower.as_str()) {
            continue;
        }
        if seen.insert(lower) {
            words.push(text.to_string());
            kinds.push(kind.clone());
        }
    }
    if words.is_empty() {
        return (None, kinds);
    }
    let matcher = AhoCorasickBuilder::new().match_kind(MatchKind::LeftmostLongest).ascii_case_insensitive(true).build(&words).ok();
    (matcher, kinds)
}

/// Not part of a longer word ("Rain" isn't looked for in "Rainbow").
fn whole_word(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
}

fn public_link(link: &str) -> bool {
    let host = link.split("://").nth(1).unwrap_or_default().split(['/', '?', '#', ':']).next().unwrap_or_default().to_lowercase();
    link.to_lowercase().starts_with("http") && PUBLIC_HOSTS.contains(&host.as_str())
}

/// What's personal in this library and on this device: songs, artists, albums, playlists, folders, the share, the
/// device's and the user's names.
fn known_here(app: &AppHandle) -> Result<Vec<Known>, String> {
    let library = app.state::<Library>();
    let mut known = Vec::new();
    {
        let conn = library.conn();
        for (kind, sql) in [
            ("song", "SELECT title FROM tracks"),
            ("artist", "SELECT name FROM artists"),
            ("artist", "SELECT DISTINCT artist_credit FROM tracks WHERE artist_credit IS NOT NULL"),
            ("album", "SELECT title FROM albums"),
            ("playlist", "SELECT name FROM playlists"),
        ] {
            let mut statement = conn.prepare(sql).map_err(|e| e.to_string())?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
            known.extend(rows.flatten().map(|text| (kind.to_string(), text)));
        }
    }
    // Names from folders() (made from the path where the database has none), never with the database held.
    for folder in library.folders()? {
        known.push(("folder".into(), folder.name.clone()));
        known.push(("path".into(), folder.location.clone()));
    }
    if let Some(settings) = library.share_settings()? {
        let json = serde_json::to_value(&settings.setup).map_err(|e| e.to_string())?;
        for value in json.as_object().into_iter().flat_map(|o| o.values()) {
            if let Some(text) = value.as_str().filter(|t| !matches!(*t, "smb" | "drive")) {
                known.push(("share".into(), text.to_string()));
            }
        }
    }
    if let Some(name) = crate::sync::handoff::stored_settings(&library).ok().and_then(|s| s.device_name) {
        known.push(("device".into(), name));
    }
    for (kind, variable) in [("device", "COMPUTERNAME"), ("device", "HOSTNAME"), ("name", "USERNAME"), ("name", "USER")] {
        if let Ok(value) = std::env::var(variable) {
            known.push((kind.into(), value));
        }
    }
    Ok(known)
}

/// The report's text with what's personal replaced. `extra`: more the app knows (other devices' names).
#[tauri::command(async)]
pub fn report_scrub(app: AppHandle, text: String, extra: Vec<Known>) -> Result<Scrubbed, String> {
    let mut known = known_here(&app)?;
    known.extend(extra);
    Ok(scrub(&text, &known))
}

/// The log's last lines (not yet scrubbed: that's done with the whole report).
#[tauri::command(async)]
pub fn report_log() -> Result<String, String> {
    let Some(file) = crate::logging::file() else { return Ok(String::new()) };
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    Ok(lines[lines.len().saturating_sub(LOG_LINES)..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Vec<Known> {
        [("song", "Golden Hour"), ("song", "Rain"), ("artist", "Amara Sol"), ("song", "Home"), ("device", "TROLLGANG"), ("share", "192.168.1.20")]
            .iter()
            .map(|(k, t)| (k.to_string(), t.to_string()))
            .collect()
    }

    #[test]
    fn names_become_placeholders_the_same_each_time() {
        let s = scrub("Played Golden Hour by Amara Sol, then golden hour again on TROLLGANG", &known());
        assert_eq!(s.text, "Played [song 1] by [artist 1], then [song 1] again on [device 1]");
        assert_eq!(s.replaced.len(), 3);
        assert_eq!(s.replaced[0], Replaced { placeholder: "[song 1]".into(), original: "Golden Hour".into() });
    }

    #[test]
    fn short_login_names_too_but_not_short_titles() {
        let mut known = known();
        known.push(("share".into(), "me".into()));
        known.push(("song".into(), "Go".into()));
        let s = scrub("Share: me@nas, then Go", &known);
        assert_eq!(s.text, "Share: [share 1]@nas, then Go");
    }

    #[test]
    fn only_whole_words_and_not_melues_own() {
        let s = scrub("Rainbow on the Home page; Rain played", &known());
        assert_eq!(s.text, "Rainbow on the Home page; [song 1] played");
    }

    #[test]
    fn paths_keep_their_kind_of_file() {
        let s = scrub(r"can't read C:\Users\someone\Music\Amara Sol\Golden Hour.flac: no such file", &known());
        assert_eq!(s.text, "can't read [path 1].flac: no such file");
        let s = scrub("copied /storage/emulated/0/Music/Rain.mp3", &known());
        assert_eq!(s.text, "copied [path 1].mp3");
    }

    #[test]
    fn addresses_emails_and_links_but_not_public_ones() {
        let s = scrub(
            "smb://192.168.1.20/music failed; 10.0.0.7 too; mail me@example.com; see https://github.com/Sjims05/Melue/issues and https://example.com/x",
            &known(),
        );
        assert_eq!(s.text, "[link 1] failed; [address 1] too; mail [email 1]; see https://github.com/Sjims05/Melue/issues and [link 2]");
    }
}
