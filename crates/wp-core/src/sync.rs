//! Keeping church computers alike through a shared folder (OneDrive, Google Drive, a
//! network folder, a USB drive): one computer shares its library, the others follow it.
//! Shared: songs, Bibles, themes, theme rules, media, presentations, services, collections.
//! Each computer keeps its own settings and outputs (projectors, NDI, MIDI, keys).

use crate::{Error, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// When it was shared (ms since 1970).
    pub shared_ms: i64,
    /// The computer that shared it.
    pub computer: String,
    pub format: u32,
}

/// Tables that belong to this computer, never replaced by an update.
pub const LOCAL_TABLES: [&str; 2] = ["setting", "output"];

pub fn place(folder: &Path, profile: &str) -> PathBuf {
    let safe: String = profile.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    folder.join("Worship Presenter").join(safe.trim())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Copies new or changed files (by size) from one media folder to another.
fn copy_media(from: &Path, to: &Path) -> Result<usize> {
    let mut n = 0;
    if !from.is_dir() {
        return Ok(0);
    }
    for entry in walk(from) {
        let rel = entry.strip_prefix(from).unwrap_or(&entry);
        let dest = to.join(rel);
        let same = std::fs::metadata(&dest).ok().zip(std::fs::metadata(&entry).ok()).is_some_and(|(a, b)| a.len() == b.len());
        if !same {
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            let tmp = dest.with_extension("wp-part");
            std::fs::copy(&entry, &tmp)?;
            std::fs::rename(&tmp, &dest)?;
            n += 1;
        }
    }
    Ok(n)
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else if !p.extension().is_some_and(|x| x == "wp-part") {
                out.push(p);
            }
        }
    }
    out
}

/// Shares this computer's library: a consistent copy of the database, new media, and a
/// note of when and from where. Returns the number of media files copied.
pub fn share(conn: &Connection, media: &Path, folder: &Path, profile: &str, computer: &str) -> Result<usize> {
    let to = place(folder, profile);
    std::fs::create_dir_all(&to)?;
    let tmp = to.join("library.db.wp-part");
    let _ = std::fs::remove_file(&tmp);
    // a consistent snapshot even while the app is using the database
    conn.execute("VACUUM INTO ?1", [tmp.to_string_lossy().as_ref()])?;
    std::fs::rename(&tmp, to.join("library.db"))?;
    let copied = copy_media(media, &to.join("media"))?;
    let m = Manifest { shared_ms: now_ms(), computer: computer.to_string(), format: 1 };
    std::fs::write(to.join("manifest.json"), serde_json::to_vec_pretty(&m).unwrap_or_default())?;
    Ok(copied)
}

/// The update waiting in the shared folder, if it is newer than the last one taken.
pub fn waiting(folder: &Path, profile: &str, last_taken_ms: i64) -> Option<Manifest> {
    let to = place(folder, profile);
    let m: Manifest = serde_json::from_slice(&std::fs::read(to.join("manifest.json")).ok()?).ok()?;
    (m.shared_ms > last_taken_ms && to.join("library.db").exists()).then_some(m)
}

/// This computer's own rows (settings, outputs), kept across an update.
pub fn local_rows(conn: &Connection) -> Result<Vec<(String, Vec<Vec<rusqlite::types::Value>>, Vec<String>)>> {
    let mut out = vec![];
    for t in LOCAL_TABLES {
        let mut st = conn.prepare(&format!("SELECT * FROM {t}"))?;
        let cols: Vec<String> = st.column_names().map(|c| c.to_string()).collect();
        let n = cols.len();
        let rows = st.query_map([], |r| (0..n).map(|i| r.get::<_, rusqlite::types::Value>(i)).collect::<std::result::Result<Vec<_>, _>>())?.collect::<std::result::Result<Vec<_>, _>>()?;
        out.push((t.to_string(), rows, cols));
    }
    Ok(out)
}

pub fn restore_local_rows(conn: &Connection, rows: &[(String, Vec<Vec<rusqlite::types::Value>>, Vec<String>)]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (t, data, cols) in rows {
        tx.execute(&format!("DELETE FROM {t}"), [])?;
        let marks = vec!["?"; cols.len()].join(", ");
        let mut st = tx.prepare(&format!("INSERT INTO {t} ({}) VALUES ({marks})", cols.join(", ")))?;
        for r in data {
            st.execute(rusqlite::params_from_iter(r.iter()))?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Takes the shared library: media first (new files only), then the database file is
/// copied over `db_path`. The caller has closed its connection to `db_path` and reopens
/// it afterwards (then puts back `local_rows`).
pub fn take(folder: &Path, profile: &str, media: &Path, db_path: &Path) -> Result<Manifest> {
    let from = place(folder, profile);
    let m: Manifest = serde_json::from_slice(&std::fs::read(from.join("manifest.json"))?).map_err(|_| Error::Format("Sync".into(), "manifest", "The shared folder's note is damaged.".into()))?;
    copy_media(&from.join("media"), media)?;
    let tmp = db_path.with_extension("wp-part");
    std::fs::copy(from.join("library.db"), &tmp)?;
    // the copy must be a readable library before it replaces this one
    Connection::open(&tmp)?.query_row("SELECT count(*) FROM song", [], |r| r.get::<_, i64>(0))?;
    std::fs::rename(&tmp, db_path)?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_then_follow() {
        let root = std::env::temp_dir().join(format!("wp-sync-{}", now_ms()));
        let (main_dir, laptop_dir, shared) = (root.join("main"), root.join("laptop"), root.join("OneDrive"));
        for d in [&main_dir, &laptop_dir, &shared] {
            std::fs::create_dir_all(d.join("media")).unwrap();
        }
        // the main computer: a song, a picture, its own settings
        let main = crate::db::open(&main_dir.join("library.db")).unwrap();
        main.execute("INSERT INTO song (title) VALUES ('MHB 1')", []).unwrap();
        main.execute("INSERT INTO setting (key, value) VALUES ('speech_mic', 'Main desk mic')", []).unwrap();
        std::fs::write(main_dir.join("media").join("sunrise.jpg"), b"picture").unwrap();
        assert_eq!(share(&main, &main_dir.join("media"), &shared, "Main", "MAIN-PC").unwrap(), 1);
        // the laptop: its own settings, an older library
        let lap_db = laptop_dir.join("library.db");
        {
            let lap = crate::db::open(&lap_db).unwrap();
            lap.execute("INSERT INTO setting (key, value) VALUES ('speech_mic', 'Laptop mic')", []).unwrap();
        }
        let w = waiting(&shared, "Main", 0).unwrap();
        assert_eq!(w.computer, "MAIN-PC");
        let keep = local_rows(&crate::db::open(&lap_db).unwrap()).unwrap();
        take(&shared, "Main", &laptop_dir.join("media"), &lap_db).unwrap();
        let lap = crate::db::open(&lap_db).unwrap();
        restore_local_rows(&lap, &keep).unwrap();
        let title: String = lap.query_row("SELECT title FROM song", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "MHB 1", "the shared library arrived");
        let mic: String = lap.query_row("SELECT value FROM setting WHERE key = 'speech_mic'", [], |r| r.get(0)).unwrap();
        assert_eq!(mic, "Laptop mic", "this computer's own settings kept");
        assert_eq!(std::fs::read(laptop_dir.join("media").join("sunrise.jpg")).unwrap(), b"picture");
        assert!(waiting(&shared, "Main", w.shared_ms).is_none(), "nothing newer");
        let _ = std::fs::remove_dir_all(&root);
    }
}
