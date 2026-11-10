//! Canonical book numbering and the scripture reference parser.
//!
//! Book numbers follow Bible Show: 1–66 for the Protestant canon, then 67 Tobit,
//! 68 Judith, 69 Baruch, 73 Wisdom, 74 Sirach, 77 1 Maccabees, 78 2 Maccabees.
//! EasyWorship plug-ins are mapped onto the same numbers by their abbreviations.

use crate::text::fold;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

pub struct Book {
    pub no: u16,
    pub code: &'static str,      // OSIS-style code
    pub ew: &'static str,        // EasyWorship plug-in abbreviation
    pub english: &'static [&'static str], // full name first, then accepted abbreviations
}

macro_rules! b {
    ($no:expr, $code:expr, $ew:expr, [$($n:expr),*]) => {
        Book { no: $no, code: $code, ew: $ew, english: &[$($n),*] }
    };
}

pub const BOOKS: &[Book] = &[
    b!(1, "GEN", "Gn", ["Genesis", "Gen", "Ge", "Gn"]),
    b!(2, "EXO", "Ex", ["Exodus", "Exo", "Exod", "Ex"]),
    b!(3, "LEV", "Lv", ["Leviticus", "Lev", "Lv"]),
    b!(4, "NUM", "Nm", ["Numbers", "Num", "Nm", "Nb"]),
    b!(5, "DEU", "Dt", ["Deuteronomy", "Deut", "Deu", "Dt"]),
    b!(6, "JOS", "Jos", ["Joshua", "Josh", "Jos", "Jsh"]),
    b!(7, "JDG", "Jgs", ["Judges", "Judg", "Jdg", "Jgs", "Jg"]),
    b!(8, "RUT", "Ru", ["Ruth", "Rut", "Ru", "Rth"]),
    b!(9, "1SA", "1 Sm", ["1 Samuel", "1 Sam", "1 Sa", "1 Sm", "1Sam"]),
    b!(10, "2SA", "2 Sm", ["2 Samuel", "2 Sam", "2 Sa", "2 Sm", "2Sam"]),
    b!(11, "1KI", "1 Kgs", ["1 Kings", "1 Kgs", "1 Ki", "1 Kin"]),
    b!(12, "2KI", "2 Kgs", ["2 Kings", "2 Kgs", "2 Ki", "2 Kin"]),
    b!(13, "1CH", "1 Chr", ["1 Chronicles", "1 Chr", "1 Chron", "1 Ch"]),
    b!(14, "2CH", "2 Chr", ["2 Chronicles", "2 Chr", "2 Chron", "2 Ch"]),
    b!(15, "EZR", "Ezr", ["Ezra", "Ezr"]),
    b!(16, "NEH", "Neh", ["Nehemiah", "Neh", "Ne"]),
    b!(17, "EST", "Est", ["Esther", "Est", "Esth"]),
    b!(18, "JOB", "Jb", ["Job", "Jb"]),
    b!(19, "PSA", "Ps", ["Psalms", "Psalm", "Ps", "Psa", "Pss"]),
    b!(20, "PRO", "Prv", ["Proverbs", "Prov", "Pro", "Prv", "Pr"]),
    b!(21, "ECC", "Eccl", ["Ecclesiastes", "Eccl", "Ecc", "Qoh"]),
    b!(22, "SNG", "Sg", ["Song of Solomon", "Song of Songs", "Song", "Sg", "SoS"]),
    b!(23, "ISA", "Is", ["Isaiah", "Isa", "Is"]),
    b!(24, "JER", "Jer", ["Jeremiah", "Jer", "Je"]),
    b!(25, "LAM", "Lam", ["Lamentations", "Lam", "La"]),
    b!(26, "EZK", "Ez", ["Ezekiel", "Ezek", "Eze", "Ezk"]),
    b!(27, "DAN", "Dn", ["Daniel", "Dan", "Dn", "Da"]),
    b!(28, "HOS", "Hos", ["Hosea", "Hos", "Ho"]),
    b!(29, "JOL", "Jl", ["Joel", "Jl", "Joe"]),
    b!(30, "AMO", "Am", ["Amos", "Am", "Amo"]),
    b!(31, "OBA", "Ob", ["Obadiah", "Obad", "Ob"]),
    b!(32, "JON", "Jon", ["Jonah", "Jon", "Jnh"]),
    b!(33, "MIC", "Mi", ["Micah", "Mic", "Mi"]),
    b!(34, "NAM", "Na", ["Nahum", "Nah", "Na"]),
    b!(35, "HAB", "Hb", ["Habakkuk", "Hab", "Hb"]),
    b!(36, "ZEP", "Zep", ["Zephaniah", "Zeph", "Zep"]),
    b!(37, "HAG", "Hg", ["Haggai", "Hag", "Hg"]),
    b!(38, "ZEC", "Zec", ["Zechariah", "Zech", "Zec"]),
    b!(39, "MAL", "Mal", ["Malachi", "Mal"]),
    b!(40, "MAT", "Mt", ["Matthew", "Matt", "Mat", "Mt"]),
    b!(41, "MRK", "Mk", ["Mark", "Mrk", "Mk", "Mar"]),
    b!(42, "LUK", "Lk", ["Luke", "Luk", "Lk"]),
    b!(43, "JHN", "Jn", ["John", "Jhn", "Jn", "Joh"]),
    b!(44, "ACT", "Acts", ["Acts", "Act", "Ac"]),
    b!(45, "ROM", "Rom", ["Romans", "Rom", "Ro", "Rm"]),
    b!(46, "1CO", "1 Cor", ["1 Corinthians", "1 Cor", "1 Co"]),
    b!(47, "2CO", "2 Cor", ["2 Corinthians", "2 Cor", "2 Co"]),
    b!(48, "GAL", "Gal", ["Galatians", "Gal", "Ga"]),
    b!(49, "EPH", "Eph", ["Ephesians", "Eph", "Ep"]),
    b!(50, "PHP", "Phil", ["Philippians", "Phil", "Php", "Pp"]),
    b!(51, "COL", "Col", ["Colossians", "Col", "Co"]),
    b!(52, "1TH", "1 Thes", ["1 Thessalonians", "1 Thess", "1 Thes", "1 Th"]),
    b!(53, "2TH", "2 Thes", ["2 Thessalonians", "2 Thess", "2 Thes", "2 Th"]),
    b!(54, "1TI", "1 Tm", ["1 Timothy", "1 Tim", "1 Tm", "1 Ti"]),
    b!(55, "2TI", "2 Tm", ["2 Timothy", "2 Tim", "2 Tm", "2 Ti"]),
    b!(56, "TIT", "Ti", ["Titus", "Tit", "Ti"]),
    b!(57, "PHM", "Phlm", ["Philemon", "Phlm", "Phm"]),
    b!(58, "HEB", "Heb", ["Hebrews", "Heb", "He"]),
    b!(59, "JAS", "Jas", ["James", "Jas", "Jm"]),
    b!(60, "1PE", "1 Pt", ["1 Peter", "1 Pet", "1 Pt", "1 Pe"]),
    b!(61, "2PE", "2 Pt", ["2 Peter", "2 Pet", "2 Pt", "2 Pe"]),
    b!(62, "1JN", "1 Jn", ["1 John", "1 Jn", "1 Jhn", "1 Jo"]),
    b!(63, "2JN", "2 Jn", ["2 John", "2 Jn", "2 Jhn", "2 Jo"]),
    b!(64, "3JN", "3 Jn", ["3 John", "3 Jn", "3 Jhn", "3 Jo"]),
    b!(65, "JUD", "Jude", ["Jude", "Jud", "Jd"]),
    b!(66, "REV", "Rv", ["Revelation", "Rev", "Rv", "Re", "Revelations"]),
    b!(67, "TOB", "Tb", ["Tobit", "Tob", "Tb"]),
    b!(68, "JDT", "Jdt", ["Judith", "Jdt", "Jdth"]),
    b!(69, "BAR", "Bar", ["Baruch", "Bar"]),
    b!(73, "WIS", "Ws", ["Wisdom of Solomon", "Wisdom", "Wis", "Ws"]),
    b!(74, "SIR", "Sir", ["Sirach", "Ecclesiasticus", "Sir"]),
    b!(77, "1MA", "1 Mc", ["1 Maccabees", "1 Macc", "1 Mc", "1 Ma"]),
    b!(78, "2MA", "2 Mc", ["2 Maccabees", "2 Macc", "2 Mc", "2 Ma"]),
];

