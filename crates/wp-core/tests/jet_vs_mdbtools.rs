//! Proves the Rust Jet reader returns exactly what mdbtools returns, row for row,
//! for every table of every Bible Show file. Needs WP_BIB and mdb-json on PATH.

use serde_json::{Map, Value};
use std::path::PathBuf;
use wp_core::jet::Database;

fn key(m: &Map<String, Value>) -> String {
    let mut v: Vec<(String, String)> = m.iter().map(|(k, v)| (k.clone(), match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map(|f| format!("{f}")).unwrap_or_default(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    })).collect();
    v.sort();
    format!("{v:?}")
}

#[test]
fn matches_mdbtools_exactly() {
    let Some(dir) = std::env::var_os("WP_BIB").map(PathBuf::from) else { eprintln!("WP_BIB not set; skipping"); return };
    for f in ["NKK2011.bib", "TK2012.bib", "YGZP2020.bib"] {
        let path = dir.join(f);
        let db = Database::open(&path).unwrap();
        let tables = db.tables().unwrap();
        for (t, _) in &tables {
            let ours: Vec<String> = { let mut v: Vec<String> = db.rows(t).unwrap().iter().map(key).collect(); v.sort(); v };
            let out = std::process::Command::new("mdb-json").arg(&path).arg(t).output().unwrap();
            let theirs: Vec<String> = { let mut v: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().filter(|l| !l.trim().is_empty())
                .map(|l| match serde_json::from_str::<Value>(l).unwrap() { Value::Object(m) => key(&m), _ => String::new() }).collect(); v.sort(); v };
            let diff = ours.iter().zip(&theirs).filter(|(a, b)| a != b).count();
            println!("{f:<14} {t:<11} rows ours {:>6} mdbtools {:>6} differing {diff}", ours.len(), theirs.len());
            if diff > 0 { let (a, b) = ours.iter().zip(&theirs).find(|(a, b)| a != b).unwrap(); println!("  ours:     {}\n  mdbtools: {}", &a[..a.len().min(300)], &b[..b.len().min(300)]); }
            assert_eq!(ours.len(), theirs.len(), "{f} {t} row count");
            assert_eq!(diff, 0, "{f} {t} contents");
        }
    }
}
