//! Finding a song's words online: Hymnary.org (the hymn library; full texts of public-domain
//! hymns) and LRCLIB (an open lyrics database with a public interface for apps). This
//! module reads their answers; the app makes the requests.
//!
//! Projecting copyrighted songs still needs the church's CCLI (or similar) licence.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Found {
    /// "Hymnary" or "LRCLIB"
    pub source: String,
    /// What to ask the source for the words (a Hymnary text id, an LRCLIB id).
    pub key: String,
    pub title: String,
    pub author: String,
    /// The words already, when the search gives them (LRCLIB).
    pub words: Option<String>,
}

// ---------- Hymnary.org

fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                out.last_mut().unwrap().push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(String::new()),
            _ => out.last_mut().unwrap().push(c),
        }
    }
    out
}

/// Hymnary's search export (CSV): title, …, text id, …, author (columns found by name).
pub fn hymnary_results(csv: &str) -> Vec<Found> {
    let mut rows = csv.lines().filter(|l| !l.trim().is_empty()).map(split_csv_line);
    let Some(head) = rows.next() else { return vec![] };
    let col = |names: &[&str]| head.iter().position(|h| names.iter().any(|n| h.trim().eq_ignore_ascii_case(n)));
    let title = col(&["displayTitle", "title", "textTitle"]).unwrap_or(0);
    let key = col(&["textAuthNumber", "text id", "textID", "id"]).unwrap_or(4);
    let author = col(&["authors", "author", "textAuthors"]).unwrap_or(6);
    rows.filter(|r| r.len() > key && !r[key].trim().is_empty())
        .map(|r| Found { source: "Hymnary".into(), key: r[key].trim().to_string(), title: r[title].trim().to_string(), author: r.get(author).map(|a| a.trim().to_string()).unwrap_or_default(), words: None })
        .take(8)
        .collect()
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#039;", "'").replace("&#39;", "'").replace("&rsquo;", "’").replace("&lsquo;", "‘").replace("&nbsp;", " ")
}

/// The words on a Hymnary text page (the block marked property="text"), one stanza per
/// paragraph, with the verse numbers Hymnary puts in front taken off.
pub fn hymnary_words(html: &str) -> Option<String> {
    let start = html.find("property=\"text\"").or_else(|| html.find("property='text'"))?;
    let body_start = html[start..].find('>')? + start + 1;
    let end = html[body_start..].find("</div>")? + body_start;
    let block = &html[body_start..end];
    let mut stanzas: Vec<String> = vec![];
    for para in block.split("</p>") {
        let text = para.replace("<br />", "\n").replace("<br/>", "\n").replace("<br>", "\n");
        // drop the remaining tags
        let mut plain = String::new();
        let mut in_tag = false;
        for c in text.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => plain.push(c),
                _ => {}
            }
        }
        let lines: Vec<String> = unescape(&plain).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        let mut lines = lines;
        // "1 Amazing grace…" → "Amazing grace…"
        let first = lines[0].trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches(['.', ' ']).to_string();
        lines[0] = first;
        stanzas.push(lines.join("\n"));
    }
    (!stanzas.is_empty()).then(|| stanzas.join("\n\n"))
}

// ---------- LRCLIB

/// LRCLIB's search answer (JSON): songs with their plain words.
pub fn lrclib_results(json: &str) -> Vec<Found> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    v.as_array()
        .map(|a| {
            a.iter()
                .filter(|s| s["instrumental"].as_bool() != Some(true))
                .filter_map(|s| {
                    let words = s["plainLyrics"].as_str().filter(|w| !w.trim().is_empty())?;
                    Some(Found {
                        source: "LRCLIB".into(),
                        key: s["id"].as_i64().map(|i| i.to_string()).unwrap_or_default(),
                        title: s["trackName"].as_str().unwrap_or("").to_string(),
                        author: s["artistName"].as_str().unwrap_or("").to_string(),
                        words: Some(words.replace("\r\n", "\n")),
                    })
                })
                .take(8)
                .collect()
        })
        .unwrap_or_default()
}

// ---------- into a song

