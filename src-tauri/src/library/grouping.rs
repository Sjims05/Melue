//! The grouping rules: albums and artists built from the tags as the files have them (tags.rs), and how
//! names sort. Run after every scan; only what changed is written, and albums and artists keep their ids
//! (they're found again by their keys), so likes and pins on them survive.
//!
//! - Artists: an artist credit is split on ";" (several artist values) and "feat." / "ft." / "featuring" /
//!   "with" (main and featured artists); "(feat. X)" in a title adds X as featured, the title stays as it
//!   is. Not split on "&" or ",": Simon & Garfunkel and Earth, Wind & Fire stay one artist (the featured
//!   part is, though: "feat. A & B"). No artist: "Unknown artist".
//! - Albums: the same album title and album artist. Songs without an album artist that share an album
//!   title in one folder are one album: by their artist when they all have the same one, else a
//!   compilation. A compilation flag or "Various Artists" as album artist makes a compilation. An album's
//!   year and cover are the ones most of its songs have. Songs without an album tag have no album.
//! - Names are compared without case, accents or extra spaces: "Beyoncé" and "beyonce" are one artist.
//! - Sorting: a sort tag wins ("Beatles, The"); otherwise without case and accents, and without a leading
//!   "The" (The Beatles under B) or punctuation.
//! - Songs without tags: "01 Artist - Title.mp3" gives the track number, artist and title (fallbacks).

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::atomic::Ordering;

use rusqlite::{params, Transaction};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use super::scan::ScanSummary;
use super::{err, Library, Result};

pub const UNKNOWN_ARTIST: &str = "Unknown artist";

/// A name made comparable: no accents, lower case, single spaces.
pub fn fold(name: &str) -> String {
    let plain: String = name.nfkd().filter(|c| !is_combining_mark(*c)).collect();
    plain.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// How a name sorts: folded, without leading punctuation or a leading "The ".
pub fn sort_key(name: &str) -> String {
    let folded = fold(name);
    let trimmed = folded.trim_start_matches(|c: char| !c.is_alphanumeric());
    let trimmed = trimmed.strip_prefix("the ").filter(|rest| !rest.is_empty()).unwrap_or(trimmed);
    if trimmed.is_empty() { folded } else { trimmed.to_string() }
}

// ---- artist credits

/// The artists of a credit like "A; B feat. C & D": main (A, B) and featured (C, D), in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Credit {
    pub main: Vec<String>,
    pub featured: Vec<String>,
}

const FEATURING: &[&str] = &["feat.", "feat", "ft.", "ft", "featuring", "with"];

/// Splits "Main feat. Guest" / "Main (feat. Guest)" into the main part and the guests. In titles only the
/// bracketed forms and feat. / ft. / featuring count ("Dancing with Myself" has no guest).
fn split_featured(text: &str, in_title: bool) -> (String, Vec<String>) {
    let lower = text.to_ascii_lowercase(); // the same byte positions as `text`
    let mut best: Option<(usize, usize, Option<char>)> = None; // where, how long, closing bracket
    for word in FEATURING {
        let forms: [(String, Option<char>); 3] =
            [(format!(" {word} "), None), (format!("({word} "), Some(')')), (format!("[{word} "), Some(']'))];
        for (form, close) in forms {
            if in_title && close.is_none() && *word == "with" {
                continue;
            }
            if let Some(at) = lower.find(&form) {
                if best.is_none_or(|(b, _, _)| at < b) {
                    best = Some((at, form.len(), close));
                }
            }
        }
    }
    let Some((at, len, close)) = best else { return (text.trim().to_string(), Vec::new()) };
    let rest = &text[at + len..];
    let (guests, after) = match close.and_then(|c| rest.find(c)) {
        Some(end) => (&rest[..end], &rest[end + 1..]),
        None => (rest, ""),
    };
    let main = format!("{} {}", text[..at].trim(), after.trim()).trim().to_string();
    (main, split_guests(guests))
}

/// "A, B & C" (a featured list) as A, B, C.
fn split_guests(guests: &str) -> Vec<String> {
    let mut names = vec![guests.to_string()];
    for separator in [", ", " & ", " and ", " x "] {
        names = names.iter().flat_map(|n| n.split(separator).map(String::from).collect::<Vec<_>>()).collect();
    }
    names.into_iter().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect()
}

