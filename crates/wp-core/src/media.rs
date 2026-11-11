//! Media library: pictures, videos and audio copied into the profile's media/
//! folder, so a profile (and its backups and copies) carries everything it uses.
//! Paths stored in the database and in themes are relative to that folder.
//! Thumbnails are made by the UI (it can decode video frames) and saved here.

use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MediaItem {
    pub id: i64,
    pub kind: String,
    /// Relative to the media folder, with forward slashes.
    pub path: String,
    pub title: String,
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub thumb: Option<String>,
    pub tags: Option<String>,
    pub bytes: u64,
    /// The file is no longer in the media folder; use Relink.
    pub missing: bool,
    /// "loop" or "once"; None = play once (the behaviour before this option existed).
    pub repeat: Option<String>,
    /// "fit", "fill" or "stretch"; None = fit.
    pub fit: Option<String>,
}

pub fn kind_of(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => Some("image"),
        "mp4" | "m4v" | "webm" | "mov" => Some("video"),
        "mp3" | "wav" | "ogg" | "m4a" | "aac" | "flac" => Some("audio"),
        _ => None,
    }
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Media".into(), "media", msg.into())
}

fn safe_name(stem: &str) -> String {
    let s: String = stem.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "media".into() } else { s.chars().take(60).collect() }
}

/// Copies a file into `media_dir/<kind>s/` and records it. Re-importing the same
/// file (same name and size) returns the existing item instead of a duplicate.
pub fn import(conn: &Connection, media_dir: &Path, src: &Path) -> Result<MediaItem> {
    let kind = kind_of(src).ok_or_else(|| bad(format!("{} is not a picture, video or sound file Worship Presenter can use.", src.display())))?;
    let meta = std::fs::metadata(src)?;
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "media".into());
    let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let title = stem.replace(['_', '-'], " ").trim().to_string();

    // same title and size already imported: reuse it
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM media WHERE title = ?1 AND kind = ?2", params![title, kind], |r| r.get(0))
        .ok();
    if let Some(id) = existing {
        let item = get(conn, media_dir, id)?;
        if item.bytes == meta.len() {
            return Ok(item);
        }
    }

    let sub = format!("{kind}s");
    std::fs::create_dir_all(media_dir.join(&sub))?;
    let base = safe_name(&stem);
    let mut rel = format!("{sub}/{base}.{ext}");
    let mut n = 2;
    while media_dir.join(&rel).exists() {
        rel = format!("{sub}/{base}-{n}.{ext}");
        n += 1;
    }
    std::fs::copy(src, media_dir.join(&rel))?;
    conn.execute("INSERT INTO media (kind, path, title) VALUES (?1, ?2, ?3)", params![kind, rel, title])?;
    get(conn, media_dir, conn.last_insert_rowid())
}

fn row(r: &rusqlite::Row, media_dir: &Path) -> rusqlite::Result<MediaItem> {
    let path: String = r.get(2)?;
    let meta = std::fs::metadata(media_dir.join(&path)).ok();
    Ok(MediaItem {
        id: r.get(0)?,
        kind: r.get(1)?,
        bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        missing: meta.is_none(),
        path,
        title: r.get(3)?,
        duration_ms: r.get(4)?,
        width: r.get(5)?,
        height: r.get(6)?,
        thumb: r.get(7)?,
        tags: r.get(8)?,
        repeat: r.get(9)?,
        fit: r.get(10)?,
    })
}

const COLS: &str = "id, kind, path, title, duration_ms, width, height, thumb_path, tags, repeat, fit";

/// EasyWorship's Repeating and Aspect Ratio for one item.
pub fn set_options(conn: &Connection, id: i64, repeat: Option<&str>, fit: Option<&str>) -> Result<()> {
    let ok_r = matches!(repeat, None | Some("loop") | Some("once"));
    let ok_f = matches!(fit, None | Some("fit") | Some("fill") | Some("stretch"));
    if !ok_r || !ok_f {
        return Err(crate::Error::Format("Media".into(), "options", "Unknown repeat or fit choice.".into()));
    }
    conn.execute("UPDATE media SET repeat = ?2, fit = ?3 WHERE id = ?1", params![id, repeat, fit])?;
    Ok(())
}

