//! Songs from files other programs use:
//! OpenLyrics (.xml, OpenLP and others), OpenSong (often no extension),
//! ProPresenter 6 (.pro6), ProPresenter 7 (.pro, best effort), CCLI SongSelect
//! (.txt and .usr), ChordPro (.cho, .chordpro) and plain text (.txt).
//!
//! Every file becomes a `SongDoc` and is saved through `songs::save`, so labels,
//! bilingual pairing, search and hymn numbers work as for songs typed in by hand.
//! The format is told by content where the extension is ambiguous.

use super::review::{Line, Review};
use super::{create_collection, Collection, Report};
use crate::songs::{Lyric, Number, SongDoc};
use crate::{text, Error, Result};
use base64::Engine;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Song file".into(), "song file", msg.into())
}

fn doc(title: &str) -> SongDoc {
    SongDoc { id: None, title: title.trim().to_string(), author: None, copyright: None, ccli: None, numbers: vec![], lyrics: vec![], sequence: String::new(), lines_per_slide: None, key: None }
}

/// Our labels: "v1" → V1, "Verse 2" → V2, "chorus" → C, "pre-chorus" → PC …
fn label(raw: &str) -> String {
    let r = raw.trim();
    let lower = r.to_lowercase();
    let num: String = lower.chars().filter(|c| c.is_ascii_digit()).collect();
    let word = lower.trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace());
    let base = match word {
        "v" | "verse" | "stanza" => "V",
        "c" | "chorus" | "refrain" | "r" => "C",
        "b" | "bridge" => "B",
        "p" | "pc" | "pre-chorus" | "prechorus" | "pre chorus" => "PC",
        "t" | "tag" => "T",
        "e" | "ending" | "end" => "E",
        "i" | "intro" => "I",
        "o" | "outro" => "O",
        _ => return crate::import::ew_songs::short_label(r),
    };
    format!("{base}{num}")
}

fn body_of(sections: &[(String, Vec<String>)]) -> String {
    sections.iter().filter(|(_, l)| !l.is_empty()).map(|(lab, lines)| format!("[{lab}]\n{}", lines.join("\n"))).collect::<Vec<_>>().join("\n\n")
}

fn title_from_path(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().replace(['_'], " ")).unwrap_or_else(|| "Untitled".into())
}

// ---------- OpenLyrics ----------

fn ol_lines(node: roxmltree::Node) -> Vec<String> {
    // <lines> text with <br/> breaks; <chord/> and <comment> are left out
    let mut out = vec![String::new()];
    fn walk(n: roxmltree::Node, out: &mut Vec<String>) {
        for c in n.children() {
            match c.tag_name().name() {
                "br" => out.push(String::new()),
                "comment" => {}
                "chord" => {
                    if let Some(name) = c.attribute("name").or_else(|| c.attribute("root")) {
                        out.last_mut().unwrap().push_str(&format!("[{name}]"));
                    }
                    walk(c, out)
                }
                "" if c.is_text() => out.last_mut().unwrap().push_str(c.text().unwrap_or("")),
                _ => walk(c, out),
            }
        }
    }
    walk(node, &mut out);
    out.into_iter().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect()
}

pub fn parse_openlyrics(xml: &str) -> Result<SongDoc> {
    let d = roxmltree::Document::parse(xml).map_err(|e| bad(format!("not valid OpenLyrics ({e})")))?;
    let root = d.root_element();
    let find = |name: &str| root.descendants().find(|n| n.tag_name().name() == name);
    let mut s = doc(find("title").and_then(|n| n.text()).unwrap_or(""));
    let authors: Vec<String> = root.descendants().filter(|n| n.tag_name().name() == "author").filter_map(|n| n.text().map(str::trim).map(str::to_string)).collect();
    if !authors.is_empty() {
        s.author = Some(authors.join(", "));
    }
    s.copyright = find("copyright").and_then(|n| n.text()).map(|t| t.trim().to_string());
    s.ccli = find("ccliNo").and_then(|n| n.text()).map(|t| t.trim().to_string());
    s.key = find("key").and_then(|n| n.text()).map(|t| t.trim().to_string()).filter(|k| !k.is_empty());
    for b in root.descendants().filter(|n| n.tag_name().name() == "songbook") {
        if let (Some(name), Some(entry)) = (b.attribute("name"), b.attribute("entry")) {
            s.numbers.push(Number { hymnal: name.trim().to_uppercase(), number: entry.trim().to_string() });
        }
    }
    s.sequence = find("verseOrder").and_then(|n| n.text()).map(|t| t.split_whitespace().map(label).collect::<Vec<_>>().join(" ")).unwrap_or_default();
    // one set of sections per language (translations are separate <verse> with lang)
    let mut langs: Vec<(String, Vec<(String, Vec<String>)>)> = vec![];
    for v in root.descendants().filter(|n| n.tag_name().name() == "verse") {
        let lang = v.attribute("lang").unwrap_or("en").split('-').next().unwrap_or("en").to_lowercase();
        let lang = if lang == "fa" || lang == "fat" { "fat".to_string() } else if lang == "ak" || lang == "tw" { "tw".into() } else { lang };
        let name = label(v.attribute("name").unwrap_or("v"));
        let lines: Vec<String> = v.children().filter(|n| n.tag_name().name() == "lines").flat_map(ol_lines).collect();
        let entry = match langs.iter_mut().find(|(l, _)| *l == lang) {
            Some(e) => e,
            None => {
                langs.push((lang.clone(), vec![]));
                langs.last_mut().unwrap()
            }
        };
        entry.1.push((name, lines));
    }
    if langs.is_empty() {
        return Err(bad("the song has no verses"));
    }
    s.lyrics = langs.into_iter().take(2).map(|(lang, secs)| Lyric { lang, body: body_of(&secs) }).collect();
    Ok(s)
}

