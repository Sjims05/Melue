//! Melue's music library: an SQLite database in the app's data folder, owned by the Rust side. The UI
//! asks it things through the commands in commands.rs (wrapped for TypeScript in src/library/).

mod android;
mod commands;
mod folders;
mod migrations;
pub mod scan;
pub mod scanner;
#[cfg(test)]
mod schema_tests;
#[cfg(test)]
pub mod testkit;

pub use commands::*;
pub use folders::{Folder, FolderKind};

use std::fmt::Display;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension};

pub type Result<T> = std::result::Result<T, String>;

/// Now, in milliseconds since 1970 (how the library stores times).
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Turns any error into the message the commands hand to the UI.
pub(crate) fn err(e: impl Display) -> String {
    e.to_string()
}

pub struct Library {
    conn: Mutex<Connection>,
}

impl Library {
    /// Opens the library at `path` (creating it the first time) and brings its schema up to date.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
        }
        let mut conn = Connection::open(path).map_err(err)?;
        // WAL: reading (the UI) and writing (a scan) don't block each other.
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")
            .map_err(err)?;
        migrations::run(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The schema version the database is at (see migrations.rs).
    pub fn schema_version(&self) -> Result<i64> {
        self.conn().pragma_query_value(None, "user_version", |row| row.get(0)).map_err(err)
    }

    /// The schema version and how much is in the library.
    pub fn status(&self) -> Result<LibraryStatus> {
        let schema_version = self.schema_version()?;
        let conn = self.conn();
        let count = |sql: &str| conn.query_row(sql, [], |row| row.get::<_, i64>(0)).map_err(err);
        Ok(LibraryStatus {
            schema_version,
            folders: count("SELECT count(*) FROM folders")?,
            tracks: count("SELECT count(*) FROM tracks WHERE missing_since IS NULL")?,
            albums: count("SELECT count(*) FROM albums")?,
            artists: count("SELECT count(*) FROM artists")?,
            playlists: count("SELECT count(*) FROM playlists")?,
        })
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        self.conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
            .optional()
            .map_err(err)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .map(|_| ())
            .map_err(err)
    }
}
