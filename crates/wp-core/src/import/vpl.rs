//! Free Bibles from eBible.org: its catalogue (translations.csv) and its "verse per
//! line" downloads ({id}_vpl.zip), as plain text ("GEN 1:1 In the beginning…") or XML
//! (<v b="GEN" c="1" v="1">…</v>). Book codes are the standard USFM ones.

use super::{create_collection, insert_book, insert_verse, BookIn, Collection, Report, FTS_SQL, VERSE_SQL};
use crate::reference::BOOKS;
use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::io::Read;

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Bible download".into(), "vpl", msg.into())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Offer {
    pub id: String,
    pub language: String,
    pub title: String,
}

/// The shareable translations in eBible.org's catalogue (columns found by their names).
pub fn catalogue(csv: &str) -> Vec<Offer> {
    let mut rows = csv.lines().map(split_csv);
    let Some(head) = rows.next() else { return vec![] };
    let col = |names: &[&str]| head.iter().position(|h| names.iter().any(|n| h.trim().eq_ignore_ascii_case(n)));
    let (Some(id), Some(title)) = (col(&["translationId", "id"]), col(&["shortTitle", "title"])) else { return vec![] };
    let lang = col(&["languageNameInEnglish", "languageName", "languageCode"]);
    let shareable = col(&["Redistributable", "redistributable"]);
    let downloadable = col(&["downloadable"]);
    let yes = |r: &Vec<String>, c: Option<usize>| c.map_or(true, |c| r.get(c).is_some_and(|v| v.trim().eq_ignore_ascii_case("true") || v.trim() == "1"));
    let mut out: Vec<Offer> = rows
        .filter(|r| r.len() > id && yes(r, shareable) && yes(r, downloadable))
        .map(|r| Offer { id: r[id].trim().to_string(), language: lang.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default(), title: r.get(title).map(|s| s.trim().to_string()).unwrap_or_default() })
        .filter(|o| !o.id.is_empty() && o.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .collect();
    out.sort_by(|a, b| a.language.cmp(&b.language).then(a.title.cmp(&b.title)));
    out
}

fn split_csv(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                out.last_mut().unwrap().push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(String::new()),
            _ => out.last_mut().unwrap().push(c),
        }
    }
    out
}

/// Verses from a VPL file: (book number, chapter, verse, text).
pub fn parse(text: &str) -> Vec<(u16, u16, u16, String)> {
    let code = |c: &str| BOOKS.iter().find(|b| b.code.eq_ignore_ascii_case(c)).map(|b| b.no);
    let mut out = vec![];
    if text.contains("<v ") {
        // XML: <v b="GEN" c="1" v="1">In the beginning…</v>
        for part in text.split("<v ").skip(1) {
            let attr = |name: &str| part.split(&format!("{name}=\"")).nth(1).and_then(|s| s.split('"').next()).map(str::to_string);
            let body = part.split_once('>').map(|x| x.1).and_then(|b| b.split("</v>").next()).unwrap_or("");
            if let (Some(b), Some(c), Some(v)) = (attr("b").and_then(|b| code(&b)), attr("c").and_then(|c| c.parse().ok()), attr("v").and_then(|v| v.parse().ok())) {
                let t = body.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'");
                if !t.trim().is_empty() {
                    out.push((b, c, v, t.trim().to_string()));
                }
            }
        }
        return out;
    }
    for line in text.lines() {
        // GEN 1:1 In the beginning…
        let mut it = line.trim_start_matches('\u{feff}').splitn(3, ' ');
        let (Some(b), Some(cv), Some(t)) = (it.next(), it.next(), it.next()) else { continue };
        let Some(b) = code(b) else { continue };
        let Some((c, v)) = cv.split_once(':') else { continue };
        let (Ok(c), Ok(v)) = (c.parse::<u16>(), v.trim_end_matches(|x: char| !x.is_ascii_digit()).parse::<u16>()) else { continue };
        if !t.trim().is_empty() {
            out.push((b, c, v, t.trim().to_string()));
        }
    }
    out
}

/// The VPL text inside a downloaded zip (or the bytes themselves if they are the text).
pub fn text_of(bytes: &[u8]) -> Result<String> {
    if !bytes.starts_with(b"PK") {
        return Ok(String::from_utf8_lossy(bytes).into_owned());
    }
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| bad("The download is damaged."))?;
    let mut best: Option<String> = None;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|_| bad("The download is damaged."))?;
        let name = String::from_utf8_lossy(f.name_raw()).to_lowercase();
        if (name.ends_with(".txt") || name.ends_with(".xml")) && f.size() < 60_000_000 {
            let mut s = String::new();
            f.read_to_string(&mut s).map_err(|_| bad("The download is damaged."))?;
            if parse(&s).len() > 100 {
                best = Some(s);
                break;
            }
        }
    }
    best.ok_or_else(|| bad("No Bible text was found in the download."))
}

