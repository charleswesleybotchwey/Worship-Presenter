//! Recognising a verse from its words: when the preacher reads a passage aloud without
//! naming it, the last few seconds of speech are matched against the Bibles (the word
//! search). Only a confident match is suggested, and only ever as a suggestion.
//!
//! "Confident" means at least six words of the verse heard in the same order (allowing a
//! misheard word or two), so everyday phrases ("the Lord is good") do not trigger it.

use crate::browse::search_bible;
use crate::text::fold;
use crate::Result;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quote {
    pub reference: String,
    pub book_no: u16,
    pub chapter: u16,
    pub verse: u16,
    pub version: String,
    /// 0–100: how much of the verse was heard, in order.
    pub score: u8,
    pub text: String,
}

const STOP: [&str; 48] = [
    "the", "and", "of", "to", "a", "in", "that", "is", "for", "he", "his", "it", "was", "i", "you", "be", "with", "as", "they", "all", "not", "but", "unto", "shall", "him", "them", "my", "me", "we", "us", "our", "your", "on", "by", "are", "this", "from", "have", "so", "which", "will", "who", "at", "or", "an", "there", "their", "were",
];

fn words(s: &str) -> Vec<String> {
    fold(s).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// Longest run of verse words heard in order, allowing one misheard or missing word.
fn in_order(heard: &[String], verse: &[String]) -> usize {
    let mut best = 0;
    for start in 0..heard.len() {
        for vstart in 0..verse.len() {
            if heard[start] != verse[vstart] {
                continue;
            }
            let (mut h, mut v, mut run, mut slips) = (start, vstart, 0, 0);
            while h < heard.len() && v < verse.len() {
                if heard[h] == verse[v] {
                    run += 1;
                    h += 1;
                    v += 1;
                } else if slips < 2 {
                    slips += 1;
                    // a misheard word, an extra word, or a missing word
                    if h + 1 < heard.len() && v + 1 < verse.len() && heard[h + 1] == verse[v + 1] {
                        h += 1;
                        v += 1;
                    } else if h + 1 < heard.len() && heard[h + 1] == verse[v] {
                        h += 1;
                    } else if v + 1 < verse.len() && heard[h] == verse[v + 1] {
                        v += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            best = best.max(run);
        }
    }
    best
}

/// The verse the speech most likely quotes, if any (`spoken`: the last ~15 seconds).
pub fn find(conn: &Connection, versions: &[String], spoken: &str) -> Result<Option<Quote>> {
    let heard = words(spoken);
    if heard.len() < 6 {
        return Ok(None);
    }
    let recent = &heard[heard.len().saturating_sub(30)..];
    // stretches of ~8 words, newest first: talk before the reading ("church, let us
    // read") must not spoil the search, so each stretch is searched on its own rarer words
    let mut best: Option<Quote> = None;
    let mut end = recent.len();
    let mut tries = 0;
    while end >= 6 && tries < 6 {
        let stretch = &recent[end.saturating_sub(8)..end];
        let mut key: Vec<&String> = vec![];
        for w in stretch {
            if w.len() > 2 && !STOP.contains(&w.as_str()) && !key.contains(&w) {
                key.push(w);
            }
        }
        key.sort_by_key(|w| std::cmp::Reverse(w.len()));
        if key.len() >= 2 {
            let q = key[..key.len().min(3)].iter().map(|w| w.as_str()).collect::<Vec<_>>().join(" ");
            for hit in search_bible(conn, versions, &q, 15)? {
                let v = words(&hit.text);
                let run = in_order(recent, &v);
                let score = ((run as f32 / v.len().min(recent.len()).max(1) as f32) * 100.0).round().min(100.0) as u8;
                if run >= 6 && best.as_ref().map_or(true, |b| score > b.score) {
                    best = Some(Quote { reference: hit.reference.clone(), book_no: hit.book_no, chapter: hit.chapter, verse: hit.verse, version: hit.version.clone(), score, text: hit.text.clone() });
                }
            }
        }
        if best.as_ref().is_some_and(|b| b.score >= 60) {
            break;
        }
        end = end.saturating_sub(4);
        tries += 1;
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bible() -> Connection {
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO bible_version (id, code, name, lang) VALUES (1, 'NKJV', 'New King James', 'en')", []).unwrap();
        for (no, name) in [(19, "Psalm"), (43, "John"), (45, "Romans")] {
            c.execute("INSERT INTO bible_book (version_id, book_no, name, chapters) VALUES (1, ?1, ?2, 150)", rusqlite::params![no, name]).unwrap();
        }
        for (b, ch, v, t) in [
            (43, 3, 16, "For God so loved the world that He gave His only begotten Son, that whoever believes in Him should not perish but have everlasting life."),
            (43, 3, 17, "For God did not send His Son into the world to condemn the world, but that the world through Him might be saved."),
            (19, 23, 1, "The Lord is my shepherd; I shall not want."),
            (19, 100, 5, "For the Lord is good; His mercy is everlasting, and His truth endures to all generations."),
            (45, 8, 28, "And we know that all things work together for good to those who love God, to those who are the called according to His purpose."),
        ] {
            c.execute("INSERT INTO bible_verse (version_id, book_no, chapter, verse, text) VALUES (1, ?1, ?2, ?3, ?4)", rusqlite::params![b, ch, v, t]).unwrap();
        }
        c
    }

    #[test]
    fn a_verse_read_aloud_is_recognised() {
        let c = bible();
        let v = vec!["NKJV".to_string()];
        let q = find(&c, &v, "church let us read together we know that all things work together for good to those who love God").unwrap().unwrap();
        assert_eq!(q.reference, "Romans 8:28");
        assert!(q.score >= 50, "{}", q.score);
        // a misheard word or two still matches
        let q = find(&c, &v, "for god so love the world that he gave his only begotten son").unwrap().unwrap();
        assert_eq!(q.reference, "John 3:16");
    }

    #[test]
    fn everyday_phrases_do_not_trigger() {
        let c = bible();
        let v = vec!["NKJV".to_string()];
        assert_eq!(find(&c, &v, "the Lord is good all the time").unwrap(), None);
        assert_eq!(find(&c, &v, "we love God and we know he is good to us").unwrap(), None);
        assert_eq!(find(&c, &v, "amen").unwrap(), None);
    }
}
