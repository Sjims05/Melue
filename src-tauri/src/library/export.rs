//! Exporting a list to play (a playlist, Liked songs, Top songs…) as a playlist file other players read: an .m3u8
//! (UTF-8 .m3u), each song as its title, artist and length (#EXTINF) and its file's full path.

use std::path::Path;

use super::{Library, Result};

impl Library {
    /// Writes the songs `source` plays ("playlist:3", "liked", "top:month", "album:7"…), in its order, to `path` as an
    /// .m3u8. Songs that can't play here (missing, not on this device) are left out. Returns how many were written.
    pub fn export_m3u(&self, source: &str, path: &Path) -> Result<usize> {
        let items = self.queue_items(source)?;
        let ids: Vec<i64> = items.iter().filter_map(|i| i.id).collect();
        let lengths: std::collections::HashMap<i64, i64> =
            self.tracks_by_ids(&ids)?.into_iter().map(|t| (t.id, t.duration_ms.unwrap_or(0))).collect();
        let newline = if cfg!(windows) { "\r\n" } else { "\n" };
        let mut text = format!("#EXTM3U{newline}");
        for item in &items {
            let seconds = item.id.and_then(|id| lengths.get(&id)).map_or(-1, |ms| (ms + 500) / 1000);
            let title = item.title.as_deref().unwrap_or("");
            let name = match item.artist.as_deref() {
                Some(artist) if !artist.is_empty() => format!("{artist} - {title}"),
                _ => title.to_string(),
            };
            text.push_str(&format!("#EXTINF:{seconds},{name}{newline}{}{newline}", item.src));
        }
        std::fs::write(path, text).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        Ok(items.len())
    }
}