pub fn get(conn: &Connection, media_dir: &Path, id: i64) -> Result<MediaItem> {
    Ok(conn.query_row(&format!("SELECT {COLS} FROM media WHERE id = ?1"), [id], |r| row(r, media_dir))?)
}

/// Lists media, newest first, optionally of one kind and matching a search.
pub fn list(conn: &Connection, media_dir: &Path, kind: Option<&str>, query: &str) -> Result<Vec<MediaItem>> {
    let q = format!("%{}%", query.trim());
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM media WHERE (?1 IS NULL OR kind = ?1) AND (title LIKE ?2 OR coalesce(tags,'') LIKE ?2) ORDER BY id DESC"
    ))?;
    let v = st.query_map(params![kind, q], |r| row(r, media_dir))?.collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

/// Facts only the UI can read (video length and size), saved after import.
pub fn set_meta(conn: &Connection, id: i64, duration_ms: Option<i64>, width: Option<i64>, height: Option<i64>) -> Result<()> {
    conn.execute("UPDATE media SET duration_ms = ?2, width = ?3, height = ?4 WHERE id = ?1", params![id, duration_ms, width, height])?;
    Ok(())
}

/// Saves a JPEG thumbnail (bytes) made by the UI and records its relative path.
pub fn save_thumb(conn: &Connection, media_dir: &Path, id: i64, jpeg: &[u8]) -> Result<String> {
    if jpeg.len() > 2_000_000 {
        return Err(bad("Thumbnail too large."));
    }
    std::fs::create_dir_all(media_dir.join("thumbs"))?;
    let rel = format!("thumbs/{id}.jpg");
    std::fs::write(media_dir.join(&rel), jpeg)?;
    conn.execute("UPDATE media SET thumb_path = ?2 WHERE id = ?1", params![id, rel])?;
    Ok(rel)
}

pub fn rename(conn: &Connection, id: i64, title: &str, tags: Option<&str>) -> Result<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(bad("Media needs a name."));
    }
    conn.execute("UPDATE media SET title = ?2, tags = ?3 WHERE id = ?1", params![id, title, tags])?;
    Ok(())
}

