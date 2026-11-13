//! A sermon outline pasted in, turned into slides: each line a point slide, and every
//! scripture reference in it ("Romans 8:28", "1 Cor. 13:4-7", "Ps 23", "Yohane 3:16")
//! followed by the passage itself on reading slides from the chosen Bible.

use crate::announcements::{Deck, DeckSlide};
use crate::reference::{self, Alias};
use crate::Result;
use rusqlite::Connection;

/// The references written in a line, in order (each as typed, tidied: "1 Cor 13:4-7").
pub fn references_in(line: &str, aliases: &[Alias]) -> Vec<String> {
    let words: Vec<String> = line
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | ',' | ';' | ':' | '.' | '“' | '”' | '"')).to_string())
        .filter(|w| !w.is_empty())
        .collect();
    let mut out = vec![];
    let mut i = 0;
    while i < words.len() {
        let mut found: Option<(String, usize)> = None;
        // longest first: "1 Cor 13:4-7" (3 words) before "Cor 13:4" (2)
        for n in (2..=4).rev() {
            if i + n > words.len() {
                continue;
            }
            let cand = words[i..i + n].join(" ");
            let last = &words[i + n - 1];
            if !last.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }
            if reference::parse(&cand, aliases).is_ok() {
                found = Some((cand, n));
                break;
            }
        }
        match found {
            Some((r, n)) => {
                out.push(r);
                i += n;
            }
            None => i += 1,
        }
    }
    out
}

/// Strips list marks: "-", "•", "*", "1.", "2)", "a)", "i.".
fn tidy(line: &str) -> String {
    let t = line.trim().trim_start_matches(['-', '•', '*', '–', '·']).trim();
    let mut chars = t.char_indices();
    let mut end = 0;
    for (k, c) in chars.by_ref() {
        if c.is_ascii_alphanumeric() && k < 4 {
            end = k + c.len_utf8();
            continue;
        }
        if (c == '.' || c == ')') && end > 0 && end <= 4 {
            // only a short mark ("1.", "ii)", "a)") before the words
            let mark = &t[..end];
            if mark.chars().all(|x| x.is_ascii_digit()) || mark.chars().all(|x| "ivxlIVXL".contains(x)) || mark.len() == 1 {
                return t[k + 1..].trim().to_string();
            }
        }
        break;
    }
    t.to_string()
}

/// The slides for an outline: the first line is the title slide; every other line is a
/// point, followed by reading slides for the references in it (about two verses each).
pub fn to_deck(conn: &Connection, text: &str, version: &str) -> Result<Deck> {
    let aliases = crate::library::aliases(conn)?;
    let lines: Vec<String> = text.lines().map(tidy).filter(|l| !l.is_empty()).collect();
    let title = lines.first().cloned().unwrap_or_else(|| "Sermon".into());
    let mut slides = vec![DeckSlide { kind: "text".into(), title: title.clone(), body: String::new(), image: None, seconds: None }];
    for line in lines.iter().skip(1) {
        slides.push(DeckSlide { kind: "text".into(), title: String::new(), body: line.clone(), image: None, seconds: None });
        for r in references_in(line, &aliases) {
            let Ok(p) = crate::library::lookup(conn, &r, &[version]) else { continue };
            let mut chunk: Vec<String> = vec![];
            let mut len = 0;
            let flush = |chunk: &mut Vec<String>, slides: &mut Vec<DeckSlide>| {
                if !chunk.is_empty() {
                    slides.push(DeckSlide { kind: "text".into(), title: p.display.clone(), body: chunk.join(" "), image: None, seconds: None });
                    chunk.clear();
                }
            };
            for v in &p.verses {
                if len > 0 && (chunk.len() >= 2 || len + v.text.len() > 380) {
                    flush(&mut chunk, &mut slides);
                    len = 0;
                }
                chunk.push(format!("{} {}", v.verse, v.text));
                len += v.text.len();
            }
            flush(&mut chunk, &mut slides);
        }
    }
    Ok(Deck { id: None, title, looping: false, seconds: 0, slides })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bible() -> Connection {
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO bible_version (id, code, name, lang) VALUES (1, 'NKJV', 'NKJV', 'en')", []).unwrap();
        for (no, name) in [(43, "John"), (45, "Romans"), (46, "1 Corinthians")] {
            c.execute("INSERT INTO bible_book (version_id, book_no, name, aliases, chapters) VALUES (1, ?1, ?2, ?2, 30)", rusqlite::params![no, name]).unwrap();
        }
        for (b, ch, v, t) in [
            (45, 8, 28, "And we know that all things work together for good to those who love God."),
            (46, 13, 4, "Love suffers long and is kind; love does not envy;"),
            (46, 13, 5, "does not behave rudely, does not seek its own;"),
            (46, 13, 6, "does not rejoice in iniquity, but rejoices in the truth;"),
            (46, 13, 7, "bears all things, believes all things, hopes all things, endures all things."),
            (43, 3, 16, "For God so loved the world that He gave His only begotten Son."),
        ] {
            c.execute("INSERT INTO bible_verse (version_id, book_no, chapter, verse, text) VALUES (1, ?1, ?2, ?3, ?4)", rusqlite::params![b, ch, v, t]).unwrap();
        }
        c
    }

    #[test]
    fn references_as_pastors_write_them() {
        let a = crate::reference::english_aliases();
        assert_eq!(references_in("All things work together (Romans 8:28).", &a), vec!["Romans 8:28"]);
        assert_eq!(references_in("Love is patient – 1 Cor. 13:4-7; see also Jn 3:16", &a), vec!["1 Cor 13:4-7", "Jn 3:16"]);
        assert_eq!(references_in("Read Ps 23 together", &a), vec!["Ps 23"]);
        assert!(references_in("God is good all the time", &a).is_empty());
        assert!(references_in("Point 1: we have 3 reasons", &a).is_empty(), "numbers alone are not references");
    }

    #[test]
    fn an_outline_becomes_slides() {
        let c = bible();
        let outline = "The God Who Works for Our Good\n1. He works in all things (Romans 8:28)\n2. His love never fails - 1 Cor 13:4-7\n- Response: believe (John 3:16)";
        let d = to_deck(&c, outline, "NKJV").unwrap();
        let s: Vec<(String, String)> = d.slides.iter().map(|x| (x.title.clone(), x.body.clone())).collect();
        assert_eq!(d.title, "The God Who Works for Our Good");
        assert_eq!(s[1].1, "He works in all things (Romans 8:28)", "list mark gone");
        assert_eq!(s[2].0, "Romans 8:28");
        assert!(s[2].1.starts_with("28 And we know"));
        assert_eq!(s[3].1, "His love never fails - 1 Cor 13:4-7");
        assert_eq!((s[4].0.as_str(), s[5].0.as_str()), ("1 Corinthians 13:4-7", "1 Corinthians 13:4-7"), "four verses over two slides");
        assert!(s[4].1.starts_with("4 Love suffers") && s[5].1.starts_with("6 does not rejoice"));
        assert_eq!(s.last().unwrap().0, "John 3:16");
        assert_eq!(d.slides.len(), 8);
    }
}