// ---------- OpenSong ----------

pub fn parse_opensong(xml: &str) -> Result<SongDoc> {
    let d = roxmltree::Document::parse(xml).map_err(|e| bad(format!("not valid OpenSong ({e})")))?;
    let root = d.root_element();
    if root.tag_name().name() != "song" {
        return Err(bad("not an OpenSong song"));
    }
    let get = |n: &str| root.children().find(|c| c.tag_name().name() == n).and_then(|c| c.text()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let mut s = doc(&get("title").unwrap_or_default());
    s.author = get("author");
    s.copyright = get("copyright");
    s.ccli = get("ccli");
    if let (Some(book), Some(num)) = (get("hymn_number").and(None::<String>).or(None), get("hymn_number")) {
        let _ = book;
        s.numbers.push(Number { hymnal: "HYMN".into(), number: num });
    }
    s.sequence = get("presentation").map(|p| p.split_whitespace().map(label).collect::<Vec<_>>().join(" ")).unwrap_or_default();
    let lyrics = get("lyrics").ok_or_else(|| bad("the song has no lyrics"))?;
    s.key = get("key");
    let mut pending_chords: Option<String> = None;
    let mut secs: Vec<(String, Vec<String>)> = vec![];
    for raw in lyrics.lines() {
        let line = raw.trim_end();
        if line.starts_with('[') && line.ends_with(']') {
            secs.push((label(&line[1..line.len() - 1]), vec![]));
            continue;
        }
        // '.' chord lines (kept for the line below), ';' comments, '-' and '|' breaks
        if let Some(ch) = line.strip_prefix('.') {
            pending_chords = Some(ch.to_string());
            continue;
        }
        if line.starts_with(';') || line.trim() == "---" || line.trim() == "-!!" {
            continue;
        }
        // the lyric line starts one column later than its chord line ('.' vs ' ')
        let lyric = line.strip_prefix(' ').unwrap_or(line);
        let line_with_chords = match pending_chords.take() {
            Some(ch) => merge_chord_line(&ch, lyric),
            None => lyric.to_string(),
        };
        let text = line_with_chords.trim_start_matches(|c: char| c.is_ascii_digit()).replace("||", "").replace('|', " ").replace('_', "");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            continue;
        }
        if secs.is_empty() {
            secs.push(("V1".into(), vec![]));
        }
        secs.last_mut().unwrap().1.push(text);
    }
    s.lyrics = vec![Lyric { lang: "en".into(), body: body_of(&secs) }];
    Ok(s)
}

/// "G       C" over "Amazing grace" → "[G]Amazing [C]grace" (chords placed by column).
fn merge_chord_line(chords: &str, lyric: &str) -> String {
    let lyric: Vec<char> = lyric.chars().collect();
    let mut marks: Vec<(usize, String)> = vec![];
    let mut col = 0;
    let cs: Vec<char> = chords.chars().collect();
    while col < cs.len() {
        if cs[col].is_whitespace() {
            col += 1;
            continue;
        }
        let start = col;
        while col < cs.len() && !cs[col].is_whitespace() {
            col += 1;
        }
        let name: String = cs[start..col].iter().collect();
        if crate::chords::is_chord(&name) {
            marks.push((start, name));
        }
    }
    let mut out = String::new();
    let mut m = marks.into_iter().peekable();
    for (i, ch) in lyric.iter().enumerate() {
        while m.peek().map_or(false, |(c, _)| *c <= i) {
            out.push_str(&format!("[{}]", m.next().unwrap().1));
        }
        out.push(*ch);
    }
    for (_, name) in m {
        out.push_str(&format!(" [{name}]"));
    }
    out
}

// ---------- ProPresenter ----------

