//! Songs out of the app, one or many (as ProPresenter exports them): OpenLyrics (the open
//! standard OpenLP, FreeShow and others read, every language kept), plain text (for printing,
//! WhatsApp or the choir), and file names for one file per song.

use crate::songs::SongDoc;

/// A body's sections in order: ("V1", lines).
fn sections(body: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = vec![];
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') && t.len() > 2 && !t[1..t.len() - 1].contains('[') {
            out.push((t[1..t.len() - 1].trim().to_string(), vec![]));
        } else if !t.is_empty() {
            if out.is_empty() {
                out.push(("V1".into(), vec![]));
            }
            out.last_mut().unwrap().1.push(t.to_string());
        }
    }
    out
}

/// "V1" → "v1", "C" → "c", "PC" → "p" (OpenLyrics' verse names).
fn ol_name(label: &str) -> String {
    let num: String = label.chars().filter(|c| c.is_ascii_digit()).collect();
    let word = label.trim_end_matches(|c: char| c.is_ascii_digit()).to_uppercase();
    let base = match word.as_str() {
        "V" => "v",
        "C" => "c",
        "B" => "b",
        "PC" => "p",
        "T" => "t",
        "E" => "e",
        "I" => "i",
        "O" => "o",
        _ => return label.to_lowercase().replace(' ', ""),
    };
    format!("{base}{num}")
}

