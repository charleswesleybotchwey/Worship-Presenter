//! Spelling for languages without a dictionary here (Fante, Twi, Ga…), learnt from the
//! church's own songs: a word that is rare in the library but one letter from a common one
//! is probably a typo ("Nyankopon" where the songs say "Nyankopɔn" 212 times).

use crate::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

/// Words of a text: letters (with ɛ ɔ ŋ and tone marks) and inner apostrophes, lower case.
fn words(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            continue; // a section label
        }
        let mut cur = String::new();
        let mut skip = false;
        for c in line.chars() {
            match c {
                '[' => skip = true,
                ']' if skip => skip = false,
                _ if skip => {}
                c if c.is_alphabetic() || ((c == '\'' || c == '’') && !cur.is_empty()) => cur.push(c),
                _ => {
                    push(&mut out, n, &mut cur);
                }
            }
        }
        push(&mut out, n, &mut cur);
    }
    out
}

fn push(out: &mut Vec<(usize, String)>, n: usize, cur: &mut String) {
    let w = cur.trim_end_matches(['\'', '’']).to_lowercase();
    if w.chars().count() >= 3 {
        out.push((n, w));
    }
    cur.clear();
}

static CACHE: Mutex<Option<HashMap<String, (i64, HashMap<String, usize>)>>> = Mutex::new(None);

/// How often each word appears in the library's songs in this language (cached until songs change).
fn counts(conn: &Connection, lang: &str) -> Result<HashMap<String, usize>> {
    let stamp: i64 = conn.query_row("SELECT count(*) * 1000003 + coalesce(sum(length(body)), 0) FROM song_lyric WHERE lang = ?1", [lang], |r| r.get(0))?;
    if let Some((s, c)) = CACHE.lock().unwrap().as_ref().and_then(|m| m.get(lang)) {
        if *s == stamp {
            return Ok(c.clone());
        }
    }
    let mut c: HashMap<String, usize> = HashMap::new();
    let mut st = conn.prepare("SELECT body FROM song_lyric WHERE lang = ?1")?;
    for body in st.query_map([lang], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()) {
        for (_, w) in words(&body) {
            *c.entry(w).or_insert(0) += 1;
        }
    }
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(lang.to_string(), (stamp, c.clone()));
    Ok(c)
}

/// Words one change away: a letter swapped (ɔ/o, ɛ/e, ŋ/n first), dropped, added or two swapped.
fn near(w: &str) -> Vec<String> {
    const LETTERS: &str = "abcdefghijklmnopqrstuvwxyzɛɔŋ";
    let c: Vec<char> = w.chars().collect();
    let mut out = Vec::new();
    for i in 0..c.len() {
        for l in LETTERS.chars() {
            if l != c[i] {
                let mut x = c.clone();
                x[i] = l;
                out.push(x.into_iter().collect());
            }
        }
        let mut d = c.clone();
        d.remove(i);
        out.push(d.into_iter().collect());
        if i + 1 < c.len() {
            let mut s = c.clone();
            s.swap(i, i + 1);
            out.push(s.into_iter().collect());
        }
    }
    for i in 0..=c.len() {
        for l in LETTERS.chars() {
            let mut x = c.clone();
            x.insert(i, l);
            out.push(x.into_iter().collect());
        }
    }
    out
}

/// The likely mistakes in Ghanaian spelling: o for ɔ, e for ɛ, n for ŋ (and back).
fn likely(a: &str, b: &str) -> bool {
    let (x, y): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    x.len() == y.len() && x.iter().zip(&y).filter(|(p, q)| p != q).all(|(p, q)| matches!((p, q), ('o', 'ɔ') | ('ɔ', 'o') | ('e', 'ɛ') | ('ɛ', 'e') | ('n', 'ŋ') | ('ŋ', 'n')))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestion {
    /// line number in the text (from 0)
    pub line: usize,
    pub word: String,
    pub suggestion: String,
    /// how often the library's songs use the suggestion
    pub count: usize,
}

/// Probable typos in `text` (lyrics in `lang`), against the library's own spelling.
pub fn check(conn: &Connection, lang: &str, text: &str) -> Result<Vec<Suggestion>> {
    let c = counts(conn, lang)?;
    let mut out: Vec<Suggestion> = Vec::new();
    for (line, w) in words(text) {
        let here = c.get(&w).copied().unwrap_or(0);
        if here > 2 || out.iter().any(|s| s.word == w) {
            continue; // the library uses it: fine
        }
        let mut best: Option<(String, usize, bool)> = None;
        for cand in near(&w) {
            let n = c.get(&cand).copied().unwrap_or(0);
            let l = likely(&w, &cand);
            // common enough to trust; ɔ/ɛ/ŋ swaps need less
            if n >= if l { 3 } else { 8 } && n > here * 10 {
                let better = match &best {
                    None => true,
                    Some((_, bn, bl)) => (l && !bl) || (l == *bl && n > *bn),
                };
                if better {
                    best = Some((cand, n, l));
                }
            }
        }
        if let Some((s, n, _)) = best {
            out.push(Suggestion { line, word: w, suggestion: s, count: n });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typos_are_found_against_the_library() {
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO song (title) VALUES ('t')", []).unwrap();
        let id = c.last_insert_rowid();
        let fante = "[V1]\nNyankopɔn yɛ kɛse\nNyankopɔn dɔ hɛn\n".repeat(6);
        c.execute("INSERT INTO song_lyric (song_id, lang, body) VALUES (?1, 'fat', ?2)", rusqlite::params![id, fante]).unwrap();
        let s = check(&c, "fat", "[V1]\nNyankopon yɛ kɛse\nO Nyankopɔn dɔ hɛn").unwrap();
        assert_eq!(s, vec![Suggestion { line: 1, word: "nyankopon".into(), suggestion: "nyankopɔn".into(), count: 12 }], "o for ɔ caught; the library's own words left alone");
        // a word the library does not know at all and is near nothing common: left alone
        assert!(check(&c, "fat", "Abibirim").unwrap().is_empty());
        // chords and labels are not words
        assert!(check(&c, "fat", "[C]\n[G]Nyankopɔn").unwrap().is_empty());
    }
}