/// General RTF to text lines (ProPresenter's slide text): paragraphs and line
/// breaks become lines; \uN and \'hh escapes become characters (ɛ, ɔ, accents);
/// font, colour and style tables are skipped.
pub fn rtf_text(rtf: &str) -> Vec<String> {
    let b: Vec<char> = rtf.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut skip_depth: Vec<bool> = vec![false];
    let mut uc_skip = 0usize; // characters to drop after a \uN
    let cp1252 = |byte: u8| -> char {
        const HIGH: [u16; 32] = [0x20AC, 0x81, 0x201A, 0x192, 0x201E, 0x2026, 0x2020, 0x2021, 0x2C6, 0x2030, 0x160, 0x2039, 0x152, 0x8D, 0x17D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x2DC, 0x2122, 0x161, 0x203A, 0x153, 0x9D, 0x17E, 0x178];
        if (0x80..0xA0).contains(&byte) { char::from_u32(HIGH[(byte - 0x80) as usize] as u32).unwrap_or('?') } else { byte as char }
    };
    while i < b.len() {
        let skipping = *skip_depth.last().unwrap_or(&false);
        match b[i] {
            '{' => {
                skip_depth.push(skipping);
                i += 1;
            }
            '}' => {
                if skip_depth.len() > 1 {
                    skip_depth.pop();
                }
                i += 1;
            }
            '\\' => {
                i += 1;
                let Some(&c) = b.get(i) else { break };
                if c == '\\' || c == '{' || c == '}' {
                    if !skipping { out.push(c); }
                    i += 1;
                    continue;
                }
                if c == '\'' {
                    let hex: String = b.get(i + 1..i + 3).map(|s| s.iter().collect()).unwrap_or_default();
                    i += 3;
                    if uc_skip > 0 { uc_skip -= 1; continue; }
                    if let (false, Ok(v)) = (skipping, u8::from_str_radix(&hex, 16)) { out.push(cp1252(v)); }
                    continue;
                }
                if c == '*' {
                    *skip_depth.last_mut().unwrap() = true;
                    i += 1;
                    continue;
                }
                let start = i;
                while i < b.len() && b[i].is_ascii_alphabetic() { i += 1; }
                let word: String = b[start..i].iter().collect();
                let ns = i;
                if i < b.len() && (b[i] == '-' || b[i].is_ascii_digit()) {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() { i += 1; }
                }
                let num: Option<i32> = b[ns..i].iter().collect::<String>().parse().ok();
                if i < b.len() && b[i] == ' ' { i += 1; }
                match word.as_str() {
                    "fonttbl" | "colortbl" | "stylesheet" | "info" | "expandedcolortbl" | "listtable" | "listoverridetable" | "pict" => *skip_depth.last_mut().unwrap() = true,
                    "par" | "line" if !skipping => out.push('\n'),
                    "tab" if !skipping => out.push(' '),
                    "u" if !skipping => {
                        if let Some(n) = num {
                            let code = if n < 0 { n + 65536 } else { n } as u32;
                            if let Some(ch) = char::from_u32(code) { out.push(ch); }
                            uc_skip = 1;
                        }
                    }
                    _ => {}
                }
            }
            '\r' | '\n' => i += 1,
            c => {
                if uc_skip > 0 { uc_skip -= 1; } else if !skipping { out.push(c); }
                i += 1;
            }
        }
    }
    out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect()
}

pub fn parse_pro6(xml: &str) -> Result<SongDoc> {
    let d = roxmltree::Document::parse(xml).map_err(|e| bad(format!("not a ProPresenter 6 file ({e})")))?;
    let root = d.root_element();
    let attr = |n: &str| root.attribute(n).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let mut s = doc(&attr("CCLISongTitle").unwrap_or_default());
    s.author = attr("CCLIAuthor");
    s.copyright = match (attr("CCLICopyrightYear"), attr("CCLIPublisher")) {
        (Some(y), Some(p)) => Some(format!("© {y} {p}")),
        (None, Some(p)) => Some(p),
        (Some(y), None) => Some(format!("© {y}")),
        _ => None,
    };
    s.ccli = attr("CCLISongNumber");
    let mut secs: Vec<(String, Vec<String>)> = vec![];
    for g in root.descendants().filter(|n| n.tag_name().name() == "RVSlideGrouping") {
        let name = label(g.attribute("name").unwrap_or("Verse"));
        let mut lines = vec![];
        for el in g.descendants().filter(|n| n.tag_name().name() == "NSString" && n.attribute("rvXMLIvarName") == Some("RTFData")) {
            if let Some(b) = el.text().and_then(|t| base64::engine::general_purpose::STANDARD.decode(t.trim()).ok()) {
                lines.extend(rtf_text(&String::from_utf8_lossy(&b)));
            }
        }
        // the same group name twice (two "Verse" groups) gets numbered
        let name = if secs.iter().any(|(l, _)| *l == name) { format!("{name}{}", secs.len() + 1) } else { name };
        secs.push((name, lines));
    }
    if secs.iter().all(|(_, l)| l.is_empty()) {
        return Err(bad("no slide text found"));
    }
    s.lyrics = vec![Lyric { lang: "en".into(), body: body_of(&secs) }];
    Ok(s)
}

/// Minimal protobuf reader: every length-delimited field, recursively, in order.
fn proto_strings(buf: &[u8], depth: usize, out: &mut Vec<(usize, u64, Vec<u8>)>) -> bool {
    let mut i = 0;
    let varint = |i: &mut usize| -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *buf.get(*i)?;
            *i += 1;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    };
    while i < buf.len() {
        let Some(key) = varint(&mut i) else { return false };
        let (field, wt) = (key >> 3, key & 7);
        match wt {
            0 => {
                if varint(&mut i).is_none() {
                    return false;
                }
            }
            1 => i += 8,
            5 => i += 4,
            2 => {
                let Some(len) = varint(&mut i) else { return false };
                let end = i + len as usize;
                if end > buf.len() {
                    return false;
                }
                let part = &buf[i..end];
                out.push((depth, field, part.to_vec()));
                if depth < 24 {
                    let mut inner = vec![];
                    if proto_strings(part, depth + 1, &mut inner) {
                        out.extend(inner);
                    }
                }
                i = end;
            }
            _ => return false,
        }
    }
    i == buf.len()
}