/// "V1" → "Verse 1", "C" → "Chorus".
pub fn section_name(label: &str) -> String {
    let num: String = label.chars().filter(|c| c.is_ascii_digit()).collect();
    let word = label.trim_end_matches(|c: char| c.is_ascii_digit()).to_uppercase();
    let name = match word.as_str() {
        "V" => "Verse",
        "C" => "Chorus",
        "B" => "Bridge",
        "PC" => "Pre-chorus",
        "T" => "Tag",
        "E" => "Ending",
        "I" => "Intro",
        "O" => "Outro",
        _ => return label.to_string(),
    };
    if num.is_empty() { name.to_string() } else { format!("{name} {num}") }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A line with chords in brackets ("[G]Come, let us") as OpenLyrics: <chord name="G"/>Come…
fn ol_line(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(i) = rest.find('[') {
        match rest[i..].find(']') {
            Some(j) if j > 1 && j < 12 => {
                out.push_str(&esc(&rest[..i]));
                out.push_str(&format!("<chord name=\"{}\"/>", esc(&rest[i + 1..i + j])));
                rest = &rest[i + j + 1..];
            }
            _ => {
                out.push_str(&esc(&rest[..=i]));
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(&esc(rest));
    out
}

/// One song as OpenLyrics 0.9: every language, numbers, play order, key, authors.
pub fn openlyrics(doc: &SongDoc) -> String {
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<song xmlns=\"http://openlyrics.info/namespace/2009/song\" version=\"0.9\" createdIn=\"Worship Presenter\" modifiedIn=\"Worship Presenter\">\n  <properties>\n");
    x.push_str(&format!("    <titles><title>{}</title></titles>\n", esc(doc.title.trim())));
    if let Some(a) = doc.author.as_deref().filter(|a| !a.trim().is_empty()) {
        x.push_str("    <authors>");
        for one in a.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            x.push_str(&format!("<author>{}</author>", esc(one)));
        }
        x.push_str("</authors>\n");
    }
    if let Some(c) = doc.copyright.as_deref().filter(|c| !c.trim().is_empty()) {
        x.push_str(&format!("    <copyright>{}</copyright>\n", esc(c.trim())));
    }
    if let Some(c) = doc.ccli.as_deref().filter(|c| !c.trim().is_empty()) {
        x.push_str(&format!("    <ccliNo>{}</ccliNo>\n", esc(c.trim())));
    }
    if let Some(k) = doc.key.as_deref().filter(|k| !k.trim().is_empty()) {
        x.push_str(&format!("    <key>{}</key>\n", esc(k.trim())));
    }
    if !doc.sequence.trim().is_empty() {
        let order: Vec<String> = doc.sequence.split_whitespace().map(ol_name).collect();
        x.push_str(&format!("    <verseOrder>{}</verseOrder>\n", order.join(" ")));
    }
    if !doc.numbers.is_empty() {
        x.push_str("    <songbooks>");
        for n in &doc.numbers {
            x.push_str(&format!("<songbook name=\"{}\" entry=\"{}\"/>", esc(&n.hymnal), esc(&n.number)));
        }
        x.push_str("</songbooks>\n");
    }
    x.push_str("  </properties>\n  <lyrics>\n");
    for lyric in &doc.lyrics {
        for (label, lines) in sections(&lyric.body) {
            let body = lines.iter().map(|l| ol_line(l)).collect::<Vec<_>>().join("<br/>");
            x.push_str(&format!("    <verse name=\"{}\" lang=\"{}\"><lines>{}</lines></verse>\n", ol_name(&label), esc(&lyric.lang), body));
        }
    }
    x.push_str("  </lyrics>\n</song>\n");
    x
}

/// One song as plain text: title, numbers and author, then each section with its
/// languages one after the other. `lang_name` turns "fat" into "Fante".
pub fn text(doc: &SongDoc, lang_name: &dyn Fn(&str) -> String) -> String {
    let mut t = format!("{}\n", doc.title.trim());
    let numbers: Vec<String> = doc.numbers.iter().map(|n| format!("{} {}", n.hymnal, n.number)).collect();
    let by: Vec<String> = numbers.into_iter().chain(doc.author.clone().filter(|a| !a.trim().is_empty())).collect();
    if !by.is_empty() {
        t.push_str(&format!("{}\n", by.join(" · ")));
    }
    if !doc.sequence.trim().is_empty() {
        t.push_str(&format!("Order: {}\n", doc.sequence.trim()));
    }
    let langs: Vec<(String, Vec<(String, Vec<String>)>)> = doc.lyrics.iter().map(|l| (l.lang.clone(), sections(&l.body))).collect();
    let several = langs.len() > 1;
    if let Some((_, first)) = langs.first() {
        for (label, lines) in first {
            t.push_str(&format!("\n{}\n", section_name(label)));
            t.push_str(&strip_chords(lines).join("\n"));
            t.push('\n');
            for (lang, secs) in langs.iter().skip(1) {
                if let Some((_, other)) = secs.iter().find(|(l, _)| l == label) {
                    t.push_str(&format!("\n({})\n", lang_name(lang)));
                    t.push_str(&strip_chords(other).join("\n"));
                    t.push('\n');
                }
            }
        }
    }
    if several {
        let names: Vec<String> = langs.iter().map(|(l, _)| lang_name(l)).collect();
        t.push_str(&format!("\nLanguages: {}\n", names.join(", ")));
    }
    if let Some(c) = doc.copyright.as_deref().filter(|c| !c.trim().is_empty()) {
        t.push_str(&format!("{}\n", c.trim()));
    }
    t
}

fn strip_chords(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|l| {
            let mut out = String::new();
            let mut skip = false;
            for c in l.chars() {
                match c {
                    '[' => skip = true,
                    ']' if skip => skip = false,
                    _ if !skip => out.push(c),
                    _ => {}
                }
            }
            out.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect()
}

/// A file name from a song's title (characters Windows refuses taken out).
pub fn file_name(title: &str) -> String {
    let n: String = title.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let n = n.trim().trim_end_matches('.').trim();
    let n: String = n.chars().take(120).collect();
    if n.is_empty() { "Song".into() } else { n }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::songs::{Lyric, Number};

    fn hymn() -> SongDoc {
        SongDoc {
            id: None,
            title: "MHB 1, ENG&FANTE".into(),
            author: Some("Charles Wesley".into()),
            copyright: Some("Public Domain".into()),
            ccli: None,
            numbers: vec![Number { hymnal: "MHB".into(), number: "1".into() }],
            lyrics: vec![
                Lyric { lang: "en".into(), body: "[V1]\n[G]O for a thousand tongues to sing\nMy great Redeemer's praise\n\n[C]\nGlory & praise".into() },
                Lyric { lang: "fat".into(), body: "[V1]\nMeenya ngyiramá apem matow ndwom\nMeyi mo Pomfò ayɛw\n\n[C]\nAnyimnyam".into() },
            ],
            sequence: "V1 C V1 C".into(),
            lines_per_slide: None,
            key: Some("G".into()),
        }
    }

    #[test]
    fn openlyrics_reads_back_with_every_language() {
        let x = openlyrics(&hymn());
        assert!(x.contains("<chord name=\"G\"/>O for") && x.contains("Glory &amp; praise"));
        let back = crate::import::songfiles::parse_openlyrics(&x).unwrap();
        assert_eq!(back.title, "MHB 1, ENG&FANTE");
        assert_eq!(back.lyrics.iter().map(|l| l.lang.as_str()).collect::<Vec<_>>(), vec!["en", "fat"], "both languages");
        assert!(back.lyrics[1].body.contains("Meyi mo Pomfò ayɛw"));
        assert!(back.lyrics[0].body.contains("[G]O for a thousand"), "chords come back");
        assert_eq!((back.sequence.as_str(), back.key.as_deref(), back.author.as_deref()), ("V1 C V1 C", Some("G"), Some("Charles Wesley")));
        assert_eq!((back.numbers[0].hymnal.as_str(), back.numbers[0].number.as_str()), ("MHB", "1"));
    }

    #[test]
    fn text_has_each_section_in_both_languages() {
        let t = text(&hymn(), &|l: &str| if l == "fat" { "Fante".into() } else { "English".into() });
        assert!(t.starts_with("MHB 1, ENG&FANTE\nMHB 1 · Charles Wesley\nOrder: V1 C V1 C\n"));
        assert!(t.contains("\nVerse 1\nO for a thousand tongues to sing\nMy great Redeemer's praise\n\n(Fante)\nMeenya"), "chords left out, the Fante under the English");
        assert!(t.contains("\nChorus\n"));
        assert_eq!(file_name("Who? Me: \"Yes\"/No"), "Who_ Me_ _Yes__No");
    }
}
