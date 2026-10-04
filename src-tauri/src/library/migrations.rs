//! The library's schema, one migration per version. The database records its version (SQLite's
//! `user_version`), so opening an older library runs the migrations it's missing, in order.
//!
//! Once a version has been released, its migration never changes: changes go in a new one.

use rusqlite::Connection;

use super::{err, Result};

const MIGRATIONS: &[&str] = &[
    // 1: settings the library keeps for itself and the app.
    "CREATE TABLE settings (
        key TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT;",
];

/// The newest schema version.
pub fn latest() -> i64 {
    MIGRATIONS.len() as i64
}

/// Brings the database up to the newest version, one migration (and transaction) at a time.
pub fn run(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(err)?;
    if current > latest() {
        return Err(format!("the library was made by a newer Melue (schema {current}, this one knows {})", latest()));
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = index as i64 + 1;
        let tx = conn.transaction().map_err(err)?;
        tx.execute_batch(sql).map_err(|e| format!("library migration {version}: {e}"))?;
        tx.pragma_update(None, "user_version", version).map_err(err)?;
        tx.commit().map_err(err)?;
    }
    Ok(())
}
