//! Song editing: load a song as an editable document, preview it unsaved, save it,
//! create new songs, delete songs.
//!
//! Lyrics are edited as plain text per language:
//!
//! ```text
//! [V1]
//! O for a thousand tongues to sing
//! My great Redeemer's praise,
//!
//! [C]
//! ...
//! ```
//!
//! A `[label]` line starts a section (V1, C, B, PC, T, E, or "Verse 2", "Chorus").
//! Text before the first label belongs to V1. Blank lines are only for readability;
//! slides are made by the lines-per-slide setting. A second language uses the same
//! labels; its lines are paired with the first language's section by section.
//! When neither language's text changed, the existing line pairing is kept, so
//! re-saving an imported bilingual hymn never disturbs its alignment.

use crate::import::ew_songs::short_label;
use crate::splitter::{self, Options, Section as SplitSection, Segment, Slide};
use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lyric {
    pub lang: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Number {
    pub hymnal: String,
    pub number: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SongDoc {
    pub id: Option<i64>,
    pub title: String,
    pub author: Option<String>,
    pub copyright: Option<String>,
    pub ccli: Option<String>,
    pub numbers: Vec<Number>,
    /// First entry is the primary language.
    pub lyrics: Vec<Lyric>,
    /// Play order as labels, e.g. "V1 C V2 C"; empty = written order.
    pub sequence: String,
    pub lines_per_slide: Option<u8>,
    /// Musical key, e.g. "G" (for the musicians' stage view).
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSection {
    pub label: String,
    pub lines: Vec<String>,
}

fn invalid(msg: impl Into<String>) -> Error {
    Error::Format("Song".into(), "song", msg.into())
}

/// Splits one language's text into labelled sections.
pub fn parse_body(body: &str) -> Vec<ParsedSection> {
    let mut out: Vec<ParsedSection> = Vec::new();
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
            let mut label = short_label(&line[1..line.len() - 1]);
            if label.is_empty() {
                label = "V".into();
            }
            out.push(ParsedSection { label, lines: Vec::new() });
            continue;
        }
        if out.is_empty() {
            out.push(ParsedSection { label: "V1".into(), lines: Vec::new() });
        }
        out.last_mut().expect("section exists").lines.push(line.to_string());
    }
    out.retain(|s| !s.lines.is_empty() && s.label != "TITLE");
    // a bare "Verse" label gets the next verse number
    let mut n = 0;
    for s in out.iter_mut() {
        if s.label.starts_with('V') {
            n += 1;
            if s.label == "V" {
                s.label = format!("V{n}");
            }
        }
    }
    out
}

/// Builds aligned sections from the edited lyrics. Lines of the second language
/// are spread across the first language's lines in the section with the same label.
pub fn compile(lyrics: &[Lyric]) -> Result<Vec<SplitSection>> {
    let Some(primary) = lyrics.first() else { return Err(invalid("A song needs lyrics.")) };
    let main = parse_body(&primary.body);
    if main.is_empty() {
        return Err(invalid("A song needs at least one line of lyrics."));
    }
    let others: Vec<(String, Vec<ParsedSection>)> = lyrics[1..].iter().map(|l| (l.lang.clone(), parse_body(&l.body))).collect();
    for (lang, secs) in &others {
        for s in secs {
            if !main.iter().any(|m| m.label == s.label) {
                return Err(invalid(format!("The {lang} lyrics have a [{}] section the first language does not have.", s.label)));
            }
        }
    }
    let mut key = 0i64;
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (i, m) in main.iter().enumerate() {
        if seen.contains(&m.label) {
            return Err(invalid(format!("[{}] appears twice. Use the play order to repeat a section.", m.label)));
        }
        seen.push(m.label.clone());
        let mut segs: Vec<Segment> = Vec::new();
        let mut n = m.lines.len();
        for (_, secs) in &others {
            if let Some(s) = secs.iter().find(|s| s.label == m.label) {
                n = n.max(s.lines.len());
            }
        }
        let spread = |idx: usize, count: usize| key + (idx * n / count.max(1)) as i64;
        for (j, l) in m.lines.iter().enumerate() {
            let (text, chords) = crate::chords::split_line(l);
            segs.push(Segment { lang: primary.lang.clone(), text, align_key: spread(j, m.lines.len()), chords });
        }
        for (lang, secs) in &others {
            if let Some(s) = secs.iter().find(|s| s.label == m.label) {
                for (j, l) in s.lines.iter().enumerate() {
                    let (text, chords) = crate::chords::split_line(l);
                    segs.push(Segment { lang: lang.clone(), text, align_key: spread(j, s.lines.len()), chords });
                }
            }
        }
        key += n as i64;
        out.push(SplitSection { id: i as i64, label: m.label.clone(), segments: segs });
    }
    Ok(out)
}

fn sequence_list(seq: &str) -> Vec<String> {
    seq.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).map(|s| short_label(s)).collect()
}

