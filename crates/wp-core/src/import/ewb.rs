//! EasyWorship Bible plug-in (.ewb) importer.
//!
//! Format, verified in Phase 0 against 17 real plug-ins: a SQLite file with
//! `header`, `books` (name, abbrev_name, alt_name, book_info, verse_info) and
//! `streams` (one zlib blob per book). After a 2-byte prefix the stream is the
//! whole book as one UTF-8 string. `verse_info` holds 8 bytes per verse:
//!
//! ```text
//! u32 a   = (character offset << 10) | length, bits 29–31 are flags
//! u32 key = verse in (b0 >> 2 | (b1 & 3) << 6), chapter in (b1 >> 2 | (b2 & 3) << 6)
//! ```
//!
//! The 10-bit length saturates, so each verse runs to the next verse's offset.

use super::*;
use crate::reference::book_by_ew_abbrev;
use crate::text::clean;
use crate::Error;
use flate2::read::ZlibDecoder;
use rusqlite::OpenFlags;
use std::io::Read;
use std::path::Path;

pub struct Verse {
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
}

pub struct EwbBook {
    pub source_no: i64,
    pub name: String,
    pub abbrev: String,
    pub alt_name: Option<String>,
    pub verses: Vec<Verse>,
}

pub struct EwbBible {
    pub name: String,
    pub code: String,
    pub lang: String,
    pub copyright: Option<String>,
    pub licence: Option<String>,
    pub books: Vec<EwbBook>,
}

fn bad(path: &Path, why: impl Into<String>) -> Error {
    Error::Format(path.display().to_string(), "EasyWorship Bible", why.into())
}

/// Reads a .ewb file without touching the library.
pub fn read(path: &Path) -> Result<EwbBible> {
    let src = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let (name, code, lang, copyright, licence): (String, String, String, Option<String>, Option<String>) = src
        .query_row("SELECT name, abbrev_name, lang_code, copyright, license FROM header", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(|e| bad(path, format!("no header table ({e})")))?;
    let mut books = Vec::new();
    let mut q = src.prepare("SELECT b.rowid, b.name, b.abbrev_name, b.alt_name, b.verse_info, s.stream FROM books b JOIN streams s ON s.rowid = b.rowid ORDER BY b.rowid")?;
    let rows = q.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Vec<u8>>(4)?, r.get::<_, Vec<u8>>(5)?))
    })?;
    for row in rows {
        let (no, bname, abbrev, alt, vi, stream) = row?;
        let mut raw = Vec::new();
        ZlibDecoder::new(&stream[..]).read_to_end(&mut raw).map_err(|e| bad(path, format!("{bname}: stream ({e})")))?;
        if raw.len() < 2 {
            return Err(bad(path, format!("{bname}: empty stream")));
        }
        let text: Vec<char> = String::from_utf8(raw[2..].to_vec())
            .map_err(|_| bad(path, format!("{bname}: text is not UTF-8")))?
            .chars()
            .collect();
        let ents: Vec<(u32, u32)> = vi
            .chunks_exact(8)
            .map(|c| (u32::from_le_bytes([c[0], c[1], c[2], c[3]]), u32::from_le_bytes([c[4], c[5], c[6], c[7]])))
            .collect();
        let mut verses = Vec::with_capacity(ents.len());
        for (i, (a, k)) in ents.iter().enumerate() {
            let off = ((a & 0x1FFF_FFFF) >> 10) as usize;
            let end = ents.get(i + 1).map(|(n, _)| ((n & 0x1FFF_FFFF) >> 10) as usize).unwrap_or(text.len());
            if off > end || end > text.len() {
                return Err(bad(path, format!("{bname}: verse index out of range")));
            }
            let b = k.to_le_bytes();
            let verse = ((b[0] >> 2) as u16) | (((b[1] & 3) as u16) << 6);
            let chapter = ((b[1] >> 2) as u16) | (((b[2] & 3) as u16) << 6);
            verses.push(Verse { chapter, verse, text: text[off..end].iter().collect::<String>().trim().to_string() });
        }
        books.push(EwbBook { source_no: no, name: bname, abbrev, alt_name: alt, verses });
    }
    Ok(EwbBible { name, code, lang, copyright, licence, books })
}

/// Imports a .ewb into the library as a new version. `code` overrides the plug-in's
/// short code when two files share one (both Twi files say "TW").
pub fn import(conn: &Connection, path: &Path, code: Option<&str>) -> Result<Report> {
    let bible = read(path)?;
    let code = code.map(str::to_string).unwrap_or_else(|| bible.code.clone());
    let tx = conn.unchecked_transaction()?;
    let licence = bible.licence.as_deref().or(bible.copyright.as_deref());
    let cid = create_collection(&tx, &Collection {
        kind: "bible",
        name: &bible.name,
        source_format: "ewb",
        source_file: &path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
        publisher: None,
        licence_note: licence,
        transferable: licence.is_none(),
    })?;
    tx.execute(
        "INSERT INTO bible_version (collection_id, code, name, lang, copyright) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![cid, code, bible.name, bible.lang.to_lowercase(), bible.copyright],
    )?;
    let vid = tx.last_insert_rowid();
    let mut rep = Report { collection_id: cid, name: bible.name.clone(), ..Default::default() };
    {
        let mut sv = tx.prepare(VERSE_SQL)?;
        let mut sf = tx.prepare(FTS_SQL)?;
        for b in &bible.books {
            let canon = book_by_ew_abbrev(&b.abbrev).map(|c| c.no).or_else(|| (b.source_no <= 66).then_some(b.source_no as u16));
            let Some(book_no) = canon else {
                log(&tx, cid, "skipped", &b.name, &format!("book abbreviation {:?} is not in the canon table", b.abbrev))?;
                rep.skipped += b.verses.len();
                continue;
            };
            let chapters = b.verses.iter().map(|v| v.chapter).max().unwrap_or(0);
            insert_book(&tx, vid, &BookIn { book_no, name: b.name.clone(), abbrev: Some(b.abbrev.clone()), aliases: b.alt_name.iter().cloned().collect(), chapters })?;
            for v in &b.verses {
                rep.read += 1;
                if v.text.is_empty() {
                    log(&tx, cid, "skipped", &format!("{} {}:{}", b.name, v.chapter, v.verse), "empty verse")?;
                    rep.skipped += 1;
                    continue;
                }
                let (t, fixes) = clean(&v.text);
                rep.fixed_chars += log_fixes(&tx, cid, &format!("{} {}:{}", b.name, v.chapter, v.verse), &fixes)?;
                insert_verse(&mut sv, &mut sf, vid, book_no, v.chapter, v.verse, &t, None, None)?;
                rep.imported += 1;
            }
        }
    }
    log(&tx, cid, "info", &bible.name, &format!("read {} verses, imported {}, skipped {}", rep.read, rep.imported, rep.skipped))?;
    tx.commit()?;
    Ok(rep)
}
