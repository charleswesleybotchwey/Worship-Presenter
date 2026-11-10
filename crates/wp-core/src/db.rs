//! Opening the library database, running migrations and taking safe backups.

use crate::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../migrations/0001_init.sql")),
    ("0002_announcements", include_str!("../migrations/0002_announcements.sql")),
    ("0003_my_collections", include_str!("../migrations/0003_my_collections.sql")),
    ("0004_service_intervals", include_str!("../migrations/0004_service_intervals.sql")),
];

/// Opens (or creates) a profile's library database with WAL and foreign keys on,
/// backing the file up before any pending migration runs.
pub fn open(path: &Path) -> Result<Connection> {
    let fresh = !path.exists();
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    let pending = pending_migrations(&conn)?;
    if !fresh && !pending.is_empty() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        backup(&conn, &path.with_extension(format!("pre-migrate-{stamp}.db")))?;
    }
    migrate(&conn, &pending)?;
    upgrade_search_index(&conn)?;
    crate::themes::seed(&conn)?;
    crate::cues::seed(&conn)?;
    Ok(conn)
}

/// Version 2 of the song search index stores folded text (ɛ→e, ɔ→o) so plain
/// keyboards find Akan words. Libraries imported before that are rebuilt once.
fn upgrade_search_index(conn: &Connection) -> Result<()> {
    let v: Option<String> = conn.query_row("SELECT value FROM setting WHERE key = 'search_index'", [], |r| r.get(0)).ok();
    if v.as_deref() == Some("2") {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    let ids: Vec<i64> = tx.prepare("SELECT id FROM song")?.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    for id in ids {
        crate::songs::reindex(&tx, id)?;
    }
    tx.execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('search_index', '2')", [])?;
    tx.commit()?;
    Ok(())
}

/// In-memory database with the full schema, for tests and dry-run imports.
pub fn open_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let pending = pending_migrations(&conn)?;
    migrate(&conn, &pending)?;
    upgrade_search_index(&conn)?;
    crate::themes::seed(&conn)?;
    crate::cues::seed(&conn)?;
    Ok(conn)
}

fn pending_migrations(conn: &Connection) -> Result<Vec<&'static (&'static str, &'static str)>> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migration (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')))",
    )?;
    let mut out = Vec::new();
    for m in MIGRATIONS {
        let done: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migration WHERE name = ?1)",
            [m.0],
            |r| r.get(0),
        )?;
        if !done {
            out.push(m);
        }
    }
    Ok(out)
}

fn migrate(conn: &Connection, pending: &[&(&str, &str)]) -> Result<()> {
    for (name, sql) in pending {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute("INSERT INTO schema_migration (name) VALUES (?1)", [name])?;
        tx.commit()?;
    }
    Ok(())
}

/// Copies the live database with SQLite's online backup API (never a file copy),
/// so a backup taken mid-service is consistent.
pub fn backup(conn: &Connection, dest: &Path) -> Result<()> {
    let mut out = Connection::open(dest)?;
    let b = rusqlite::backup::Backup::new(conn, &mut out)?;
    b.run_to_completion(256, std::time::Duration::from_millis(5), None)?;
    Ok(())
}

/// Integrity check run on start. A damaged file is reported, never silently replaced.
pub fn integrity_ok(conn: &Connection) -> Result<bool> {
    let r: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    Ok(r == "ok")
}
