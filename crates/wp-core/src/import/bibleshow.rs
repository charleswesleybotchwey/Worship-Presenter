//! Bible Show (.bib) importer.
//!
//! Format, verified in Phase 0 against NKK2011 (Dagbani), TK2012 (Asante Twi) and
//! YGZP2020 (Farefare NT): a Microsoft Access (Jet 4) database with tables
//! Bible(Book, Chapter, Verse, Scripture), Titles, Footnotes, Info(Parameter, Value)
//! and Structure(BibPosition, ListTitle, Abbreviation, FullTitle, OtherAbbrev,
//! DisplayTitle, Chapters). Book numbers already follow the canon used here.
//!
//! Scripture carries light HTML: <span class='Isus'> marks words of Jesus, <p>
//! starts a paragraph, <br> and <span style='padding:1em;'> lay out poetry, * marks
//! a footnote anchor, <i> and <h> are emphasis. We keep red letters and paragraph
//! starts in `marked`, line breaks in the plain text, and drop the rest.
//!
//! The file is read by our own Jet 4 reader (crate::jet), checked row for row
//! against mdbtools on all three files. No Access drivers or helper programs.

use super::*;
use crate::text::clean;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub trait Source {
    /// Every row of a table as JSON objects (column name → value).
    fn rows(&self, table: &str) -> Result<Vec<serde_json::Map<String, Value>>>;
}