/// ProPresenter 7 (.pro): best effort without Renewed Vision's schema. Slide text
/// is found as the RTF stored in each slide; the title is the presentation name.
pub fn parse_pro7(bytes: &[u8], fallback_title: &str) -> Result<SongDoc> {
    let mut parts = vec![];
    if !proto_strings(bytes, 0, &mut parts) {
        return Err(bad("not a ProPresenter 7 file"));
    }
    let mut s = doc(fallback_title);
    // presentation name: field 3 at the top level
    if let Some((_, _, name)) = parts.iter().find(|(d, f, b)| *d == 0 && *f == 3 && std::str::from_utf8(b).is_ok()) {
        let n = String::from_utf8_lossy(name).trim().to_string();
        if !n.is_empty() {
            s.title = n;
        }
    }
    let mut secs: Vec<(String, Vec<String>)> = vec![];
    let mut seen = HashSet::new();
    for (_, _, b) in &parts {
        if b.starts_with(b"{\\rtf") && seen.insert(b.clone()) {
            let lines = rtf_text(&String::from_utf8_lossy(b));
            if !lines.is_empty() {
                secs.push((format!("V{}", secs.len() + 1), lines));
            }
        }
    }
    if secs.is_empty() {
        return Err(bad("no slide text found in this ProPresenter 7 file"));
    }
    s.lyrics = vec![Lyric { lang: "en".into(), body: body_of(&secs) }];
    Ok(s)
}

// ---------- SongSelect, ChordPro, plain text ----------

fn is_header(l: &str) -> bool {
    let w = l.trim().trim_end_matches(':').to_lowercase();
    let base = w.trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace());
    ["verse", "chorus", "bridge", "pre-chorus", "prechorus", "tag", "ending", "intro", "outro", "refrain", "interlude", "misc", "stanza"].contains(&base)
}

/// CCLI SongSelect lyrics (.txt): title, sections with headers, then the CCLI footer.
pub fn parse_songselect_txt(t: &str) -> Result<SongDoc> {
    let lines: Vec<&str> = t.lines().map(|l| l.trim_end()).collect();
    let first = lines.iter().position(|l| !l.trim().is_empty()).ok_or_else(|| bad("the file is empty"))?;
    let mut s = doc(lines[first]);
    let footer = lines.iter().position(|l| l.starts_with("CCLI Song #") || l.starts_with("CCLI Song Number")).unwrap_or(lines.len());
    let mut secs: Vec<(String, Vec<String>)> = vec![];
    for l in &lines[first + 1..footer] {
        if l.trim().is_empty() {
            continue;
        }
        if is_header(l) {
            secs.push((label(l.trim().trim_end_matches(':')), vec![]));
        } else {
            if secs.is_empty() {
                secs.push(("V1".into(), vec![]));
            }
            secs.last_mut().unwrap().1.push(l.trim().to_string());
        }
    }
    for l in &lines[footer.min(lines.len())..] {
        let l = l.trim();
        if let Some(n) = l.strip_prefix("CCLI Song #").or_else(|| l.strip_prefix("CCLI Song Number")) {
            s.ccli = Some(n.trim_matches(|c: char| !c.is_ascii_digit()).to_string());
        } else if l.starts_with('©') || l.to_lowercase().starts_with("copyright") {
            s.copyright = Some(l.to_string());
        } else if !l.is_empty() && !l.starts_with("For use solely") && !l.starts_with("CCLI License") && !l.starts_with("Note:") && s.author.is_none() {
            s.author = Some(l.replace(" | ", ", "));
        }
    }
    if secs.is_empty() {
        return Err(bad("no verses found"));
    }
    s.lyrics = vec![Lyric { lang: "en".into(), body: body_of(&secs) }];
    Ok(s)
}

/// Older SongSelect format (.usr): key=value lines, sections separated by /t, lines by /n.
pub fn parse_usr(t: &str) -> Result<SongDoc> {
    let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let mut s = doc(&get("Title").ok_or_else(|| bad("not a SongSelect .usr file"))?);
    s.author = get("Author").map(|a| a.replace(" | ", ", "));
    s.copyright = get("Copyright");
    s.ccli = t.lines().find_map(|l| l.strip_prefix("[S A")).map(|n| n.trim_end_matches(']').to_string());
    let fields: Vec<String> = get("Fields").map(|f| f.split("/t").map(str::to_string).collect()).unwrap_or_default();
    let words: Vec<String> = get("Words").map(|w| w.split("/t").map(str::to_string).collect()).unwrap_or_default();
    let secs: Vec<(String, Vec<String>)> = words
        .iter()
        .enumerate()
        .map(|(i, w)| (label(fields.get(i).map(String::as_str).unwrap_or("Verse")), w.split("/n").map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()))
        .collect();
    let mut fixed: Vec<(String, Vec<String>)> = vec![];
    for (l, lines) in secs {
        let l = if fixed.iter().any(|(x, _)| *x == l) { format!("{l}{}", fixed.len() + 1) } else { l };
        fixed.push((l, lines));
    }
    s.lyrics = vec![Lyric { lang: "en".into(), body: body_of(&fixed) }];
    Ok(s)
}