pub fn book_by_no(no: u16) -> Option<&'static Book> {
    BOOKS.iter().find(|b| b.no == no)
}

pub fn book_by_ew_abbrev(abbrev: &str) -> Option<&'static Book> {
    let f = fold(abbrev);
    BOOKS.iter().find(|b| fold(b.ew) == f)
}

/// One book name or abbreviation that resolves to a book number. The library
/// builds this list from the canonical English names plus every imported
/// version's own names and abbreviations (Yohane, Gyenesis, Jɛnɛsis ...).
#[derive(Debug, Clone)]
pub struct Alias {
    pub folded: String,
    pub book_no: u16,
}

pub fn english_aliases() -> Vec<Alias> {
    BOOKS
        .iter()
        .flat_map(|b| b.english.iter().map(move |n| Alias { folded: fold(n), book_no: b.no }))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub book_no: u16,
    pub chapter: u16,
    /// None means the whole chapter.
    pub verse_from: Option<u16>,
    /// End of range; may be in a later chapter (John 3:16-4:2).
    pub to_chapter: Option<u16>,
    pub verse_to: Option<u16>,
}

/// Parses "Jn 3:16", "John 3 16-18", "Yohane 3:16", "1 Kor 13", "Ps 23:1-4:2".
/// A typed prefix also matches ("Yoh 3 16"), choosing the shortest matching alias.
pub fn parse(input: &str, aliases: &[Alias]) -> Result<Reference> {
    let raw = input.trim();
    // split into the book part (leading number allowed) and the numeric tail
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    if chars.first().map_or(false, |c| c.is_ascii_digit()) {
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == ' ') {
            i += 1;
        }
    }
    while i < chars.len() && !chars[i].is_ascii_digit() {
        i += 1;
    }
    let book_part: String = chars[..i].iter().collect();
    let tail: String = chars[i..].iter().collect();
    let key = fold(book_part.trim_end_matches(|c: char| c == '.' || c.is_whitespace()));
    if key.is_empty() {
        return Err(Error::Reference(raw.to_string()));
    }
    let key_tight = key.replace(' ', "");
    let book_no = aliases
        .iter()
        .find(|a| a.folded == key || a.folded.replace(' ', "") == key_tight)
        .or_else(|| {
            aliases
                .iter()
                .filter(|a| a.folded.replace(' ', "").starts_with(&key_tight))
                .min_by_key(|a| a.folded.len())
        })
        .map(|a| a.book_no)
        .ok_or_else(|| Error::Reference(raw.to_string()))?;

    let nums: Vec<u16> = tail
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let has_dash = tail.contains('-') || tail.contains('–');
    let colons = tail.matches(|c| c == ':' || c == '.').count();
    let r = match (nums.as_slice(), has_dash) {
        ([c], _) => Reference { book_no, chapter: *c, verse_from: None, to_chapter: None, verse_to: None },
        ([c, v], false) => Reference { book_no, chapter: *c, verse_from: Some(*v), to_chapter: None, verse_to: None },
        ([c, v, e], true) => Reference { book_no, chapter: *c, verse_from: Some(*v), to_chapter: None, verse_to: Some(*e) },
        ([c, v, c2, e], true) if colons >= 2 => {
            Reference { book_no, chapter: *c, verse_from: Some(*v), to_chapter: Some(*c2), verse_to: Some(*e) }
        }
        _ => return Err(Error::Reference(raw.to_string())),
    };
    if r.chapter == 0 || r.verse_from == Some(0) {
        return Err(Error::Reference(raw.to_string()));
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn al() -> Vec<Alias> {
        let mut a = english_aliases();
        a.push(Alias { folded: fold("Yohane"), book_no: 43 });
        a.push(Alias { folded: fold("1 Korintofoɔ"), book_no: 46 });
        a.push(Alias { folded: fold("Nnwom"), book_no: 19 });
        a
    }

    #[test]
    fn english_forms() {
        let a = al();
        assert_eq!(parse("Jn 3:16", &a).unwrap().verse_from, Some(16));
        let r = parse("John 3 16-18", &a).unwrap();
        assert_eq!((r.book_no, r.chapter, r.verse_from, r.verse_to), (43, 3, Some(16), Some(18)));
        assert_eq!(parse("1 Cor 13", &a).unwrap().verse_from, None);
        assert_eq!(parse("1cor 13:4", &a).unwrap().book_no, 46);
        let r = parse("Ps 23:1-24:2", &a).unwrap();
        assert_eq!((r.chapter, r.to_chapter, r.verse_to), (23, Some(24), Some(2)));
    }

    #[test]
    fn local_language_names_and_prefixes() {
        let a = al();
        assert_eq!(parse("Yohane 3:16", &a).unwrap().book_no, 43);
        assert_eq!(parse("Yɔhane 3:16", &a).unwrap().book_no, 43);
        assert_eq!(parse("1 Korintofo 13", &a).unwrap().book_no, 46);
        assert_eq!(parse("Nnwom 23", &a).unwrap().book_no, 19);
        assert_eq!(parse("Gen 1.1", &a).unwrap().verse_from, Some(1));
    }

    #[test]
    fn rejects_nonsense() {
        let a = al();
        assert!(parse("Xyzzy 3:16", &a).is_err());
        assert!(parse("John", &a).is_err());
        assert!(parse("John 0:3", &a).is_err());
    }
}
