//! Lists named so another of your devices finds its own copy (Connect device: a list played on another device plays
//! that device's version of it, its songs only there included). A playlist by its sync id, an album by its title and
//! album artist, an artist by name; Liked songs, the Top songs lists and All songs are what they are. A playlist
//! that's only on this device, and a music folder, aren't on the others.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use super::{err, Library, Result};

/// A list, as any of your devices knows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SharedList {
    #[serde(rename_all = "camelCase")]
    Playlist { sync_id: String },
    Liked,
    Top { period: String },
    /// Its album key (the title and album artist, normalized).
    Album { key: String },
    /// Its name key.
    Artist { key: String },
    /// All songs, in a sort order of the Songs tab.
    Songs { sort: Option<String> },
}

impl Library {
    /// A list here (a play source: "playlist:12", "liked"…), named for another device. Lists only on this device
    /// can't be.
    pub fn shared_list(&self, source: &str) -> Result<SharedList> {
        let (kind, rest) = source.split_once(':').map_or((source, None), |(k, r)| (k, Some(r)));
        let id = || rest.and_then(|r| r.parse::<i64>().ok()).ok_or_else(|| format!("not a list: {source}"));
        let conn = self.conn();
        let one = |sql: &str, id: i64| conn.query_row(sql, [id], |r| r.get::<_, Option<String>>(0)).optional().map_err(err);
        Ok(match kind {
            "liked" => SharedList::Liked,
            "top" => SharedList::Top { period: rest.unwrap_or_default().to_string() },
            "songs" => SharedList::Songs { sort: rest.map(str::to_string) },
            "playlist" => {
                let synced = conn
                    .query_row("SELECT sync_id, local_only FROM playlists WHERE id = ?1", [id()?], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, bool>(1)?)))
                    .optional()
                    .map_err(err)?
                    .ok_or("no such playlist")?;
                match synced {
                    (Some(sync_id), false) => SharedList::Playlist { sync_id },
                    _ => return Err("that playlist is only on this device".into()),
                }
            }
            "album" => SharedList::Album { key: one("SELECT album_key FROM albums WHERE id = ?1", id()?)?.flatten().ok_or("no such album")? },
            "artist" => SharedList::Artist { key: one("SELECT name_key FROM artists WHERE id = ?1", id()?)?.flatten().ok_or("no such artist")? },
            "folder" => return Err("that folder is only on this device".into()),
            _ => return Err(format!("not a list: {source}")),
        })
    }

    /// This device's copy of a list from another device, as a play source (None: it isn't here, e.g. a playlist
    /// that hasn't synced yet).
    pub fn shared_list_here(&self, list: &SharedList) -> Result<Option<String>> {
        let conn = self.conn();
        let find = |sql: &str, key: &str| conn.query_row(sql, [key], |r| r.get::<_, i64>(0)).optional().map_err(err);
        Ok(match list {
            SharedList::Liked => Some("liked".into()),
            SharedList::Top { period } => Some(format!("top:{period}")),
            SharedList::Songs { sort: Some(sort) } => Some(format!("songs:{sort}")),
            SharedList::Songs { sort: None } => Some("songs".into()),
            SharedList::Playlist { sync_id } => find("SELECT id FROM playlists WHERE sync_id = ?1", sync_id)?.map(|id| format!("playlist:{id}")),
            SharedList::Album { key } => find("SELECT id FROM albums WHERE album_key = ?1", key)?.map(|id| format!("album:{id}")),
            SharedList::Artist { key } => find("SELECT id FROM artists WHERE name_key = ?1", key)?.map(|id| format!("artist:{id}")),
        })
    }
}
