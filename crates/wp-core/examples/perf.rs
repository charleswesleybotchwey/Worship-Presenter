//! Performance benchmark: a church-sized library (4,500 bilingual hymns, a 31,000-verse
//! Bible) and the operations used during a service, timed many times each.
//! Run: cargo run --release -p wp-core --example perf

use std::time::{Duration, Instant};
use wp_core::songs::{Lyric, Number, SongDoc};

const WORDS: [&str; 40] = [
    "lord", "god", "love", "grace", "praise", "sing", "heart", "soul", "light", "peace", "joy", "glory", "king", "holy", "spirit", "jesus", "saviour", "mercy", "faith", "hope",
    "world", "life", "blood", "cross", "heaven", "earth", "power", "name", "word", "truth", "way", "day", "night", "song", "voice", "hand", "throne", "lamb", "fire", "rock",
];
const FANTE: [&str; 12] = ["Nyankopɔn", "Ewuradze", "dɔ", "ayɛyi", "ndwom", "akoma", "kra", "hann", "asomdwee", "enyigye", "anuonyam", "ahenfo"];

fn line(seed: &mut u64, n: usize, words: &[&str]) -> String {
    (0..n).map(|_| {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        words[(*seed >> 33) as usize % words.len()]
    }).collect::<Vec<_>>().join(" ")
}

fn time<T>(label: &str, runs: usize, mut f: impl FnMut(usize) -> T) {
    let mut v: Vec<Duration> = (0..runs).map(|i| {
        let t = Instant::now();
        std::hint::black_box(f(i));
        t.elapsed()
    }).collect();
    v.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("{label:<52} median {:>8.2} ms   worst {:>8.2} ms", ms(v[v.len() / 2]), ms(v[v.len() - 1]));
}