/// Names of themes that use this file, so deleting it can warn first.
pub fn used_by(conn: &Connection, media_dir: &Path, id: i64) -> Result<Vec<String>> {
    let item = get(conn, media_dir, id)?;
    let mut st = conn.prepare("SELECT name, json FROM theme")?;
    let rows: Vec<(String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
    let needle = format!("\"{}\"", item.path);
    Ok(rows.into_iter().filter(|(_, j)| j.contains(&needle)).map(|(n, _)| n).collect())
}

/// Removes the record, the file and its thumbnail.
pub fn delete(conn: &Connection, media_dir: &Path, id: i64) -> Result<()> {
    let item = get(conn, media_dir, id)?;
    conn.execute("DELETE FROM media WHERE id = ?1", [id])?;
    let _ = std::fs::remove_file(media_dir.join(&item.path));
    if let Some(t) = item.thumb {
        let _ = std::fs::remove_file(media_dir.join(t));
    }
    Ok(())
}

/// Points an item at a replacement file (same kind), copied into the item's place so
/// themes and service orders that use it keep working. The thumbnail is remade.
pub fn relink(conn: &Connection, media_dir: &Path, id: i64, src: &Path) -> Result<MediaItem> {
    let item = get(conn, media_dir, id)?;
    let kind = kind_of(src).ok_or_else(|| bad("That is not a picture, video or sound file."))?;
    if kind != item.kind {
        return Err(bad(format!("“{}” is a {}; choose a {} file.", item.title, item.kind, item.kind)));
    }
    std::fs::create_dir_all(media_dir.join(&item.path).parent().unwrap_or(media_dir))?;
    std::fs::copy(src, media_dir.join(&item.path))?;
    if let Some(t) = &item.thumb {
        let _ = std::fs::remove_file(media_dir.join(t));
    }
    conn.execute("UPDATE media SET thumb_path = NULL, duration_ms = NULL WHERE id = ?1", [id])?;
    get(conn, media_dir, id)
}

/// Copies a whole media folder (used when a profile starts as a copy of another).
pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dest: PathBuf = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &dest)?;
        } else {
            std::fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod options_tests {
    #[test]
    fn repeating_and_aspect_ratio_are_kept() {
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO media (kind, path, title) VALUES ('video', 'v/a.mp4', 'Loop')", []).unwrap();
        let id = c.last_insert_rowid();
        super::set_options(&c, id, Some("once"), Some("fill")).unwrap();
        let m = super::get(&c, std::path::Path::new("/nowhere"), id).unwrap();
        assert_eq!((m.repeat.as_deref(), m.fit.as_deref()), (Some("once"), Some("fill")));
        assert!(super::set_options(&c, id, Some("forever"), None).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(name: &str) -> (Connection, PathBuf, PathBuf) {
        let d = std::env::temp_dir().join(format!("wp-media-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap();
        (crate::db::open_memory().unwrap(), d.join("media"), d.join("src"))
    }

    #[test]
    fn import_list_dedupe_thumb_delete() {
        let (c, media, src) = setup("cycle");
        std::fs::write(src.join("Sunrise_Worship.mp4"), vec![0u8; 1000]).unwrap();
        std::fs::write(src.join("Church Logo.png"), vec![1u8; 50]).unwrap();
        std::fs::write(src.join("notes.txt"), b"x").unwrap();

        let v = import(&c, &media, &src.join("Sunrise_Worship.mp4")).unwrap();
        assert_eq!((v.kind.as_str(), v.path.as_str(), v.title.as_str(), v.bytes), ("video", "videos/Sunrise_Worship.mp4", "Sunrise Worship", 1000));
        assert!(media.join("videos/Sunrise_Worship.mp4").exists());
        assert_eq!(import(&c, &media, &src.join("Sunrise_Worship.mp4")).unwrap().id, v.id, "same file is not imported twice");
        let logo = import(&c, &media, &src.join("Church Logo.png")).unwrap();
        assert_eq!(logo.path, "images/Church-Logo.png");
        assert!(import(&c, &media, &src.join("notes.txt")).is_err());

        set_meta(&c, v.id, Some(12_000), Some(1920), Some(1080)).unwrap();
        save_thumb(&c, &media, v.id, b"\xff\xd8jpeg").unwrap();
        let l = list(&c, &media, Some("video"), "").unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].duration_ms, l[0].thumb.as_deref()), (Some(12_000), Some("thumbs/1.jpg")));
        assert_eq!(list(&c, &media, None, "logo").unwrap().len(), 1);

        c.execute(
            "INSERT INTO theme (name, kind, json) VALUES ('Sunrise', 'song', ?1)",
            [r#"{"background":{"kind":"video","video":"videos/Sunrise_Worship.mp4"}}"#],
        )
        .unwrap();
        assert_eq!(used_by(&c, &media, v.id).unwrap(), vec!["Sunrise".to_string()]);
        std::fs::remove_file(media.join(&v.path)).unwrap();
        assert!(get(&c, &media, v.id).unwrap().missing, "a removed file shows as missing");
        assert!(relink(&c, &media, v.id, &src.join("Church Logo.png")).is_err(), "relink keeps the kind");
        std::fs::write(src.join("new.mp4"), vec![2u8; 77]).unwrap();
        let r = relink(&c, &media, v.id, &src.join("new.mp4")).unwrap();
        assert!(!r.missing && r.bytes == 77 && r.path == v.path && r.thumb.is_none());
        delete(&c, &media, v.id).unwrap();
        assert!(!media.join("videos/Sunrise_Worship.mp4").exists() && !media.join("thumbs/1.jpg").exists());
    }

    #[test]
    fn copies_media_folder() {
        let (_c, media, src) = setup("copy");
        std::fs::create_dir_all(media.join("images")).unwrap();
        std::fs::write(media.join("images/a.png"), b"a").unwrap();
        copy_dir(&media, &src.join("copy")).unwrap();
        assert!(src.join("copy/images/a.png").exists());
    }
}

/// True for a path like "images/logo.png": relative, made only of ordinary names
/// (no "..", no root, no drive letter, no UNC share), in either slash style.
pub fn is_plain_relative(path: &str) -> bool {
    let unified = path.replace('\\', "/");
    if unified.is_empty() || unified.starts_with('/') || unified.contains(':') {
        return false;
    }
    unified.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}
