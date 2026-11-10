//! Import review (design brief §5: choose source → review → commit → report).
//! The file is imported into a throw-away in-memory library first, so the operator
//! sees what will be recognised, what needs a look, what is rejected and what is
//! already in the library, before anything is written.

use super::Report;
use crate::{db, Result};
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub item: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Review {
    pub name: String,
    pub read: usize,
    /// Will be imported as they are.
    pub recognised: usize,
    /// Imported, but changed on the way (look-alike characters corrected): worth a look.
    pub ambiguous: Vec<Line>,
    /// Cannot be imported, with the reason.
    pub rejected: Vec<Line>,
    /// Already in this library (songs by title; Bibles by short code).
    pub duplicates: Vec<String>,
    pub notes: Vec<String>,
}

const CAP: usize = 300;

fn lines(c: &Connection, level: &str) -> Result<Vec<Line>> {
    let mut st = c.prepare("SELECT coalesce(item,''), group_concat(message, '; ') FROM import_log WHERE level = ?1 GROUP BY item ORDER BY min(id) LIMIT ?2")?;
    let v = st.query_map(rusqlite::params![level, CAP as i64], |r| Ok(Line { item: r.get(0)?, reason: r.get(1)? }))?.collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

/// Runs the import against a scratch library and compares it with `library`.
pub fn review(library: &Connection, kind: &str, path: &Path, code: Option<&str>) -> Result<Review> {
    let scratch = db::open_memory()?;
    let rep: Report = match kind {
        "ewb" => super::ewb::import(&scratch, path, code)?,
        "bibleshow" => super::bibleshow::import_file(&scratch, path, code)?,
        _ => {
            let name = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            super::ew_songs::import(&scratch, &path.join("Songs.db"), &path.join("SongWords.db"), &name)?
        }
    };
    let mut duplicates = Vec::new();
    if kind == "ew-songs" {
        let mut have = std::collections::HashSet::new();
        let mut st = library.prepare("SELECT lower(trim(title)) FROM song")?;
        for t in st.query_map([], |r| r.get::<_, String>(0))? {
            have.insert(t?);
        }
        let mut st = scratch.prepare("SELECT title FROM song ORDER BY id")?;
        for t in st.query_map([], |r| r.get::<_, String>(0))? {
            let t = t?;
            if have.contains(&t.trim().to_lowercase()) {
                duplicates.push(t);
            }
        }
    } else {
        let mut st = scratch.prepare("SELECT code FROM bible_version")?;
        for c in st.query_map([], |r| r.get::<_, String>(0))? {
            let c = c?;
            if library.query_row("SELECT 1 FROM bible_version WHERE code = ?1", [&c], |_| Ok(())).is_ok() {
                duplicates.push(c);
            }
        }
    }
    let rejected = lines(&scratch, "skipped")?;
    let ambiguous = lines(&scratch, "fixed")?;
    Ok(Review {
        name: rep.name,
        read: rep.read,
        recognised: rep.imported.saturating_sub(ambiguous.len()),
        ambiguous,
        rejected,
        duplicates,
        notes: rep.notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_writes_nothing_and_finds_duplicates() {
        let Some(dir) = std::env::var_os("WP_SAMPLES").map(std::path::PathBuf::from) else { return };
        let lib = db::open_memory().unwrap();
        lib.execute("INSERT INTO song (title) VALUES ('MHB 1'), ('mhb 2 ')", []).unwrap();
        let r = review(&lib, "ew-songs", &dir, None).unwrap();
        assert_eq!(r.read, 4531);
        assert_eq!(r.duplicates.len(), 2, "MHB 1 and MHB 2 are already here");
        assert!(!r.ambiguous.is_empty(), "songs with corrected characters are flagged");
        let n: i64 = lib.query_row("SELECT count(*) FROM song", [], |x| x.get(0)).unwrap();
        assert_eq!(n, 2, "review wrote nothing to the library");

        let skip: std::collections::HashSet<String> = r.duplicates.iter().map(|t| t.trim().to_lowercase()).collect();
        let rep = super::super::ew_songs::import_filtered(&lib, &dir.join("Songs.db"), &dir.join("SongWords.db"), "x", Some(&skip)).unwrap();
        assert_eq!((rep.imported, rep.skipped), (4529, 2));
    }
}