/// ChordPro: {title:}, {artist:}, {start_of_chorus}, {comment: Verse 2}; chords in [ ] removed.
pub fn parse_chordpro(t: &str, fallback_title: &str) -> Result<SongDoc> {
    let mut s = doc(fallback_title);
    let mut secs: Vec<(String, Vec<String>)> = vec![];
    let mut verse_no = 0;
    let mut new_section = true;
    for raw in t.lines() {
        let l = raw.trim();
        if l.starts_with('#') {
            continue;
        }
        if l.starts_with('{') && l.ends_with('}') {
            let inner = &l[1..l.len() - 1];
            let (k, v) = inner.split_once(':').map(|(k, v)| (k.trim().to_lowercase(), v.trim())).unwrap_or((inner.trim().to_lowercase(), ""));
            match k.as_str() {
                "title" | "t" => s.title = v.to_string(),
                "artist" | "composer" | "lyricist" => s.author = Some(v.to_string()),
                "copyright" => s.copyright = Some(v.to_string()),
                "ccli" => s.ccli = Some(v.to_string()),
                "key" => s.key = Some(v.to_string()).filter(|k| !k.is_empty()),
                "start_of_chorus" | "soc" => {
                    secs.push(("C".into(), vec![]));
                    new_section = false;
                }
                "start_of_bridge" | "sob" => {
                    secs.push(("B".into(), vec![]));
                    new_section = false;
                }
                "start_of_verse" | "sov" => {
                    verse_no += 1;
                    secs.push((format!("V{verse_no}"), vec![]));
                    new_section = false;
                }
                "end_of_chorus" | "eoc" | "end_of_bridge" | "eob" | "end_of_verse" | "eov" => new_section = true,
                "comment" | "c" | "ci" if is_header(v) => {
                    let lab = label(v);
                    if lab.starts_with('V') {
                        verse_no += 1;
                    }
                    secs.push((lab, vec![]));
                    new_section = false;
                }
                _ => {}
            }
            continue;
        }
        if l.is_empty() {
            new_section = true;
            continue;
        }
        // chords stay inline ("[G]Amazing"); they reach the musicians, not the screens
        let text = l.split_whitespace().collect::<Vec<_>>().join(" ");
        if crate::chords::split_line(&text).0.is_empty() {
            continue;
        }
        if new_section || secs.is_empty() {
            verse_no += 1;
            secs.push((format!("V{verse_no}"), vec![]));
            new_section = false;
        }
        secs.last_mut().unwrap().1.push(text);
    }
    let secs = dedupe(secs);
    if secs.iter().all(|(_, l)| l.is_empty()) {
        return Err(bad("no lyrics found"));
    }
    s.lyrics = vec![Lyric { lang: "en".into(), body: body_of(&secs) }];
    Ok(s)
}

/// Repeated chorus blocks (written out each time) keep the first; labels stay unique.
fn dedupe(secs: Vec<(String, Vec<String>)>) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = vec![];
    for (l, lines) in secs {
        if lines.is_empty() {
            continue;
        }
        if let Some(prev) = out.iter().find(|(x, _)| *x == l) {
            if prev.1 == lines {
                continue;
            }
            let n = out.iter().filter(|(x, _)| x.starts_with(&l)).count() + 1;
            out.push((format!("{l}{n}"), lines));
        } else {
            out.push((l, lines));
        }
    }
    out
}

/// Plain text: verses separated by blank lines; section headers are understood.
pub fn parse_plain(t: &str, fallback_title: &str) -> Result<SongDoc> {
    let mut s = doc(fallback_title);
    let mut secs: Vec<(String, Vec<String>)> = vec![];
    let mut n = 0;
    let mut fresh = true;
    for l in t.lines().map(str::trim) {
        if l.is_empty() {
            fresh = true;
            continue;
        }
        if is_header(l) {
            secs.push((label(l.trim_end_matches(':')), vec![]));
            fresh = false;
            continue;
        }
        if fresh {
            n += 1;
            secs.push((format!("V{n}"), vec![]));
            fresh = false;
        }
        secs.last_mut().unwrap().1.push(l.to_string());
    }
    let secs = dedupe(secs);
    if secs.is_empty() {
        return Err(bad("the file is empty"));
    }
    s.lyrics = vec![Lyric { lang: "en".into(), body: body_of(&secs) }];
    Ok(s)
}

// ---------- detection, review, import ----------

pub const EXTENSIONS: &[&str] = &["xml", "pro6", "pro", "txt", "usr", "cho", "chopro", "chordpro", "crd", ""];

