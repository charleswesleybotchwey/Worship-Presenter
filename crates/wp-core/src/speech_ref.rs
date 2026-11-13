//! Scripture references in transcribed speech ("John three sixteen", "first
//! Corinthians thirteen verses four to seven", "Psalm twenty three", "Yohane 3 16").
//! Used by scripture recognition (experimental): what is found here is only ever
//! a suggestion for the operator; nothing goes live on its own.
//!
//! English number words are understood; book names are the English names, common
//! spoken forms and the names from the Bibles in the library (Twi, Ga…). A book name
//! counts only when a chapter number follows, so "a good job" is not Job.

use crate::reference::{self, Alias, BOOKS};
use crate::text::fold;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Heard {
    /// "1 Corinthians 13:4-7"
    pub reference: String,
    pub book_no: u16,
    pub chapter: u16,
    pub verse_from: Option<u16>,
    pub verse_to: Option<u16>,
    /// The words it came from.
    pub heard: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Num(u32),
}

fn unit(w: &str) -> Option<u32> {
    Some(match w {
        "zero" | "oh" => 0,
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        _ => return None,
    })
}

fn teen(w: &str) -> Option<u32> {
    Some(match w {
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        _ => return None,
    })
}

fn tens(w: &str) -> Option<u32> {
    Some(match w {
        "twenty" => 20,
        "thirty" => 30,
        "forty" | "fourty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        _ => return None,
    })
}

fn ordinal(w: &str) -> Option<u32> {
    Some(match w {
        "first" | "1st" | "i" => 1,
        "second" | "2nd" | "ii" => 2,
        "third" | "3rd" | "iii" => 3,
        _ => return None,
    })
}

/// Words and digits into tokens; number words into numbers. "three sixteen" stays two
/// numbers; "twenty three" is one; "one hundred and nineteen" is 119.
/// Written "3:16-18" becomes "3 16 to 18" before folding (folding drops punctuation).
fn spoken(text: &str) -> String {
    fold(&text.replace(':', " ").replace(['-', '–'], " to "))
}

fn tokens(text: &str) -> Vec<(Tok, usize)> {
    let folded = spoken(text);
    let raw: Vec<String> = folded
        .split(|c: char| !(c.is_alphanumeric() || c == ':' || c == '-' || c == '\''))
        .filter(|w| !w.is_empty())
        .map(|w| w.trim_matches('\'').to_string())
        .filter(|w| !w.is_empty())
        .collect();
    let mut out: Vec<(Tok, usize)> = vec![];
    let mut i = 0;
    while i < raw.len() {
        let w = raw[i].as_str();
        let start = i;
        if let Ok(n) = w.parse::<u32>() {
            out.push((Tok::Num(n), start));
            i += 1;
            continue;
        }
        // a number spoken in words
        let mut val: Option<u32> = None;
        let mut j = i;
        let mut hundreds = 0;
        if (unit(w).is_some() || w == "a") && raw.get(i + 1).map(String::as_str) == Some("hundred") {
            hundreds = if w == "a" { 100 } else { unit(w).unwrap() * 100 };
            j = i + 2;
            if raw.get(j).map(String::as_str) == Some("and") {
                j += 1;
            }
            val = Some(hundreds);
        }
        let w2 = raw.get(j).map(String::as_str).unwrap_or("");
        if let Some(t) = tens(w2) {
            let u = raw.get(j + 1).and_then(|x| unit(x)).filter(|u| *u > 0);
            val = Some(hundreds + t + u.unwrap_or(0));
            j += if u.is_some() { 2 } else { 1 };
        } else if let Some(t) = teen(w2) {
            val = Some(hundreds + t);
            j += 1;
        } else if let Some(u) = unit(w2) {
            // "oh" alone is not a number
            if !(w2 == "oh" && val.is_none()) {
                val = Some(hundreds + u);
                j += 1;
            }
        }
        match val {
            Some(v) => {
                out.push((Tok::Num(v), start));
                i = j.max(i + 1);
            }
            None => {
                out.push((Tok::Word(w.to_string()), start));
                i += 1;
            }
        }
    }
    out
}