/// Stanzas (blank-line separated) into the song editor's sections: a stanza marked
/// "Chorus" / "Refrain", or one that comes back word for word, is the chorus (C);
/// the others are verses V1, V2…
pub fn to_song_body(words: &str) -> String {
    let stanzas: Vec<Vec<String>> = words
        .replace("\r\n", "\n")
        .split("\n\n")
        .map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect::<Vec<_>>())
        .filter(|s| !s.is_empty())
        .collect();
    let label_of = |s: &[String]| -> Option<&'static str> {
        let first = s[0].to_lowercase();
        let first = first.trim_matches(|c: char| c == '[' || c == ']' || c == ':' || c == '(' || c == ')');
        if first.starts_with("chorus") || first.starts_with("refrain") {
            Some("C")
        } else if first.starts_with("bridge") {
            Some("B")
        } else {
            None
        }
    };
    let mut out = String::new();
    let mut verse = 0;
    let mut chorus: Option<Vec<String>> = None;
    for s in &stanzas {
        let (label, lines): (String, Vec<String>) = match label_of(s) {
            Some(l) => (l.to_string(), s[1..].to_vec()),
            None if chorus.as_ref() == Some(s) => ("C".to_string(), s.clone()),
            None if stanzas.iter().filter(|o| *o == s).count() > 1 => ("C".to_string(), s.clone()),
            None => {
                verse += 1;
                (format!("V{verse}"), s.clone())
            }
        };
        if lines.is_empty() {
            continue;
        }
        if label == "C" {
            if chorus.is_some() {
                continue; // the chorus once; the song's order repeats it
            }
            chorus = Some(lines.clone());
        }
        out.push_str(&format!("[{label}]\n{}\n\n", lines.join("\n")));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hymnary_search_and_text() {
        let csv = "displayTitle,textFirstLine,textRefrainFirstLine,meter,textAuthNumber,textAuthNumberLink,authors\n\
                   \"Amazing grace! (how sweet the sound)\",Amazing grace! (how sweet the sound),,C M,amazing_grace_how_sweet_the_sound,x,\"John Newton\"\n\
                   \"O for a thousand tongues to sing\",O for a thousand tongues to sing,,C M,o_for_a_thousand_tongues_to_sing,x,\"Charles Wesley\"";
        let r = hymnary_results(csv);
        assert_eq!(r.len(), 2);
        assert_eq!((r[1].key.as_str(), r[1].author.as_str()), ("o_for_a_thousand_tongues_to_sing", "Charles Wesley"));
        let html = r#"<div class="x"><div property="text"><p>1 O for a thousand tongues to sing<br />
my great Redeemer&#039;s praise,</p><p>2 My gracious Master and my God,<br />assist me to proclaim,</p></div></div>"#;
        let w = hymnary_words(html).unwrap();
        assert_eq!(w, "O for a thousand tongues to sing\nmy great Redeemer's praise,\n\nMy gracious Master and my God,\nassist me to proclaim,");
        assert_eq!(hymnary_words("<html>no text here</html>"), None);
    }

    #[test]
    fn lrclib_search() {
        let json = r#"[{"id":123,"trackName":"Way Maker","artistName":"Sinach","instrumental":false,"plainLyrics":"You are here, moving in our midst\nI worship You\n\nWay maker, miracle worker\nPromise keeper"},
                      {"id":124,"trackName":"Instrumental","artistName":"X","instrumental":true,"plainLyrics":""}]"#;
        let r = lrclib_results(json);
        assert_eq!(r.len(), 1, "instrumentals left out");
        assert_eq!((r[0].title.as_str(), r[0].author.as_str(), r[0].key.as_str()), ("Way Maker", "Sinach", "123"));
        assert!(r[0].words.as_ref().unwrap().contains("Way maker"));
        assert!(lrclib_results("not json").is_empty());
    }

    #[test]
    fn words_become_sections() {
        let words = "You are here\nmoving in our midst\n\nWay maker, miracle worker\nPromise keeper\n\nYou are here\ntouching every heart\n\nWay maker, miracle worker\nPromise keeper";
        assert_eq!(to_song_body(words), "[V1]\nYou are here\nmoving in our midst\n\n[C]\nWay maker, miracle worker\nPromise keeper\n\n[V2]\nYou are here\ntouching every heart");
        let marked = "Verse one line\n\nRefrain:\nAnd can it be\nthat I should gain";
        assert_eq!(to_song_body(marked), "[V1]\nVerse one line\n\n[C]\nAnd can it be\nthat I should gain");
        // the editor reads it back as sections
        let parsed = crate::songs::parse_body(&to_song_body(words));
        assert_eq!(parsed.len(), 3);
    }
}