fn s(m: &serde_json::Map<String, Value>, k: &str) -> Option<String> {
    match m.get(k)? {
        Value::String(x) => Some(x.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
fn n(m: &serde_json::Map<String, Value>, k: &str) -> Option<u16> {
    m.get(k)?.as_u64().map(|v| v as u16).or_else(|| s(m, k)?.parse().ok())
}

/// Converts Bible Show markup into (plain text, marked text).
pub fn convert_markup(src: &str) -> (String, String) {
    let mut plain = String::new();
    let mut marked = String::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut rest = src;
    while let Some(lt) = rest.find('<') {
        let (before, after) = rest.split_at(lt);
        push_text(before, &mut plain, &mut marked);
        let Some(gt) = after.find('>') else { break };
        let tag = after[1..gt].trim().to_ascii_lowercase();
        rest = &after[gt + 1..];
        if tag.starts_with("span") {
            let red = tag.contains("isus");
            stack.push(if red { "r" } else { "" });
            if red {
                marked.push_str("<r>");
            }
        } else if tag == "/span" {
            if stack.pop() == Some("r") {
                marked.push_str("</r>");
            }
        } else if tag == "br" || tag == "br/" || tag == "br /" {
            plain.push('\n');
            marked.push('\n');
        } else if tag == "p" {
            if !marked.contains("<p>") {
                marked.insert_str(0, "<p>");
            }
        }
        // <i>, </i>, <h>, </h> and anything unknown: drop the tag, keep the text
    }
    push_text(rest, &mut plain, &mut marked);
    let tidy = |t: String| {
        t.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
    };
    (tidy(plain), tidy(marked))
}

fn push_text(t: &str, plain: &mut String, marked: &mut String) {
    let t = t.replace('*', "").replace("&nbsp;", " ").replace("&amp;", "&").replace('\u{00A0}', " ");
    plain.push_str(&t);
    marked.push_str(&t);
}

/// Opens a .bib file with the built-in Jet reader and imports it.
pub fn import_file(conn: &Connection, path: &Path, code: Option<&str>) -> Result<Report> {
    let db = crate::jet::Database::open(path)?;
    let name = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    import(conn, &db, &name, code)
}

pub fn import(conn: &Connection, src: &dyn Source, file_name: &str, code: Option<&str>) -> Result<Report> {
    let info: HashMap<String, String> = src
        .rows("Info")?
        .iter()
        .filter_map(|r| Some((s(r, "Parameter")?, s(r, "Value").unwrap_or_default())))
        .collect();
    let name = info.get("BibleFullName").cloned().unwrap_or_else(|| file_name.to_string());
    let code = code.map(str::to_string).or_else(|| info.get("BibleShortName").cloned()).unwrap_or_else(|| file_name.to_string());
    let copyright = info.get("Copyright").filter(|v| !v.is_empty()).cloned();
    let protected = info.get("Protected").map(|v| v == "1").unwrap_or(false);
    let lang = info.get("Language").cloned();

    let tx = conn.unchecked_transaction()?;
    let licence = match (&copyright, protected) {
        (Some(c), true) => Some(format!("© {c}. Marked protected by the source file; do not redistribute.")),
        (Some(c), false) => Some(format!("© {c}")),
        (None, true) => Some("Marked protected by the source file; do not redistribute.".to_string()),
        (None, false) => None,
    };
    let cid = create_collection(&tx, &Collection {
        kind: "bible",
        name: &name,
        source_format: "bibleshow",
        source_file: file_name,
        publisher: copyright.as_deref(),
        licence_note: licence.as_deref(),
        transferable: !protected,
    })?;
    tx.execute(
        "INSERT INTO bible_version (collection_id, code, name, lang, copyright) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![cid, code, name, lang, copyright],
    )?;
    let vid = tx.last_insert_rowid();
    let mut rep = Report { collection_id: cid, name: name.clone(), ..Default::default() };

    for b in src.rows("Structure")? {
        let Some(no) = n(&b, "BibPosition") else { continue };
        let display = s(&b, "DisplayTitle").or_else(|| s(&b, "ListTitle")).unwrap_or_else(|| format!("Book {no}"));
        let mut aliases: Vec<String> = ["ListTitle", "FullTitle"].iter().filter_map(|k| s(&b, k)).collect();
        if let Some(other) = s(&b, "OtherAbbrev") {
            aliases.extend(other.split([',', ';']).map(|a| a.trim().trim_matches('\'').to_string()));
        }
        if let Some(canon) = crate::reference::book_by_no(no) {
            aliases.push(canon.english[0].to_string());
        }
        insert_book(&tx, vid, &BookIn { book_no: no, name: display, abbrev: s(&b, "Abbreviation"), aliases, chapters: n(&b, "Chapters").unwrap_or(0) })?;
    }

    let mut headings: HashMap<(u16, u16, u16), String> = HashMap::new();
    for t in src.rows("Titles")? {
        if let (Some(b), Some(c), Some(v), Some(title)) = (n(&t, "Book"), n(&t, "Chapter"), n(&t, "Verse"), s(&t, "Title")) {
            let (plain, _) = convert_markup(&title);
            headings.entry((b, c, v)).and_modify(|h| { h.push('\n'); h.push_str(&plain) }).or_insert(plain);
        }
    }
    {
        let mut sv = tx.prepare(VERSE_SQL)?;
        let mut sf = tx.prepare(FTS_SQL)?;
        for r in src.rows("Bible")? {
            rep.read += 1;
            let (Some(b), Some(c), Some(v)) = (n(&r, "Book"), n(&r, "Chapter"), n(&r, "Verse")) else {
                log(&tx, cid, "skipped", "row", "missing book, chapter or verse number")?;
                rep.skipped += 1;
                continue;
            };
            let item = format!("{b} {c}:{v}");
            let raw = s(&r, "Scripture").unwrap_or_default();
            let (plain, marked) = convert_markup(&raw);
            if plain.is_empty() {
                log(&tx, cid, "skipped", &item, "empty verse")?;
                rep.skipped += 1;
                continue;
            }
            let (plain, fixes) = clean(&plain);
            let (marked, _) = clean(&marked);
            rep.fixed_chars += log_fixes(&tx, cid, &item, &fixes)?;
            let marked = (marked != plain).then_some(marked);
            insert_verse(&mut sv, &mut sf, vid, b, c, v, &plain, marked.as_deref(), headings.get(&(b, c, v)).map(String::as_str))?;
            rep.imported += 1;
        }
    }
    let footnotes = src.rows("Footnotes").map(|r| r.len()).unwrap_or(0);
    if footnotes > 0 {
        rep.notes.push(format!("{footnotes} footnotes not imported (not projected in this version)"));
        log(&tx, cid, "info", &name, &format!("{footnotes} footnotes not imported"))?;
    }
    log(&tx, cid, "info", &name, &format!("read {} verses, imported {}, skipped {}", rep.read, rep.imported, rep.skipped))?;
    tx.commit()?;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::convert_markup;

    #[test]
    fn red_letters_paragraphs_and_poetry() {
        let (p, m) = convert_markup("*<span class='Isus'>Na sɛdeɛ Onyankopɔn dɔɔ wiase nie.</span><p>");
        assert_eq!(p, "Na sɛdeɛ Onyankopɔn dɔɔ wiase nie.");
        assert_eq!(m, "<p><r>Na sɛdeɛ Onyankopɔn dɔɔ wiase nie.</r>");
        let (p, _) = convert_markup("Line one,<br><span style='padding:1em;'>line two.</span>");
        assert_eq!(p, "Line one,\nline two.");
        let (p, _) = convert_markup("<i>BUTU: </i>Mfonini [a] Hebri");
        assert_eq!(p, "BUTU: Mfonini [a] Hebri");
    }
}
