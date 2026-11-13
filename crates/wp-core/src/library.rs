//! Read queries used by the control window: song search, song sections, versions,
//! and scripture lookup by typed reference.

use crate::reference::{self, Alias, Reference};
use crate::splitter::{Section, Segment};
use crate::text::fold;
use crate::Result;
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SongHit {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub numbers: Vec<String>,
    pub first_line: Option<String>,
}

/// Searches by hymn number ("MHB 50", "ncan 12"), title, author or any lyric line.
pub fn search_songs(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SongHit>> {
    let q = query.trim();
    let mut ids: Vec<i64> = Vec::new();
    let mut parts = q.split_whitespace();
    if let (Some(code), Some(num), None) = (parts.next(), parts.next(), parts.next()) {
        if num.chars().all(|c| c.is_ascii_digit()) {
            let mut st = conn.prepare(
                "SELECT n.song_id FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE upper(h.code) = upper(?1) AND n.number = ?2 ORDER BY n.song_id",
            )?;
            ids.extend(st.query_map(params![code, num], |r| r.get::<_, i64>(0))?.filter_map(|r| r.ok()));
        }
    }
    // an exact hymn number answers the question; skip fuzzy matches
    if !ids.is_empty() {
        return hits(conn, ids);
    }
    let terms: Vec<String> = fold(q).split_whitespace().map(|t| format!("\"{}\"*", t.replace('"', ""))).collect();
    if !terms.is_empty() {
        let mut st = conn.prepare("SELECT rowid FROM song_fts WHERE song_fts MATCH ?1 ORDER BY rank LIMIT ?2")?;
        for id in st.query_map(params![terms.join(" "), limit as i64], |r| r.get::<_, i64>(0))?.filter_map(|r| r.ok()) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids.truncate(limit);
    hits(conn, ids)
}

/// The quick box's hymn numbers: "555" (the main hymnbook), "MHB 555", "mhb555",
/// "ncan 12". Returns nothing for anything else (so "John 3" stays scripture). Where a
/// number has an English and a bilingual song, the one with more languages comes first.
pub fn quick_song(conn: &Connection, typed: &str) -> Result<Vec<SongHit>> {
    let t = typed.trim();
    let split = t.find(|c: char| c.is_ascii_digit()).unwrap_or(t.len());
    let (code, num) = (t[..split].trim(), t[split..].trim());
    if num.is_empty() || !num.chars().next().unwrap().is_ascii_digit() || !num.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(vec![]);
    }
    let code: String = if code.is_empty() {
        // the chosen main hymnbook, else the one with the most songs
        let chosen: Option<String> = conn.query_row("SELECT value FROM setting WHERE key = 'quick_hymnal'", [], |r| r.get(0)).ok();
        match chosen.filter(|c| !c.is_empty()) {
            Some(c) => c,
            None => match conn
                .query_row("SELECT h.code FROM hymnal h JOIN song_number n ON n.hymnal_id = h.id GROUP BY h.id ORDER BY count(*) DESC LIMIT 1", [], |r| r.get(0))
                .ok()
            {
                Some(c) => c,
                None => return Ok(vec![]),
            },
        }
    } else {
        let known: Option<String> = conn.query_row("SELECT code FROM hymnal WHERE upper(code) = upper(?1)", [code], |r| r.get(0)).ok();
        match known {
            Some(c) => c,
            None => return Ok(vec![]), // not a hymnbook: a Bible book perhaps
        }
    };
    let mut st = conn.prepare(
        "SELECT n.song_id FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id
         WHERE h.code = ?1 AND upper(n.number) = upper(?2)
         ORDER BY (SELECT count(DISTINCT lang) FROM song_lyric l WHERE l.song_id = n.song_id) DESC, n.song_id",
    )?;
    let ids: Vec<i64> = st.query_map(params![code, num], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
    hits(conn, ids)
}

fn hits(conn: &Connection, ids: Vec<i64>) -> Result<Vec<SongHit>> {
    let mut out = Vec::new();
    for id in ids {
        let (title, author): (String, Option<String>) = conn.query_row("SELECT title, author FROM song WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut st = conn.prepare("SELECT h.code || ' ' || n.number FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE n.song_id = ?1")?;
        let numbers = st.query_map([id], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()).collect();
        let first_line = conn
            .query_row(
                "SELECT g.text FROM song_segment g JOIN song_section s ON s.id = g.section_id WHERE s.song_id = ?1 AND s.label <> 'TITLE' ORDER BY s.idx, g.align_key, g.idx LIMIT 1",
                [id],
                |r| r.get(0),
            )
            .ok();
        out.push(SongHit { id, title, author, numbers, first_line });
    }
    Ok(out)
}

pub fn song_sections(conn: &Connection, song_id: i64) -> Result<Vec<Section>> {
    let mut st = conn.prepare("SELECT id, label FROM song_section WHERE song_id = ?1 ORDER BY idx")?;
    let secs: Vec<(i64, String)> = st.query_map([song_id], |r| Ok((r.get(0)?, r.get(1)?)))?.filter_map(|r| r.ok()).collect();
    let mut seg = conn.prepare("SELECT lang, text, align_key, chords FROM song_segment WHERE section_id = ?1 ORDER BY align_key, lang, idx")?;
    let mut out = Vec::new();
    for (id, label) in secs {
        let segments = seg
            .query_map([id], |r| Ok(Segment { lang: r.get(0)?, text: r.get(1)?, align_key: r.get(2)?, chords: r.get(3)? }))?
            .filter_map(|r| r.ok())
            .collect();
        out.push(Section { id, label, segments });
    }
    Ok(out)
}

/// Languages present in a song, primary first.
pub fn song_langs(conn: &Connection, song_id: i64) -> Result<Vec<String>> {
    let mut st = conn.prepare("SELECT lang FROM song_lyric WHERE song_id = ?1 ORDER BY id")?;
    let v = st.query_map([song_id], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
    Ok(v)
}

#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub lang: Option<String>,
}

pub fn versions(conn: &Connection) -> Result<Vec<Version>> {
    let mut st = conn.prepare("SELECT id, code, name, lang FROM bible_version ORDER BY id")?;
    let v = st.query_map([], |r| Ok(Version { id: r.get(0)?, code: r.get(1)?, name: r.get(2)?, lang: r.get(3)? }))?.filter_map(|r| r.ok()).collect();
    Ok(v)
}

/// Canonical English aliases plus every name and abbreviation of every imported version.
pub fn aliases(conn: &Connection) -> Result<Vec<Alias>> {
    let mut a = reference::english_aliases();
    let mut st = conn.prepare("SELECT book_no, aliases FROM bible_book")?;
    for r in st.query_map([], |r| Ok((r.get::<_, u16>(0)?, r.get::<_, String>(1)?)))?.filter_map(|r| r.ok()) {
        for name in r.1.split('|') {
            let f = fold(name);
            if !f.is_empty() && !a.iter().any(|x| x.folded == f) {
                a.push(Alias { folded: f, book_no: r.0 });
            }
        }
    }
    Ok(a)
}

#[derive(Debug, Clone, Serialize)]
pub struct VerseOut {
    pub version: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
    pub marked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Passage {
    pub reference: Reference,
    /// "Yohane 3:16-18" in the first version's own book name.
    pub display: String,
    pub verses: Vec<VerseOut>,
}

/// Looks up a typed reference in one or more versions (parallel display).
pub fn lookup(conn: &Connection, typed: &str, version_codes: &[&str]) -> Result<Passage> {
    let r = reference::parse(typed, &aliases(conn)?)?;
    let to_ch = r.to_chapter.unwrap_or(r.chapter);
    let mut verses = Vec::new();
    let mut display_name = reference::book_by_no(r.book_no).map(|b| b.english[0].to_string()).unwrap_or_default();
    for (i, code) in version_codes.iter().enumerate() {
        let vid: i64 = conn.query_row("SELECT id FROM bible_version WHERE code = ?1", [code], |x| x.get(0))?;
        if i == 0 {
            if let Ok(n) = conn.query_row("SELECT name FROM bible_book WHERE version_id = ?1 AND book_no = ?2", params![vid, r.book_no], |x| x.get::<_, String>(0)) {
                display_name = n;
            }
        }
        let mut st = conn.prepare(
            "SELECT chapter, verse, text, marked FROM bible_verse WHERE version_id = ?1 AND book_no = ?2
             AND (chapter * 1000 + verse) BETWEEN ?3 AND ?4 ORDER BY chapter, verse",
        )?;
        let lo = r.chapter as i64 * 1000 + r.verse_from.unwrap_or(1) as i64;
        let hi = to_ch as i64 * 1000 + r.verse_to.or(r.verse_from.filter(|_| r.to_chapter.is_none())).unwrap_or(999) as i64;
        let rows = st.query_map(params![vid, r.book_no, lo, hi], |x| {
            Ok(VerseOut { version: code.to_string(), chapter: x.get(0)?, verse: x.get(1)?, text: x.get(2)?, marked: x.get(3)? })
        })?;
        verses.extend(rows.filter_map(|x| x.ok()));
    }
    let span = match (r.verse_from, r.to_chapter, r.verse_to) {
        (None, _, _) => format!("{}", r.chapter),
        (Some(v), Some(c2), Some(e)) => format!("{}:{}-{}:{}", r.chapter, v, c2, e),
        (Some(v), None, Some(e)) => format!("{}:{}-{}", r.chapter, v, e),
        (Some(v), _, None) => format!("{}:{}", r.chapter, v),
    };
    Ok(Passage { display: format!("{display_name} {span}"), reference: r, verses })
}

#[cfg(test)]
mod quick_tests {
    use super::*;

    fn song(c: &Connection, title: &str, code: &str, num: &str, langs: &[&str]) {
        let d = crate::songs::SongDoc {
            id: None,
            title: title.into(),
            author: None,
            copyright: None,
            ccli: None,
            numbers: vec![crate::songs::Number { hymnal: code.into(), number: num.into() }],
            lyrics: langs.iter().map(|l| crate::songs::Lyric { lang: (*l).into(), body: "[V1]\nline".into() }).collect(),
            sequence: String::new(),
            lines_per_slide: None,
            key: None,
        };
        crate::songs::save(c, &d).unwrap();
    }

    #[test]
    fn hymn_numbers_in_the_quick_box() {
        let c = crate::db::open_memory().unwrap();
        song(&c, "MHB 555", "MHB", "555", &["en"]);
        song(&c, "MHB 555, ENG&FANTE", "MHB", "555", &["en", "fat"]);
        song(&c, "MHB 1", "MHB", "1", &["en"]);
        song(&c, "NCAN 12", "NCAN", "12", &["en"]);
        let t = |q: &str| quick_song(&c, q).unwrap().into_iter().map(|h| h.title).collect::<Vec<_>>();
        assert_eq!(t("555"), vec!["MHB 555, ENG&FANTE", "MHB 555"], "bare number: the main hymnbook, bilingual first");
        assert_eq!(t("MHB 555")[0], "MHB 555, ENG&FANTE");
        assert_eq!(t("mhb555")[0], "MHB 555, ENG&FANTE");
        assert_eq!(t("ncan 12"), vec!["NCAN 12"]);
        assert!(t("John 3").is_empty(), "a Bible book is not a hymnbook");
        assert!(t("Yohane 3:16").is_empty());
        assert!(t("9999").is_empty());
        c.execute("INSERT INTO setting (key, value) VALUES ('quick_hymnal', 'NCAN')", []).unwrap();
        assert_eq!(t("12"), vec!["NCAN 12"], "the chosen main hymnbook");
    }
}