fn push_new(names: &mut Vec<String>, name: String, others: &[String]) {
    let key = fold(&name);
    if !key.is_empty() && !names.iter().chain(others).any(|n| fold(n) == key) {
        names.push(name);
    }
}

/// The artists of a credit (an artist or album artist tag).
pub fn parse_credit(credit: &str) -> Credit {
    let mut parsed = Credit::default();
    let mut guests = Vec::new();
    for part in credit.split(';') {
        let (main, featured) = split_featured(part, false);
        push_new(&mut parsed.main, main, &[]);
        guests.extend(featured);
    }
    for guest in guests {
        push_new(&mut parsed.featured, guest, &parsed.main.clone());
    }
    parsed
}

/// The guests named in a title: "Song (feat. A & B)" gives A and B.
pub fn featured_in_title(title: &str) -> Vec<String> {
    split_featured(title, true).1
}

// ---- file names

/// What a file name says when the tags don't: "1-03 Artist - Title" is disc 1, track 3, Artist, Title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameParts {
    pub disc: Option<u32>,
    pub track: Option<u32>,
    pub artist: Option<String>,
    pub title: String,
}

/// The file name of a track's path: the last part of a path, or of a content:// address (decoded).
pub fn file_name_of(path: &str) -> String {
    if path.starts_with("content://") {
        let last = path.rsplit('/').next().unwrap_or(path);
        let decoded = percent_decode(last);
        let name = decoded.rsplit(['/', ':']).next().unwrap_or(&decoded);
        return name.to_string();
    }
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// The folder a track's path is in (the same form as the path, decoded for content:// addresses).
pub fn folder_of(path: &str) -> String {
    let path = if path.starts_with("content://") {
        percent_decode(path.rsplit('/').next().unwrap_or(path))
    } else {
        path.to_string()
    };
    path.rfind(['/', '\\']).map_or(String::new(), |end| path[..end].to_string())
}

fn percent_decode(text: &str) -> String {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if let (b'%', Some(&high), Some(&low)) = (bytes[i], bytes.get(i + 1), bytes.get(i + 2)) {
            if let (Some(high), Some(low)) = (hex(high), hex(low)) {
                out.push(high * 16 + low);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Reads a file name (without its extension) the way untagged files are usually named.
pub fn parse_file_name(stem: &str) -> NameParts {
    let stem = stem.trim();
    let mut parts = NameParts::default();
    let mut rest = stem;

    let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
    let n = digits(rest);
    // Disc and track: "1-03 ...", "2-11. ..."
    if (1..=2).contains(&n) && rest[n..].starts_with('-') && digits(&rest[n + 1..]) == 2 {
        let after = &rest[n + 3..];
        if let Some(title) = strip_separator(after) {
            parts.disc = rest[..n].parse().ok();
            parts.track = rest[n + 1..n + 3].parse().ok();
            rest = title;
        }
    } else if (1..=3).contains(&n) {
        let number = &rest[..n];
        let after = &rest[n..];
        let strong = [". ", " - ", ".", "_", "-", " _ "].iter().find(|s| after.starts_with(**s));
        let title = match strong {
            Some(separator) => Some(after[separator.len()..].trim_start()),
            // "12 Song" or "01 Song", but not "50 Cent - In da Club" or "1 Thing".
            None if after.starts_with(' ') && n == 2 && (number.starts_with('0') || !after.contains(" - ")) => {
                Some(after.trim_start())
            }
            None => None,
        };
        if let Some(title) = title.filter(|t| !t.is_empty()) {
            parts.track = number.parse().ok().filter(|t| *t > 0);
            rest = title;
        }
    }
    match rest.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            parts.artist = Some(artist.trim().to_string());
            parts.title = title.trim().to_string();
        }
        _ => parts.title = rest.trim().to_string(),
    }
    if parts.title.is_empty() {
        parts.title = stem.to_string();
    }
    parts
}

fn strip_separator(text: &str) -> Option<&str> {
    let trimmed = text.trim_start_matches([' ', '.', '-', '_']);
    (trimmed.len() < text.len() && !trimmed.is_empty()).then_some(trimmed)
}

// ---- building albums and artists

/// A track's tags, as grouping needs them.
struct Row {
    id: i64,
    folder_id: i64,
    path: String,
    title: String,
    title_sort: Option<String>,
    credit: Option<String>,
    artist_sort: Option<String>,
    album: Option<String>,
    album_sort: Option<String>,
    album_artist: Option<String>,
    album_artist_sort: Option<String>,
    compilation: bool,
    year: Option<i64>,
    cover_id: Option<i64>,
    sort_title: String,
    album_id: Option<i64>,
}

#[derive(Default)]
struct ArtistAcc {
    names: HashMap<String, u32>,
    sort: Option<String>,
}

#[derive(Default)]
struct AlbumAcc {
    titles: HashMap<String, u32>,
    artist: Option<String>,
    compilation: bool,
    years: HashMap<i64, u32>,
    covers: HashMap<i64, u32>,
    sort: Option<String>,
}

/// The most common value (ties: the smallest, so it doesn't change between runs).
fn most_common<T: Ord + Hash + Clone>(counts: &HashMap<T, u32>) -> Option<T> {
    counts.iter().max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0))).map(|(v, _)| v.clone())
}

