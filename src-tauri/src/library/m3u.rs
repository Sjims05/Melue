//! Playlist files (.m3u, .m3u8): reading their songs' lines, and writing them back in the style they had
//! (encoding, line endings, #EXTINF lines, relative or absolute paths, slashes), so other apps still read
//! them as before. Paths here are '/'-separated "logical" paths (see linked.rs).

use super::grouping::percent_decode;

/// A song line in a playlist file, with what its #EXTINF line said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLine {
    /// The line as written (a path or address), trimmed.
    pub entry: String,
    pub info: Option<ExtInf>,
}

/// "#EXTINF:215,Artist - Title"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtInf {
    pub seconds: Option<i64>,
    pub artist: Option<String>,
    pub title: String,
}

/// How a playlist file is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    /// UTF-8, or else Latin-1 (old .m3u files).
    pub utf8: bool,
    pub bom: bool,
    pub crlf: bool,
    /// "#EXTM3U" with #EXTINF lines.
    pub extended: bool,
    /// Paths written with '\' (Windows apps).
    pub backslashes: bool,
    /// Absolute paths rather than relative to the playlist file.
    pub absolute: bool,
}

impl Style {
    /// For a new file, or one that can't be read: what most apps write.
    pub fn new(crlf: bool) -> Self {
        Self { utf8: true, bom: false, crlf, extended: true, backslashes: false, absolute: false }
    }
}

pub fn is_playlist(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".m3u") || lower.ends_with(".m3u8")
}

/// Whether a line is an absolute path (or a file:// address).
pub fn is_absolute(line: &str) -> bool {
    let bytes = line.as_bytes();
    line.starts_with('/')
        || line.starts_with("\\\\")
        || line.to_ascii_lowercase().starts_with("file://")
        || (bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/'))
}

/// An internet address (a stream), which no file in the library is.
fn is_url(line: &str) -> bool {
    line.contains("://") && !line.to_ascii_lowercase().starts_with("file://")
}

/// Reads a playlist file: its song lines in order, and its style.
pub fn parse(bytes: &[u8]) -> (Vec<FileLine>, Style) {
    let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let bytes = if bom { &bytes[3..] } else { bytes };
    let (text, utf8) = match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_string(), true),
        Err(_) => (bytes.iter().map(|b| *b as char).collect(), false),
    };
    let mut style = Style { utf8, bom, crlf: text.contains("\r\n"), extended: false, backslashes: false, absolute: false };
    let mut lines = Vec::new();
    let mut info = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if line.eq_ignore_ascii_case("#EXTM3U") {
            style.extended = true;
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            style.extended = true;
            info = parse_extinf(rest);
        } else if !line.starts_with('#') {
            lines.push(FileLine { entry: line.to_string(), info: info.take() });
        }
    }
    let paths: Vec<&str> = lines.iter().map(|l| l.entry.as_str()).filter(|l| !is_url(l)).collect();
    let back = paths.iter().filter(|p| p.contains('\\')).count();
    let forward = paths.iter().filter(|p| p.contains('/')).count();
    style.backslashes = back > forward;
    style.absolute = paths.iter().filter(|p| is_absolute(p)).count() * 2 > paths.len();
    (lines, style)
}

fn parse_extinf(rest: &str) -> Option<ExtInf> {
    let (duration, display) = rest.split_once(',')?;
    // "-1 tvg-id=..." : the length is the first word.
    let seconds = duration.split_whitespace().next().and_then(|d| d.parse::<i64>().ok()).filter(|s| *s >= 0);
    let display = display.trim();
    let (artist, title) = match display.split_once(" - ") {
        Some((artist, title)) => (Some(artist.trim().to_string()), title.trim().to_string()),
        None => (None, display.to_string()),
    };
    (!title.is_empty()).then_some(ExtInf { seconds, artist, title })
}

/// Writes a playlist file's songs in `style`. Lines are written as given (except for slashes).
pub fn write(lines: &[FileLine], style: &Style) -> Vec<u8> {
    let newline = if style.crlf { "\r\n" } else { "\n" };
    let mut text = String::new();
    if style.extended {
        text.push_str("#EXTM3U");
        text.push_str(newline);
    }
    for line in lines {
        if style.extended {
            if let Some(info) = &line.info {
                let seconds = info.seconds.unwrap_or(-1);
                let display = match &info.artist {
                    Some(artist) => format!("{artist} - {}", info.title),
                    None => info.title.clone(),
                };
                text.push_str(&format!("#EXTINF:{seconds},{display}{newline}"));
            }
        }
        text.push_str(&line.entry);
        text.push_str(newline);
    }
    let mut bytes = Vec::with_capacity(text.len() + 3);
    if style.bom {
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    if style.utf8 {
        bytes.extend_from_slice(text.as_bytes());
    } else {
        // Latin-1: characters it doesn't have become '?'.
        bytes.extend(text.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')));
    }
    bytes
}

// ---- paths

/// A path with '/' separators and "." / ".." resolved.
pub fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                // Never above a drive or volume ("C:", "primary:Music").
                if parts.last().is_some_and(|p| *p != ".." && !(parts.len() == 1 && p.ends_with(':'))) {
                    parts.pop();
                }
            }
            part => parts.push(part),
        }
    }
    let joined = parts.join("/");
    if absolute { format!("/{joined}") } else { joined }
}