/// Imports a downloaded Bible as a new version (English book names; they can be renamed).
pub fn import(conn: &Connection, text: &str, code: &str, name: &str, lang: &str, source: &str) -> Result<Report> {
    let verses = parse(text);
    if verses.len() < 100 {
        return Err(bad("This does not look like a Bible (too few verses)."));
    }
    let code = code.to_uppercase();
    let exists: i64 = conn.query_row("SELECT count(*) FROM bible_version WHERE code = ?1", [&code], |r| r.get(0))?;
    if exists > 0 {
        return Err(bad(format!("A Bible called {code} is already in the library.")));
    }
    let tx = conn.unchecked_transaction()?;
    let cid = create_collection(&tx, &Collection { kind: "bible", name, source_format: "ebible-vpl", source_file: source, publisher: Some("eBible.org"), licence_note: None, transferable: true })?;
    tx.execute("INSERT INTO bible_version (collection_id, code, name, lang, copyright) VALUES (?1, ?2, ?3, ?4, ?5)", params![cid, code, name, lang, "eBible.org (free to share)"])?;
    let vid = tx.last_insert_rowid();
    let mut rep = Report { collection_id: cid, name: name.to_string(), ..Default::default() };
    for book in BOOKS.iter().filter(|b| verses.iter().any(|v| v.0 == b.no)) {
        let chapters = verses.iter().filter(|v| v.0 == book.no).map(|v| v.1).max().unwrap_or(1);
        insert_book(&tx, vid, &BookIn { book_no: book.no, name: book.english[0].to_string(), abbrev: Some(book.ew.to_string()), aliases: book.english.iter().map(|s| s.to_string()).collect(), chapters })?;
    }
    {
        let mut sv = tx.prepare(VERSE_SQL)?;
        let mut sf = tx.prepare(FTS_SQL)?;
        for (b, c, v, t) in &verses {
            if insert_verse(&mut sv, &mut sf, vid, *b, *c, *v, t, None, None).is_ok() {
                rep.read += 1;
            } else {
                rep.skipped += 1;
            }
        }
    }
    tx.commit()?;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_lists_shareable_bibles() {
        let csv = "languageCode,translationId,languageName,languageNameInEnglish,shortTitle,Redistributable,downloadable\n\
                   eng,engwebp,English,English,World English Bible,True,True\n\
                   twi,twiXYZ,Twi,\"Akan, Twi\",Twi New Testament,False,True\n\
                   eng,engkjv,English,English,\"King James Version, 1769\",True,True";
        let c = catalogue(csv);
        assert_eq!(c.len(), 2, "not shareable: left out");
        assert_eq!(c[0].title, "King James Version, 1769", "quoted commas kept");
        assert_eq!(c[1].id, "engwebp");
    }

    #[test]
    fn verse_per_line_text_and_xml_import() {
        let mut text = String::from("\u{feff}GEN 1:1 In the beginning God created the heavens and the earth.\nGEN 1:2 The earth was formless and empty.\n");
        for v in 1..=120 {
            text.push_str(&format!("JHN 3:{v} Verse {v} of John three.\n"));
        }
        let p = parse(&text);
        assert_eq!(p[0], (1, 1, 1, "In the beginning God created the heavens and the earth.".to_string()));
        assert_eq!(p.len(), 122);
        let xml = r#"<vpl><v b="PSA" c="23" v="1">The LORD is my shepherd; I shall not want.</v><v b="1CO" c="13" v="4">Love is patient &amp; kind.</v></vpl>"#;
        let x = parse(xml);
        assert_eq!(x[0].0, 19);
        assert_eq!(x[1], (46, 13, 4, "Love is patient & kind.".to_string()));
        let c = crate::db::open_memory().unwrap();
        let r = import(&c, &text, "web", "World English Bible", "en", "engwebp_vpl.zip").unwrap();
        assert_eq!(r.read, 122);
        let v = crate::browse::browse_chapter(&c, "WEB", "John 3:16").unwrap();
        assert_eq!(v.rows.iter().find(|r| r.verse == 16).unwrap().text, "Verse 16 of John three.");
        assert_eq!(crate::browse::search_bible(&c, &["WEB".into()], "formless empty", 5).unwrap()[0].reference, "Genesis 1:2", "searchable at once");
        assert!(import(&c, &text, "WEB", "again", "en", "x").is_err(), "no duplicates");
    }
}