fn is_various(name: &str) -> bool {
    matches!(fold(name).as_str(), "various artists" | "various" | "va" | "v.a." | "v/a")
}

/// What grouping changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GroupSummary {
    pub albums: u32,
    pub artists: u32,
    /// Album, artist and track rows written (0: nothing changed).
    pub written: u32,
}

#[derive(Debug, Clone, PartialEq)]
struct AlbumRow {
    title: String,
    sort_title: String,
    artist_id: Option<i64>,
    compilation: bool,
    year: Option<i64>,
    cover_id: Option<i64>,
}

impl Library {
    /// After a scan: regroups when the scan changed something (or a folder was removed, or it's the first
    /// scan since start-up). Returns None when it wasn't needed.
    pub fn regroup_after(&self, scan: &ScanSummary) -> Result<Option<GroupSummary>> {
        let needed = self.regroup_needed.swap(false, Ordering::SeqCst);
        if !needed && !scan.changed_anything() {
            return Ok(None);
        }
        self.regroup().inspect_err(|_| self.regroup_needed.store(true, Ordering::SeqCst)).map(Some)
    }

    /// Builds the albums and artists from every track's tags (missing songs too, so they're still in
    /// place when their files come back), writing only what changed.
    pub fn regroup(&self) -> Result<GroupSummary> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        let summary = regroup(&tx)?;
        tx.commit().map_err(err)?;
        Ok(summary)
    }
}