/// Spoken names for books: the full English name, extra spoken forms, and the names
/// from the Bibles in the library (at least 4 letters, to avoid short abbreviations).
fn book_names(aliases: &[Alias]) -> Vec<(Vec<String>, u16)> {
    let mut v: Vec<(Vec<String>, u16)> = vec![];
    let mut add = |name: &str, no: u16| {
        let words: Vec<String> = fold(name).split_whitespace().map(str::to_string).collect();
        if !words.is_empty() && !v.iter().any(|(w, n)| *w == words && *n == no) {
            v.push((words, no));
        }
    };
    for b in BOOKS {
        add(b.english[0], b.no);
    }
    for (name, no) in [
        ("psalm", 19), ("the psalms", 19), ("song of songs", 22), ("songs of solomon", 22), ("revelations", 66), ("the revelation", 66),
        ("acts of the apostles", 44), ("the acts", 44), ("saint john", 43), ("saint matthew", 40), ("saint mark", 41), ("saint luke", 42),
        ("ecclesiastics", 21), ("proverb", 20), ("lamentation", 25),
    ] {
        add(name, no);
    }
    for a in aliases {
        let letters = a.folded.chars().filter(|c| c.is_alphabetic()).count();
        if letters >= 4 {
            add(&a.folded, a.book_no);
        }
    }
    // longest names first, so "1 john" wins over "john"
    v.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    v
}

/// The English name of a book ("1 Corinthians").
pub fn book_display(no: u16) -> &'static str {
    BOOKS.iter().find(|b| b.no == no).map(|b| b.english[0]).unwrap_or("?")
}