fn main() {
    let dir = std::env::temp_dir().join(format!("wp-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let conn = wp_core::db::open(&dir.join("library.db")).unwrap();
    let mut seed = 7u64;

    // ---- a church-sized library
    let t = Instant::now();
    for n in 1..=4500 {
        let mut en = String::new();
        let mut fa = String::new();
        for v in 1..=6 {
            en.push_str(&format!("[V{v}]\n"));
            fa.push_str(&format!("[V{v}]\n"));
            for _ in 0..4 {
                en.push_str(&line(&mut seed, 7, &WORDS));
                en.push('\n');
                fa.push_str(&line(&mut seed, 6, &FANTE));
                fa.push('\n');
            }
        }
        let doc = SongDoc {
            id: None,
            title: format!("MHB {n}, ENG&FANTE"),
            author: Some("Charles Wesley".into()),
            copyright: Some("Public Domain".into()),
            ccli: None,
            numbers: vec![Number { hymnal: "MHB".into(), number: n.to_string() }],
            lyrics: vec![Lyric { lang: "en".into(), body: en }, Lyric { lang: "fat".into(), body: fa }],
            sequence: String::new(),
            lines_per_slide: None,
            key: None,
        };
        wp_core::songs::save(&conn, &doc).unwrap();
    }
    conn.execute_batch("BEGIN").unwrap();
    conn.execute("INSERT INTO bible_version (id, code, name, lang) VALUES (1, 'NKJV', 'New King James', 'en')", []).unwrap();
    let mut verses = 0;
    for b in 1..=66u16 {
        let name = wp_core::speech_ref::book_display(b);
        let chapters = if b == 19 { 150 } else { 18 };
        conn.execute("INSERT INTO bible_book (version_id, book_no, name, aliases, chapters) VALUES (1, ?1, ?2, ?2, ?3)", rusqlite::params![b, name, chapters]).unwrap();
        for c in 1..=chapters {
            for v in 1..=if b == 19 { 12 } else { 26 } {
                let text = line(&mut seed, 22, &WORDS);
                conn.execute("INSERT INTO bible_verse (version_id, book_no, chapter, verse, text) VALUES (1, ?1, ?2, ?3, ?4)", rusqlite::params![b, c, v, text]).unwrap();
                verses += 1;
            }
        }
    }
    conn.execute_batch("COMMIT").unwrap();
    // more Bibles, as a church with many translations has (copies of the first)
    let extra: i64 = std::env::var("WP_PERF_BIBLES").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    for id in 2..=extra {
        conn.execute_batch(&format!(
            "INSERT INTO bible_version (id, code, name, lang) VALUES ({id}, 'B{id}', 'Bible {id}', 'en');
             INSERT INTO bible_book (version_id, book_no, name, aliases, chapters) SELECT {id}, book_no, name, aliases, chapters FROM bible_book WHERE version_id = 1;
             INSERT INTO bible_verse (version_id, book_no, chapter, verse, text) SELECT {id}, book_no, chapter, verse, text FROM bible_verse WHERE version_id = 1;"
        ))
        .unwrap();
    }
    println!("built: 4,500 bilingual hymns and {} Bibles of {verses} verses in {:.1} s\n", extra.max(1), t.elapsed().as_secs_f64());
    if let Some(out) = std::env::var_os("WP_PERF_OUT") {
        drop(conn);
        std::fs::copy(dir.join("library.db"), &out).unwrap();
        println!("library kept at {}", std::path::Path::new(&out).display());
        return;
    }

    // ---- songs
    time("song search: one word (\"grace\")", 30, |_| wp_core::library::search_songs(&conn, "grace", 200).unwrap().len());
    time("song search: three words", 30, |_| wp_core::library::search_songs(&conn, "love grace heaven", 200).unwrap().len());
    time("song search: Fante word (\"Nyankopon\")", 30, |_| wp_core::library::search_songs(&conn, "Nyankopon", 200).unwrap().len());
    time("song list: all 4,500 (Songs tab)", 10, |_| wp_core::browse::browse_songs(&conn, &wp_core::browse::Scope::All, "").unwrap().len());
    time("quick box: hymn number \"555\"", 50, |_| wp_core::library::quick_song(&conn, "555").unwrap().len());
    time("song slides: load + build (go live)", 50, |i| {
        let doc = wp_core::songs::load(&conn, (i % 4500 + 1) as i64).unwrap();
        wp_core::songs::preview(&doc, 2).unwrap().len()
    });

    // ---- scripture
    time("scripture: open John 3 (reference)", 50, |_| wp_core::browse::browse_chapter(&conn, "NKJV", "John 3:16").unwrap().rows.len());
    let t = Instant::now();
    let first = wp_core::browse::search_bible(&conn, &["NKJV".into()], "grace", 200).unwrap().len();
    println!("{:<52} once     {:>8.2} ms   ({first} hits)", "Bible word search: first time (builds the index)", t.elapsed().as_secs_f64() * 1000.0);
    time("Bible word search: \"grace\" (after the index)", 30, |_| wp_core::browse::search_bible(&conn, &["NKJV".into()], "grace", 200).unwrap().len());
    time("Bible word search: three words", 30, |_| wp_core::browse::search_bible(&conn, &["NKJV".into()], "love grace heaven", 200).unwrap().len());

    // ---- scripture recognition
    let aliases = wp_core::library::aliases(&conn).unwrap();
    time("recognition: follow one sentence (Tracker)", 200, |_| {
        let mut tr = wp_core::speech_ref::Tracker::new();
        tr.feed("please turn with me to John chapter three verse sixteen", &aliases).len()
    });
    let spoken = line(&mut seed, 30, &WORDS);
    time("recognition: verse read aloud? (last 15 s)", 30, |_| wp_core::speech_quote::find(&conn, &[], &spoken).unwrap().is_some());
    time("sermon outline → slides (6 lines, 3 readings)", 20, |_| {
        wp_core::outline::to_deck(&conn, "The God of grace\n1. Love (John 3:16)\n2. Grace (Romans 8:28)\n3. Hope - Psalm 23:1\n4. Faith\n5. Amen", "NKJV").unwrap().slides.len()
    });

    // ---- NDI picture conversion (every frame sent)
    let frame = vec![128u8; 1920 * 1080 * 4];
    time("NDI: convert one 1080p frame (BGRA → UYVA)", 20, |_| wp_core::ndi::convert::bgra_to_uyva(&frame, 1920, 1080).len());

    let size = std::fs::metadata(dir.join("library.db")).map(|m| m.len()).unwrap_or(0);
    println!("\nlibrary file: {:.1} MB", size as f64 / 1e6);
    let _ = std::fs::remove_dir_all(&dir);
}
