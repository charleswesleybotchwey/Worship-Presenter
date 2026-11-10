//! PowerPoint and PDF into Presentations (spec: three ways for PowerPoint).
//!
//! 1. PowerPoint installed (Windows): PowerPoint exports every slide as a picture.
//! 2. LibreOffice installed: it converts the file to PDF; the PDF's pages become pictures.
//! 3. Neither: the .pptx is read here; each slide keeps its title and text, and the
//!    import says per slide what was left out (pictures, charts, tables, video).
//!
//! PDF pages are drawn by the window (pdf.js) and handed back as pictures. In every
//! case the result is a deck of slides in Presentations, like announcements.

use crate::announcements::{Deck, DeckSlide};
use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Presentation".into(), "presentation", msg.into())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TextSlide {
    pub title: String,
    pub lines: Vec<String>,
    /// What could not be kept, e.g. "2 pictures", "a chart".
    pub lost: Vec<String>,
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Text runs of each paragraph inside a piece of DrawingML.
fn paragraphs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for para in xml.split("<a:p>").skip(1).chain(xml.split("<a:p ").skip(1)) {
        let end = para.find("</a:p>").unwrap_or(para.len());
        let para = &para[..end];
        let mut line = String::new();
        let mut rest = para;
        while let Some(a) = rest.find("<a:t") {
            let after = &rest[a..];
            let Some(gt) = after.find('>') else { break };
            if after[..gt].ends_with('/') {
                rest = &after[gt + 1..];
                continue;
            }
            let body = &after[gt + 1..];
            let Some(close) = body.find("</a:t>") else { break };
            line.push_str(&unescape(&body[..close]));
            rest = &body[close..];
        }
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let k = format!("{name}=\"");
    let a = tag.find(&k)? + k.len();
    let b = tag[a..].find('"')?;
    Some(&tag[a..a + b])
}

/// Reads a .pptx: slides in presentation order, each with its title, text lines and
/// what was left out.
pub fn read_pptx(path: &Path) -> Result<Vec<TextSlide>> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| bad("This is not a PowerPoint .pptx file (or it is damaged). Old .ppt files need PowerPoint or LibreOffice."))?;
    let pres = read_entry(&mut zip, "ppt/presentation.xml").ok_or_else(|| bad("The file has no slides list."))?;
    let rels = read_entry(&mut zip, "ppt/_rels/presentation.xml.rels").unwrap_or_default();
    let mut targets: HashMap<String, String> = HashMap::new();
    for rel in rels.split("<Relationship ").skip(1) {
        if let (Some(id), Some(t)) = (attr(rel, "Id"), attr(rel, "Target")) {
            targets.insert(id.to_string(), t.trim_start_matches('/').trim_start_matches("ppt/").to_string());
        }
    }
    let mut order = Vec::new();
    for sld in pres.split("<p:sldId ").skip(1) {
        if let Some(t) = attr(sld, "r:id").and_then(|id| targets.get(id)) {
            order.push(format!("ppt/{t}"));
        }
    }
    let mut out = Vec::new();
    for name in order {
        let Some(xml) = read_entry(&mut zip, &name) else { continue };
        if xml.contains("show=\"0\"") && xml.find("<p:sld ").map_or(false, |i| xml[i..].split('>').next().unwrap_or("").contains("show=\"0\"")) {
            continue; // hidden slide
        }
        let mut title = String::new();
        let mut lines = Vec::new();
        for sp in xml.split("<p:sp>").skip(1).chain(xml.split("<p:sp ").skip(1)) {
            let sp = &sp[..sp.find("</p:sp>").unwrap_or(sp.len())];
            let paras = paragraphs(sp);
            let is_title = sp.contains("type=\"title\"") || sp.contains("type=\"ctrTitle\"");
            if is_title && title.is_empty() {
                title = paras.join(" ");
            } else if !sp.contains("type=\"sldNum\"") && !sp.contains("type=\"dt\"") && !sp.contains("type=\"ftr\"") {
                lines.extend(paras);
            }
        }
        // tables keep their text, a cell per line
        let has_table = xml.contains("<a:tbl>");
        if has_table {
            for cell in xml.split("<a:tc>").skip(1) {
                lines.extend(paragraphs(&cell[..cell.find("</a:tc>").unwrap_or(cell.len())]));
            }
        }
        let mut lost = Vec::new();
        let pics = xml.matches("<p:pic>").count() + xml.matches("<p:pic ").count();
        let video = xml.contains("<a:videoFile") || xml.contains("<p14:media");
        let charts = xml.matches("/chart\"").count() + xml.matches("drawingml/2006/chart").count();
        if pics > 0 {
            lost.push(if pics == 1 { "1 picture".into() } else { format!("{pics} pictures") });
        }
        if charts > 0 {
            lost.push("a chart".into());
        }
        if has_table {
            lost.push("the table layout (its text is kept)".into());
        }
        if video {
            lost.push("a video".into());
        }
        if xml.contains("<p:bg>") || xml.contains("<a:blipFill") && pics == 0 {
            lost.push("the background".into());
        }
        out.push(TextSlide { title, lines, lost });
    }
    if out.is_empty() {
        return Err(bad("No slides were found in this file."));
    }
    Ok(out)
}