/// Slides for an unsaved document, for the editor's live preview.
pub fn preview(doc: &SongDoc, lines_per_slide: u8) -> Result<Vec<Slide>> {
    let sections = compile(&doc.lyrics)?;
    let seq = sequence_list(&doc.sequence);
    for l in &seq {
        if !sections.iter().any(|s| &s.label == l) {
            return Err(invalid(format!("The play order uses {l}, but there is no [{l}] section.")));
        }
    }
    let opts = Options {
        lines_per_slide: doc.lines_per_slide.unwrap_or(lines_per_slide),
        lang_mode: if doc.lyrics.len() > 1 { splitter::LangMode::Both } else { splitter::LangMode::Primary },
        primary: doc.lyrics[0].lang.clone(),
        second: doc.lyrics.get(1).map(|l| l.lang.clone()),
        sequence: seq,
        include_title: false,
    };
    Ok(splitter::split(&sections, &opts))
}

pub fn load(conn: &Connection, id: i64) -> Result<SongDoc> {
    #[allow(clippy::type_complexity)]
    let (title, author, copyright, ccli, sequence, lps, key): (String, Option<String>, Option<String>, Option<String>, Option<String>, Option<u8>, Option<String>) = conn.query_row(
        "SELECT title, author, copyright, ccli, default_sequence, lines_per_slide, song_key FROM song WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    )?;
    let mut st = conn.prepare("SELECT h.code, n.number FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE n.song_id = ?1 ORDER BY h.code")?;
    let numbers = st.query_map([id], |r| Ok(Number { hymnal: r.get(0)?, number: r.get(1)? }))?.collect::<std::result::Result<_, _>>()?;
    let mut st = conn.prepare("SELECT lang, body FROM song_lyric WHERE song_id = ?1 ORDER BY id")?;
    let lyrics = st
        .query_map([id], |r| Ok(Lyric { lang: r.get(0)?, body: strip_title(&r.get::<_, String>(1)?) }))?
        .collect::<std::result::Result<_, _>>()?;
    // An imported default_sequence that is just the written order is shown as empty.
    let written: Vec<String> = conn
        .prepare("SELECT label FROM song_section WHERE song_id = ?1 AND label <> 'TITLE' ORDER BY idx")?
        .query_map([id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let seq = sequence.unwrap_or_default();
    let sequence = if sequence_list(&seq) == written { String::new() } else { seq };
    Ok(SongDoc { id: Some(id), title, author, copyright, ccli, numbers, lyrics, sequence, lines_per_slide: lps, key })
}

/// Imported hymns keep their title card ("MHB 1 / — / author") as a [TITLE] block in
/// the stored text. It is not lyrics, so the editor neither shows nor compares it.
pub fn strip_title(body: &str) -> String {
    let mut out = Vec::new();
    let mut skipping = false;
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            skipping = t.eq_ignore_ascii_case("[TITLE]");
        }
        if !skipping {
            out.push(line);
        }
    }
    out.join("\n").trim().to_string()
}

fn clean_opt(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
}

/// Saves a document (insert when id is None) and returns the song id.
pub fn save(conn: &Connection, doc: &SongDoc) -> Result<i64> {
    let title = doc.title.trim();
    if title.is_empty() {
        return Err(invalid("A song needs a title."));
    }
    let mut langs: Vec<&str> = Vec::new();
    for l in &doc.lyrics {
        if langs.contains(&l.lang.as_str()) {
            return Err(invalid(format!("{} lyrics are listed twice.", l.lang)));
        }
        langs.push(&l.lang);
    }
    preview(doc, 2)?; // validates lyrics and play order
    let lyrics: Vec<Lyric> = doc.lyrics.iter().filter(|l| !l.body.trim().is_empty()).cloned().collect();
    let tx = conn.unchecked_transaction()?;
    let lps = doc.lines_per_slide.map(|l| l.clamp(1, 8));
    let seq = sequence_list(&doc.sequence).join(" ");
    let id = match doc.id {
        Some(id) => {
            let n = tx.execute(
                "UPDATE song SET title=?2, author=?3, copyright=?4, ccli=?5, default_sequence=?6, lines_per_slide=?7, song_key=?8, updated_at=datetime('now') WHERE id=?1",
                params![id, title, clean_opt(&doc.author), clean_opt(&doc.copyright), clean_opt(&doc.ccli), seq, lps, clean_opt(&doc.key)],
            )?;
            if n == 0 {
                return Err(invalid("This song was deleted while it was being edited."));
            }
            id
        }
        None => {
            tx.execute(
                "INSERT INTO song (title, author, copyright, ccli, default_sequence, lines_per_slide, song_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![title, clean_opt(&doc.author), clean_opt(&doc.copyright), clean_opt(&doc.ccli), seq, lps, clean_opt(&doc.key)],
            )?;
            tx.last_insert_rowid()
        }
    };

    // hymn numbers
    tx.execute("DELETE FROM song_number WHERE song_id = ?1", [id])?;
    for n in &doc.numbers {
        let code = n.hymnal.trim().to_uppercase();
        let num = n.number.trim();
        if code.is_empty() || num.is_empty() {
            continue;
        }
        tx.execute("INSERT OR IGNORE INTO hymnal (code, name) VALUES (?1, ?1)", [&code])?;
        let hid: i64 = tx.query_row("SELECT id FROM hymnal WHERE code = ?1", [&code], |r| r.get(0))?;
        tx.execute("INSERT OR IGNORE INTO song_number (song_id, hymnal_id, number) VALUES (?1, ?2, ?3)", params![id, hid, num])?;
    }

    // lyrics: rebuild sections only when the text changed
    let stored: Vec<Lyric> = tx
        .prepare("SELECT lang, body FROM song_lyric WHERE song_id = ?1 ORDER BY id")?
        .query_map([id], |r| Ok(Lyric { lang: r.get(0)?, body: strip_title(&r.get::<_, String>(1)?) }))?
        .collect::<std::result::Result<_, _>>()?;
    let lyrics: Vec<Lyric> = lyrics.into_iter().map(|l| Lyric { body: strip_title(&l.body), ..l }).collect();
    if stored != lyrics {
        tx.execute("DELETE FROM song_lyric WHERE song_id = ?1", [id])?;
        for l in &lyrics {
            tx.execute("INSERT INTO song_lyric (song_id, lang, body) VALUES (?1, ?2, ?3)", params![id, l.lang, l.body.trim_end()])?;
        }
        // keep an imported title card, replace everything else
        tx.execute("DELETE FROM song_section WHERE song_id = ?1 AND label <> 'TITLE'", [id])?;
        let offset: i64 = tx.query_row("SELECT count(*) FROM song_section WHERE song_id = ?1", [id], |r| r.get(0))?;
        for (i, sec) in compile(&lyrics)?.iter().enumerate() {
            tx.execute("INSERT INTO song_section (song_id, label, idx) VALUES (?1, ?2, ?3)", params![id, sec.label, offset + i as i64])?;
            let sid = tx.last_insert_rowid();
            let mut per_lang: std::collections::HashMap<&str, i64> = Default::default();
            for g in &sec.segments {
                let idx = per_lang.entry(g.lang.as_str()).or_insert(0);
                tx.execute(
                    "INSERT INTO song_segment (section_id, lang, idx, text, align_key, chords) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![sid, g.lang, *idx, g.text, g.align_key, g.chords],
                )?;
                *idx += 1;
            }
        }
    }
    reindex(&tx, id)?;
    tx.commit()?;
    Ok(id)
}

/// Rewrites a song's search entry from what is stored.
pub fn reindex(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM song_fts WHERE rowid = ?1", [id])?;
    let (title, author): (String, Option<String>) = conn.query_row("SELECT title, author FROM song WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let lyrics: String = conn
        .prepare("SELECT g.text FROM song_segment g JOIN song_section s ON s.id = g.section_id WHERE s.song_id = ?1 AND s.label <> 'TITLE' ORDER BY s.idx, g.align_key")?
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("\n");
    let numbers: String = conn
        .prepare("SELECT h.code, n.number FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE n.song_id = ?1")?
        .query_map([id], |r| Ok(format!("{0} {1} {0}{1}", r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join(" ");
    // indexed folded (ɛ→e, ɔ→o, accents off) because searches are folded the same way,
    // so "hyen" on a plain keyboard finds "hyɛn"
    let fold = crate::text::fold;
    conn.execute(
        "INSERT INTO song_fts (rowid, title, author, lyrics, numbers) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, fold(&title), author.as_deref().map(fold), fold(&lyrics.replace('\n', " ")), numbers],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM song_fts WHERE rowid = ?1", [id])?;
    tx.execute("DELETE FROM song WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::search_songs;

    fn doc() -> SongDoc {
        SongDoc {
            id: None,
            title: "Yesu ne me hyɛn".into(),
            author: Some(" Traditional ".into()),
            copyright: None,
            ccli: None,
            numbers: vec![Number { hymnal: "pan".into(), number: "12".into() }],
            lyrics: vec![
                Lyric { lang: "en".into(), body: "[V1]\nJesus is my ship\nHe carries me home\n\n[Chorus]\nSing, sing, sing".into() },
                Lyric { lang: "tw".into(), body: "[V1]\nYesu ne me hyɛn\nƆde me kɔ fie\n[C]\nTo dwom, to dwom".into() },
            ],
            sequence: "V1 C V1 C".into(),
            lines_per_slide: None,
                key: None,
        }
    }

    #[test]
    fn parses_labels_and_unlabelled_start() {
        let p = parse_body("First line\nSecond\n\n[Chorus]\nRefrain\n[Verse]\nMore");
        assert_eq!(p.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), vec!["V1", "C", "V2"]);
    }

    #[test]
    fn create_search_edit_delete() {
        let c = crate::db::open_memory().unwrap();
        let id = save(&c, &doc()).unwrap();
        assert_eq!(search_songs(&c, "PAN 12", 5).unwrap()[0].id, id);
        assert_eq!(search_songs(&c, "hyen", 5).unwrap()[0].id, id, "accent-folded lyric search");
        let mut d = load(&c, id).unwrap();
        assert_eq!(d.author.as_deref(), Some("Traditional"));
        assert_eq!(d.sequence, "V1 C V1 C");
        assert_eq!(preview(&d, 2).unwrap().len(), 4);

        d.lyrics[0].body = "[V1]\nJesus is my ship\nHe carries me home\nThrough every storm\n[C]\nSing, sing, sing".into();
        d.numbers.push(Number { hymnal: "PH".into(), number: "3".into() });
        save(&c, &d).unwrap();
        assert!(search_songs(&c, "every storm", 5).unwrap().iter().any(|h| h.id == id));
        assert_eq!(search_songs(&c, "ph 3", 5).unwrap()[0].id, id);
        assert!(search_songs(&c, "carries", 5).unwrap().iter().any(|h| h.id == id));

        delete(&c, id).unwrap();
        assert!(search_songs(&c, "every storm", 5).unwrap().is_empty());
    }

    #[test]
    fn unchanged_lyrics_keep_imported_alignment() {
        let c = crate::db::open_memory().unwrap();
        let id = save(&c, &doc()).unwrap();
        // simulate an import-time pairing different from the editor's own
        c.execute("UPDATE song_segment SET align_key = align_key + 100 WHERE lang = 'tw'", []).unwrap();
        let mut d = load(&c, id).unwrap();
        d.title = "Renamed".into();
        save(&c, &d).unwrap();
        let moved: i64 = c.query_row("SELECT count(*) FROM song_segment WHERE align_key >= 100", [], |r| r.get(0)).unwrap();
        assert_eq!(moved, 3, "pairing untouched when only the title changed");
    }

    #[test]
    fn imported_title_card_is_hidden_and_kept() {
        let c = crate::db::open_memory().unwrap();
        let id = save(&c, &doc()).unwrap();
        c.execute("UPDATE song_lyric SET body = '[TITLE]' || char(10) || 'PAN 12' || char(10) || char(10) || body WHERE lang = 'en'", []).unwrap();
        c.execute("INSERT INTO song_section (song_id, label, idx) VALUES (?1, 'TITLE', 99)", [id]).unwrap();
        let mut d = load(&c, id).unwrap();
        assert!(!d.lyrics[0].body.contains("TITLE"));
        let keys_before: i64 = c.query_row("SELECT sum(align_key) FROM song_segment", [], |r| r.get(0)).unwrap();
        d.ccli = Some("123".into());
        save(&c, &d).unwrap();
        let keys_after: i64 = c.query_row("SELECT sum(align_key) FROM song_segment", [], |r| r.get(0)).unwrap();
        assert_eq!(keys_before, keys_after, "title card alone does not count as a lyric change");
        d.lyrics[0].body.push_str("\n[B]\nA new bridge");
        save(&c, &d).unwrap();
        let titles: i64 = c.query_row("SELECT count(*) FROM song_section WHERE song_id = ?1 AND label = 'TITLE'", [id], |r| r.get(0)).unwrap();
        assert_eq!(titles, 1);
    }

    #[test]
    fn helpful_errors() {
        let mut d = doc();
        d.sequence = "V1 B".into();
        assert!(preview(&d, 2).unwrap_err().to_string().contains("no [B] section"));
        let mut d = doc();
        d.lyrics[1].body = "[B]\nOnly in Twi".into();
        assert!(compile(&d.lyrics).unwrap_err().to_string().contains("[B]"));
        let mut d = doc();
        d.title = "  ".into();
        let c = crate::db::open_memory().unwrap();
        assert!(save(&c, &d).is_err());
    }
}