/// Where a playlist line points, as a logical path: relative lines from the playlist's folder `dir`.
/// None for internet addresses.
pub fn resolve(dir: &str, line: &str) -> Option<String> {
    if is_url(line) {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("file://") {
        let path = percent_decode(&line[7..]);
        // file:///C:/Music → C:/Music
        let path = match path.as_bytes() {
            [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => path[1..].to_string(),
            _ => path,
        };
        return Some(normalize(&path));
    }
    if is_absolute(line) {
        return Some(normalize(line));
    }
    Some(normalize(&format!("{dir}/{line}")))
}

/// `to` written relative to the folder `from` ("../Other/song.mp3"), or None when they don't share a
/// drive or volume.
pub fn relative(from: &str, to: &str) -> Option<String> {
    let from: Vec<&str> = from.split('/').filter(|p| !p.is_empty()).collect();
    let to: Vec<&str> = to.split('/').filter(|p| !p.is_empty()).collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a.eq_ignore_ascii_case(b)).count();
    if common == 0 {
        return None;
    }
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&to[common..]);
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_lines_and_style() {
        let text = "\u{feff}#EXTM3U\r\n#EXTINF:215,ABBA - Waterloo\r\nABBA\\Waterloo.mp3\r\n\r\n# a comment\r\nOther\\b.flac\r\nhttp://radio.example/stream\r\n";
        let (lines, style) = parse(text.as_bytes());
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].entry, "ABBA\\Waterloo.mp3");
        assert_eq!(lines[0].info, Some(ExtInf { seconds: Some(215), artist: Some("ABBA".into()), title: "Waterloo".into() }));
        assert_eq!(lines[1].info, None);
        assert_eq!(style, Style { utf8: true, bom: true, crlf: true, extended: true, backslashes: true, absolute: false });

        let (lines, style) = parse(b"/music/a.mp3\n/music/b.mp3\nc.mp3\n");
        assert_eq!(lines.len(), 3);
        assert!(style.absolute && !style.extended && !style.crlf && !style.backslashes);
    }

    #[test]
    fn latin1_files_are_read_and_written_as_latin1() {
        let bytes = b"Bj\xF6rk/J\xF3ga.mp3\n";
        let (lines, style) = parse(bytes);
        assert_eq!(lines[0].entry, "Björk/Jóga.mp3");
        assert!(!style.utf8);
        assert_eq!(write(&lines, &style), bytes.to_vec());
    }

    #[test]
    fn writing_keeps_the_style() {
        let text = "#EXTM3U\r\n#EXTINF:215,ABBA - Waterloo\r\nABBA/Waterloo.mp3\r\n#EXTINF:-1,Intro\r\nintro.mp3\r\n";
        let (lines, style) = parse(text.as_bytes());
        assert_eq!(String::from_utf8(write(&lines, &style)).unwrap(), text);
    }

    #[test]
    fn paths() {
        assert_eq!(normalize("C:\\Music\\.\\A\\..\\B\\song.mp3"), "C:/Music/B/song.mp3");
        assert_eq!(normalize("/storage/emulated/0/Music/../Music/x.mp3"), "/storage/emulated/0/Music/x.mp3");
        assert_eq!(normalize("C:/../x.mp3"), "C:/x.mp3", "not above the drive");
        assert_eq!(resolve("C:/Music/Lists", "../ABBA/Waterloo.mp3").unwrap(), "C:/Music/ABBA/Waterloo.mp3");
        assert_eq!(resolve("C:/Music", "D:\\Other\\x.mp3").unwrap(), "D:/Other/x.mp3");
        assert_eq!(resolve("C:/Music", "file:///C:/My%20Music/x.mp3").unwrap(), "C:/My Music/x.mp3");
        assert_eq!(resolve("primary:Music", "Album/01.mp3").unwrap(), "primary:Music/Album/01.mp3");
        assert_eq!(resolve("C:/Music", "https://radio.example/stream"), None);
        assert_eq!(relative("C:/Music/Lists", "C:/Music/ABBA/Waterloo.mp3").unwrap(), "../ABBA/Waterloo.mp3");
        assert_eq!(relative("C:/Music", "c:/music/a.mp3").unwrap(), "a.mp3");
        assert_eq!(relative("C:/Music", "D:/a.mp3"), None);
        assert!(is_playlist("Mix.M3U8") && is_playlist("a.m3u") && !is_playlist("a.mp3"));
    }
}
