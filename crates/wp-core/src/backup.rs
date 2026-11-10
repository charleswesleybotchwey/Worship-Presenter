//! Library backups. A backup is a consistent copy of library.db made with SQLite's
//! online backup API, so it is safe to take during a service. Restoring checks the
//! backup first and always saves the current library as a safety copy before
//! replacing anything; nothing is ever overwritten silently.

use crate::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub path: String,
    pub file: String,
    /// "YYYY-MM-DD HH:MM" from the file name (local time when taken).
    pub taken: String,
    /// manual, weekly, before-import, before-restore
    pub reason: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Contents {
    pub songs: i64,
    pub bibles: i64,
    pub schedules: i64,
}

const AUTO_KEEP: usize = 12;

fn now_stamp(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT strftime('%Y%m%d-%H%M%S', 'now', 'localtime')", [], |r| r.get(0))?)
}

/// Takes a backup into `dir` and returns its details.
pub fn create(conn: &Connection, dir: &Path, reason: &str) -> Result<BackupInfo> {
    std::fs::create_dir_all(dir)?;
    let reason: String = reason.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let mut path = dir.join(format!("library-{}-{reason}.db", now_stamp(conn)?));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("library-{}-{reason}-{n}.db", now_stamp(conn)?));
        n += 1;
    }
    crate::db::backup(conn, &path)?;
    if reason != "manual" {
        prune(dir)?;
    }
    info(&path).ok_or_else(|| Error::Format(path.display().to_string(), "backup", "could not read it back".into()))
}

fn info(path: &Path) -> Option<BackupInfo> {
    let file = path.file_name()?.to_string_lossy().into_owned();
    let rest = file.strip_prefix("library-")?.strip_suffix(".db")?;
    // library-YYYYMMDD-HHMMSS-reason[-n].db
    let (date, rest) = rest.split_once('-')?;
    let (time, reason) = rest.split_once('-')?;
    if date.len() != 8 || time.len() != 6 {
        return None;
    }
    let reason = reason.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('-').to_string();
    Some(BackupInfo {
        path: path.to_string_lossy().into_owned(),
        taken: format!("{}-{}-{} {}:{}", &date[..4], &date[4..6], &date[6..], &time[..2], &time[2..4]),
        reason,
        bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        file,
    })
}

/// Backups in `dir`, newest first.
pub fn list(dir: &Path) -> Result<Vec<BackupInfo>> {
    let mut v: Vec<BackupInfo> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).filter_map(|e| info(&e.path())).collect(),
        Err(_) => Vec::new(),
    };
    v.sort_by(|a, b| b.file.cmp(&a.file));
    Ok(v)
}

/// Keeps the newest automatic backups; manual ones are never deleted.
pub fn prune(dir: &Path) -> Result<()> {
    let autos: Vec<BackupInfo> = list(dir)?.into_iter().filter(|b| b.reason != "manual").collect();
    for b in autos.iter().skip(AUTO_KEEP) {
        let _ = std::fs::remove_file(&b.path);
    }
    Ok(())
}

/// Opens a backup read-only, checks it is intact and a Worship Presenter library,
/// and reports what is in it.
pub fn verify(path: &Path) -> Result<Contents> {
    let bad = |why: &str| Error::Format(path.display().to_string(), "Worship Presenter backup", why.to_string());
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_| bad("it cannot be opened"))?;
    let ok: String = c.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|_| bad("it is not a database"))?;
    if ok != "ok" {
        return Err(bad("it is damaged"));
    }
    let count = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(|_| bad("it is not a Worship Presenter library"));
    Ok(Contents {
        songs: count("SELECT count(*) FROM song")?,
        bibles: count("SELECT count(*) FROM bible_version")?,
        schedules: count("SELECT count(*) FROM schedule WHERE template = 0")?,
    })
}

/// Replaces the open library with a backup. Saves the current library first and
/// returns that safety copy, so a wrong restore can itself be undone.
pub fn restore(conn: &mut Connection, from: &Path, dir: &Path) -> Result<BackupInfo> {
    verify(from)?;
    let safety = create(conn, dir, "before-restore")?;
    let src = Connection::open_with_flags(from, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let b = rusqlite::backup::Backup::new(&src, conn)?;
    b.run_to_completion(256, std::time::Duration::from_millis(5), None)?;
    drop(b);
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(safety)
}

/// Weekly automatic backup: takes one if the newest automatic backup is over 7 days old.
pub fn weekly_if_due(conn: &Connection, dir: &Path) -> Result<Option<BackupInfo>> {
    let newest = list(dir)?.into_iter().find(|b| b.reason == "weekly").map(|b| b.taken);
    let due = match newest {
        None => true,
        Some(t) => conn.query_row("SELECT julianday('now','localtime') - julianday(?1) > 7", [t], |r| r.get::<_, bool>(0))?,
    };
    if due {
        Ok(Some(create(conn, dir, "weekly")?))
    } else {
        Ok(None)
    }
}

pub fn default_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("backups")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wp-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn backup_change_restore_and_undo() {
        let d = tmp("cycle");
        let mut c = crate::db::open(&d.join("library.db")).unwrap();
        c.execute("INSERT INTO song (title) VALUES ('Before')", []).unwrap();
        let b = create(&c, &d.join("backups"), "manual").unwrap();
        assert_eq!(b.reason, "manual");
        assert_eq!(verify(Path::new(&b.path)).unwrap().songs, 1);

        c.execute("INSERT INTO song (title) VALUES ('After')", []).unwrap();
        let safety = restore(&mut c, Path::new(&b.path), &d.join("backups")).unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM song", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "restored to the backup");
        assert_eq!(verify(Path::new(&safety.path)).unwrap().songs, 2, "safety copy holds what was replaced");

        restore(&mut c, Path::new(&safety.path), &d.join("backups")).unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM song", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2, "a restore can be undone");
        assert_eq!(list(&d.join("backups")).unwrap().len(), 3);
    }

    #[test]
    fn rejects_damaged_or_foreign_files() {
        let d = tmp("bad");
        std::fs::write(d.join("junk.db"), b"not a database at all").unwrap();
        assert!(verify(&d.join("junk.db")).is_err());
        Connection::open(d.join("other.db")).unwrap().execute("CREATE TABLE x (a)", []).unwrap();
        assert!(verify(&d.join("other.db")).unwrap_err().to_string().contains("not a Worship Presenter library"));
    }

    #[test]
    fn weekly_and_pruning() {
        let d = tmp("weekly");
        let c = crate::db::open(&d.join("library.db")).unwrap();
        let dir = d.join("backups");
        assert!(weekly_if_due(&c, &dir).unwrap().is_some());
        assert!(weekly_if_due(&c, &dir).unwrap().is_none(), "not due again the same day");
        for _ in 0..15 {
            create(&c, &dir, "before-import").unwrap();
        }
        create(&c, &dir, "manual").unwrap();
        let all = list(&dir).unwrap();
        assert_eq!(all.iter().filter(|b| b.reason != "manual").count(), AUTO_KEEP);
        assert_eq!(all.iter().filter(|b| b.reason == "manual").count(), 1);
    }
}
