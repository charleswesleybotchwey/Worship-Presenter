//! Profiles: one library per church or venue.
//!
//! Each profile is a folder under `<app data>/profiles/<slug>/` holding library.db,
//! media/, backups/ and state.json, plus profile.json with its display name. The
//! active profile is remembered in `profiles/active`. Deleting a profile moves its
//! folder to `profiles/.deleted/`, so a mistaken delete can be recovered by hand.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub slug: String,
    pub name: String,
    pub active: bool,
    pub songs: i64,
    pub bibles: i64,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    name: String,
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Profile".into(), "profile", msg.into())
}

pub fn slugify(name: &str) -> String {
    let s: String = crate::text::fold(name).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "profile".into() } else { s.chars().take(40).collect() }
}

pub fn dir_of(root: &Path, slug: &str) -> PathBuf {
    root.join(slug)
}

fn read_name(dir: &Path) -> Option<String> {
    let m: Meta = serde_json::from_slice(&std::fs::read(dir.join("profile.json")).ok()?).ok()?;
    Some(m.name)
}

fn write_name(dir: &Path, name: &str) -> Result<()> {
    std::fs::write(dir.join("profile.json"), serde_json::to_vec_pretty(&Meta { name: name.to_string() })?)?;
    Ok(())
}

pub fn active(root: &Path) -> String {
    std::fs::read_to_string(root.join("active")).map(|s| s.trim().to_string()).ok().filter(|s| root.join(s).join("library.db").exists()).unwrap_or_else(|| "default".into())
}

pub fn set_active(root: &Path, slug: &str) -> Result<()> {
    if !root.join(slug).join("library.db").exists() {
        return Err(bad("That profile no longer exists."));
    }
    std::fs::write(root.join("active"), slug)?;
    Ok(())
}

/// Makes sure the default profile exists (first run) and returns the active slug.
pub fn ensure_default(root: &Path) -> Result<String> {
    let d = root.join("default");
    std::fs::create_dir_all(d.join("media"))?;
    if read_name(&d).is_none() {
        write_name(&d, "Main")?;
    }
    if !d.join("library.db").exists() {
        crate::db::open(&d.join("library.db"))?;
    }
    Ok(active(root))
}

pub fn list(root: &Path) -> Result<Vec<Profile>> {
    let act = active(root);
    let mut out = Vec::new();
    for e in std::fs::read_dir(root)?.filter_map(|e| e.ok()) {
        let p = e.path();
        let slug = e.file_name().to_string_lossy().into_owned();
        if slug.starts_with('.') || !p.join("library.db").exists() {
            continue;
        }
        let (songs, bibles) = rusqlite::Connection::open_with_flags(p.join("library.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()
            .and_then(|c| {
                Some((
                    c.query_row("SELECT count(*) FROM song", [], |r| r.get(0)).ok()?,
                    c.query_row("SELECT count(*) FROM bible_version", [], |r| r.get(0)).ok()?,
                ))
            })
            .unwrap_or((0, 0));
        out.push(Profile { name: read_name(&p).unwrap_or_else(|| slug.clone()), active: slug == act, slug, songs, bibles });
    }
    out.sort_by(|a, b| b.active.cmp(&a.active).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

/// Creates a profile. With `copy_from`, it starts as a consistent copy of that
/// profile's library (songs, Bibles, service orders), taken with the backup API.
pub fn create(root: &Path, name: &str, copy_from: Option<&rusqlite::Connection>) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(bad("A profile needs a name."));
    }
    if list(root)?.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
        return Err(bad(format!("There is already a profile called “{name}”.")));
    }
    let base = slugify(name);
    let mut slug = base.clone();
    let mut n = 2;
    while root.join(&slug).exists() {
        slug = format!("{base}-{n}");
        n += 1;
    }
    let d = root.join(&slug);
    std::fs::create_dir_all(d.join("media"))?;
    write_name(&d, name)?;
    match copy_from {
        Some(src) => {
            crate::db::backup(src, &d.join("library.db"))?;
            // a copy starts with no open service and no backups of its own
            let c = crate::db::open(&d.join("library.db"))?;
            c.execute("DELETE FROM setting WHERE key = 'last_schedule'", [])?;
            // the copy needs the pictures and videos its themes and media items point at
            if let Some(src_dir) = src.path().and_then(|p| Path::new(p).parent().map(Path::to_path_buf)) {
                if src_dir.join("media").exists() {
                    crate::media::copy_dir(&src_dir.join("media"), &d.join("media"))?;
                }
            }
        }
        None => {
            crate::db::open(&d.join("library.db"))?;
        }
    }
    Ok(slug)
}

pub fn rename(root: &Path, slug: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(bad("A profile needs a name."));
    }
    if list(root)?.iter().any(|p| p.slug != slug && p.name.eq_ignore_ascii_case(name)) {
        return Err(bad(format!("There is already a profile called “{name}”.")));
    }
    write_name(&root.join(slug), name)
}

/// Moves a profile to profiles/.deleted/. The active profile cannot be deleted.
pub fn delete(root: &Path, slug: &str) -> Result<PathBuf> {
    if active(root) == slug {
        return Err(bad("Switch to another profile before deleting this one."));
    }
    let from = root.join(slug);
    if !from.join("library.db").exists() {
        return Err(bad("That profile no longer exists."));
    }
    let bin = root.join(".deleted");
    std::fs::create_dir_all(&bin)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let to = bin.join(format!("{slug}-{stamp}"));
    std::fs::rename(&from, &to)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wp-prof-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lifecycle() {
        let r = root("life");
        assert_eq!(ensure_default(&r).unwrap(), "default");
        let main = crate::db::open(&r.join("default/library.db")).unwrap();
        main.execute("INSERT INTO song (title) VALUES ('MHB 1')", []).unwrap();
        main.execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('last_schedule', '3')", []).unwrap();

        std::fs::create_dir_all(r.join("default/media/images")).unwrap();
        std::fs::write(r.join("default/media/images/logo.png"), b"png").unwrap();
        let youth = create(&r, "Youth Service", None).unwrap();
        assert_eq!(youth, "youth-service");
        let copy = create(&r, "Akwatia Branch", Some(&main)).unwrap();
        let c = crate::db::open(&r.join(&copy).join("library.db")).unwrap();
        let n: i64 = c.query_row("SELECT count(*) FROM song", [], |x| x.get(0)).unwrap();
        assert_eq!(n, 1, "copy carries the songs");
        assert!(r.join(&copy).join("media/images/logo.png").exists(), "copy carries the media files");
        assert!(!r.join(&youth).join("media/images/logo.png").exists(), "an empty profile does not");
        let last: Option<String> = c.query_row("SELECT value FROM setting WHERE key='last_schedule'", [], |x| x.get(0)).ok();
        assert!(last.is_none(), "copy does not reopen the other profile's service");

        assert!(create(&r, "youth service", None).is_err(), "names are unique");
        let l = list(&r).unwrap();
        assert_eq!(l.len(), 3);
        assert!(l[0].active && l[0].name == "Main" && l[0].songs == 1);

        set_active(&r, &youth).unwrap();
        assert_eq!(active(&r), youth);
        assert!(delete(&r, &youth).is_err(), "active profile cannot be deleted");
        rename(&r, &copy, "Akwatia").unwrap();
        let moved = delete(&r, &copy).unwrap();
        assert!(moved.join("library.db").exists(), "deleted profile is recoverable");
        assert_eq!(list(&r).unwrap().len(), 2);
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Ɔdomankoma Chapel!"), "odomankoma-chapel");
        assert_eq!(slugify("  "), "profile");
    }
}
