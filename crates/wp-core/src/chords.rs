//! Chords written inline in lyrics, ChordPro style: "[G]Amazing [C]grace".
//! The congregation's screens, search and slide lists get the words only; the
//! musicians' stage view gets the line with its chords (and can transpose them).
//! A line that is only "[Label]" is still a section header, as before.

/// True for chord names: G, F#m, Bb7, Csus4, Dmaj7, G/B, Am7b5, E7#9 …
pub fn is_chord(s: &str) -> bool {
    let mut parts = s.splitn(2, '/');
    let main = parts.next().unwrap_or("");
    let bass_ok = parts.next().map_or(true, |b| root_len(b) == b.len() && !b.is_empty());
    let r = root_len(main);
    if r == 0 || !bass_ok {
        return false;
    }
    let rest = &main[r..];
    // quality and extensions, from a small vocabulary
    let mut t = rest;
    for q in ["maj", "min", "sus", "dim", "aug", "add", "m", "M", "+", "°", "ø"] {
        if let Some(x) = t.strip_prefix(q) {
            t = x;
            break;
        }
    }
    t.chars().all(|c| c.is_ascii_digit() || matches!(c, '#' | 'b' | '(' | ')' | '+' | '-') || "susaddmajdimaug".contains(c))
}

fn root_len(s: &str) -> usize {
    let b = s.as_bytes();
    if b.is_empty() || !(b'A'..=b'G').contains(&b[0]) {
        return 0;
    }
    if b.len() > 1 && (b[1] == b'#' || b[1] == b'b') {
        2
    } else {
        1
    }
}

/// Splits a lyric line into its words and, when it has chords, the original line.
pub fn split_line(line: &str) -> (String, Option<String>) {
    if !line.contains('[') {
        return (line.to_string(), None);
    }
    let mut clean = String::new();
    let mut found = false;
    let mut rest = line;
    while let Some(a) = rest.find('[') {
        clean.push_str(&rest[..a]);
        let after = &rest[a + 1..];
        match after.find(']') {
            Some(b) if is_chord(after[..b].trim()) => {
                found = true;
                rest = &after[b + 1..];
            }
            _ => {
                clean.push('[');
                rest = after;
            }
        }
    }
    clean.push_str(rest);
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    (clean, found.then(|| line.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_are_told_apart_from_words() {
        for c in ["G", "F#m", "Bb7", "Csus4", "Dmaj7", "G/B", "Am7b5", "E7#9", "Cadd9", "Ebm"] {
            assert!(is_chord(c), "{c}");
        }
        for w in ["Verse", "Amen", "x2", "Chorus", "H", "", "God"] {
            assert!(!is_chord(w), "{w}");
        }
    }

    #[test]
    fn lines_keep_words_and_chords_apart() {
        assert_eq!(split_line("[G]Amazing [C]grace how [G]sweet"), ("Amazing grace how sweet".into(), Some("[G]Amazing [C]grace how [G]sweet".into())));
        assert_eq!(split_line("Onyankopɔn [D]ye"), ("Onyankopɔn ye".into(), Some("Onyankopɔn [D]ye".into())));
        assert_eq!(split_line("Sing [x2] again"), ("Sing [x2] again".into(), None), "brackets that are not chords stay words");
        assert_eq!(split_line("No chords here"), ("No chords here".into(), None));
    }
}
