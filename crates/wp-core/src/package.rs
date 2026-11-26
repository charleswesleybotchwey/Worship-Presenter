//! The Worship Presenter package (.wpp): the app's own import and export. One zip file with
//! songs (every language, numbers, play order, key, collections), themes (with their
//! pictures and videos), Bibles, props and macros — so nothing is lost between computers or
//! churches. Import shows what is new first; duplicates are skipped unless replacing is chosen.

use crate::songs::{self, SongDoc};
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const FORMAT: &str = "worship-presenter-package";

fn bad(m: impl Into<String>) -> Error {
    Error::Format("Package".into(), "package", m.into())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub created: String,
    pub songs: usize,
    pub themes: usize,
    pub bibles: Vec<String>,
    pub props: usize,
    pub macros: usize,
}

/// What to put in a package.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Choice {
    /// every song
    #[serde(default)]
    pub all_songs: bool,
    /// these songs (when not all)
    #[serde(default)]
    pub song_ids: Vec<i64>,
    #[serde(default)]
    pub themes: bool,
    /// Bible codes ("NKJV")
    #[serde(default)]
    pub bibles: Vec<String>,
    #[serde(default)]
    pub props_and_macros: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PkgSong {
    #[serde(flatten)]
    doc: SongDoc,
    #[serde(default)]
    collections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PkgBible {
    code: String,
    name: String,
    lang: Option<String>,
    copyright: Option<String>,
    /// (book_no, name, abbrev, aliases, chapters)
    books: Vec<(i64, String, Option<String>, Option<String>, i64)>,
    /// (book_no, chapter, verse, text, marked, heading)
    verses: Vec<(i64, i64, i64, String, Option<String>, Option<String>)>,
}

fn song_collections(conn: &Connection, id: i64) -> Vec<String> {
    let mut st = match conn.prepare("SELECT c.name FROM my_collection c JOIN my_collection_item i ON i.collection_id = c.id WHERE i.song_id = ?1 ORDER BY c.name") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    st.query_map([id], |r| r.get(0)).map(|r| r.filter_map(|x| x.ok()).collect()).unwrap_or_default()
}

/// Makes a package. Returns its bytes.
pub fn export(conn: &Connection, media_dir: &std::path::Path, choice: &Choice) -> Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buf);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut man = Manifest { format: FORMAT.into(), version: 1, created: chrono_now(), ..Default::default() };

    // songs
    let ids: Vec<i64> = if choice.all_songs {
        let mut st = conn.prepare("SELECT id FROM song ORDER BY title")?;
        let v = st.query_map([], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
        v
    } else {
        choice.song_ids.clone()
    };
    if !ids.is_empty() {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let mut doc = songs::load(conn, id)?;
            doc.id = None;
            out.push(PkgSong { doc, collections: song_collections(conn, id) });
        }
        man.songs = out.len();
        zip.start_file("songs.json", opts).map_err(|e| bad(e.to_string()))?;
        zip.write_all(&serde_json::to_vec(&out)?)?;
    }
    // themes, with their pictures and videos
    if choice.themes {
        for t in crate::themes::list(conn)? {
            let bytes = crate::themes::export(conn, media_dir, t.id)?;
            zip.start_file(format!("themes/{}.wptheme", t.id), opts).map_err(|e| bad(e.to_string()))?;
            zip.write_all(&bytes)?;
            man.themes += 1;
        }
    }
    // Bibles
    for code in &choice.bibles {
        let v: Option<(i64, String, Option<String>, Option<String>)> = conn
            .query_row("SELECT id, name, lang, copyright FROM bible_version WHERE code = ?1", [code], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()?;
        let Some((vid, name, lang, copyright)) = v else { continue };
        let mut st = conn.prepare("SELECT book_no, name, abbrev, aliases, chapters FROM bible_book WHERE version_id = ?1 ORDER BY book_no")?;
        let books = st.query_map([vid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.filter_map(|r| r.ok()).collect();
        let mut st = conn.prepare("SELECT book_no, chapter, verse, text, marked, heading FROM bible_verse WHERE version_id = ?1 ORDER BY book_no, chapter, verse")?;
        let verses = st.query_map([vid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.filter_map(|r| r.ok()).collect();
        let b = PkgBible { code: code.clone(), name, lang, copyright, books, verses };
        zip.start_file(format!("bibles/{code}.json"), opts).map_err(|e| bad(e.to_string()))?;
        zip.write_all(&serde_json::to_vec(&b)?)?;
        man.bibles.push(code.clone());
    }
    // props and macros
    if choice.props_and_macros {
        let props = crate::props::list(conn)?;
        let macros = crate::macros::list(conn)?;
        man.props = props.len();
        man.macros = macros.len();
        zip.start_file("props.json", opts).map_err(|e| bad(e.to_string()))?;
        zip.write_all(&serde_json::to_vec(&props)?)?;
        zip.start_file("macros.json", opts).map_err(|e| bad(e.to_string()))?;
        zip.write_all(&serde_json::to_vec(&macros)?)?;
    }
    zip.start_file("manifest.json", opts).map_err(|e| bad(e.to_string()))?;
    zip.write_all(&serde_json::to_vec_pretty(&man)?)?;
    zip.finish().map_err(|e| bad(e.to_string()))?;
    Ok(buf.into_inner())
}

fn chrono_now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{s}")
}

struct Reader {
    zip: zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
}

impl Reader {
    fn open(bytes: &[u8]) -> Result<(Self, Manifest)> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).map_err(|_| bad("This is not a Worship Presenter package."))?;
        let man: Manifest = {
            let mut f = zip.by_name("manifest.json").map_err(|_| bad("This is not a Worship Presenter package."))?;
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            serde_json::from_str(&s).map_err(|_| bad("This package is damaged."))?
        };
        if man.format != FORMAT {
            return Err(bad("This is not a Worship Presenter package."));
        }
        if man.version > 1 {
            return Err(bad("This package comes from a newer Worship Presenter: update to open it."));
        }
        Ok((Reader { zip }, man))
    }
    fn json<T: for<'a> Deserialize<'a>>(&mut self, name: &str) -> Result<Option<T>> {
        let Ok(mut f) = self.zip.by_name(name) else { return Ok(None) };
        let mut v = Vec::new();
        f.read_to_end(&mut v)?;
        Ok(Some(serde_json::from_slice(&v).map_err(|_| bad(format!("{name} in the package is damaged.")))?))
    }
    fn names(&self, prefix: &str) -> Vec<String> {
        self.zip.file_names().filter_map(|n| n.ok()).filter(|n| n.starts_with(prefix)).map(|n| n.into_owned()).collect()
    }
    fn bytes(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut f = self.zip.by_name(name).map_err(|_| bad("The package is damaged."))?;
        let mut v = Vec::new();
        f.read_to_end(&mut v)?;
        Ok(v)
    }
}

/// The library song a packaged song duplicates: the same title and the same numbers.
fn duplicate_of(conn: &Connection, doc: &SongDoc) -> Result<Option<i64>> {
    let mut st = conn.prepare("SELECT id FROM song WHERE lower(trim(title)) = lower(trim(?1))")?;
    let ids: Vec<i64> = st.query_map([&doc.title], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
    let want: std::collections::BTreeSet<(String, String)> = doc.numbers.iter().map(|n| (n.hymnal.to_lowercase(), n.number.to_lowercase())).collect();
    for id in ids {
        let have: std::collections::BTreeSet<(String, String)> = songs::load(conn, id)?.numbers.iter().map(|n| (n.hymnal.to_lowercase(), n.number.to_lowercase())).collect();
        if have == want {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Review {
    pub manifest: Manifest,
    pub new_songs: usize,
    pub duplicate_songs: usize,
    /// a few titles, for the operator to recognise the package
    pub sample_titles: Vec<String>,
    /// how many songs have more than one language
    pub multilingual_songs: usize,
    pub new_themes: usize,
    pub new_bibles: Vec<String>,
    pub present_bibles: Vec<String>,
}

/// What a package holds, and what is new here.
pub fn review(conn: &Connection, bytes: &[u8]) -> Result<Review> {
    let (mut r, man) = Reader::open(bytes)?;
    let mut rev = Review { manifest: man.clone(), ..Default::default() };
    let songs: Vec<PkgSong> = r.json("songs.json")?.unwrap_or_default();
    for s in &songs {
        if duplicate_of(conn, &s.doc)?.is_some() {
            rev.duplicate_songs += 1;
        } else {
            rev.new_songs += 1;
        }
        if s.doc.lyrics.len() > 1 {
            rev.multilingual_songs += 1;
        }
    }
    rev.sample_titles = songs.iter().take(5).map(|s| s.doc.title.clone()).collect();
    let have: Vec<String> = crate::themes::list(conn)?.into_iter().map(|t| t.name.to_lowercase()).collect();
    for n in r.names("themes/") {
        let b = r.bytes(&n)?;
        let name = serde_json::from_slice::<serde_json::Value>(&b).ok().and_then(|v| v.pointer("/theme/name").or(v.get("name")).and_then(|x| x.as_str()).map(String::from)).unwrap_or_default();
        if !have.contains(&name.to_lowercase()) {
            rev.new_themes += 1;
        }
    }
    for code in &man.bibles {
        let here: bool = conn.query_row("SELECT count(*) FROM bible_version WHERE code = ?1", [code], |r| r.get::<_, i64>(0))? > 0;
        if here { rev.present_bibles.push(code.clone()) } else { rev.new_bibles.push(code.clone()) }
    }
    Ok(rev)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub songs_added: usize,
    pub songs_replaced: usize,
    pub songs_skipped: usize,
    pub themes_added: usize,
    pub bibles_added: Vec<String>,
    pub props_added: usize,
    pub macros_added: usize,
}

/// Brings a package in. Duplicates (same title and numbers) are skipped, or replaced in place
/// when `replace` (the song keeps its id, so services still find it).
pub fn import(conn: &Connection, media_dir: &std::path::Path, bytes: &[u8], replace: bool) -> Result<Report> {
    let (mut r, man) = Reader::open(bytes)?;
    let mut rep = Report::default();
    // each song is saved whole on its own (a save is its own transaction)
    let tx = conn;
    // songs
    let songs: Vec<PkgSong> = r.json("songs.json")?.unwrap_or_default();
    for s in songs {
        let mut doc = s.doc;
        let id = match duplicate_of(&tx, &doc)? {
            Some(existing) if replace => {
                doc.id = Some(existing);
                rep.songs_replaced += 1;
                songs::save(&tx, &doc)?
            }
            Some(_) => {
                rep.songs_skipped += 1;
                continue;
            }
            None => {
                doc.id = None;
                rep.songs_added += 1;
                songs::save(&tx, &doc)?
            }
        };
        for c in s.collections {
            let cid: i64 = match tx.query_row("SELECT id FROM my_collection WHERE name = ?1", [&c], |r| r.get(0)).optional()? {
                Some(c) => c,
                None => crate::browse::create_collection(&tx, &c)?,
            };
            crate::browse::add_to_collection(&tx, cid, &[id])?;
        }
    }
    // themes (one of the same name already here is kept)
    let have: Vec<String> = crate::themes::list(&tx)?.into_iter().map(|t| t.name.to_lowercase()).collect();
    for n in r.names("themes/") {
        let b = r.bytes(&n)?;
        let name = serde_json::from_slice::<serde_json::Value>(&b).ok().and_then(|v| v.pointer("/theme/name").or(v.get("name")).and_then(|x| x.as_str()).map(String::from)).unwrap_or_default();
        if have.contains(&name.to_lowercase()) {
            continue;
        }
        crate::themes::import(&tx, media_dir, &b)?;
        rep.themes_added += 1;
    }
    // Bibles (one already here is kept)
    for code in &man.bibles {
        if tx.query_row("SELECT count(*) FROM bible_version WHERE code = ?1", [code], |r| r.get::<_, i64>(0))? > 0 {
            continue;
        }
        let Some(b): Option<PkgBible> = r.json(&format!("bibles/{code}.json"))? else { continue };
        // a Bible goes in whole or not at all (and fast: one transaction)
        let tx = conn.unchecked_transaction()?;
        tx.execute("INSERT INTO bible_version (code, name, lang, copyright) VALUES (?1, ?2, ?3, ?4)", params![b.code, b.name, b.lang, b.copyright])?;
        let vid = tx.last_insert_rowid();
        for (no, name, abbrev, aliases, chapters) in &b.books {
            tx.execute("INSERT INTO bible_book (version_id, book_no, name, abbrev, aliases, chapters) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![vid, no, name, abbrev, aliases, chapters])?;
        }
        {
            let mut v = tx.prepare(crate::import::VERSE_SQL)?;
            let mut f = tx.prepare(crate::import::FTS_SQL)?;
            for (book, ch, verse, text, marked, heading) in &b.verses {
                v.execute(params![vid, book, ch, verse, text, marked, heading])?;
                f.execute(params![text, vid, book, ch, verse])?;
            }
        }
        tx.commit()?;
        rep.bibles_added.push(code.clone());
    }
    // props, then macros pointed at the props as they are numbered here
    let mut prop_ids: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    if let Some(props) = r.json::<Vec<crate::props::Prop>>("props.json")? {
        let have: Vec<String> = crate::props::list(&tx)?.into_iter().map(|p| p.name.to_lowercase()).collect();
        for p in props {
            let old = p.id;
            if have.contains(&p.name.to_lowercase()) {
                continue;
            }
            // a picture prop's picture lives in the other library: it comes as text-less
            if p.kind == "image" {
                continue;
            }
            let new = crate::props::save(&tx, &crate::props::Prop { id: None, ..p })?;
            if let Some(o) = old {
                prop_ids.insert(o, new);
            }
            rep.props_added += 1;
        }
    }
    if let Some(macros) = r.json::<Vec<crate::macros::Macro>>("macros.json")? {
        let have: Vec<String> = crate::macros::list(&tx)?.into_iter().map(|m| m.name.to_lowercase()).collect();
        for mut m in macros {
            if have.contains(&m.name.to_lowercase()) {
                continue;
            }
            for a in m.actions.iter_mut() {
                if a.kind == "prop_on" || a.kind == "prop_off" {
                    if let Some(n) = a.value.as_i64().and_then(|o| prop_ids.get(&o)) {
                        a.value = serde_json::json!(n);
                    }
                }
            }
            m.id = None;
            crate::macros::save(&tx, &m)?;
            rep.macros_added += 1;
        }
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::songs::{Lyric, Number};

    fn bilingual() -> SongDoc {
        SongDoc {
            id: None,
            title: "MHB 1, ENG&FANTE".into(),
            author: Some("Charles Wesley".into()),
            copyright: Some("Public Domain".into()),
            ccli: None,
            numbers: vec![Number { hymnal: "MHB".into(), number: "1".into() }],
            lyrics: vec![
                Lyric { lang: "en".into(), body: "[V1]\nO for a thousand tongues to sing\nMy great Redeemer's praise\n\n[C]\nGlory to God".into() },
                Lyric { lang: "fat".into(), body: "[V1]\nMeenya ngyiramá apem matow ndwom\nMeyi mo Pomfò ayɛw\n\n[C]\nAnyimnyam mma Nyame".into() },
            ],
            sequence: "V1 C V1 C".into(),
            lines_per_slide: Some(2),
            key: Some("G".into()),
        }
    }

    #[test]
    fn a_package_carries_every_language_and_is_brought_in_once() {
        let from = crate::db::open_memory().unwrap();
        let id = songs::save(&from, &bilingual()).unwrap();
        let cid = crate::browse::create_collection(&from, "Sunday hymns").unwrap();
        crate::browse::add_to_collection(&from, cid, &[id]).unwrap();
        from.execute("INSERT INTO bible_version (code, name, lang) VALUES ('TK', 'Twerɛ Kronkron', 'tw')", []).unwrap();
        let vid = from.last_insert_rowid();
        from.execute("INSERT INTO bible_book (version_id, book_no, name, chapters) VALUES (?1, 43, 'Yohane', 21)", [vid]).unwrap();
        from.execute("INSERT INTO bible_verse (version_id, book_no, chapter, verse, text) VALUES (?1, 43, 3, 16, 'Na sɛnea Onyankopɔn dɔ wiase nie')", [vid]).unwrap();
        let tmp = std::env::temp_dir().join(format!("wp-pkg-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let pkg = export(&from, &tmp, &Choice { all_songs: true, bibles: vec!["TK".into()], ..Default::default() }).unwrap();

        let to = crate::db::open_memory().unwrap();
        let rev = review(&to, &pkg).unwrap();
        assert_eq!((rev.new_songs, rev.duplicate_songs, rev.multilingual_songs, rev.new_bibles.clone()), (1, 0, 1, vec!["TK".to_string()]));
        let rep = import(&to, &tmp, &pkg, false).unwrap();
        assert_eq!((rep.songs_added, rep.bibles_added.len()), (1, 1));
        let sid: i64 = to.query_row("SELECT id FROM song", [], |r| r.get(0)).unwrap();
        let back = songs::load(&to, sid).unwrap();
        assert_eq!(back.lyrics.iter().map(|l| l.lang.as_str()).collect::<Vec<_>>(), vec!["en", "fat"], "both languages travel");
        assert!(back.lyrics[1].body.contains("Meenya"));
        assert_eq!((back.sequence.as_str(), back.key.as_deref(), back.numbers.len()), ("V1 C V1 C", Some("G"), 1));
        assert_eq!(song_collections(&to, sid), vec!["Sunday hymns".to_string()]);
        // the Bible came with its search
        let hits: i64 = to.query_row("SELECT count(*) FROM bible_fts WHERE bible_fts MATCH 'Onyankopɔn'", [], |r| r.get(0)).unwrap();
        assert_eq!(hits, 1, "the Bible is searchable here too");
        // a second time: nothing new, duplicates skipped (or replaced on request)
        let again = review(&to, &pkg).unwrap();
        assert_eq!((again.new_songs, again.duplicate_songs, again.present_bibles.len()), (0, 1, 1));
        assert_eq!(import(&to, &tmp, &pkg, false).unwrap().songs_skipped, 1);
        assert_eq!(import(&to, &tmp, &pkg, true).unwrap().songs_replaced, 1);
        assert_eq!(to.query_row("SELECT count(*) FROM song", [], |r| r.get::<_, i64>(0)).unwrap(), 1, "never doubled");
        assert!(review(&to, b"not a zip").is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
