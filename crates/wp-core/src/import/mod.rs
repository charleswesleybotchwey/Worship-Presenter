//! Importers. Each one reads a source file, cleans text (crate::text), writes into
//! the library inside a single transaction and returns a report with counts.
//! Nothing is truncated or dropped without a line in `import_log`.

pub mod bibleshow;
pub mod ew_songs;
pub mod ewb;
pub mod review;
pub mod foreign_themes;
pub mod songfiles;
pub mod vpl;

use crate::Result;
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Report {
    pub collection_id: i64,
    pub name: String,
    pub read: usize,
    pub imported: usize,
    pub skipped: usize,
    pub fixed_chars: usize,
    pub notes: Vec<String>,
}

pub(crate) struct Collection<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub source_format: &'a str,
    pub source_file: &'a str,
    pub publisher: Option<&'a str>,
    pub licence_note: Option<&'a str>,
    pub transferable: bool,
}

pub(crate) fn create_collection(conn: &Connection, c: &Collection) -> Result<i64> {
    conn.execute(
        "INSERT INTO collection (kind, name, source_format, source_file, publisher, licence_note, transferable)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![c.kind, c.name, c.source_format, c.source_file, c.publisher, c.licence_note, c.transferable],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn log(conn: &Connection, collection: i64, level: &str, item: &str, message: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO import_log (collection_id, level, item, message) VALUES (?1, ?2, ?3, ?4)",
        params![collection, level, item, message],
    )?;
    Ok(())
}

/// Logs look-alike fixes for one item and returns how many characters changed.
pub(crate) fn log_fixes(conn: &Connection, collection: i64, item: &str, fixes: &[crate::text::Fix]) -> Result<usize> {
    let mut n = 0;
    for f in fixes {
        n += f.count;
        log(conn, collection, "fixed", item, &format!("{}× U+{:04X} → {:?} ({})", f.count, f.from as u32, f.to, f.reason))?;
    }
    Ok(n)
}

/// Writes one Bible version's books and verses; shared by the .ewb and Bible Show importers.
pub(crate) struct BookIn {
    pub book_no: u16,
    pub name: String,
    pub abbrev: Option<String>,
    pub aliases: Vec<String>,
    pub chapters: u16,
}

pub(crate) fn insert_book(conn: &Connection, version_id: i64, b: &BookIn) -> Result<()> {
    let mut aliases: Vec<String> = Vec::new();
    for a in std::iter::once(&b.name).chain(b.abbrev.iter()).chain(b.aliases.iter()) {
        let a = a.trim().trim_matches('\'').to_string();
        if !a.is_empty() && !aliases.contains(&a) {
            aliases.push(a);
        }
    }
    conn.execute(
        "INSERT INTO bible_book (version_id, book_no, name, abbrev, aliases, chapters) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![version_id, b.book_no, b.name, b.abbrev, aliases.join("|"), b.chapters],
    )?;
    Ok(())
}

pub(crate) fn insert_verse(
    stmt_v: &mut rusqlite::Statement,
    stmt_f: &mut rusqlite::Statement,
    version_id: i64,
    book: u16,
    ch: u16,
    vs: u16,
    text: &str,
    marked: Option<&str>,
    heading: Option<&str>,
) -> Result<()> {
    stmt_v.execute(params![version_id, book, ch, vs, text, marked, heading])?;
    // the word index holds folded text (ɛ→e, ɔ→o …) so a plain keyboard finds Twi words
    stmt_f.execute(params![crate::text::fold(text), version_id, book, ch, vs])?;
    Ok(())
}

pub(crate) const VERSE_SQL: &str =
    "INSERT OR REPLACE INTO bible_verse (version_id, book_no, chapter, verse, text, marked, heading) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
pub(crate) const FTS_SQL: &str =
    "INSERT INTO bible_fts (text, version_id, book_no, chapter, verse) VALUES (?1, ?2, ?3, ?4, ?5)";
