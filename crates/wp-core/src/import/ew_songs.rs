//! EasyWorship 6/7 song importer (Songs.db + SongWords.db).
//!
//! Verified in Phase 0 against 4,531 songs. Each song's words are EasyWorship RTF:
//! one `{\pard ... \par}` group per line, `\sdslidemarker` starts a new slide, a
//! `\sdparawysiwghidden` paragraph is the hidden section label ("Verse 1", "Chorus"),
//! `\uNNNN?` escapes Unicode and `\cfN` picks a colour. Bilingual hymns colour
//! English white and Fante yellow; we turn the colours into languages and align the
//! two by position within each source slide, so uneven translations stay paired.

use super::*;
use crate::text::clean;
use rusqlite::OpenFlags;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct RtfLine {
    pub text: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RtfSlide {
    pub label: Option<String>,
    pub lines: Vec<RtfLine>,
}

/// Parses EasyWorship song RTF into slides. Unknown control words are dropped.
pub fn parse_rtf(rtf: &str) -> Vec<RtfSlide> {
    let colors = color_table(rtf);
    let mut slides = vec![RtfSlide::default()];
    let mut rest = rtf;
    while let Some(start) = rest.find("{\\pard") {
        let after = &rest[start + 6..];
        let Some(end) = find_group_end(after) else { break };
        let body = &after[..end];
        rest = &after[end..];
        if body.contains("\\sdslidemarker") {
            slides.push(RtfSlide::default());
            continue;
        }
        let hidden = body.contains("\\sdparawysiwghidden");
        let color = cf_index(body).and_then(|i| colors.get(i).cloned().flatten());
        let text = rtf_text(body);
        let cur = slides.last_mut().expect("at least one slide");
        if hidden {
            cur.label = Some(text);
        } else if !text.is_empty() {
            cur.lines.push(RtfLine { text, color });
        }
    }
    slides.into_iter().filter(|s| !s.lines.is_empty() || s.label.is_some()).collect()
}

fn find_group_end(s: &str) -> Option<usize> {
    let mut depth = 1i32;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1, // skip escaped char
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn color_table(rtf: &str) -> Vec<Option<String>> {
    let mut out = vec![None];
    if let Some(i) = rtf.find("{\\colortbl") {
        let tbl = &rtf[i..rtf[i..].find('}').map(|e| i + e).unwrap_or(rtf.len())];
        for entry in tbl.split(';').skip(1) {
            let num = |k: &str| {
                entry.find(k).and_then(|p| entry[p + k.len()..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u8>().ok())
            };
            if let (Some(r), Some(g), Some(b)) = (num("\\red"), num("\\green"), num("\\blue")) {
                out.push(Some(format!("#{r:02x}{g:02x}{b:02x}")));
            }
        }
    }
    out
}

fn cf_index(body: &str) -> Option<usize> {
    let p = body.find("\\cf")?;
    body[p + 3..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

/// Extracts visible text from one paragraph body.
fn rtf_text(body: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut skip_depth: Option<i32> = None;
    let mut depth = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            '{' => {
                depth += 1;
                if b.get(i + 1) == Some(&'\\') && b.get(i + 2) == Some(&'*') && skip_depth.is_none() {
                    skip_depth = Some(depth);
                }
                i += 1;
            }
            '}' => {
                if skip_depth == Some(depth) {
                    skip_depth = None;
                }
                depth -= 1;
                i += 1;
            }
            '\\' => {
                let next = b.get(i + 1).copied().unwrap_or(' ');
                if next == '\'' {
                    let hex: String = b.iter().skip(i + 2).take(2).collect();
                    if skip_depth.is_none() {
                        if let Ok(v) = u8::from_str_radix(&hex, 16) {
                            out.push(cp1252(v));
                        }
                    }
                    i += 4;
                } else if next.is_ascii_alphabetic() {
                    let mut j = i + 1;
                    while j < b.len() && b[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    let word: String = b[i + 1..j].iter().collect();
                    let mut k = j;
                    if k < b.len() && b[k] == '-' {
                        k += 1;
                    }
                    while k < b.len() && b[k].is_ascii_digit() {
                        k += 1;
                    }
                    let num: Option<i32> = b[j..k].iter().collect::<String>().parse().ok();
                    if k < b.len() && b[k] == ' ' {
                        k += 1;
                    }
                    if skip_depth.is_none() && word == "u" {
                        if let Some(n) = num {
                            if let Some(ch) = char::from_u32(n.rem_euclid(65536) as u32) {
                                out.push(ch);
                            }
                            if b.get(k) == Some(&'?') {
                                k += 1;
                            }
                        }
                    } else if skip_depth.is_none() && (word == "line" || word == "tab") {
                        out.push(' ');
                    }
                    i = k;
                } else {
                    if skip_depth.is_none() && matches!(next, '\\' | '{' | '}') {
                        out.push(next);
                    }
                    i += 2;
                }
            }
            '\r' | '\n' => i += 1,
            _ => {
                if skip_depth.is_none() {
                    out.push(c);
                }
                i += 1;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cp1252(v: u8) -> char {
    const HI: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F,
        0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    if (0x80..0xA0).contains(&v) {
        char::from_u32(HI[(v - 0x80) as usize] as u32).unwrap_or('?')
    } else {
        v as char
    }
}

/// Maps an EasyWorship label to the short form used in sequences.
pub fn short_label(label: &str) -> String {
    let l = label.trim().to_lowercase();
    let num: String = l.chars().filter(|c| c.is_ascii_digit()).collect();
    let base = if l.starts_with("verse") {
        "V"
    } else if l.starts_with("chorus") || l.starts_with("refrain") {
        "C"
    } else if l.starts_with("pre") {
        "PC"
    } else if l.starts_with("bridge") {
        "B"
    } else if l.starts_with("tag") {
        "T"
    } else if l.starts_with("ending") {
        "E"
    } else {
        return label.trim().to_string();
    };
    format!("{base}{num}")
}

/// Primary language from the EasyWorship tag, e.g. "NCAN (Fante)" → "fat".
pub fn lang_from_tag(tag: &str) -> &'static str {
    let t = tag.to_lowercase();
    if t.contains("fante") && !t.contains("eng") {
        "fat"
    } else if t.contains("(ga)") || t.contains("ga)") {
        "gaa"
    } else if t.contains("twi") {
        "tw"
    } else if t.contains("ewe") {
        "ee"
    } else {
        "en"
    }
}

/// Colours used for the second language in bilingual hymns.
const SECOND_LANG_COLOURS: &[&str] = &["#ffff00"];

#[derive(Debug, Clone)]
pub struct Section {
    pub label: String,
    /// (lang, text, align_key)
    pub segments: Vec<(String, String, i64)>,
}

/// Turns parsed slides into sections with aligned segments.
/// True when the first slide only repeats the title, number or author (EasyWorship
/// hymnals open with "MHB 1 / — / Charles Wesley"); canticles start with words.
pub fn is_title_card(slide: &RtfSlide, title: &str) -> bool {
    if slide.label.is_some() || slide.lines.is_empty() {
        return false;
    }
    let norm = |t: &str| crate::text::fold(t.split(',').next().unwrap_or(t));
    let t = norm(title);
    slide.lines.iter().any(|l| l.text.trim() == "—") || slide.lines.first().map_or(false, |l| { let f = norm(&l.text); !f.is_empty() && (f == t || t.starts_with(&f)) })
}

pub fn build_sections(slides: &[RtfSlide], title: &str, primary: &str, second: Option<&str>) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut key: i64 = 0;
    let next_verse = |out: &Vec<Section>| format!("V{}", out.iter().filter(|s| s.label.starts_with('V')).count() + 1);
    for (i, s) in slides.iter().enumerate() {
        if i == 0 && is_title_card(s, title) {
            out.push(Section { label: "TITLE".into(), segments: Vec::new() });
        } else if let Some(l) = &s.label {
            let mut label = short_label(l);
            if label == "V" {
                label = next_verse(&out);
            }
            out.push(Section { label, segments: Vec::new() });
        } else if out.is_empty() || out.last().map_or(false, |l| l.label == "TITLE") {
            let label = next_verse(&out);
            out.push(Section { label, segments: Vec::new() });
        }
        let sec = out.last_mut().expect("section exists");
        let (a, b): (Vec<&RtfLine>, Vec<&RtfLine>) = s.lines.iter().partition(|l| {
            second.is_none() || !l.color.as_deref().map_or(false, |c| SECOND_LANG_COLOURS.contains(&c))
        });
        let n = a.len().max(b.len()).max(1);
        for (idx, l) in a.iter().enumerate() {
            let k = key + (idx * n / a.len().max(1)) as i64;
            sec.segments.push((primary.to_string(), l.text.clone(), k));
        }
        if let Some(lang2) = second {
            for (idx, l) in b.iter().enumerate() {
                let k = key + (idx * n / b.len().max(1)) as i64;
                sec.segments.push((lang2.to_string(), l.text.clone(), k));
            }
        }
        key += n as i64;
    }
    out.retain(|s| !s.segments.is_empty());
    out
}

/// Source text kept in `song_lyric` for editing: "[V1]\nline\nline\n\n[C]\n...".
pub fn lyric_body(sections: &[Section], lang: &str) -> String {
    sections
        .iter()
        .filter(|s| s.segments.iter().any(|g| g.0 == lang))
        .map(|s| {
            let lines: Vec<&str> = s.segments.iter().filter(|g| g.0 == lang).map(|g| g.1.as_str()).collect();
            format!("[{}]\n{}", s.label, lines.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Imports Songs.db + SongWords.db from an EasyWorship profile folder (or the two
/// file paths). Songs are grouped into hymnals by their title prefix ("MHB 1").
pub fn import(conn: &Connection, songs_db: &Path, words_db: &Path, collection_name: &str) -> Result<Report> {
    import_filtered(conn, songs_db, words_db, collection_name, None)
}

/// As `import`, leaving out songs whose title (case-insensitive) is in `skip`;
/// used when the operator chose not to bring in duplicates at the review step.
pub fn import_filtered(conn: &Connection, songs_db: &Path, words_db: &Path, collection_name: &str, skip: Option<&std::collections::HashSet<String>>) -> Result<Report> {
    let s = Connection::open_with_flags(songs_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    s.create_collation("UTF8_U_CI", |a: &str, b: &str| a.to_lowercase().cmp(&b.to_lowercase()))?;
    let w = Connection::open_with_flags(words_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut words: HashMap<i64, String> = HashMap::new();
    {
        let mut q = w.prepare("SELECT song_id, words FROM word")?;
        let rows = q.query_map([], |r| {
            let v: rusqlite::types::Value = r.get(1)?;
            let text = match v {
                rusqlite::types::Value::Text(t) => t,
                rusqlite::types::Value::Blob(b) => String::from_utf8_lossy(&b).into_owned(),
                _ => String::new(),
            };
            Ok((r.get::<_, i64>(0)?, text))
        })?;
        for r in rows {
            let (id, t) = r?;
            words.insert(id, t);
        }
    }

    let tx = conn.unchecked_transaction()?;
    let cid = create_collection(&tx, &Collection {
        kind: "songs",
        name: collection_name,
        source_format: "ew-songs",
        source_file: &songs_db.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
        publisher: None,
        licence_note: None,
        transferable: true,
    })?;
    let mut rep = Report { collection_id: cid, name: collection_name.to_string(), ..Default::default() };
    let mut hymnals: HashMap<String, i64> = HashMap::new();
    let mut q = s.prepare("SELECT rowid, title, author, copyright, reference_number, tags, song_item_uid FROM song ORDER BY rowid")?;
    let rows = q.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
        ))
    })?;
    for row in rows {
        let (sid, title, author, copyright, refno, tags, uid) = row?;
        rep.read += 1;
        if skip.map_or(false, |k| k.contains(&title.trim().to_lowercase())) {
            log(&tx, cid, "skipped", &title, "already in the library (left out at review)")?;
            rep.skipped += 1;
            continue;
        }
        let Some(rtf) = words.get(&sid) else {
            log(&tx, cid, "skipped", &title, "no words record")?;
            rep.skipped += 1;
            continue;
        };
        let mut slides = parse_rtf(rtf);
        let mut all_fixes: Vec<crate::text::Fix> = Vec::new();
        for sl in slides.iter_mut() {
            for line in sl.lines.iter_mut() {
                let (t, f) = clean(&line.text);
                line.text = t;
                for x in f {
                    match all_fixes.iter_mut().find(|a| a.from == x.from) {
                        Some(a) => a.count += x.count,
                        None => all_fixes.push(x),
                    }
                }
            }
        }
        rep.fixed_chars += log_fixes(&tx, cid, &title, &all_fixes)?;
        let tag = tags.clone().unwrap_or_default();
        let bilingual = tag.to_lowercase().contains("eng&fante") || tag.contains('+');
        let primary = if bilingual { "en" } else { lang_from_tag(&tag) };
        let second = bilingual.then_some("fat");
        let sections = build_sections(&slides, &title, primary, second);
        if sections.iter().all(|s| s.label == "TITLE") {
            log(&tx, cid, "skipped", &title, "no lyrics found in the words record")?;
            rep.skipped += 1;
            continue;
        }
        tx.execute(
            "INSERT INTO song (collection_id, title, author, copyright, source_ref, default_sequence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                cid,
                title.trim(),
                author.filter(|a| !a.trim().is_empty()),
                copyright.filter(|c| !c.trim().is_empty()),
                uid.unwrap_or_else(|| sid.to_string()),
                sections.iter().filter(|s| s.label != "TITLE").map(|s| s.label.as_str()).collect::<Vec<_>>().join(" "),
            ],
        )?;
        let song_id = tx.last_insert_rowid();
        // hymnal from the title prefix: "MHB 1", "NCAN 12", "MHB 1, ENG&FANTE"
        let mut numbers = Vec::new();
        let mut parts = title.split_whitespace();
        if let (Some(code), Some(num)) = (parts.next(), parts.next()) {
            let num = num.trim_end_matches(',');
            if code.chars().all(|c| c.is_ascii_uppercase()) && num.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                let hid = match hymnals.get(code) {
                    Some(h) => *h,
                    None => {
                        tx.execute("INSERT OR IGNORE INTO hymnal (code, name) VALUES (?1, ?2)", params![code, tag])?;
                        let h: i64 = tx.query_row("SELECT id FROM hymnal WHERE code = ?1", [code], |r| r.get(0))?;
                        hymnals.insert(code.to_string(), h);
                        h
                    }
                };
                tx.execute("INSERT OR IGNORE INTO song_number (song_id, hymnal_id, number) VALUES (?1, ?2, ?3)", params![song_id, hid, num])?;
                numbers.push(format!("{code} {num} {code}{num}"));
                if refno.as_deref().map_or(false, |r| r != num) {
                    log(&tx, cid, "info", &title, &format!("reference number {:?} differs from title number {num}; title number used", refno))?;
                }
            }
        }
        for (si, sec) in sections.iter().enumerate() {
            tx.execute("INSERT INTO song_section (song_id, label, idx) VALUES (?1, ?2, ?3)", params![song_id, sec.label, si as i64])?;
            let sec_id = tx.last_insert_rowid();
            let mut per_lang: HashMap<&str, i64> = HashMap::new();
            for (lang, text, key) in &sec.segments {
                let idx = per_lang.entry(lang.as_str()).or_insert(0);
                tx.execute(
                    "INSERT INTO song_segment (section_id, lang, idx, text, align_key) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![sec_id, lang, *idx, text, key],
                )?;
                *idx += 1;
            }
        }
        for lang in std::iter::once(primary).chain(second) {
            let body = lyric_body(&sections, lang);
            if !body.is_empty() {
                tx.execute("INSERT INTO song_lyric (song_id, lang, body) VALUES (?1, ?2, ?3)", params![song_id, lang, body])?;
            }
        }
        crate::songs::reindex(&tx, song_id)?;
        rep.imported += 1;
    }
    log(&tx, cid, "info", collection_name, &format!("read {} songs, imported {}, skipped {}", rep.read, rep.imported, rep.skipped))?;
    tx.commit()?;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r"{\rtf1\ansi\deff0\sdeasyworship2
{\fonttbl{\f0 Tahoma;}}
{\colortbl ;\red255\green255\blue255;\red255\green255\blue0;}
{\pard\qc\plain\fs64{\*\sdfsreal 32.4}\sdfsauto MHB 1\par}
{\pard\sdslidemarker\qc\plain\sdfsauto\par}
{\pard\sdparawysiwghidden\qc\plain\sdfsauto Verse 1\par}
{\pard\qc\plain\cf1\sdfsauto O For a thousand tongues to sing\par}
{\pard\qc\plain\cf1\sdfsauto My great Redeemer\u8217?s praise,\par}
{\pard\qc\plain\cf2\sdfsauto Meenya ngyiram\u225? apem matow ndwom\par}
{\pard\sdslidemarker\qc\plain\sdfsauto\par}
{\pard\qc\plain\cf1\sdfsauto The glories of my God and King,\par}
{\pard\qc\plain\cf2\sdfsauto Makamfo me Hen N\u8217?enyimnyam,\par}
{\pard\qc\plain\cf2\sdfsauto N\u8217?adom no konyimdzi.\par}
}";

    #[test]
    fn parses_slides_labels_unicode_and_colours() {
        let s = parse_rtf(SAMPLE);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].lines[0].text, "MHB 1");
        assert_eq!(s[1].label.as_deref(), Some("Verse 1"));
        assert_eq!(s[1].lines[1].text, "My great Redeemer’s praise,");
        assert_eq!(s[1].lines[2].color.as_deref(), Some("#ffff00"));
    }

    #[test]
    fn aligns_uneven_bilingual_lines() {
        let s = parse_rtf(SAMPLE);
        let secs = build_sections(&s, "MHB 1, ENG&FANTE", "en", Some("fat"));
        assert_eq!(secs[0].label, "TITLE");
        let v1 = &secs[1];
        assert_eq!(v1.label, "V1");
        // slide 1: 2 en lines, 1 fat line → the fat line pairs with the first en line
        let keys = |lang: &str| v1.segments.iter().filter(|g| g.0 == lang).map(|g| g.2).collect::<Vec<_>>();
        assert_eq!(keys("en"), vec![1, 2, 3]);
        assert_eq!(keys("fat"), vec![1, 3, 4]);
        assert!(lyric_body(&secs, "fat").starts_with("[V1]\nMeenya ngyiramá"));
    }

    #[test]
    fn canticle_without_title_card_keeps_first_slide() {
        let slides = vec![
            RtfSlide { label: None, lines: vec![RtfLine { text: "O COME, let us sing".into(), color: None }] },
            RtfSlide { label: None, lines: vec![RtfLine { text: "Let us come before His presence".into(), color: None }] },
        ];
        let secs = build_sections(&slides, "Canticle 1 - VENITE", "en", None);
        assert_eq!(secs.len(), 1);
        assert_eq!(secs[0].label, "V1");
        assert_eq!(secs[0].segments.len(), 2);
    }

    #[test]
    fn labels() {
        assert_eq!(short_label("Verse 12"), "V12");
        assert_eq!(short_label("Chorus Fante Only"), "C");
        assert_eq!(short_label("Bridge"), "B");
    }
}