/// Reads one file in whichever of the formats it is, with the format's name.
pub fn parse_file(path: &Path) -> Result<(SongDoc, &'static str)> {
    let bytes = std::fs::read(path)?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let title = title_from_path(path);
    let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes)).into_owned();
    let head = text.trim_start();
    let r = if ext == "pro" && !head.starts_with('{') && !head.starts_with('<') {
        parse_pro7(&bytes, &title).map(|d| (d, "ProPresenter 7"))
    } else if ext == "pro6" || head.contains("<RVPresentationDocument") {
        parse_pro6(&text).map(|d| (d, "ProPresenter 6"))
    } else if head.starts_with('<') && head.contains("openlyrics.info") {
        parse_openlyrics(&text).map(|d| (d, "OpenLyrics"))
    } else if head.starts_with('<') && (head.contains("<song>") || head.contains("<song ")) {
        parse_opensong(&text).map(|d| (d, "OpenSong"))
    } else if ext == "usr" || head.starts_with("[S A") {
        parse_usr(&text).map(|d| (d, "SongSelect (.usr)"))
    } else if ["cho", "chopro", "chordpro", "crd", "pro"].contains(&ext.as_str()) || head.starts_with("{t") || head.contains("{title") {
        parse_chordpro(&text, &title).map(|d| (d, "ChordPro"))
    } else if text.contains("CCLI Song #") || text.contains("CCLI Song Number") {
        parse_songselect_txt(&text).map(|d| (d, "SongSelect"))
    } else if ext == "txt" {
        parse_plain(&text, &title).map(|d| (d, "plain text"))
    } else {
        Err(bad("not a song format Worship Presenter knows"))
    }?;
    let mut d = r.0;
    if d.title.trim().is_empty() {
        d.title = title;
    }
    Ok((d, r.1))
}

/// Files inside a folder (and its folders) that might be songs.
pub fn expand(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = vec![];
    for p in paths {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                let mut kids: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
                kids.sort();
                out.extend(expand(&kids));
            }
        } else {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if EXTENSIONS.contains(&ext.as_str()) && !p.file_name().map_or(true, |n| n.to_string_lossy().starts_with('.')) {
                out.push(p.clone());
            }
        }
    }
    out
}

/// Cleans look-alike characters in every text field; returns how many were changed.
fn clean_doc(d: &mut SongDoc) -> (usize, Vec<String>) {
    let mut count = 0;
    let mut reasons = vec![];
    let mut fix = |s: &mut String| {
        let (c, fixes) = text::clean(s);
        for f in fixes {
            count += f.count;
            reasons.push(format!("{}× U+{:04X} → “{}” ({})", f.count, f.from as u32, f.to, f.reason));
        }
        *s = c;
    };
    fix(&mut d.title);
    for l in d.lyrics.iter_mut() {
        fix(&mut l.body);
    }
    (count, reasons)
}

/// What importing these files would do; writes nothing.
pub fn review(lib: &Connection, paths: &[PathBuf]) -> Result<Review> {
    let files = expand(paths);
    let mut have = HashSet::new();
    let mut st = lib.prepare("SELECT lower(trim(title)) FROM song")?;
    for t in st.query_map([], |r| r.get::<_, String>(0))? {
        have.insert(t?);
    }
    let mut rv = Review { name: "Song files".into(), read: files.len(), recognised: 0, ambiguous: vec![], rejected: vec![], duplicates: vec![], notes: vec![] };
    let mut formats: Vec<&str> = vec![];
    for f in &files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match parse_file(f) {
            Ok((mut d, fmt)) => {
                if !formats.contains(&fmt) {
                    formats.push(fmt);
                }
                let (n, reasons) = clean_doc(&mut d);
                if have.contains(&d.title.trim().to_lowercase()) {
                    rv.duplicates.push(d.title.clone());
                }
                if n > 0 {
                    rv.ambiguous.push(Line { item: d.title.clone(), reason: reasons.join("; ") });
                } else if crate::songs::preview(&d, 2).is_err() {
                    rv.ambiguous.push(Line { item: d.title.clone(), reason: "its play order or second language needs checking".into() });
                } else {
                    rv.recognised += 1;
                }
            }
            Err(e) => rv.rejected.push(Line { item: name, reason: e.to_string() }),
        }
    }
    if !formats.is_empty() {
        rv.notes.push(format!("Formats found: {}.", formats.join(", ")));
    }
    if formats.contains(&"ProPresenter 7") {
        rv.notes.push("ProPresenter 7 files are read as best we can: each slide's words come in, but group names (Verse, Chorus) and arrangements do not. Check these songs.".into());
    }
    Ok(rv)
}