/// Every reference in a piece of transcribed speech, in order.
pub fn find(text: &str, aliases: &[Alias]) -> Vec<Heard> {
    let toks = tokens(text);
    let words_src: Vec<&str> = text.split_whitespace().collect();
    let names = book_names(aliases);
    let word_at = |i: usize| -> Option<&str> {
        match toks.get(i) {
            Some((Tok::Word(w), _)) => Some(w.as_str()),
            _ => None,
        }
    };
    let num_at = |i: usize| -> Option<u32> {
        match toks.get(i) {
            Some((Tok::Num(n), _)) => Some(*n),
            _ => None,
        }
    };
    let mut out: Vec<Heard> = vec![];
    let mut i = 0;
    while i < toks.len() {
        // optional ordinal for numbered books: "first", "1st", "one", "1"
        let (ord, after_ord) = match (&toks[i].0, word_at(i).and_then(ordinal)) {
            (_, Some(o)) => (Some(o), i + 1),
            (Tok::Num(n @ 1..=3), _) => (Some(*n), i + 1),
            _ => (None, i),
        };
        let mut matched: Option<(u16, usize)> = None;
        for (words, no) in &names {
            for (use_ord, from) in [(true, after_ord), (false, i)] {
                if use_ord && ord.is_none() {
                    continue;
                }
                let (lead, rest): (Option<u32>, &[String]) = match words.first().and_then(|w| w.parse::<u32>().ok()) {
                    Some(n) => (Some(n), &words[1..]),
                    None => (None, &words[..]),
                };
                if use_ord != lead.is_some() || (use_ord && lead != ord) {
                    continue;
                }
                if rest.iter().enumerate().all(|(k, w)| word_at(from + k) == Some(w.as_str())) && !rest.is_empty() {
                    matched = Some((*no, from + rest.len()));
                    break;
                }
            }
            if matched.is_some() {
                break;
            }
        }
        let Some((book, mut j)) = matched else {
            i += 1;
            continue;
        };
        if word_at(j) == Some("chapter") {
            j += 1;
        }
        let Some(chapter) = num_at(j) else {
            i += 1;
            continue;
        };
        j += 1;
        let mut from = None;
        let mut to = None;
        let mut k = j;
        if matches!(word_at(k), Some(":") | Some("verse") | Some("verses") | Some("vs")) {
            k += 1;
        }
        if let Some(v) = num_at(k) {
            from = Some(v);
            j = k + 1;
            let mut m = j;
            if matches!(word_at(m), Some("to") | Some("through") | Some("thru") | Some("till") | Some("until") | Some("-") | Some("and")) {
                m += 1;
                if word_at(m) == Some("verse") {
                    m += 1;
                }
                if let Some(t) = num_at(m) {
                    if t > v {
                        to = Some(t);
                        j = m + 1;
                    }
                }
            }
        }
        if chapter == 0 || chapter > 150 || from == Some(0) {
            i += 1;
            continue;
        }
        let text_ref = match (from, to) {
            (Some(f), Some(t)) => format!("{} {}:{}-{}", book_display(book), chapter, f, t),
            (Some(f), None) => format!("{} {}:{}", book_display(book), chapter, f),
            _ => format!("{} {}", book_display(book), chapter),
        };
        if reference::parse(&text_ref, &reference::english_aliases()).is_ok() {
            let a = toks[i].1;
            let b = toks.get(j.saturating_sub(1)).map(|t| t.1).unwrap_or(a);
            let heard = spoken(text).split(|c: char| !(c.is_alphanumeric() || c == ':' || c == '-' || c == '\'')).filter(|w| !w.is_empty()).skip(a).take(b - a + 1).collect::<Vec<_>>().join(" ");
            let _ = &words_src;
            out.push(Heard { reference: text_ref, book_no: book, chapter: chapter as u16, verse_from: from.map(|v| v as u16), verse_to: to.map(|v| v as u16), heard });
        }
        i = j.max(i + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(t: &str) -> Vec<String> {
        let mut al = reference::english_aliases();
        al.push(Alias { folded: fold("Yohane"), book_no: 43 });
        al.push(Alias { folded: fold("Nnwom"), book_no: 19 });
        find(t, &al).into_iter().map(|h| h.reference).collect()
    }

    #[test]
    fn spoken_numbers() {
        assert_eq!(refs("turn with me to John three sixteen"), vec!["John 3:16"]);
        assert_eq!(refs("John chapter three verse sixteen"), vec!["John 3:16"]);
        assert_eq!(refs("Psalm twenty three"), vec!["Psalms 23"]);
        assert_eq!(refs("psalm one hundred and nineteen verse one hundred and five"), vec!["Psalms 119:105"]);
        assert_eq!(refs("Romans eight twenty eight"), vec!["Romans 8:28"]);
        assert_eq!(refs("Isaiah forty verse thirty one"), vec!["Isaiah 40:31"]);
    }

    #[test]
    fn numbered_books_and_ranges() {
        assert_eq!(refs("first Corinthians thirteen verses four to seven"), vec!["1 Corinthians 13:4-7"]);
        assert_eq!(refs("second Timothy three sixteen through seventeen"), vec!["2 Timothy 3:16-17"]);
        assert_eq!(refs("one John one nine"), vec!["1 John 1:9"]);
        assert_eq!(refs("1 Peter 5:7"), vec!["1 Peter 5:7"]);
        assert_eq!(refs("John 3:16-18"), vec!["John 3:16-18"]);
    }

    #[test]
    fn local_names_and_several_in_one_sentence() {
        assert_eq!(refs("yɛbɛkenkan Yohane three sixteen"), vec!["John 3:16"]);
        assert_eq!(refs("Nnwom 23 1"), vec!["Psalms 23:1"]);
        assert_eq!(refs("compare Genesis one one with John one one"), vec!["Genesis 1:1", "John 1:1"]);
    }

    #[test]
    fn not_fooled() {
        assert!(refs("you did a good job today").is_empty(), "a book name needs a chapter");
        assert!(refs("Mark my words").is_empty());
        assert!(refs("the first time I read it").is_empty());
        assert!(refs("Acts").is_empty());
        assert_eq!(refs("Job three one"), vec!["Job 3:1"]);
    }
}

// ---------- following the preacher (context) ----------

fn make(book: u16, chapter: u16, from: Option<u16>, to: Option<u16>, heard: &str) -> Option<Heard> {
    let text_ref = match (from, to) {
        (Some(f), Some(t)) if t > f => format!("{} {}:{}-{}", book_display(book), chapter, f, t),
        (Some(f), _) => format!("{} {}:{}", book_display(book), chapter, f),
        _ => format!("{} {}", book_display(book), chapter),
    };
    reference::parse(&text_ref, &reference::english_aliases()).ok()?;
    Some(Heard { reference: text_ref, book_no: book, chapter, verse_from: from, verse_to: to.filter(|t| from.is_some_and(|f| *t > f)), heard: heard.to_string() })
}

/// Book names that are also ordinary words: alone (without a chapter) they don't count.
const COMMON: [&str; 12] = ["job", "mark", "acts", "numbers", "james", "john", "ruth", "jude", "amos", "daniel", "kings", "judges"];

/// Follows the preacher like an operator would: remembers the book, chapter and verse,
/// so "verse seventeen", "verses sixteen to eighteen", "chapter four", "the next verse"
/// and a reference spread over two sentences ("turn to the book of Romans" … "chapter
/// eight verse one") all become suggestions.
#[derive(Debug, Default, Clone)]
pub struct Tracker {
    book: Option<u16>,
    chapter: Option<u16>,
    verse: Option<u16>,
    /// A book named on its own, waiting for its chapter (a few sentences at most).
    pending: Option<(u16, u8)>,
    /// The end of the previous sentence (a reference may straddle two).
    tail: String,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the preacher is now ("John 3:16"), if known.
    pub fn place(&self) -> Option<(u16, u16, Option<u16>)> {
        Some((self.book?, self.chapter?, self.verse))
    }

    fn note(&mut self, h: &Heard) {
        self.book = Some(h.book_no);
        self.chapter = Some(h.chapter);
        self.verse = h.verse_to.or(h.verse_from);
        self.pending = None;
    }

    /// One more piece of transcribed speech; returns the references it adds.
    pub fn feed(&mut self, text: &str, aliases: &[Alias]) -> Vec<Heard> {
        let mut out: Vec<Heard> = vec![];
        // 1. whole references, also across the end of the previous sentence
        let joined = format!("{} {}", self.tail, text);
        let tail_len = tokens(&self.tail).len();
        let toks = tokens(&joined);
        let in_text = find(text, aliases);
        let in_tail = if self.tail.is_empty() { vec![] } else { find(&self.tail, aliases) };
        let tail_l = fold(&self.tail);
        let announced = tail_l.contains("book of") || tail_l.split_whitespace().rev().nth(1).is_some_and(|w| matches!(w, "to" | "in" | "from"));
        let mut full: Vec<Heard> = vec![];
        for h in find(&joined, aliases) {
            if in_tail.iter().any(|e| e.reference == h.reference) && !in_text.iter().any(|e| e.reference == h.reference) {
                continue; // it came from the previous sentence, already reported
            }
            let straddles = !in_text.iter().any(|e| e.reference == h.reference);
            // across two sentences, a book that is also an ordinary word ("a good job" …
            // "chapter two of my life") counts only when it was announced
            let common = COMMON.contains(&book_display(h.book_no).to_lowercase().as_str());
            if straddles && common && !announced {
                continue;
            }
            full.push(h);
        }
        for h in &full {
            if !out.iter().any(|o| o.reference == h.reference) {
                out.push(h.clone());
            }
        }
        if let Some(last) = full.last() {
            self.note(last);
        }
        let words: Vec<String> = toks.iter().map(|(t, _)| match t { Tok::Word(w) => w.clone(), Tok::Num(n) => n.to_string() }).collect();
        let num = |i: usize| -> Option<u32> { match toks.get(i) { Some((Tok::Num(n), _)) => Some(*n), _ => None } };
        let word = |i: usize| -> &str { words.get(i).map(String::as_str).unwrap_or("") };
        // 2. phrases that rely on where we are (only in the new words)
        let names = book_names(aliases);
        let mut i = tail_len;
        while i < toks.len() {
            // a book named on its own ("the book of Romans"), waiting for its chapter
            if full.is_empty() {
                for (ws, no) in &names {
                    let n = ws.len();
                    if n > 0 && i + n <= words.len() && words[i..i + n].iter().zip(ws).all(|(a, b)| a == b) {
                        let led = i >= 2 && word(i - 2) == "book" && word(i - 1) == "of" || i >= 1 && matches!(word(i - 1), "to" | "in" | "from");
                        if led || !(n == 1 && COMMON.contains(&ws[0].as_str())) {
                            if num(i + n).is_none() && !(word(i + n) == "chapter" && num(i + n + 1).is_some()) {
                                self.pending = Some((*no, 3));
                            }
                        }
                    }
                }
            }
            match word(i) {
                // "chapter four (verse two)": in the pending or current book
                "chapter" if num(i + 1).is_some() && !(i >= 1 && names.iter().any(|(ws, _)| ws.last().map(String::as_str) == Some(word(i - 1)))) => {
                    let book = self.pending.map(|p| p.0).or(self.book);
                    if let Some(book) = book {
                        let ch = num(i + 1).unwrap() as u16;
                        let (mut from, mut to) = (None, None);
                        let mut k = i + 2;
                        if word(k) == "and" {
                            k += 1;
                        }
                        if matches!(word(k), "verse" | "verses" | ":") && num(k + 1).is_some() {
                            from = num(k + 1).map(|v| v as u16);
                            k += 2;
                            if matches!(word(k), "to" | "through" | "till" | "until" | "-") && num(k + 1).is_some() {
                                to = num(k + 1).map(|v| v as u16);
                                k += 2;
                            }
                        }
                        if let Some(h) = make(book, ch, from, to, &words[i..k.min(words.len())].join(" ")) {
                            if !out.iter().any(|o| o.reference == h.reference) {
                                self.note(&h);
                                out.push(h);
                            }
                        }
                        i = k;
                        continue;
                    }
                }
                // "verse seventeen", "verses sixteen to eighteen": in the current chapter
                "verse" | "verses" if num(i + 1).is_some() && !(i >= 1 && num(i - 1).is_some()) => {
                    if let (Some(book), Some(ch)) = (self.book, self.chapter) {
                        let from = num(i + 1).map(|v| v as u16);
                        let mut to = None;
                        let mut k = i + 2;
                        if matches!(word(k), "to" | "through" | "till" | "until" | "-" | "and") && num(k + 1).is_some() {
                            to = num(k + 1).map(|v| v as u16);
                            k += 2;
                        }
                        if let Some(h) = make(book, ch, from, to, &words[i..k.min(words.len())].join(" ")) {
                            if !out.iter().any(|o| o.reference == h.reference) {
                                self.note(&h);
                                out.push(h);
                            }
                        }
                        i = k;
                        continue;
                    }
                }
                // "the next verse", "the following verse", "the next chapter"
                "next" | "following" if matches!(word(i + 1), "verse" | "chapter") => {
                    if let (Some(book), Some(ch)) = (self.book, self.chapter) {
                        let h = if word(i + 1) == "verse" { make(book, ch, Some(self.verse.unwrap_or(0) + 1), None, &format!("{} verse", word(i))) } else { make(book, ch + 1, Some(1), None, &format!("{} chapter", word(i))) };
                        if let Some(h) = h {
                            self.note(&h);
                            out.push(h);
                        }
                    }
                    i += 2;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        // a vaguer suggestion gives way to a more precise one for the same chapter
        let precise: Vec<(u16, u16)> = out.iter().filter(|h| h.verse_from.is_some()).map(|h| (h.book_no, h.chapter)).collect();
        out.retain(|h| h.verse_from.is_some() || !precise.contains(&(h.book_no, h.chapter)));
        // a pending book waits for a few sentences only
        if let Some((b, n)) = self.pending {
            self.pending = if n > 1 && out.is_empty() { Some((b, n - 1)) } else if out.is_empty() { None } else { self.pending.filter(|_| false) };
            let _ = b;
        }
        let w: Vec<&str> = text.split_whitespace().collect();
        self.tail = w[w.len().saturating_sub(6)..].join(" ");
        out
    }
}

#[cfg(test)]
mod tracker_tests {
    use super::*;

    fn refs(t: &mut Tracker, s: &str) -> Vec<String> {
        t.feed(s, &[]).into_iter().map(|h| h.reference).collect()
    }

    #[test]
    fn follows_the_preacher() {
        let mut t = Tracker::new();
        assert_eq!(refs(&mut t, "please turn with me to John chapter three"), vec!["John 3"]);
        assert_eq!(refs(&mut t, "and let us read from verse sixteen"), vec!["John 3:16"]);
        assert_eq!(refs(&mut t, "the next verse says"), vec!["John 3:17"]);
        assert_eq!(refs(&mut t, "now look at verses nineteen to twenty one"), vec!["John 3:19-21"]);
        assert_eq!(refs(&mut t, "go to chapter four verse seven"), vec!["John 4:7"]);
        assert_eq!(refs(&mut t, "the next chapter"), vec!["John 5:1"]);
    }

    #[test]
    fn a_reference_over_two_sentences() {
        let mut t = Tracker::new();
        assert!(refs(&mut t, "open your Bibles to the book of Romans").is_empty(), "a book alone waits");
        assert_eq!(refs(&mut t, "chapter eight and verse one"), vec!["Romans 8:1"]);
        let mut t = Tracker::new();
        assert_eq!(refs(&mut t, "we will be reading from first Corinthians"), Vec::<String>::new());
        assert_eq!(refs(&mut t, "chapter thirteen verses four to seven"), vec!["1 Corinthians 13:4-7"]);
    }

    #[test]
    fn ordinary_words_are_not_books() {
        let mut t = Tracker::new();
        assert!(refs(&mut t, "you have done a good job").is_empty());
        assert!(refs(&mut t, "chapter two of my life was hard").is_empty(), "no book yet, so no chapter");
        assert!(refs(&mut t, "verse one of the song").is_empty(), "no chapter yet, so no verse");
        assert_eq!(refs(&mut t, "John three sixteen"), vec!["John 3:16"]);
        assert_eq!(refs(&mut t, "John three sixteen"), vec!["John 3:16"], "said again: suggested again (the chip ignores repeats)");
    }
}