fn load_rows(tx: &Transaction) -> Result<Vec<Row>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, folder_id, path, title, tag_title_sort, artist_credit, tag_artist_sort, tag_album, tag_album_sort,
                    tag_album_artist, tag_album_artist_sort, tag_compilation, year, cover_id, sort_title, album_id
             FROM tracks ORDER BY id",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Row {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                path: r.get(2)?,
                title: r.get(3)?,
                title_sort: r.get(4)?,
                credit: r.get(5)?,
                artist_sort: r.get(6)?,
                album: r.get(7)?,
                album_sort: r.get(8)?,
                album_artist: r.get(9)?,
                album_artist_sort: r.get(10)?,
                compilation: r.get(11)?,
                year: r.get(12)?,
                cover_id: r.get(13)?,
                sort_title: r.get(14)?,
                album_id: r.get(15)?,
            })
        })
        .map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn regroup(tx: &Transaction) -> Result<GroupSummary> {
    let rows = load_rows(tx)?;
    let mut artists: HashMap<String, ArtistAcc> = HashMap::new();
    let mut add_artist = |name: &str, sort: Option<&str>| -> String {
        let key = fold(name);
        let acc = artists.entry(key.clone()).or_default();
        *acc.names.entry(name.trim().to_string()).or_default() += 1;
        if let Some(sort) = sort.filter(|s| !s.trim().is_empty()) {
            acc.sort.get_or_insert_with(|| sort.to_string());
        }
        key
    };

    // Each track's artists: (artist key, role, position).
    let mut track_artists: HashMap<i64, Vec<(String, &'static str, i64)>> = HashMap::new();
    // The first main artist of each track, for albums without an album artist.
    let mut first_artist: HashMap<i64, String> = HashMap::new();
    for row in &rows {
        let mut credit = parse_credit(row.credit.as_deref().unwrap_or(""));
        if credit.main.is_empty() {
            credit.main.push(UNKNOWN_ARTIST.to_string());
        }
        for guest in featured_in_title(&row.title) {
            let main = credit.main.clone();
            push_new(&mut credit.featured, guest, &main);
        }
        let single = credit.main.len() == 1;
        let mut list = Vec::new();
        for (position, name) in credit.main.iter().enumerate() {
            let sort = if single { row.artist_sort.as_deref() } else { None };
            list.push((add_artist(name, sort), "main", position as i64));
        }
        for (position, name) in credit.featured.iter().enumerate() {
            list.push((add_artist(name, None), "featured", (credit.main.len() + position) as i64));
        }
        first_artist.insert(row.id, list[0].0.clone());
        track_artists.insert(row.id, list);
    }

    // Albums without an album artist: which artists each (folder, album title) has.
    let album_title = |row: &Row| row.album.as_deref().map(str::trim).filter(|a| !a.is_empty()).map(String::from);
    let mut folder_albums: HashMap<(i64, String, String), HashSet<String>> = HashMap::new();
    for row in &rows {
        if let (Some(album), None) = (album_title(row), row.album_artist.as_deref().filter(|a| !a.trim().is_empty())) {
            let group = (row.folder_id, folder_of(&row.path), fold(&album));
            folder_albums.entry(group).or_default().insert(first_artist[&row.id].clone());
        }
    }

    let mut albums: HashMap<String, AlbumAcc> = HashMap::new();
    let mut track_album: HashMap<i64, String> = HashMap::new();
    for row in &rows {
        let Some(album) = album_title(row) else { continue };
        let album_artist = row.album_artist.as_deref().map(str::trim).filter(|a| !a.is_empty());
        let artist: Option<String> = if row.compilation || album_artist.is_some_and(is_various) {
            None
        } else if let Some(name) = album_artist {
            let main = parse_credit(name).main.into_iter().next().unwrap_or_else(|| name.to_string());
            Some(add_artist(&main, row.album_artist_sort.as_deref()))
        } else {
            let group = &folder_albums[&(row.folder_id, folder_of(&row.path), fold(&album))];
            (group.len() == 1).then(|| first_artist[&row.id].clone())
        };
        let key = format!("{}\u{1f}{}", artist.as_deref().unwrap_or("\u{1}various"), fold(&album));
        let acc = albums.entry(key.clone()).or_default();
        *acc.titles.entry(album).or_default() += 1;
        acc.compilation = artist.is_none();
        acc.artist = artist;
        if let Some(year) = row.year {
            *acc.years.entry(year).or_default() += 1;
        }
        if let Some(cover) = row.cover_id {
            *acc.covers.entry(cover).or_default() += 1;
        }
        if let Some(sort) = row.album_sort.as_deref().filter(|s| !s.trim().is_empty()) {
            acc.sort.get_or_insert_with(|| sort.to_string());
        }
        track_album.insert(row.id, key);
    }

    let mut summary = GroupSummary { albums: albums.len() as u32, artists: artists.len() as u32, written: 0 };

    // Artists: found again by key, updated when their name or sort name changed, new ones added.
    let mut artist_ids: HashMap<String, i64> = HashMap::new();
    let existing = query_map(tx, "SELECT name_key, id, name, sort_name FROM artists", |r| {
        Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))
    })?;
    for (key, acc) in &artists {
        let name = most_common(&acc.names).unwrap_or_default();
        let sort = sort_key(acc.sort.as_deref().unwrap_or(&name));
        match existing.get(key) {
            Some((id, old_name, old_sort)) => {
                if *old_name != name || *old_sort != sort {
                    tx.execute("UPDATE artists SET name = ?2, sort_name = ?3 WHERE id = ?1", params![id, name, sort]).map_err(err)?;
                    summary.written += 1;
                }
                artist_ids.insert(key.clone(), *id);
            }
            None => {
                tx.execute("INSERT INTO artists (name, name_key, sort_name) VALUES (?1, ?2, ?3)", params![name, key, sort])
                    .map_err(err)?;
                artist_ids.insert(key.clone(), tx.last_insert_rowid());
                summary.written += 1;
            }
        }
    }

    // Albums, the same way.
    let mut album_ids: HashMap<String, i64> = HashMap::new();
    let existing = query_map(tx, "SELECT album_key, id, title, sort_title, artist_id, compilation, year, cover_id FROM albums", |r| {
        let row = AlbumRow {
            title: r.get(2)?,
            sort_title: r.get(3)?,
            artist_id: r.get(4)?,
            compilation: r.get(5)?,
            year: r.get(6)?,
            cover_id: r.get(7)?,
        };
        Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, row)))
    })?;
    for (key, acc) in &albums {
        let title = most_common(&acc.titles).unwrap_or_default();
        let want = AlbumRow {
            sort_title: sort_key(acc.sort.as_deref().unwrap_or(&title)),
            title,
            artist_id: acc.artist.as_ref().map(|a| artist_ids[a]),
            compilation: acc.compilation,
            year: most_common(&acc.years),
            cover_id: most_common(&acc.covers),
        };
        let values = params![want.title, want.sort_title, want.artist_id, want.compilation, want.year, want.cover_id, key];
        match existing.get(key) {
            Some((id, have)) => {
                if *have != want {
                    tx.execute(
                        "UPDATE albums SET title = ?1, sort_title = ?2, artist_id = ?3, compilation = ?4, year = ?5, cover_id = ?6
                         WHERE album_key = ?7",
                        values,
                    )
                    .map_err(err)?;
                    summary.written += 1;
                }
                album_ids.insert(key.clone(), *id);
            }
            None => {
                tx.execute(
                    "INSERT INTO albums (title, sort_title, artist_id, compilation, year, cover_id, album_key)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    values,
                )
                .map_err(err)?;
                album_ids.insert(key.clone(), tx.last_insert_rowid());
                summary.written += 1;
            }
        }
    }

    // Tracks: their album and sort title.
    for row in &rows {
        let album_id = track_album.get(&row.id).map(|key| album_ids[key]);
        let sort_title = sort_key(row.title_sort.as_deref().unwrap_or(&row.title));
        if album_id != row.album_id || sort_title != row.sort_title {
            tx.execute("UPDATE tracks SET album_id = ?2, sort_title = ?3 WHERE id = ?1", params![row.id, album_id, sort_title])
                .map_err(err)?;
            summary.written += 1;
        }
    }

    // Track artists: rewritten for the tracks whose artists changed.
    let mut have: HashMap<i64, Vec<(i64, String, i64)>> = HashMap::new();
    {
        let mut stmt = tx.prepare("SELECT track_id, artist_id, role, position FROM track_artists ORDER BY track_id, position").map_err(err)?;
        let found = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, (r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))))
            .map_err(err)?;
        for item in found {
            let (track, entry) = item.map_err(err)?;
            have.entry(track).or_default().push(entry);
        }
    }
    for (track, list) in &track_artists {
        let want: Vec<(i64, String, i64)> = list.iter().map(|(key, role, pos)| (artist_ids[key], role.to_string(), *pos)).collect();
        if have.get(track) != Some(&want) {
            tx.execute("DELETE FROM track_artists WHERE track_id = ?1", [track]).map_err(err)?;
            for (artist, role, position) in &want {
                tx.execute(
                    "INSERT INTO track_artists (track_id, artist_id, role, position) VALUES (?1, ?2, ?3, ?4)",
                    params![track, artist, role, position],
                )
                .map_err(err)?;
            }
            summary.written += 1;
        }
    }

    // Albums and artists nothing uses any more.
    let kept_albums: HashSet<i64> = album_ids.values().copied().collect();
    for id in tx_ids(tx, "SELECT id FROM albums")? {
        if !kept_albums.contains(&id) {
            tx.execute("DELETE FROM albums WHERE id = ?1", [id]).map_err(err)?;
            summary.written += 1;
        }
    }
    let kept_artists: HashSet<i64> = artist_ids.values().copied().collect();
    for id in tx_ids(tx, "SELECT id FROM artists")? {
        if !kept_artists.contains(&id) {
            tx.execute("DELETE FROM artists WHERE id = ?1", [id]).map_err(err)?;
            summary.written += 1;
        }
    }
    Ok(summary)
}

fn query_map<K: Eq + Hash, V>(
    tx: &Transaction,
    sql: &str,
    row: impl FnMut(&rusqlite::Row) -> rusqlite::Result<(K, V)>,
) -> Result<HashMap<K, V>> {
    let mut stmt = tx.prepare(sql).map_err(err)?;
    let rows = stmt.query_map([], row).map_err(err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(err)
}

fn tx_ids(tx: &Transaction, sql: &str) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare(sql).map_err(err)?;
    let ids = stmt.query_map([], |r| r.get(0)).map_err(err)?;
    ids.collect::<rusqlite::Result<_>>().map_err(err)
}