/// Imports the files as a collection; titles in `skip` (lower case) are left out.
pub fn import(conn: &Connection, paths: &[PathBuf], skip: Option<&HashSet<String>>) -> Result<Report> {
    let files = expand(paths);
    let cid = create_collection(
        conn,
        &Collection { kind: "songs", name: "Song files", source_format: "song-files", source_file: &paths.first().map(|p| p.display().to_string()).unwrap_or_default(), publisher: None, licence_note: None, transferable: true },
    )?;
    let mut rep = Report { collection_id: cid, name: "Song files".into(), read: files.len(), imported: 0, skipped: 0, fixed_chars: 0, notes: vec![] };
    for f in &files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let log = |level: &str, msg: &str| {
            let _ = conn.execute("INSERT INTO import_log (collection_id, level, item, message) VALUES (?1, ?2, ?3, ?4)", rusqlite::params![cid, level, name, msg]);
        };
        match parse_file(f) {
            Ok((mut d, _)) => {
                if skip.map_or(false, |s| s.contains(&d.title.trim().to_lowercase())) {
                    rep.skipped += 1;
                    log("skipped", "already in the library (left out at review)");
                    continue;
                }
                let (n, reasons) = clean_doc(&mut d);
                rep.fixed_chars += n;
                if n > 0 {
                    log("fixed", &reasons.join("; "));
                }
                // a play order that does not fit is dropped rather than losing the song
                if crate::songs::preview(&d, 2).is_err() {
                    d.sequence.clear();
                }
                match crate::songs::save(conn, &d) {
                    Ok(id) => {
                        let _ = conn.execute("UPDATE song SET collection_id = ?2, source_ref = ?3 WHERE id = ?1", rusqlite::params![id, cid, name]);
                        rep.imported += 1;
                    }
                    Err(e) => {
                        rep.skipped += 1;
                        log("skipped", &e.to_string());
                    }
                }
            }
            Err(e) => {
                rep.skipped += 1;
                log("skipped", &e.to_string());
            }
        }
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENLYRICS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<song xmlns="http://openlyrics.info/namespace/2009/song" version="0.9">
  <properties>
    <titles><title>Ɔdomankoma Wɔ Hɔ</title></titles>
    <authors><author>Traditional</author><author>Arr. K. Mensah</author></authors>
    <copyright>Public Domain</copyright>
    <ccliNo>1234</ccliNo>
    <songbooks><songbook name="pan" entry="12"/></songbooks>
    <verseOrder>v1 c v2 c</verseOrder>
  </properties>
  <lyrics>
    <verse name="v1" lang="en"><lines>God is <chord name="G"/>here<br/>He is with us</lines></verse>
    <verse name="v1" lang="tw"><lines>Onyankopɔn wɔ ha<br/>Ɔka yɛn ho</lines></verse>
    <verse name="c" lang="en"><lines><comment>all</comment>Sing praise</lines></verse>
    <verse name="c" lang="tw"><lines>To dwom</lines></verse>
    <verse name="v2" lang="en"><lines>Second verse</lines></verse>
  </lyrics>
</song>"#;

    #[test]
    fn openlyrics_bilingual_numbers_order() {
        let d = parse_openlyrics(OPENLYRICS).unwrap();
        assert_eq!(d.title, "Ɔdomankoma Wɔ Hɔ");
        assert_eq!(d.author.as_deref(), Some("Traditional, Arr. K. Mensah"));
        assert_eq!(d.numbers, vec![Number { hymnal: "PAN".into(), number: "12".into() }]);
        assert_eq!(d.sequence, "V1 C V2 C");
        assert_eq!(d.lyrics.len(), 2);
        assert_eq!(d.lyrics[0].body, "[V1]\nGod is [G]here\nHe is with us\n\n[C]\nSing praise\n\n[V2]\nSecond verse");
        assert_eq!(d.lyrics[1].lang, "tw");
        assert_eq!(crate::songs::preview(&d, 2).unwrap().len(), 4);
    }

    #[test]
    fn opensong() {
        let x = "<?xml version=\"1.0\"?><song><title>Amazing Grace</title><author>John Newton</author><presentation>V1 C</presentation><lyrics>[V1]\n.G       C\n Amazing grace|how sweet\n;comment\n That saved a wretch\n[C]\n I once was lost</lyrics></song>";
        let d = parse_opensong(x).unwrap();
        assert_eq!(d.lyrics[0].body, "[V1]\n[G]Amazing [C]grace how sweet\nThat saved a wretch\n\n[C]\nI once was lost", "chords placed over the right word");
        assert_eq!(d.sequence, "V1 C");
    }

    #[test]
    fn songselect_and_usr() {
        let t = "Way Maker\n\nVerse 1\nYou are here\nmoving in our midst\n\nChorus\nWay maker\nmiracle worker\n\nCCLI Song # 7115744\nSinach\n© 2016 Integrity Music\nFor use solely with the SongSelect® Terms of Use.\nCCLI License # 12345\n";
        let d = parse_songselect_txt(t).unwrap();
        assert_eq!((d.title.as_str(), d.ccli.as_deref(), d.author.as_deref()), ("Way Maker", Some("7115744"), Some("Sinach")));
        assert_eq!(d.copyright.as_deref(), Some("© 2016 Integrity Music"));
        assert!(d.lyrics[0].body.starts_with("[V1]\nYou are here"));
        let u = "[S A7115744]\nTitle=Way Maker\nAuthor=Sinach\nCopyright=2016 Integrity\nFields=Verse 1/tChorus\nWords=You are here/nmoving/tWay maker/nmiracle worker";
        let d = parse_usr(u).unwrap();
        assert_eq!(d.ccli.as_deref(), Some("7115744"));
        assert_eq!(d.lyrics[0].body, "[V1]\nYou are here\nmoving\n\n[C]\nWay maker\nmiracle worker");
    }

    #[test]
    fn chordpro_and_plain() {
        let c = "{title: Blessed Assurance}\n{artist: Fanny Crosby}\n[D]Blessed as[G]surance\nJesus is mine\n\n{soc}\nThis is my story\n{eoc}\n\nHeir of sal[A]vation\n\n{soc}\nThis is my story\n{eoc}";
        let d = parse_chordpro(c, "x").unwrap();
        assert_eq!(d.title, "Blessed Assurance");
        assert_eq!(d.lyrics[0].body, "[V1]\n[D]Blessed as[G]surance\nJesus is mine\n\n[C]\nThis is my story\n\n[V2]\nHeir of sal[A]vation", "chords kept inline for the musicians, repeated chorus kept once");
        assert_eq!(d.key, None);
        let k = parse_chordpro("{title: X}\n{key: Bb}\n[Bb]Line", "x").unwrap();
        assert_eq!(k.key.as_deref(), Some("Bb"));
        let p = parse_plain("Line one\nLine two\n\nChorus\nRefrain line\n\nVerse three line", "Morning Song").unwrap();
        assert_eq!(p.lyrics[0].body, "[V1]\nLine one\nLine two\n\n[C]\nRefrain line\n\n[V2]\nVerse three line");
    }

    #[test]
    fn propresenter_6_and_7() {
        let rtf = r"{\rtf1\ansi{\fonttbl\f0 Arial;}\f0 Amazing grace\par how sweet the sound}";
        let b64 = base64::engine::general_purpose::STANDARD.encode(rtf);
        let x = format!(r#"<RVPresentationDocument CCLISongTitle="Amazing Grace" CCLIAuthor="John Newton" CCLISongNumber="22025"><array rvXMLIvarName="groups"><RVSlideGrouping name="Verse 1"><array rvXMLIvarName="slides"><RVDisplaySlide><array rvXMLIvarName="displayElements"><RVTextElement><NSString rvXMLIvarName="RTFData">{b64}</NSString></RVTextElement></array></RVDisplaySlide></array></RVSlideGrouping></array></RVPresentationDocument>"#);
        let d = parse_pro6(&x).unwrap();
        assert_eq!((d.title.as_str(), d.ccli.as_deref()), ("Amazing Grace", Some("22025")));
        assert_eq!(d.lyrics[0].body, "[V1]\nAmazing grace\nhow sweet the sound");

        // a hand-built protobuf: name (field 3) and two slides holding RTF deep inside
        fn ld(field: u8, inner: &[u8]) -> Vec<u8> {
            let mut v = vec![(field << 3) | 2, inner.len() as u8];
            v.extend_from_slice(inner);
            v
        }
        let s1 = ld(1, &ld(2, &ld(5, br"{\rtf1 First slide\par line two}")));
        let s2 = ld(1, &ld(2, &ld(5, br"{\rtf1 Second slide}")));
        let mut pro = ld(3, b"Amazing Grace");
        pro.extend(ld(13, &s1));
        pro.extend(ld(13, &s2));
        let d = parse_pro7(&pro, "file").unwrap();
        assert_eq!(d.title, "Amazing Grace");
        assert_eq!(d.lyrics[0].body, "[V1]\nFirst slide\nline two\n\n[V2]\nSecond slide");
        assert!(parse_pro7(b"\xff\xff\xff\xff", "x").is_err());
    }

    #[test]
    fn rtf_escapes() {
        let r = r"{\rtf1\ansi\uc1{\fonttbl{\f0 Arial;}}{\colortbl;\red255\green255\blue255;}\f0\cf1 Onyankop\u596?n w\u596? ha\par Caf\'e9 \{ok\}}";
        assert_eq!(rtf_text(r), vec!["Onyankopɔn wɔ ha", "Café {ok}"]);
    }

    #[test]
    fn folder_review_and_import() {
        let dir = std::env::temp_dir().join(format!("wp-songfiles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("odomankoma.xml"), OPENLYRICS).unwrap();
        std::fs::write(dir.join("sub").join("Amazing Grace"), "<song><title>Amazing Grace</title><lyrics>[V1]\n Amazing grace</lyrics></song>").unwrap();
        std::fs::write(dir.join("lookalike.txt"), "Gr\u{03B5}at is Thy faithfulness\nMorning by morning").unwrap();
        std::fs::write(dir.join("photo.jpg"), b"not a song").unwrap();
        std::fs::write(dir.join("broken.pro6"), "<RVPresentationDocument>").unwrap();
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO song (title) VALUES ('amazing grace')", []).unwrap();
        let r = review(&c, &[dir.clone()]).unwrap();
        assert_eq!(r.read, 4, "the photo is not looked at");
        assert_eq!(r.rejected.len(), 1);
        assert_eq!(r.duplicates, vec!["Amazing Grace".to_string()]);
        assert_eq!(r.ambiguous.len(), 1, "the Greek epsilon is reported");
        let skip: HashSet<String> = r.duplicates.iter().map(|t| t.to_lowercase()).collect();
        let rep = import(&c, &[dir.clone()], Some(&skip)).unwrap();
        assert_eq!((rep.imported, rep.skipped, rep.fixed_chars), (2, 2, 1));
        assert!(crate::library::search_songs(&c, "faithfulness", 5).unwrap().len() == 1);
        assert_eq!(crate::library::search_songs(&c, "PAN 12", 5).unwrap().len(), 1);
    }
}
