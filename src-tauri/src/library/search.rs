//! Search (the Search page): songs, albums, artists and playlists whose names have the words typed anywhere in them
//! (accents and case ignored, like the sort order). A song also matches by its artist or album, so searching an
//! artist finds their songs and albums too. Best first: names that start with it, then names that have it, then
//! songs and albums found through their artist or album.

use serde::Serialize;

use super::grouping::fold;
use super::tracks::{AlbumSort, ArtistSort};
use super::{err, Album, Artist, Library, Result, Track};
use super::playlists::Playlist;

/// What a search found: the first `limit` songs (with how many there are in all), and every album, artist and
/// playlist that matches.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub track_count: usize,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
}

/// How well `name` matches `query` (both folded): 0 starts with it, 1 has it at a word's start, 2 has it, None not.
fn rank(name: &str, query: &str) -> Option<u8> {
    let name = fold(name);
    if name.starts_with(query) {
        Some(0)
    } else if name.contains(&format!(" {query}")) {
        Some(1)
    } else if name.contains(query) {
        Some(2)
    } else {
        None
    }
}

impl Library {
    pub fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        let q = fold(query.trim());
        let mut found = SearchResults { tracks: Vec::new(), track_count: 0, albums: Vec::new(), artists: Vec::new(), playlists: Vec::new() };
        if q.is_empty() {
            return Ok(found);
        }
        // Songs: by their title first, then by their artist, then by their album.
        let rows: Vec<(i64, String, Option<String>, Option<String>)> = {
            let conn = self.reader();
            let mut stmt = conn
                .prepare(
                    "SELECT t.id, t.title, t.artist_credit, al.title FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
                     WHERE t.missing_since IS NULL AND t.copy_of IS NULL AND t.folder_id IN (SELECT id FROM folders WHERE enabled)
                     ORDER BY t.sort_title, t.id",
                )
                .map_err(err)?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        let mut songs: Vec<(u8, i64)> = rows
            .iter()
            .filter_map(|(id, title, artist, album)| {
                let by_title = rank(title, &q);
                let by_artist = artist.as_deref().and_then(|a| rank(a, &q)).map(|r| r + 3);
                let by_album = album.as_deref().and_then(|a| rank(a, &q)).map(|r| r + 6);
                [by_title, by_artist, by_album].into_iter().flatten().min().map(|score| (score, *id))
            })
            .collect();
        songs.sort_by_key(|(score, _)| *score);
        found.track_count = songs.len();
        let ids: Vec<i64> = songs.iter().take(limit).map(|(_, id)| *id).collect();
        let mut tracks: std::collections::HashMap<i64, Track> = self.tracks_by_ids(&ids)?.into_iter().map(|t| (t.id, t)).collect();
        found.tracks = ids.iter().filter_map(|id| tracks.remove(id)).collect();

        // Albums: by their title, then by their artist.
        let mut albums: Vec<(u8, Album)> = self
            .albums(0, u32::MAX, AlbumSort::Title)?
            .into_iter()
            .filter_map(|a| {
                let score = [rank(&a.title, &q), a.artist.as_deref().and_then(|ar| rank(ar, &q)).map(|r| r + 3)].into_iter().flatten().min()?;
                Some((score, a))
            })
            .collect();
        albums.sort_by_key(|(score, _)| *score);
        found.albums = albums.into_iter().map(|(_, a)| a).collect();

        let mut artists: Vec<(u8, Artist)> =
            self.artists(0, u32::MAX, ArtistSort::Name)?.into_iter().filter_map(|a| Some((rank(&a.name, &q)?, a))).collect();
        artists.sort_by_key(|(score, _)| *score);
        found.artists = artists.into_iter().map(|(_, a)| a).collect();

        let mut playlists: Vec<(u8, Playlist)> = self.playlists()?.into_iter().filter_map(|p| Some((rank(&p.name, &q)?, p))).collect();
        playlists.sort_by_key(|(score, _)| *score);
        found.playlists = playlists.into_iter().map(|(_, p)| p).collect();
        Ok(found)
    }
}
