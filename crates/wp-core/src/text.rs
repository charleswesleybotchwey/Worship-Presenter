//! Text clean-up applied on import: Unicode NFC plus a small, explicit table of
//! look-alike characters found in real Ghanaian hymn databases. Every change is
//! reported so the importer can log it; nothing is altered silently.

use unicode_normalization::UnicodeNormalization;

/// (found, replacement, reason). Verified against the Methodist hymn database
/// in Phase 0 (31 Oct 2026). Only unambiguous stand-ins are mapped; macrons (ā, ō)
/// and the repeat sign in "3×" are left alone because they can be intentional.
pub const LOOKALIKES: &[(char, &str, &str)] = &[
    ('\u{03B5}', "ɛ", "Greek epsilon used for open e"),
    ('\u{1D10}', "ɔ", "small capital open o used for open o"),
    ('\u{037B}', "ɔ", "Greek reversed lunate sigma used for open o"),
    ('\u{03FD}', "Ɔ", "Greek capital reversed sigma used for capital open o"),
    ('\u{2183}', "Ɔ", "Roman reversed C used for capital open o"),
    ('\u{A4DB}', "Ɔ", "Lisu letter used for capital open o"),
    ('\u{0450}', "è", "Cyrillic ie with grave used for e grave"),
    ('\u{03AD}', "é", "Greek epsilon with tonos used for e acute"),
    ('\u{1F78}', "ò", "Greek omicron with varia used for o grave"),
    ('\u{055D}', "’", "Armenian comma used for apostrophe"),
    ('\u{00AC}', "", "stray not sign left by a legacy font"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub from: char,
    pub to: &'static str,
    pub reason: &'static str,
    pub count: usize,
}

/// Returns the cleaned text (NFC) and the list of look-alike fixes applied.
pub fn clean(input: &str) -> (String, Vec<Fix>) {
    let mut fixes: Vec<Fix> = Vec::new();
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if let Some((from, to, reason)) = LOOKALIKES.iter().find(|(c, _, _)| *c == ch) {
            out.push_str(to);
            match fixes.iter_mut().find(|f| f.from == *from) {
                Some(f) => f.count += 1,
                None => fixes.push(Fix { from: *from, to, reason, count: 1 }),
            }
        } else {
            out.push(ch);
        }
    }
    (out.nfc().collect(), fixes)
}

/// Folds text for matching references and aliases: lower case, accents removed,
/// ɛ→e and ɔ→o so "Yɔhane", "yohane" and "YOHANE" all match.
pub fn fold(input: &str) -> String {
    input
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'ɛ' => 'e',
            'ɔ' => 'o',
            'ŋ' => 'n',
            'ɣ' => 'g',
            'ʒ' => 'z',
            _ => c,
        })
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_lookalikes_and_reports_them() {
        let (t, f) = clean("Ͻyε nkwa nye asomdwee nye¬");
        assert_eq!(t, "Ɔyɛ nkwa nye asomdwee nye");
        assert_eq!(f.len(), 3);
    }

    #[test]
    fn leaves_intentional_marks() {
        let (t, f) = clean("O Nyame, esō; 3× Gbaa mi");
        assert_eq!(t, "O Nyame, esō; 3× Gbaa mi");
        assert!(f.is_empty());
    }

    #[test]
    fn composes_combining_marks() {
        let (t, _) = clean("a\u{0303}");
        assert_eq!(t, "ã");
    }

    #[test]
    fn folds_for_lookup() {
        assert_eq!(fold("Yɔhane"), "yohane");
        assert_eq!(fold("  1  Korintofoɔ "), "1 korintofoo");
        assert_eq!(fold("Ŋmalɛ"), "nmale");
    }
}