/// PowerShell that has PowerPoint export each slide as a 1920×1080 PNG
/// (Slide001.png …) into `out_dir`. PowerPoint stays hidden and is closed after.
pub fn powerpoint_script(src: &Path, out_dir: &Path) -> String {
    let q = |p: &Path| p.display().to_string().replace('\'', "''");
    format!(
        "$ErrorActionPreference='Stop'; $app = New-Object -ComObject PowerPoint.Application; \
         try {{ $p = $app.Presentations.Open('{}', $true, $false, $false); \
         $i = 0; foreach ($s in $p.Slides) {{ if (-not $s.SlideShowTransition.Hidden) {{ $i++; $s.Export((Join-Path '{}' ('Slide{{0:D3}}.png' -f $i)), 'PNG', 1920, 1080) }} }}; \
         $p.Close() }} finally {{ $app.Quit() }}",
        q(src),
        q(out_dir)
    )
}

/// Where LibreOffice usually lives, in the order to try.
pub fn soffice_candidates() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("soffice")];
    if cfg!(windows) {
        for base in ["C:\\Program Files\\LibreOffice\\program", "C:\\Program Files (x86)\\LibreOffice\\program"] {
            v.insert(0, PathBuf::from(base).join("soffice.exe"));
        }
    }
    v
}

/// Saves page pictures into the media library and makes a deck of picture slides.
pub fn deck_from_pictures(conn: &Connection, media_dir: &Path, title: &str, pictures: &[(String, Vec<u8>)]) -> Result<i64> {
    if pictures.is_empty() {
        return Err(bad("No pages were made."));
    }
    let slug: String = crate::profiles::slugify(title);
    let mut sub = format!("images/presentations/{slug}");
    let mut n = 2;
    while media_dir.join(&sub).exists() {
        sub = format!("images/presentations/{slug}-{n}");
        n += 1;
    }
    std::fs::create_dir_all(media_dir.join(&sub))?;
    let tx = conn.unchecked_transaction()?;
    let mut slides = Vec::new();
    for (i, (ext, bytes)) in pictures.iter().enumerate() {
        let rel = format!("{sub}/{:03}.{ext}", i + 1);
        std::fs::write(media_dir.join(&rel), bytes)?;
        tx.execute("INSERT INTO media (kind, path, title, tags) VALUES ('image', ?1, ?2, 'presentation')", params![rel, format!("{title} – slide {}", i + 1)])?;
        slides.push(DeckSlide { kind: "picture".into(), title: format!("Slide {}", i + 1), body: String::new(), image: Some(rel), seconds: None });
    }
    tx.commit()?;
    // the deck is saved in its own transaction (announcements::save opens one)
    crate::announcements::save(conn, &Deck { id: None, title: title.to_string(), looping: false, seconds: 0, slides })
}

/// A deck of text slides from a .pptx read here (the third way).
pub fn deck_from_text(conn: &Connection, title: &str, slides: &[TextSlide]) -> Result<i64> {
    let deck = Deck {
        id: None,
        title: title.to_string(),
        looping: false,
        seconds: 0,
        slides: slides
            .iter()
            .enumerate()
            .map(|(i, s)| DeckSlide {
                kind: "text".into(),
                title: if s.title.is_empty() && s.lines.is_empty() { format!("Slide {}", i + 1) } else { s.title.clone() },
                body: s.lines.join("\n"),
                image: None,
                seconds: None,
            })
            .collect(),
    };
    crate::announcements::save(conn, &deck)
}

/// One line per slide that lost something, for the import report.
pub fn loss_report(slides: &[TextSlide]) -> Vec<String> {
    slides
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.lost.is_empty())
        .map(|(i, s)| format!("Slide {}: {} left out", i + 1, s.lost.join(", ")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_and_entities() {
        let xml = r#"<p:txBody><a:p><a:r><a:t>Tithes &amp; </a:t></a:r><a:r><a:t>offering</a:t></a:r></a:p><a:p><a:endParaRPr/></a:p><a:p><a:r><a:t>MoMo: 024 555 1234</a:t></a:r></a:p></p:txBody>"#;
        assert_eq!(paragraphs(xml), vec!["Tithes & offering", "MoMo: 024 555 1234"]);
    }

    #[test]
    fn decks_from_pictures_and_text() {
        let dir = std::env::temp_dir().join(format!("wp-pres-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = crate::db::open_memory().unwrap();
        let id = deck_from_pictures(&c, &dir, "Harvest 2026", &[("jpg".into(), vec![1, 2, 3]), ("jpg".into(), vec![4, 5])]).unwrap();
        let d = crate::announcements::get(&c, id).unwrap();
        assert_eq!(d.slides.len(), 2);
        assert_eq!(d.slides[1].image.as_deref(), Some("images/presentations/harvest-2026/002.jpg"));
        assert!(dir.join("images/presentations/harvest-2026/002.jpg").exists());
        let n: i64 = c.query_row("SELECT count(*) FROM media WHERE tags = 'presentation'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
        let t = deck_from_text(&c, "Notices", &[TextSlide { title: "Welcome".into(), lines: vec!["Akwaaba".into()], lost: vec!["1 picture".into()] }]).unwrap();
        assert_eq!(crate::announcements::get(&c, t).unwrap().slides[0].body, "Akwaaba");
    }

    #[test]
    fn real_pptx_when_available() {
        // made by tests/make_sample_pptx.py; skipped when absent
        let Some(p) = std::env::var_os("WP_PPTX").map(PathBuf::from) else { return };
        let slides = read_pptx(&p).unwrap();
        assert_eq!(slides.len(), 3, "the hidden 4th slide is left out");
        assert_eq!(slides[0].title, "Harvest Thanksgiving");
        assert!(slides[1].lines.iter().any(|l| l.contains("024 555 1234")));
        assert!(slides[2].lost.iter().any(|l| l.contains("picture")), "{:?}", slides[2].lost);
        assert!(slides[2].lines.iter().any(|l| l == "Ɔdomankoma"), "Akan text survives: {:?}", slides[2].lines);
        println!("{:?}", loss_report(&slides));
    }
}
