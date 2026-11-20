//! EasyWorship 7 schedule files (.ewsx): a zip with main.db (SQLite) and the schedule's
//! media. The items are the presentations at group level 0, in order_index order:
//! type 6 songs, type 7 scripture readings, type 1 media; others are kept as headings
//! so nothing silently disappears. Words are in each slide's main text element (style 4)
//! as RTF; a first line such as "Verse 1" names the section.

use crate::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum EwsItem {
    Song { title: String, author: String, copyright: String, number: String, slides: Vec<(Option<String>, Vec<String>)> },
    Scripture { reference: String, version: Option<String> },
    /// `file`: the file's name inside the archive's media folder.
    Media { title: String, file: Option<String>, video: bool },
    Other { title: String, kind: i64 },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EwsScripture {
    pub complete_passage: bool,
    pub abbreviate: bool,
    pub only_reference: bool,
    pub one_verse_per_slide: bool,
}

pub struct EwsSchedule {
    pub items: Vec<EwsItem>,
    pub scripture: Option<EwsScripture>,
    zip_path: std::path::PathBuf,
}

fn bad(m: &str) -> Error {
    Error::Format("EasyWorship schedule".into(), "schedule", m.into())
}

/// "Verse 1" → V1, "Chorus" → C, "Bridge 2" → B2 …
fn section_label(line: &str) -> Option<String> {
    let l = line.trim();
    let (word, num) = l.rsplit_once(' ').map(|(w, n)| if n.chars().all(|c| c.is_ascii_digit()) { (w, n) } else { (l, "") }).unwrap_or((l, ""));
    let code = match word.to_ascii_lowercase().as_str() {
        "verse" => "V",
        "chorus" | "refrain" => "C",
        "bridge" => "B",
        "pre-chorus" | "prechorus" => "PC",
        "tag" => "T",
        "ending" => "E",
        "intro" => "I",
        "outro" => "O",
        _ => return None,
    };
    Some(format!("{code}{num}"))
}

pub fn read(path: &Path) -> Result<EwsSchedule> {
    let file = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(file).map_err(|_| bad("This is not an EasyWorship 7 schedule (.ewsx), or it is damaged. Older .ews files: open and save them in EasyWorship 7 first."))?;
    let mut db = Vec::new();
    z.by_name("main.db").map_err(|_| bad("This schedule file has no schedule inside it."))?.read_to_end(&mut db)?;
    let tmp = std::env::temp_dir().join(format!("wp-ews-{}-{}.db", std::process::id(), crate::cues::now_ms()));
    std::fs::write(&tmp, &db)?;
    let result = (|| -> Result<(Vec<EwsItem>, Option<EwsScripture>)> {
        let c = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let words = |pid: i64| -> Result<Vec<(Option<String>, Vec<String>)>> {
            let mut st = c.prepare(
                "SELECT t.rtf FROM slide s JOIN element e ON e.slide_id = s.rowid JOIN resource_text t ON t.resource_id = e.foreground_resource_id
                 WHERE s.presentation_id = ?1 AND e.element_style_type = 4 ORDER BY s.order_index, e.order_index",
            )?;
            let rows: Vec<String> = st.query_map([pid], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()).collect();
            Ok(rows
                .iter()
                .map(|rtf| {
                    let mut lines: Vec<String> = crate::import::songfiles::rtf_text(rtf).into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
                    let label = lines.first().and_then(|l| section_label(l));
                    if label.is_some() {
                        lines.remove(0);
                    }
                    (label, lines)
                })
                .filter(|(_, l)| !l.is_empty())
                .collect())
        };
        let media_of = |pid: i64| -> (Option<String>, bool) {
            let q = |table: &str| -> Option<String> {
                c.query_row(
                    &format!(
                        "SELECT f.filename FROM slide s JOIN element e ON e.slide_id = s.rowid JOIN {table} m ON m.resource_id IN (e.foreground_resource_id, e.background_resource_id)
                         JOIN file f ON f.rowid = m.file_id WHERE s.presentation_id = ?1 LIMIT 1"
                    ),
                    [pid],
                    |r| r.get(0),
                )
                .ok()
            };
            match q("resource_video") {
                Some(f) => (Some(f), true),
                None => (q("resource_image"), false),
            }
        };
        let mut st = c.prepare("SELECT rowid, presentation_type, coalesce(title, ''), coalesce(author, ''), coalesce(copyright, ''), coalesce(reference_number, '') FROM presentation WHERE group_level = 0 ORDER BY order_index")?;
        let rows: Vec<(i64, i64, String, String, String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.filter_map(|r| r.ok()).collect();
        let mut items = vec![];
        for (pid, kind, title, author, copyright, number) in rows {
            items.push(match kind {
                6 => {
                    // a first slide that only repeats the title is the title page
                    let mut slides = words(pid)?;
                    if slides.first().is_some_and(|(l, s)| l.is_none() && s.first().is_some_and(|x| x.eq_ignore_ascii_case(title.trim()))) {
                        slides.remove(0);
                    }
                    EwsItem::Song { title, author, copyright, number, slides }
                }
                7 => {
                    let (reference, version) = match title.rsplit_once(" (") {
                        Some((r, v)) if v.ends_with(')') => (r.trim().to_string(), Some(v.trim_end_matches(')').trim().to_string())),
                        _ => (title.trim().to_string(), None),
                    };
                    EwsItem::Scripture { reference, version }
                }
                1 => {
                    let (file, video) = media_of(pid);
                    EwsItem::Media { title, file, video }
                }
                _ => EwsItem::Other { title, kind },
            });
        }
        let attr = |k: &str| -> Option<bool> {
            c.query_row(
                "SELECT p.value FROM presentation_property_global p JOIN presentation_property_group_global g ON g.rowid = p.group_id WHERE g.group_name = 'Scripture_Attributes' AND p.key = ?1 LIMIT 1",
                [k],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .map(|v| v == "1")
        };
        let scripture = attr("One_Verse_Per_Slide").map(|one| EwsScripture {
            complete_passage: attr("Show_Complete_Passage").unwrap_or(true),
            abbreviate: attr("Abbr_Book_Names").unwrap_or(false),
            only_reference: attr("Only_Show_Reference").unwrap_or(false),
            one_verse_per_slide: one,
        });
        Ok((items, scripture))
    })();
    let _ = std::fs::remove_file(&tmp);
    let (items, scripture) = result?;
    Ok(EwsSchedule { items, scripture, zip_path: path.to_path_buf() })
}

impl EwsSchedule {
    /// A media file from the archive, written to `dir` under its own name.
    pub fn extract_media(&self, name: &str, dir: &Path) -> Result<std::path::PathBuf> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(&self.zip_path)?).map_err(|_| bad("The schedule file is damaged."))?;
        let safe: String = name.chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).collect();
        let entry = (0..z.len())
            .find(|&i| z.by_index(i).ok().is_some_and(|f| f.name().is_ok_and(|n| n.replace('\\', "/").rsplit('/').next() == Some(name))))
            .ok_or_else(|| bad("A media file is missing from the schedule."))?;
        let mut f = z.by_index(entry).map_err(|_| bad("The schedule file is damaged."))?;
        let out = dir.join(safe);
        let mut w = std::fs::File::create(&out)?;
        std::io::copy(&mut f.by_ref().take(2 * 1024 * 1024 * 1024), &mut w)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A small schedule with EasyWorship 7's tables (only the columns the reader uses).
    fn sample(dir: &Path) -> std::path::PathBuf {
        let db = dir.join("main.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE presentation (rowid INTEGER PRIMARY KEY, presentation_type int8, group_level integer, order_index integer, title text, author text, copyright text, reference_number text);
             CREATE TABLE slide (rowid INTEGER PRIMARY KEY, presentation_id integer, order_index integer, title text);
             CREATE TABLE element (rowid INTEGER PRIMARY KEY, slide_id integer, element_style_type int8, order_index integer, background_resource_id integer, foreground_resource_id integer);
             CREATE TABLE resource_text (rowid INTEGER PRIMARY KEY, resource_id integer, rtf text);
             CREATE TABLE resource_video (rowid INTEGER PRIMARY KEY, resource_id integer, file_id integer);
             CREATE TABLE resource_image (rowid INTEGER PRIMARY KEY, resource_id integer, file_id integer);
             CREATE TABLE file (rowid INTEGER PRIMARY KEY, filename text);
             CREATE TABLE presentation_property_group_global (rowid INTEGER PRIMARY KEY, link_id integer, group_name text);
             CREATE TABLE presentation_property_global (rowid INTEGER PRIMARY KEY, group_id integer, key text, value text);
             INSERT INTO presentation VALUES (1, 6, 0, 0, 'MHB 1', 'Charles Wesley', 'Public Domain', '1');
             INSERT INTO presentation VALUES (2, 13, 1, 0, 'Master', '', '', '');
             INSERT INTO presentation VALUES (3, 7, 0, 1, 'Genesis 1:1 (AMP)', '', '', '');
             INSERT INTO presentation VALUES (4, 1, 0, 2, 'Quiet Starry Night', '', '', '');
             INSERT INTO presentation VALUES (5, 9, 0, 3, 'Welcome slides', '', '', '');
             INSERT INTO slide VALUES (1, 1, 0, ''), (2, 1, 1, ''), (3, 1, 2, ''), (4, 4, 0, '');
             INSERT INTO element VALUES (1, 1, 4, 0, NULL, 11), (2, 2, 4, 0, NULL, 12), (3, 3, 4, 0, NULL, 13), (4, 2, 8, 1, NULL, 14), (5, 4, 0, 0, 20, NULL);
             INSERT INTO resource_text VALUES (1, 11, '{\\rtf1 {\\pard MHB 1\\par}}'), (2, 12, '{\\rtf1 {\\pard Verse 1\\par}{\\pard O for a thousand tongues to sing\\par}{\\pard My great Redeemer\\u8217?s praise\\par}}'),
               (3, 13, '{\\rtf1 {\\pard Chorus\\par}{\\pard \\u596?d\\u596? w\\u596? h\\u596?\\par}}'), (4, 14, '{\\rtf1 {\\pard MHB 1 - Public Domain\\par}}');
             INSERT INTO resource_video VALUES (1, 20, 1);
             INSERT INTO file VALUES (1, 'abc.mp4');
             INSERT INTO presentation_property_group_global VALUES (1, 9, 'Scripture_Attributes');
             INSERT INTO presentation_property_global VALUES (1, 1, 'One_Verse_Per_Slide', '1'), (2, 1, 'Abbr_Book_Names', '1');",
        )
        .unwrap();
        drop(c);
        let zp = dir.join("sample.ewsx");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
        let o: zip::write::SimpleFileOptions = Default::default();
        z.start_file("main.db", o).unwrap();
        z.write_all(&std::fs::read(&db).unwrap()).unwrap();
        z.start_file("media\\abc.mp4", o).unwrap();
        z.write_all(b"video bytes").unwrap();
        z.finish().unwrap();
        zp
    }

    #[test]
    fn reads_an_easyworship_schedule() {
        let dir = std::env::temp_dir().join(format!("wp-ews-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = read(&sample(&dir)).unwrap();
        assert_eq!(s.items.len(), 4, "the master/theme at group level 1 is not an item");
        match &s.items[0] {
            EwsItem::Song { title, number, slides, .. } => {
                assert_eq!((title.as_str(), number.as_str()), ("MHB 1", "1"));
                assert_eq!(slides.len(), 2, "the title page is left out");
                assert_eq!(slides[0], (Some("V1".to_string()), vec!["O for a thousand tongues to sing".to_string(), "My great Redeemer’s praise".to_string()]));
                assert_eq!(slides[1], (Some("C".to_string()), vec!["ɔdɔ wɔ hɔ".to_string()]), "Ghanaian letters kept");
            }
            other => panic!("not a song: {other:?}"),
        }
        assert_eq!(s.items[1], EwsItem::Scripture { reference: "Genesis 1:1".into(), version: Some("AMP".into()) });
        assert_eq!(s.items[2], EwsItem::Media { title: "Quiet Starry Night".into(), file: Some("abc.mp4".into()), video: true });
        assert_eq!(s.items[3], EwsItem::Other { title: "Welcome slides".into(), kind: 9 });
        let sc = s.scripture.clone().unwrap();
        assert!(sc.one_verse_per_slide && sc.abbreviate);
        let f = s.extract_media("abc.mp4", &dir).unwrap();
        assert_eq!(std::fs::read(f).unwrap(), b"video bytes");
        assert!(read(&dir.join("main.db")).is_err(), "not a zip: a clear refusal");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
