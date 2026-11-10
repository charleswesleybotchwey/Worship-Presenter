//! Imports the real EasyWorship and Bible Show files used in Phase 0.
//! Set WP_SAMPLES to the folder holding them; skipped when unset.
//!   WP_SAMPLES=/path/to/FILES cargo test -p wp-core --test real_files -- --nocapture

use std::path::PathBuf;
use wp_core::{db, import, library};

fn samples() -> Option<PathBuf> {
    std::env::var_os("WP_SAMPLES").map(PathBuf::from).filter(|p| p.exists())
}

#[test]
fn imports_everything_and_answers_lookups() {
    let Some(dir) = samples() else { eprintln!("WP_SAMPLES not set; skipping"); return };
    let conn = db::open_memory().unwrap();

    let songs = import::ew_songs::import(&conn, &dir.join("Songs.db"), &dir.join("SongWords.db"), "Methodist hymns").unwrap();
    println!("songs: {songs:?}");
    assert_eq!(songs.read, 4531);
    assert_eq!(songs.imported + songs.skipped, songs.read, "every song accounted for");

    let mut ewb: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "ewb")).collect();
    ewb.sort();
    for p in &ewb {
        let stem = p.file_stem().unwrap().to_string_lossy().to_uppercase();
        let code = if stem.contains("TWI") { "TW-EW" } else if stem.contains("GA-BIBLE") { "GA" } else { stem.as_str() };
        let r = import::ewb::import(&conn, p, Some(code)).unwrap();
        println!("{:<28} {:>6} verses  skipped {:>3}  fixed {:>4}", p.file_name().unwrap().to_string_lossy(), r.imported, r.skipped, r.fixed_chars);
        assert_eq!(r.imported + r.skipped, r.read);
    }

    if let Some(bibs) = std::env::var_os("WP_BIB").map(PathBuf::from) {
        for f in ["NKK2011.bib", "TK2012.bib", "YGZP2020.bib"] {
            let r = import::bibleshow::import_file(&conn, &bibs.join(f), None).unwrap();
            println!("{f:<28} {:>6} verses  skipped {:>3}  {:?}", r.imported, r.skipped, r.notes);
            assert_eq!(r.imported + r.skipped, r.read);
        }
        let p = library::lookup(&conn, "Yohane 3:16", &["TK", "NKJV"]).unwrap();
        println!("{} → {:?}", p.display, p.verses.iter().map(|v| format!("[{}] {}", v.version, v.text)).collect::<Vec<_>>());
        assert_eq!(p.verses.len(), 2);
        assert!(p.verses[0].marked.as_deref().unwrap_or("").contains("<r>"), "words of Jesus kept");
        let p = library::lookup(&conn, "Nnwom 23:1-3", &["TK"]).unwrap();
        assert_eq!(p.verses.len(), 3);
    }

    let hits = library::search_songs(&conn, "MHB 1", 10).unwrap();
    println!("MHB 1 → {:?}", hits.iter().map(|h| (&h.title, &h.first_line)).collect::<Vec<_>>());
    assert!(hits.iter().any(|h| h.title == "MHB 1"));
    let hits = library::search_songs(&conn, "thousand tongues", 5).unwrap();
    assert!(!hits.is_empty());
    let hits = library::search_songs(&conn, "Meenya ngyirama", 5).unwrap();
    println!("lyric search (accent-folded) → {:?}", hits.iter().map(|h| &h.title).collect::<Vec<_>>());
    assert!(!hits.is_empty());

    let ascii = library::search_songs(&conn, "Nyame ye", 5).unwrap();
    let marked = library::search_songs(&conn, "Nyame yɛ", 5).unwrap();
    println!("plain-keyboard search 'Nyame ye' → {} hits, with ɛ → {} hits", ascii.len(), marked.len());
    assert!(!ascii.is_empty() && ascii.len() == marked.len(), "ɛ/e typed either way finds the same songs");

    // EasyWorship-style scripture table: whole chapter, typed verses marked
    let ch = wp_core::browse::browse_chapter(&conn, "NKJV", "Gen").unwrap();
    assert_eq!((ch.chapter, ch.rows.len()), (1, 31), "a book alone opens chapter 1");
    let ch = wp_core::browse::browse_chapter(&conn, "NKJV", "Yohane 3:16-18").unwrap();
    assert_eq!(ch.rows.iter().filter(|r| r.selected).map(|r| r.verse).collect::<Vec<_>>(), vec![16, 17, 18]);
    println!("chapter table: {} {} has {} verses", ch.book, ch.chapter, ch.rows.len());
    let tree = wp_core::browse::song_tree(&conn).unwrap();
    println!("collections: {:?}", tree.collections.iter().map(|f| (f.name.as_str(), f.count)).collect::<Vec<_>>());

    let p = library::lookup(&conn, "Jn 3:16-18", &["NKJV", "GA"]).unwrap();
    assert_eq!(p.verses.len(), 6);
    let p = library::lookup(&conn, "Yɔhane 3:16", &["GA"]).unwrap();
    println!("{} → {}", p.display, p.verses[0].text);

    // bilingual hymn splits and re-splits without losing place
    let id = library::search_songs(&conn, "MHB 1", 10).unwrap().into_iter().find(|h| h.title.contains("ENG")).unwrap().id;
    let secs = library::song_sections(&conn, id).unwrap();
    let mk = |lps| wp_core::splitter::Options { lines_per_slide: lps, lang_mode: wp_core::splitter::LangMode::Both, primary: "en".into(), second: Some("fat".into()), sequence: vec![], include_title: false };
    let two = wp_core::splitter::split(&secs, &mk(2));
    let one = wp_core::splitter::split(&secs, &mk(1));
    println!("MHB 1 bilingual: {} slides at 2 lines, {} at 1 line; slide 1 = {:?} / {:?}", two.len(), one.len(), two[0].primary, two[0].second);
    let pos = wp_core::splitter::position_of(&two[3]);
    assert_eq!(one[wp_core::splitter::find(&one, pos)].primary[0], two[3].primary[0]);

    let n: i64 = conn.query_row("SELECT count(*) FROM import_log WHERE level='fixed'", [], |r| r.get(0)).unwrap();
    println!("look-alike fix log lines: {n}");
}

#[test]
fn skipped_reasons() {
    let Some(dir) = samples() else { return };
    let conn = db::open_memory().unwrap();
    import::ew_songs::import(&conn, &dir.join("Songs.db"), &dir.join("SongWords.db"), "x").unwrap();
    import::ewb::import(&conn, &dir.join("EW-GA-BIBLE-GA (2).ewb"), Some("GA")).unwrap();
    let mut st = conn.prepare("SELECT message, count(*), min(item), max(item) FROM import_log WHERE level='skipped' GROUP BY message").unwrap();
    for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))).unwrap() {
        println!("{:?}", r.unwrap());
    }
}
