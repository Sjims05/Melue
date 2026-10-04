//! The schema's rules: what it accepts and refuses, what goes with what when things are deleted, and
//! upgrading an older library.

use rusqlite::{params, Connection};

use super::migrations;
use super::testkit::{TestDir, TestLibrary};
use super::Library;

const NOW: i64 = 1_760_000_000_000;

fn folder(conn: &Connection, location: &str) -> i64 {
    conn.execute("INSERT INTO folders (kind, location, added_at) VALUES ('path', ?1, ?2)", params![location, NOW])
        .unwrap();
    conn.last_insert_rowid()
}

fn track(conn: &Connection, folder_id: i64, path: &str) -> i64 {
    conn.execute(
        "INSERT INTO tracks (folder_id, path, size, modified_at, title, sort_title, added_at)
         VALUES (?1, ?2, 1000, ?3, ?2, ?2, ?3)",
        params![folder_id, path, NOW],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn artist(conn: &Connection, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO artists (name, name_key, sort_name) VALUES (?1, lower(?1), ?1)",
        params![name],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn playlist(conn: &Connection, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO playlists (name, kind, created_at, updated_at) VALUES (?1, 'app', ?2, ?2)",
        params![name, NOW],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn every_table_is_there_and_foreign_keys_are_enforced() {
    let t = TestLibrary::new("schema-tables");
    let conn = t.library.conn();
    for table in [
        "settings", "folders", "covers", "artists", "albums", "tracks", "track_artists", "playlists",
        "playlist_entries", "playlist_snapshots", "likes", "track_stats", "plays",
    ] {
        let n = count(&conn, &format!("SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = '{table}'"));
        assert_eq!(n, 1, "table {table}");
    }
    assert_eq!(count(&conn, "PRAGMA foreign_keys"), 1);
    // A track in a folder that doesn't exist is refused.
    let orphan = conn.execute(
        "INSERT INTO tracks (folder_id, path, size, modified_at, title, sort_title, added_at) VALUES (99, 'x', 1, 1, 'x', 'x', 1)",
        [],
    );
    assert!(orphan.is_err());
}

#[test]
fn a_file_is_in_the_library_once_per_folder() {
    let t = TestLibrary::new("schema-unique");
    let conn = t.library.conn();
    let a = folder(&conn, "C:/Music");
    let b = folder(&conn, "D:/More music");
    track(&conn, a, "Artist/Song.flac");
    track(&conn, b, "Artist/Song.flac"); // the same relative path in another folder is another file
    let again = conn.execute(
        "INSERT INTO tracks (folder_id, path, size, modified_at, title, sort_title, added_at) VALUES (?1, 'Artist/Song.flac', 1, 1, 'x', 'x', 1)",
        params![a],
    );
    assert!(again.is_err());
    // And a folder is added once.
    assert!(conn.execute("INSERT INTO folders (kind, location, added_at) VALUES ('path', 'C:/Music', 1)", []).is_err());
}

#[test]
fn values_outside_the_allowed_ones_are_refused() {
    let t = TestLibrary::new("schema-checks");
    let conn = t.library.conn();
    assert!(conn.execute("INSERT INTO folders (kind, location, added_at) VALUES ('ftp', 'x', 1)", []).is_err());
    let p = playlist(&conn, "Mix");
    let f = folder(&conn, "C:/Music");
    let s = track(&conn, f, "a.mp3");
    assert!(conn
        .execute("INSERT INTO playlist_entries (playlist_id, position, track_id, state, added_at) VALUES (?1, 0, ?2, 'maybe', 1)", params![p, s])
        .is_err());
    // An entry needs a track or a line from a playlist file.
    assert!(conn
        .execute("INSERT INTO playlist_entries (playlist_id, position, added_at) VALUES (?1, 0, 1)", params![p])
        .is_err());
    // Linked playlists have a file; app playlists don't.
    assert!(conn
        .execute("INSERT INTO playlists (name, kind, created_at, updated_at) VALUES ('x', 'linked', 1, 1)", [])
        .is_err());
    assert!(conn
        .execute("INSERT INTO playlists (name, kind, created_at, updated_at, file_path) VALUES ('x', 'app', 1, 1, 'a.m3u')", [])
        .is_err());
    // STRICT tables: a number column refuses text.
    assert!(conn
        .execute("INSERT INTO tracks (folder_id, path, size, modified_at, title, sort_title, added_at) VALUES (?1, 'b', 'big', 1, 'x', 'x', 1)", params![f])
        .is_err());
}

#[test]
fn removing_a_folder_takes_its_tracks_and_what_hangs_off_them() {
    let t = TestLibrary::new("schema-cascade");
    let conn = t.library.conn();
    let keep = folder(&conn, "C:/Keep");
    let gone = folder(&conn, "C:/Gone");
    let kept = track(&conn, keep, "kept.mp3");
    let removed = track(&conn, gone, "removed.mp3");
    let a = artist(&conn, "Artist");
    for id in [kept, removed] {
        conn.execute("INSERT INTO track_artists VALUES (?1, ?2, 'main', 0)", params![id, a]).unwrap();
        conn.execute("INSERT INTO likes VALUES (?1, ?2)", params![id, NOW]).unwrap();
        conn.execute("INSERT INTO plays (track_id, started_at, completed) VALUES (?1, ?2, 1)", params![id, NOW]).unwrap();
        conn.execute("INSERT INTO track_stats (track_id, play_count, last_played_at) VALUES (?1, 1, ?2)", params![id, NOW]).unwrap();
    }
    let p = playlist(&conn, "Mix");
    for (i, id) in [kept, removed].into_iter().enumerate() {
        conn.execute(
            "INSERT INTO playlist_entries (playlist_id, position, track_id, added_at) VALUES (?1, ?2, ?3, ?4)",
            params![p, i as i64, id, NOW],
        )
        .unwrap();
    }

    conn.execute("DELETE FROM folders WHERE id = ?1", params![gone]).unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM tracks"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM track_artists"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM likes"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM plays"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM track_stats"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM playlist_entries"), 1, "the playlist keeps the other song");
    assert_eq!(count(&conn, "SELECT count(*) FROM playlists"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM artists"), 1, "artists stay (they may have other tracks)");
}

#[test]
fn a_missing_file_keeps_its_likes_playlists_and_history() {
    let t = TestLibrary::new("schema-missing");
    let conn = t.library.conn();
    let f = folder(&conn, "E:/SD card");
    let s = track(&conn, f, "song.mp3");
    conn.execute("INSERT INTO likes VALUES (?1, ?2)", params![s, NOW]).unwrap();
    let p = playlist(&conn, "Mix");
    conn.execute("INSERT INTO playlist_entries (playlist_id, position, track_id, added_at) VALUES (?1, 0, ?2, ?3)", params![p, s, NOW])
        .unwrap();
    // The card is out: the scan marks the track missing instead of deleting it.
    conn.execute("UPDATE tracks SET missing_since = ?1 WHERE id = ?2", params![NOW, s]).unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM tracks WHERE missing_since IS NULL"), 0, "hidden");
    drop(conn);
    let status = t.library.status().unwrap();
    assert_eq!((status.folders, status.tracks, status.playlists), (1, 0, 1), "the status leaves missing tracks out");
    let conn = t.library.conn();
    assert_eq!(count(&conn, "SELECT count(*) FROM likes"), 1);
    assert_eq!(count(&conn, "SELECT count(*) FROM playlist_entries"), 1);
    // It's back.
    conn.execute("UPDATE tracks SET missing_since = NULL WHERE id = ?1", params![s]).unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM tracks WHERE missing_since IS NULL"), 1);
}

#[test]
fn albums_and_covers_outlive_what_they_point_to() {
    let t = TestLibrary::new("schema-set-null");
    let conn = t.library.conn();
    let a = artist(&conn, "Band");
    conn.execute("INSERT INTO covers (hash, file) VALUES ('abc', 'abc.jpg')", []).unwrap();
    let cover = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO albums (title, sort_title, artist_id, album_key, cover_id) VALUES ('Album', 'Album', ?1, 'band|album', ?2)",
        params![a, cover],
    )
    .unwrap();
    conn.execute("DELETE FROM artists WHERE id = ?1", params![a]).unwrap();
    conn.execute("DELETE FROM covers WHERE id = ?1", params![cover]).unwrap();
    let (artist_id, cover_id): (Option<i64>, Option<i64>) =
        conn.query_row("SELECT artist_id, cover_id FROM albums", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((artist_id, cover_id), (None, None));
}

#[test]
fn a_linked_playlist_has_one_row_per_file_and_a_snapshot() {
    let t = TestLibrary::new("schema-linked");
    let conn = t.library.conn();
    let f = folder(&conn, "C:/Music");
    let insert = "INSERT INTO playlists (name, kind, created_at, updated_at, file_folder_id, file_path) VALUES ('Road trip', 'linked', 1, 1, ?1, 'Road trip.m3u')";
    conn.execute(insert, params![f]).unwrap();
    let p = conn.last_insert_rowid();
    assert!(conn.execute(insert, params![f]).is_err(), "the same file is linked once");
    let (new_in_file, removed_from_file, write_back): (String, String, i64) = conn
        .query_row("SELECT new_in_file, removed_from_file, write_back FROM playlists WHERE id = ?1", params![p], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!((new_in_file.as_str(), removed_from_file.as_str(), write_back), ("added", "removed", 0));
    // A file line that isn't in the library is kept as text.
    conn.execute(
        "INSERT INTO playlist_entries (playlist_id, position, file_entry, state, added_at) VALUES (?1, 0, 'Missing/song.mp3', 'both', 1)",
        params![p],
    )
    .unwrap();
    conn.execute("INSERT INTO playlist_snapshots VALUES (?1, 0, 'Missing/song.mp3')", params![p]).unwrap();
    conn.execute("DELETE FROM playlists WHERE id = ?1", params![p]).unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM playlist_entries") + count(&conn, "SELECT count(*) FROM playlist_snapshots"), 0);
}

#[test]
fn a_library_from_the_previous_version_is_upgraded_and_keeps_its_data() {
    let dir = TestDir::new("schema-upgrade");
    let path = dir.path.join("library.db");
    {
        // A version 1 library, as the previous Melue left it.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(include_str!("sql/001_settings.sql")).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO settings VALUES ('theme', 'dark')", []).unwrap();
    }
    let library = Library::open(&path).unwrap();
    assert_eq!(library.schema_version().unwrap(), migrations::latest());
    assert_eq!(library.setting("theme").unwrap().as_deref(), Some("dark"));
    assert_eq!(count(&library.conn(), "SELECT count(*) FROM tracks"), 0);
}

#[test]
fn play_counts_survive_trimming_the_history() {
    let t = TestLibrary::new("schema-stats");
    let conn = t.library.conn();
    let f = folder(&conn, "C:/Music");
    let s = track(&conn, f, "song.mp3");
    conn.execute("INSERT INTO track_stats (track_id) VALUES (?1)", params![s]).unwrap();
    for day in 0..3 {
        conn.execute("INSERT INTO plays (track_id, started_at, completed) VALUES (?1, ?2, 1)", params![s, NOW + day]).unwrap();
        conn.execute(
            "UPDATE track_stats SET play_count = play_count + 1, last_played_at = ?2 WHERE track_id = ?1",
            params![s, NOW + day],
        )
        .unwrap();
    }
    conn.execute("DELETE FROM plays WHERE started_at < ?1", params![NOW + 2]).unwrap(); // trimmed
    assert_eq!(count(&conn, "SELECT count(*) FROM plays"), 1);
    assert_eq!(count(&conn, "SELECT play_count FROM track_stats"), 3);
    // The counts start at zero.
    let other = track(&conn, f, "other.mp3");
    conn.execute("INSERT INTO track_stats (track_id) VALUES (?1)", params![other]).unwrap();
    assert_eq!(count(&conn, &format!("SELECT play_count + skip_count FROM track_stats WHERE track_id = {other}")), 0);
}
