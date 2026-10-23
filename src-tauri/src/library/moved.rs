//! Songs that moved to another music folder: one record, not two. A scan sees a file only within its own folder, so a
//! song whose file is now another folder's (a synced folder chosen inside a music folder takes its songs over; a file
//! moved from one folder to another) went missing in one folder and turned up as new in the other. After the scan the
//! two are joined into the new one (the file that's there: what the player plays, what the library shows), which takes
//! over the old one's history: its likes, plays, playlist places, lyrics timing, and when it was first added. Where
//! both have something kept once per song (a like, the play counts), the old one's wins.
//!
//! The same song: exactly one missing and one present with the same size and modified time, the new one first seen
//! around when the old one went missing (not a copy that was there all along).

use std::collections::HashMap;

use rusqlite::params;

use super::{err, Library, Result};

/// How far apart the old one going missing and the new one turning up may be (the folders' scans, one after the
/// other; a slow phone).
const AROUND_MS: i64 = 30 * 60_000;

impl Library {
    /// Joins songs that moved to another folder (see above); how many.
    pub fn join_moved_songs(&self) -> Result<u32> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        // A song is never a copy of itself (a join of a song with what it was a copy of made one: then it was
        // hidden as a copy).
        tx.execute("UPDATE tracks SET copy_of = NULL WHERE copy_of = id", []).map_err(err)?;
        // (size, modified) → the missing ones and the new ones.
        let mut missing: HashMap<(i64, i64), Vec<(i64, i64, i64)>> = HashMap::new();
        let mut present: HashMap<(i64, i64), Vec<(i64, i64, i64)>> = HashMap::new();
        {
            let mut stmt = tx
                .prepare("SELECT id, folder_id, size, modified_at, missing_since, added_at FROM tracks")
                .map_err(err)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, Option<i64>>(4)?, r.get::<_, i64>(5)?))
                })
                .map_err(err)?;
            for row in rows {
                let (id, folder, size, modified, missing_since, added) = row.map_err(err)?;
                match missing_since {
                    Some(since) => missing.entry((size, modified)).or_default().push((id, folder, since)),
                    None => present.entry((size, modified)).or_default().push((id, folder, added)),
                }
            }
        }
        // (old, new)
        let pairs: Vec<(i64, i64)> = missing
            .iter()
            .filter_map(|(identity, gone)| {
                let [(old, old_folder, since)] = gone.as_slice() else { return None };
                let [(new, new_folder, added)] = present.get(identity)?.as_slice() else { return None };
                (old_folder != new_folder && (added - since).abs() <= AROUND_MS).then_some((*old, *new))
            })
            .collect();
        if pairs.is_empty() {
            tx.commit().map_err(err)?;
            return Ok(0);
        }
        // Every table that points at a song (found, so a table added later is included), and whether it keeps one row
        // per song (its key is the song: a like, the play counts) or many (plays, playlist places).
        let tables: Vec<(String, bool)> = {
            let mut stmt = tx
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND sql LIKE '%track_id%REFERENCES tracks%'")
                .map_err(err)?;
            let names = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
            let names: Vec<String> = names.collect::<rusqlite::Result<_>>().map_err(err)?;
            let mut tables = Vec::new();
            for name in names {
                let key: Vec<String> = tx
                    .prepare(&format!("SELECT name FROM pragma_table_info('{name}') WHERE pk > 0"))
                    .and_then(|mut s| s.query_map([], |r| r.get::<_, String>(0))?.collect())
                    .map_err(err)?;
                tables.push((name, key == ["track_id"]));
            }
            tables
        };
        for (old, new) in &pairs {
            for (table, one_per_song) in &tables {
                // One per song: the old one's wins (the new one's goes when the old one has one).
                if *one_per_song {
                    tx.execute(
                        &format!("DELETE FROM {table} WHERE track_id = ?2 AND EXISTS (SELECT 1 FROM {table} WHERE track_id = ?1)"),
                        params![old, new],
                    )
                    .map_err(err)?;
                }
                tx.execute(&format!("UPDATE OR IGNORE {table} SET track_id = ?2 WHERE track_id = ?1"), params![old, new]).map_err(err)?;
            }
            tx.execute("UPDATE tracks SET copy_of = ?2 WHERE copy_of = ?1", params![old, new]).map_err(err)?;
            tx.execute("UPDATE tracks SET copy_of = NULL WHERE id = ?1 AND copy_of = ?1", [new]).map_err(err)?;
            // First added: when the old one was.
            tx.execute(
                "UPDATE tracks SET added_at = MIN(added_at, (SELECT added_at FROM tracks WHERE id = ?1)) WHERE id = ?2",
                params![old, new],
            )
            .map_err(err)?;
            // The old one gone (what couldn't move, with it).
            tx.execute("DELETE FROM tracks WHERE id = ?1", [old]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        eprintln!("melue: scan: {} songs found in another folder (kept as they were)", pairs.len());
        Ok(pairs.len() as u32)
    }
}

