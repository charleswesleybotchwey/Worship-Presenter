//! Library browsing in the EasyWorship style: a folder tree (all songs, the hymnals
//! as Collections, the operator's own My Collections) and a sortable song table;
//! a scripture table showing a whole chapter as the reference is typed; and
//! library maintenance (rebuild search keys, reset songs to the theme in use).

use crate::library::{aliases, search_songs};
use crate::{reference, Error, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct SongRow {
    pub id: i64,
    /// First hymn number, e.g. "MHB 851".
    pub number: Option<String>,
    pub title: String,
    pub first_line: Option<String>,
    pub author: Option<String>,
    pub copyright: Option<String>,
    /// Languages stored, in order, e.g. ["en", "fat"].
    pub langs: Vec<String>,
    /// Sorts "MHB 9" before "MHB 10".
    pub sort_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Folder {
    /// "hymnal:MHB" or "mine:3"
    pub key: String,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SongTree {
    pub all: i64,
    pub collections: Vec<Folder>,
    pub mine: Vec<Folder>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scope {
    All,
    Hymnal { code: String },
    Mine { id: i64 },
}

pub fn song_tree(conn: &Connection) -> Result<SongTree> {
    let all: i64 = conn.query_row("SELECT count(*) FROM song", [], |r| r.get(0))?;
    let mut st = conn.prepare(
        "SELECT h.code, h.name, count(DISTINCT n.song_id) FROM hymnal h JOIN song_number n ON n.hymnal_id = h.id GROUP BY h.id ORDER BY h.code",
    )?;
    let collections = st
        .query_map([], |r| {
            let code: String = r.get(0)?;
            let name: String = r.get(1)?;
            Ok(Folder { key: format!("hymnal:{code}"), name: if name.starts_with(&code) { name } else { format!("{code} · {name}") }, count: r.get(2)? })
        })?
        .collect::<std::result::Result<_, _>>()?;
    let mut st = conn.prepare("SELECT c.id, c.name, (SELECT count(*) FROM my_collection_item i WHERE i.collection_id = c.id) FROM my_collection c ORDER BY c.name")?;
    let mine = st
        .query_map([], |r| Ok(Folder { key: format!("mine:{}", r.get::<_, i64>(0)?), name: r.get(1)?, count: r.get(2)? }))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(SongTree { all, collections, mine })
}

fn sort_key(number: &Option<String>, title: &str) -> String {
    // pad digit runs so text order equals number order
    let src = number.clone().unwrap_or_else(|| format!("~{title}"));
    let mut out = String::new();
    let mut digits = String::new();
    for c in src.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        if !digits.is_empty() {
            out.push_str(&format!("{:0>8}", digits));
            digits.clear();
        }
        if c != '\0' {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// Songs in a folder, optionally narrowed by a search, in hymn-number order.
pub fn browse_songs(conn: &Connection, scope: &Scope, query: &str) -> Result<Vec<SongRow>> {
    let scoped: Option<Vec<i64>> = match scope {
        Scope::All => None,
        Scope::Hymnal { code } => Some(
            conn.prepare("SELECT n.song_id FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE h.code = ?1")?
                .query_map([code], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?,
        ),
        Scope::Mine { id } => Some(
            conn.prepare("SELECT song_id FROM my_collection_item WHERE collection_id = ?1 ORDER BY idx")?
                .query_map([id], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?,
        ),
    };
    let ids: Vec<i64> = if query.trim().is_empty() {
        match scoped {
            Some(v) => v,
            None => conn.prepare("SELECT id FROM song")?.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?,
        }
    } else {
        let hits: Vec<i64> = search_songs(conn, query, 1000)?.into_iter().map(|h| h.id).collect();
        match scoped {
            Some(v) => hits.into_iter().filter(|h| v.contains(h)).collect(),
            None => hits,
        }
    };
    let prefer = match scope {
        Scope::Hymnal { code } => Some(code.clone()),
        _ => None,
    };
    let mut st = conn.prepare(
        "SELECT s.title, s.author, s.copyright,
                (SELECT h.code || ' ' || n.number FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE n.song_id = s.id ORDER BY (h.code = ?2) DESC, h.code LIMIT 1),
                (SELECT g.text FROM song_section c JOIN song_segment g ON g.section_id = c.id WHERE c.song_id = s.id AND c.label <> 'TITLE' ORDER BY c.idx, g.idx LIMIT 1),
                (SELECT group_concat(lang, ',') FROM (SELECT lang FROM song_lyric l WHERE l.song_id = s.id ORDER BY l.rowid))
         FROM song s WHERE s.id = ?1",
    )?;
    let mut rows = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Ok((title, author, copyright, number, first_line, langs)) = st.query_row(params![id, prefer.as_deref().unwrap_or("")], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?))
        }) {
            let key = sort_key(&number, &title);
            let langs = langs.map(|l| l.split(',').map(str::to_string).collect()).unwrap_or_default();
            rows.push(SongRow { id: *id, number, title, first_line, author, copyright, langs, sort_key: key });
        }
    }
    // a search keeps its relevance order; a folder is listed in number order
    if query.trim().is_empty() && !matches!(scope, Scope::Mine { .. }) {
        rows.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    }
    Ok(rows)
}

// ---------- My collections ----------

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Collections".into(), "collection", msg.into())
}

pub fn create_collection(conn: &Connection, name: &str) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(bad("A collection needs a name."));
    }
    conn.execute("INSERT INTO my_collection (name) VALUES (?1)", [name]).map_err(|_| bad(format!("There is already a collection called “{name}”.")))?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_collection(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(bad("A collection needs a name."));
    }
    conn.execute("UPDATE my_collection SET name = ?2 WHERE id = ?1", params![id, name]).map_err(|_| bad(format!("There is already a collection called “{name}”.")))?;
    Ok(())
}

pub fn delete_collection(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM my_collection WHERE id = ?1", [id])?;
    Ok(())
}

/// Adds songs at the end; songs already in it are left where they are.
pub fn add_to_collection(conn: &Connection, id: i64, song_ids: &[i64]) -> Result<usize> {
    let mut next: i64 = conn.query_row("SELECT coalesce(max(idx), -1) + 1 FROM my_collection_item WHERE collection_id = ?1", [id], |r| r.get(0))?;
    let mut added = 0;
    for s in song_ids {
        added += conn.execute("INSERT OR IGNORE INTO my_collection_item (collection_id, song_id, idx) VALUES (?1, ?2, ?3)", params![id, s, next])?;
        next += 1;
    }
    Ok(added)
}

pub fn remove_from_collection(conn: &Connection, id: i64, song_ids: &[i64]) -> Result<()> {
    for s in song_ids {
        conn.execute("DELETE FROM my_collection_item WHERE collection_id = ?1 AND song_id = ?2", params![id, s])?;
    }
    Ok(())
}

// ---------- scripture table ----------

#[derive(Debug, Clone, Serialize)]
pub struct VerseRow {
    pub verse: u16,
    pub reference: String,
    pub text: String,
    pub heading: Option<String>,
    /// Inside the range that was typed.
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChapterView {
    pub version: String,
    pub book: String,
    pub book_no: u16,
    pub chapter: u16,
    pub chapters: u16,
    pub rows: Vec<VerseRow>,
}

/// The whole chapter of what is typed ("Gen", "Gen 1", "Yohane 3:16-18"), with the
/// typed verses marked. A book alone shows chapter 1.
pub fn browse_chapter(conn: &Connection, version_code: &str, typed: &str) -> Result<ChapterView> {
    let al = aliases(conn)?;
    let typed = typed.trim();
    let r = reference::parse(typed, &al).or_else(|_| reference::parse(&format!("{typed} 1"), &al))?;
    let (vid, book, chapters): (i64, String, u16) = conn
        .query_row(
            "SELECT v.id, b.name, b.chapters FROM bible_version v JOIN bible_book b ON b.version_id = v.id WHERE v.code = ?1 AND b.book_no = ?2",
            params![version_code, r.book_no],
            |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)),
        )
        .map_err(|_| Error::Format("Scripture".into(), "bible", format!("{version_code} does not have that book.")))?;
    let chapter = r.chapter.clamp(1, chapters.max(1));
    let (from, to) = match (r.verse_from, r.verse_to, r.to_chapter) {
        (Some(f), Some(t), None) => (f, t),
        (Some(f), Some(_), Some(_)) => (f, u16::MAX), // runs into the next chapter
        (Some(f), None, _) => (f, f),
        _ => (0, 0),
    };
    let mut st = conn.prepare("SELECT verse, text, heading FROM bible_verse WHERE version_id = ?1 AND book_no = ?2 AND chapter = ?3 ORDER BY verse")?;
    let rows = st
        .query_map(params![vid, r.book_no, chapter], |x| {
            let verse: u16 = x.get(0)?;
            Ok(VerseRow { verse, reference: format!("{book} {chapter}:{verse}"), text: x.get(1)?, heading: x.get(2)?, selected: from > 0 && verse >= from && verse <= to })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(ChapterView { version: version_code.into(), book, book_no: r.book_no, chapter, chapters, rows })
}

// ---------- Bible word search ----------

#[derive(Debug, Clone, Serialize)]
pub struct VerseHit {
    pub version: String,
    pub reference: String,
    pub book_no: u16,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
}

/// Fills the word index for one Bible the first time it is searched (about a second).
/// Text is folded like song search, so "Onyankopon" finds "Onyankopɔn".
fn index_version(conn: &Connection, vid: i64) -> Result<()> {
    // Bibles imported before 11 Nov 2026 were indexed unfolded: redo each one once
    let key = format!("bible_fts_folded:{vid}");
    let done: bool = conn.query_row("SELECT count(*) FROM setting WHERE key = ?1", [&key], |r| r.get::<_, i64>(0))? > 0;
    if done {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM bible_fts WHERE version_id = ?1", [vid])?;
    tx.execute("INSERT OR REPLACE INTO setting (key, value) VALUES (?1, '1')", [&key])?;
    {
        let mut read = tx.prepare("SELECT book_no, chapter, verse, text FROM bible_verse WHERE version_id = ?1")?;
        let mut write = tx.prepare("INSERT INTO bible_fts (text, version_id, book_no, chapter, verse) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        let rows = read.query_map([vid], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?)))?;
        for row in rows {
            let (b, c, v, t) = row?;
            write.execute(params![crate::text::fold(&t), vid, b, c, v])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Verses containing all the words typed (in any order; "quotes" for an exact phrase),
/// best matches first, in the chosen Bibles (all Bibles when none are given).
pub fn search_bible(conn: &Connection, versions: &[String], query: &str, limit: usize) -> Result<Vec<VerseHit>> {
    let folded = crate::text::fold(query);
    let phrase = query.trim().starts_with('"');
    let words: Vec<String> = folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.replace('"', "")).collect();
    if words.is_empty() {
        return Ok(vec![]);
    }
    let fts = if phrase { format!("\"{}\"", words.join(" ")) } else { words.iter().map(|w| format!("\"{w}\"*")).collect::<Vec<_>>().join(" ") };
    let mut ids: Vec<(i64, String)> = conn.prepare("SELECT id, code FROM bible_version ORDER BY code")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
    if !versions.is_empty() {
        ids.retain(|(_, c)| versions.contains(c));
    }
    let mut hits = vec![];
    for (vid, code) in &ids {
        index_version(conn, *vid)?;
        let mut st = conn.prepare(
            "SELECT f.book_no, f.chapter, f.verse, v.text, b.name
             FROM bible_fts f JOIN bible_verse v ON v.version_id = f.version_id AND v.book_no = f.book_no AND v.chapter = f.chapter AND v.verse = f.verse
             JOIN bible_book b ON b.version_id = f.version_id AND b.book_no = f.book_no
             WHERE bible_fts MATCH ?1 AND f.version_id = ?2 ORDER BY bm25(bible_fts) LIMIT ?3",
        )?;
        let rows = st.query_map(params![fts, vid, limit as i64], |r| {
            let (b, c, v): (u16, u16, u16) = (r.get(0)?, r.get(1)?, r.get(2)?);
            let name: String = r.get(4)?;
            Ok(VerseHit { version: code.clone(), reference: format!("{name} {c}:{v}"), book_no: b, chapter: c, verse: v, text: r.get(3)? })
        })?;
        for h in rows {
            hits.push(h?);
        }
    }
    hits.truncate(limit);
    Ok(hits)
}

// ---------- maintenance ----------

/// EasyWorship's "Rebuild search keys": rewrites every song's search entry.
pub fn rebuild_search(conn: &Connection) -> Result<usize> {
    let ids: Vec<i64> = conn.prepare("SELECT id FROM song")?.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    let tx = conn.unchecked_transaction()?;
    for id in &ids {
        crate::songs::reindex(&tx, *id)?;
    }
    tx.commit()?;
    Ok(ids.len())
}

/// EasyWorship's "Reset songs to default theme": clears per-song theme and lines
/// settings so every song follows the theme and lines in use.
pub fn reset_song_looks(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("UPDATE song SET default_theme_id = NULL, lines_per_slide = NULL WHERE default_theme_id IS NOT NULL OR lines_per_slide IS NOT NULL", [])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Connection {
        let c = crate::db::open_memory().unwrap();
        for (title, code, num) in [("MHB 10", "MHB", "10"), ("MHB 9", "MHB", "9"), ("MHB 851", "MHB", "851"), ("NCAN 2", "NCAN", "2")] {
            let d = crate::songs::SongDoc {
                id: None,
                title: title.into(),
                author: Some("Wesley".into()),
                copyright: Some("Public Domain".into()),
                ccli: None,
                numbers: vec![crate::songs::Number { hymnal: code.into(), number: num.into() }],
                lyrics: vec![crate::songs::Lyric { lang: "en".into(), body: format!("[V1]\nFirst line of {title}") }],
                sequence: String::new(),
                lines_per_slide: Some(3),
                key: None,
            };
            crate::songs::save(&c, &d).unwrap();
        }
        c
    }

    #[test]
    fn tree_folders_and_number_order() {
        let c = lib();
        let t = song_tree(&c).unwrap();
        assert_eq!(t.all, 4);
        assert_eq!(t.collections.iter().map(|f| (f.key.as_str(), f.count)).collect::<Vec<_>>(), vec![("hymnal:MHB", 3), ("hymnal:NCAN", 1)]);
        let rows = browse_songs(&c, &Scope::Hymnal { code: "MHB".into() }, "").unwrap();
        assert_eq!(rows.iter().map(|r| r.number.clone().unwrap()).collect::<Vec<_>>(), vec!["MHB 9", "MHB 10", "MHB 851"]);
        assert_eq!(rows[0].first_line.as_deref(), Some("First line of MHB 9"));
        assert_eq!(rows[0].copyright.as_deref(), Some("Public Domain"));
        assert_eq!(rows[0].langs, vec!["en".to_string()]);
        let found = browse_songs(&c, &Scope::Hymnal { code: "NCAN".into() }, "first line").unwrap();
        assert_eq!(found.len(), 1, "a search inside a collection stays inside it");
    }

    #[test]
    fn my_collections() {
        let c = lib();
        let id = create_collection(&c, "Easter").unwrap();
        assert!(create_collection(&c, "Easter").is_err());
        let all = browse_songs(&c, &Scope::All, "").unwrap();
        assert_eq!(add_to_collection(&c, id, &[all[2].id, all[0].id, all[0].id]).unwrap(), 2);
        let mine = browse_songs(&c, &Scope::Mine { id }, "").unwrap();
        assert_eq!(mine.iter().map(|r| r.id).collect::<Vec<_>>(), vec![all[2].id, all[0].id], "kept in the order added");
        remove_from_collection(&c, id, &[all[2].id]).unwrap();
        rename_collection(&c, id, "Easter Sunday").unwrap();
        let t = song_tree(&c).unwrap();
        assert_eq!((t.mine[0].name.as_str(), t.mine[0].count), ("Easter Sunday", 1));
        delete_collection(&c, id).unwrap();
        assert!(song_tree(&c).unwrap().mine.is_empty());
    }

    #[test]
    fn maintenance() {
        let c = lib();
        assert_eq!(reset_song_looks(&c).unwrap(), 4);
        assert_eq!(rebuild_search(&c).unwrap(), 4);
        assert_eq!(search_songs(&c, "first line", 10).unwrap().len(), 4);
    }

    #[test]
    fn number_sort_keys() {
        assert!(sort_key(&Some("MHB 9".into()), "") < sort_key(&Some("MHB 10".into()), ""));
        assert!(sort_key(&Some("MHB 12a".into()), "") < sort_key(&Some("MHB 12b".into()), ""));
    }
}
