//! Builds projected slides from a song's sections and aligned segments.
//!
//! A "line" here is one align group: the primary-language line plus whatever
//! second-language lines share its align_key. Lines per slide counts groups, so
//! a bilingual 2-line slide shows two English lines with their Fante under or
//! beside them, even where the translation has more or fewer lines.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub lang: String,
    pub text: String,
    pub align_key: i64,
    /// The line with inline chords, for the musicians' stage view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chords: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: i64,
    pub label: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LangMode {
    Primary,
    Second,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    pub lines_per_slide: u8,
    pub lang_mode: LangMode,
    pub primary: String,
    pub second: Option<String>,
    /// Section labels in play order; empty means source order.
    pub sequence: Vec<String>,
    pub include_title: bool,
}

/// Where the operator is inside a song, independent of how it is split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Index into the expanded sequence (the second chorus is a different occurrence).
    pub occurrence: usize,
    pub align_key: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Slide {
    pub label: String,
    pub occurrence: usize,
    pub first_key: i64,
    pub last_key: i64,
    pub primary: Vec<String>,
    pub second: Vec<String>,
    /// A picture slide (announcements): path in the media library.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Primary lines with their chords (same order as `primary`; "" where a line has
    /// none). Empty when the song has no chords. Only the stage view uses this.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chords: Vec<String>,
}

pub fn split(sections: &[Section], o: &Options) -> Vec<Slide> {
    let lps = o.lines_per_slide.clamp(1, 8) as usize;
    let order: Vec<&Section> = if o.sequence.is_empty() {
        sections.iter().filter(|s| o.include_title || s.label != "TITLE").collect()
    } else {
        o.sequence.iter().filter_map(|l| sections.iter().find(|s| &s.label == l)).collect()
    };
    let show_primary = o.lang_mode != LangMode::Second;
    let show_second = o.lang_mode != LangMode::Primary;
    let mut out = Vec::new();
    for (occ, sec) in order.iter().enumerate() {
        let mut keys: Vec<i64> = sec.segments.iter().map(|g| g.align_key).collect();
        keys.sort_unstable();
        keys.dedup();
        // keep only groups that have something visible in the chosen mode
        let keys: Vec<i64> = keys
            .into_iter()
            .filter(|k| {
                sec.segments.iter().any(|g| {
                    g.align_key == *k
                        && ((show_primary && g.lang == o.primary) || (show_second && Some(&g.lang) == o.second.as_ref()))
                })
            })
            .collect();
        for chunk in keys.chunks(lps) {
            let pick = |lang: Option<&String>, on: bool| -> Vec<String> {
                if !on {
                    return Vec::new();
                }
                let Some(lang) = lang else { return Vec::new() };
                sec.segments.iter().filter(|g| &g.lang == lang && chunk.contains(&g.align_key)).map(|g| g.text.clone()).collect()
            };
            // the musicians' chords for the first language's lines (always, whatever
            // the congregation is shown)
            let chord_lines: Vec<String> = sec.segments.iter().filter(|g| g.lang == o.primary && chunk.contains(&g.align_key)).map(|g| g.chords.clone().unwrap_or_default()).collect();
            let chords = if chord_lines.iter().any(|c| !c.is_empty()) { chord_lines } else { Vec::new() };
            out.push(Slide {
                label: sec.label.clone(),
                occurrence: occ,
                first_key: chunk[0],
                last_key: *chunk.last().expect("chunk not empty"),
                primary: pick(Some(&o.primary), show_primary),
                second: pick(o.second.as_ref(), show_second),
                image: None,
                chords,
            });
        }
    }
    out
}

/// Index of the slide that holds `pos`, so changing lines per slide or language
/// mode keeps the same words on screen.
pub fn find(slides: &[Slide], pos: Position) -> usize {
    slides
        .iter()
        .position(|s| s.occurrence == pos.occurrence && s.first_key <= pos.align_key && pos.align_key <= s.last_key)
        .or_else(|| slides.iter().position(|s| s.occurrence == pos.occurrence && s.first_key >= pos.align_key))
        .or_else(|| slides.iter().rposition(|s| s.occurrence <= pos.occurrence))
        .unwrap_or(0)
}

pub fn position_of(slide: &Slide) -> Position {
    Position { occurrence: slide.occurrence, align_key: slide.first_key }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(lang: &str, t: &str, k: i64) -> Segment {
        Segment { lang: lang.into(), text: t.into(), align_key: k, chords: None }
    }

    fn hymn() -> Vec<Section> {
        vec![
            Section { id: 1, label: "TITLE".into(), segments: vec![seg("en", "MHB 1", 0)] },
            Section {
                id: 2,
                label: "V1".into(),
                segments: vec![
                    seg("en", "a1", 1), seg("en", "a2", 2), seg("en", "a3", 3), seg("en", "a4", 4),
                    seg("fat", "f1", 1), seg("fat", "f2", 3), seg("fat", "f3", 4),
                ],
            },
            Section { id: 3, label: "C".into(), segments: vec![seg("en", "c1", 5), seg("en", "c2", 6), seg("fat", "g1", 5)] },
        ]
    }

    fn opts(lps: u8, mode: LangMode) -> Options {
        Options { lines_per_slide: lps, lang_mode: mode, primary: "en".into(), second: Some("fat".into()), sequence: vec![], include_title: false }
    }

    #[test]
    fn two_lines_bilingual() {
        let s = split(&hymn(), &opts(2, LangMode::Both));
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].primary, vec!["a1", "a2"]);
        assert_eq!(s[0].second, vec!["f1"]);
        assert_eq!(s[1].second, vec!["f2", "f3"]);
    }

    #[test]
    fn one_line_and_resplit_keeps_place() {
        let two = split(&hymn(), &opts(2, LangMode::Primary));
        let pos = position_of(&two[1]); // a3 a4
        let one = split(&hymn(), &opts(1, LangMode::Primary));
        assert_eq!(one.len(), 6);
        assert_eq!(one[find(&one, pos)].primary, vec!["a3"]);
        let back = split(&hymn(), &opts(2, LangMode::Primary));
        assert_eq!(back[find(&back, position_of(&one[3]))].primary, vec!["a3", "a4"]);
    }

    #[test]
    fn sequence_repeats_chorus_as_new_occurrence() {
        let mut o = opts(2, LangMode::Primary);
        o.sequence = vec!["V1".into(), "C".into(), "C".into()];
        let s = split(&hymn(), &o);
        assert_eq!(s.len(), 4);
        assert_eq!((s[2].occurrence, s[3].occurrence), (1, 2));
    }

    #[test]
    fn second_language_only_skips_empty_groups() {
        let s = split(&hymn(), &opts(1, LangMode::Second));
        assert_eq!(s.iter().map(|x| x.second.join("")).collect::<Vec<_>>(), vec!["f1", "f2", "f3", "g1"]);
    }
}