#[cfg(test)]
mod tests {
    use crate::library::testkit::{MusicFolder, Tags, TestLibrary};
    use crate::library::FolderKind;

    /// Music (with a song in Music/Melue and one beside it) in the library, a liked song in a playlist; then
    /// Music/Melue chosen as the synced folder. `inner_first`: the synced folder's scan runs first.
    fn synced_inside(inner_first: bool) {
        let t = TestLibrary::new("moved");
        let music = t.music_folder("Music");
        music.add("Melue/Waterloo.wav", &Tags::song("Waterloo", "ABBA", "Waterloo", 1));
        music.add("The Sign - a longer title.wav", &Tags::song("The Sign - a longer title", "Ace of Base", "The Sign", 1));
        let outer = t.add_folder(&music);
        t.scan(outer.id, &music);
        let tracks = t.library.tracks(0, 10, Default::default()).unwrap();
        let waterloo = tracks.iter().find(|s| s.title == "Waterloo").unwrap().id;
        let added = tracks.iter().find(|s| s.title == "Waterloo").unwrap().added_at;
        t.library.set_liked(waterloo, true).unwrap();
        let list = t.library.create_playlist("Melue test").unwrap();
        t.library.add_to_playlist(list.id, &[waterloo], None).unwrap();

        let inner_music = MusicFolder { root: music.path("Melue") };
        let inner = t.library.add_synced_folder(FolderKind::Path, inner_music.root.to_str().unwrap(), None).unwrap();
        if inner_first {
            t.scan(inner.id, &inner_music);
            t.scan(outer.id, &music);
        } else {
            t.scan(outer.id, &music);
            t.scan(inner.id, &inner_music);
        }

        let all = t.library.tracks(0, 10, Default::default()).unwrap();
        let found: Vec<_> = all.iter().filter(|s| s.title == "Waterloo").collect();
        assert_eq!(found.len(), 1, "one Waterloo, not two (inner first: {inner_first})");
        let song = found[0];
        assert_ne!(song.id, waterloo, "the new record is kept (what the player plays)");
        assert_eq!((song.folder_id, song.available, song.liked), (inner.id, true, true), "in the synced folder, still liked");
        assert_eq!(song.added_at, added, "first added when the old one was");
        let entries = t.library.playlist_entries(list.id).unwrap();
        assert_eq!(entries.iter().map(|e| e.track.as_ref().map(|s| (s.id, s.available))).collect::<Vec<_>>(), [Some((song.id, true))]);
        assert!(all.iter().any(|s| s.title.starts_with("The Sign") && s.folder_id == outer.id && s.available), "the other song stays");
    }

    #[test]
    fn a_synced_folder_inside_a_music_folder_takes_its_songs_over() {
        synced_inside(false);
    }

    #[test]
    fn whichever_folder_is_scanned_first() {
        synced_inside(true);
    }

    #[test]
    fn a_song_is_never_a_copy_of_itself() {
        let t = TestLibrary::new("moved-self-copy");
        let music = t.music_folder("Music");
        music.add("Song.wav", &Tags::song("Song", "X", "Y", 1));
        let folder = t.add_folder(&music);
        t.scan(folder.id, &music);
        t.library.conn().execute("UPDATE tracks SET copy_of = id", []).unwrap();
        t.library.join_moved_songs().unwrap();
        assert!(t.library.tracks(0, 10, Default::default()).unwrap().iter().any(|s| s.title == "Song" && s.available), "shown again");
    }

    #[test]
    fn a_copy_that_was_there_all_along_isnt_joined() {
        let t = TestLibrary::new("moved-copy");
        let a = t.music_folder("A");
        let b = t.music_folder("B");
        a.add("Song.wav", &Tags::song("Song", "X", "Y", 1));
        std::fs::copy(a.path("Song.wav"), b.path("Song.wav")).unwrap();
        let fa = t.add_folder(&a);
        let fb = t.add_folder(&b);
        t.scan(fa.id, &a);
        t.scan(fb.id, &b);
        // Long after, A's file goes: B's copy isn't taken for it (it was there all along).
        t.library.conn().execute("UPDATE tracks SET added_at = added_at - 86400000", []).unwrap();
        a.remove("Song.wav");
        t.scan(fa.id, &a);
        let all = t.library.tracks(0, 10, Default::default()).unwrap();
        assert_eq!(all.iter().filter(|s| s.title == "Song").count(), 1, "the missing one isn't listed");
        let missing: i64 = t.library.conn().query_row("SELECT COUNT(*) FROM tracks WHERE missing_since IS NOT NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(missing, 1, "A's stays, missing");
    }
}
