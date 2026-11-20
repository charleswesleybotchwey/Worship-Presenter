//! Find and replace across the song library: preview what would change, then change it
//! (a backup is made first by the app). Section marks ("[V1]") are never touched; the
//! songs are saved the usual way, so their search keys stay right.

use crate::songs::{load, save};
use crate::Result;
use regex::Regex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Options {
    pub match_case: bool,
    pub whole_words: bool,
    pub in_words: bool,
    pub in_titles: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    pub song_id: i64,
    pub title: String,
    pub count: usize,
    /// The first changed line, before and after.
    pub before: String,
    pub after: String,
}

fn pattern(find: &str, o: &Options) -> Option<Regex> {
    if find.trim().is_empty() {
        return None;
    }
    let mut p = regex::escape(find);
    if o.whole_words {
        p = format!(r"\b{p}\b");
    }
    if !o.match_case {
        p = format!("(?i){p}");
    }
    Regex::new(&p).ok()
}

/// One lyrics body with the replacement made outside section marks; and the count.
fn replace_body(body: &str, re: &Regex, with: &str) -> (String, usize, Option<(String, String)>) {
    let mut count = 0;
    let mut first = None;
    let out: Vec<String> = body
        .lines()
        .map(|l| {
            if l.trim_start().starts_with('[') && l.trim_end().ends_with(']') {
                return l.to_string(); // a section mark
            }
            let n = re.find_iter(l).count();
            if n == 0 {
                return l.to_string();
            }
            count += n;
            let changed = re.replace_all(l, regex::NoExpand(with)).into_owned();
            if first.is_none() {
                first = Some((l.to_string(), changed.clone()));
            }
            changed
        })
        .collect();
    (out.join("\n"), count, first)
}

/// What would change (nothing is changed).
pub fn plan(conn: &Connection, find: &str, with: &str, o: &Options) -> Result<Vec<Change>> {
    let Some(re) = pattern(find, o) else { return Ok(vec![]) };
    let ids: Vec<i64> = conn.prepare("SELECT id FROM song ORDER BY title COLLATE NOCASE")?.query_map([], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
    let mut out = vec![];
    for id in ids {
        let doc = load(conn, id)?;
        let mut count = 0;
        let mut sample = None;
        if o.in_titles {
            let n = re.find_iter(&doc.title).count();
            if n > 0 {
                count += n;
                sample = Some((doc.title.clone(), re.replace_all(&doc.title, regex::NoExpand(with)).into_owned()));
            }
        }
        if o.in_words {
            for l in &doc.lyrics {
                let (_, n, first) = replace_body(&l.body, &re, with);
                count += n;
                if sample.is_none() {
                    sample = first;
                }
            }
        }
        if count > 0 {
            let (before, after) = sample.unwrap_or_default();
            out.push(Change { song_id: id, title: doc.title, count, before, after });
        }
    }
    Ok(out)
}

/// Makes the changes; returns how many songs changed.
pub fn apply(conn: &Connection, find: &str, with: &str, o: &Options) -> Result<usize> {
    let Some(re) = pattern(find, o) else { return Ok(0) };
    let mut changed = 0;
    for c in plan(conn, find, with, o)? {
        let mut doc = load(conn, c.song_id)?;
        if o.in_titles {
            doc.title = re.replace_all(&doc.title, regex::NoExpand(with)).into_owned();
        }
        if o.in_words {
            for l in doc.lyrics.iter_mut() {
                l.body = replace_body(&l.body, &re, with).0;
            }
        }
        save(conn, &doc)?;
        changed += 1;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::songs::{Lyric, Number, SongDoc};

    fn song(c: &Connection, title: &str, words: &str) -> i64 {
        let d = SongDoc { id: None, title: title.into(), author: None, copyright: None, ccli: None, numbers: vec![Number { hymnal: "MHB".into(), number: "1".into() }], lyrics: vec![Lyric { lang: "en".into(), body: words.into() }], sequence: String::new(), lines_per_slide: None, key: None };
        save(c, &d).unwrap()
    }

    #[test]
    fn preview_then_replace() {
        let c = crate::db::open_memory().unwrap();
        let a = song(&c, "Thee we adore", "[V1]\nWe praise Thee, O God\nThee alone, and theeology\n[C]\nThee");
        song(&c, "Other", "[V1]\nNothing here");
        let o = Options { match_case: true, whole_words: true, in_words: true, in_titles: true };
        let p = plan(&c, "Thee", "You", &o).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].count, 4, "title + three whole words (not 'theeology')");
        assert_eq!((p[0].before.as_str(), p[0].after.as_str()), ("Thee we adore", "You we adore"));
        assert_eq!(load(&c, a).unwrap().title, "Thee we adore", "a preview changes nothing");
        assert_eq!(apply(&c, "Thee", "You", &o).unwrap(), 1);
        let d = load(&c, a).unwrap();
        assert_eq!(d.title, "You we adore");
        assert_eq!(d.lyrics[0].body, "[V1]\nWe praise You, O God\nYou alone, and theeology\n[C]\nYou");
        // without whole words or case: also inside words, any case
        let o2 = Options { match_case: false, whole_words: false, in_words: true, in_titles: false };
        assert_eq!(plan(&c, "THEE", "x", &o2).unwrap()[0].count, 1, "only 'theeology' is left");
        // section marks are never touched
        assert!(plan(&c, "V1", "X", &o2).unwrap().is_empty());
        // the search finds the new words
        assert!(crate::library::search_songs(&c, "You we adore", 10).unwrap().iter().any(|h| h.id == a));
    }
}
