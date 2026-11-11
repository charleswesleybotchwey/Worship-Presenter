//! Themes ("looks"): a background, a transition and a stack of layers, stored as
//! JSON in the `theme` table. The layer model is defined by the renderer in the UI
//! (ui/src/theme/types.ts); here we store, validate the essentials, seed the
//! built-ins, and track the default theme for each kind.

use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

pub const KINDS: &[&str] = &["song", "scripture", "announcement", "lower_third", "foldback"];

const BUILTINS: &[&str] = &[
    include_str!("../themes/classic.json"),
    include_str!("../themes/scripture.json"),
    include_str!("../themes/lower-third.json"),
    include_str!("../themes/announcement.json"),
];

#[derive(Debug, Clone, Serialize)]
pub struct ThemeRow {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub is_default: bool,
    pub theme: Value,
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Theme".into(), "theme", msg.into())
}

/// Checks the parts every renderer relies on. Returns (name, kind).
pub fn validate(t: &Value) -> Result<(String, String)> {
    let name = t.get("name").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string();
    if name.is_empty() {
        return Err(bad("A theme needs a name."));
    }
    let kind = t.get("kind").and_then(Value::as_str).unwrap_or_default().to_string();
    if !KINDS.contains(&kind.as_str()) {
        return Err(bad(format!("Unknown theme type “{kind}”.")));
    }
    let layers = t.get("layers").and_then(Value::as_array).ok_or_else(|| bad("A theme needs a list of layers."))?;
    let mut ids = std::collections::HashSet::new();
    for l in layers {
        let id = l.get("id").and_then(Value::as_str).ok_or_else(|| bad("Every layer needs an id."))?;
        if !ids.insert(id.to_string()) {
            return Err(bad(format!("Two layers share the id “{id}”.")));
        }
        let k = l.get("kind").and_then(Value::as_str).unwrap_or_default();
        if !["text", "shape", "image", "ticker"].contains(&k) {
            return Err(bad(format!("Layer “{id}” has an unknown type “{k}”.")));
        }
        for f in ["x", "y", "w", "h"] {
            if l.get(f).and_then(Value::as_f64).is_none() {
                return Err(bad(format!("Layer “{id}” is missing its {f} position.")));
            }
        }
    }
    let size = serde_json::to_vec(t)?.len();
    if size > 12_000_000 {
        return Err(bad("This theme is over 12 MB, usually because of a large image. Use a smaller image."));
    }
    Ok((name, kind))
}

/// Adds the built-in themes to an empty library and makes them the defaults.
pub fn seed(conn: &Connection) -> Result<()> {
    let n: i64 = conn.query_row("SELECT count(*) FROM theme", [], |r| r.get(0))?;
    if n > 0 {
        // a library made before a theme kind existed gets that kind's built-in once
        for src in BUILTINS {
            let v: Value = serde_json::from_str(src)?;
            let kind = v["kind"].as_str().unwrap_or_default();
            let have: i64 = conn.query_row("SELECT count(*) FROM theme WHERE kind = ?1", [kind], |r| r.get(0))?;
            if have == 0 {
                let id = save(conn, None, &v)?;
                set_default(conn, kind, id)?;
            }
        }
        return Ok(());
    }
    for src in BUILTINS {
        let v: Value = serde_json::from_str(src)?;
        let id = save(conn, None, &v)?;
        let kind = v["kind"].as_str().unwrap_or_default();
        set_default(conn, kind, id)?;
    }
    Ok(())
}

pub fn save(conn: &Connection, id: Option<i64>, theme: &Value) -> Result<i64> {
    let (name, kind) = validate(theme)?;
    let json = serde_json::to_string(theme)?;
    let version = theme.get("schema_version").and_then(Value::as_i64).unwrap_or(1);
    match id {
        Some(id) => {
            let n = conn.execute("UPDATE theme SET name=?2, kind=?3, schema_version=?4, json=?5 WHERE id=?1", params![id, name, kind, version, json])?;
            if n == 0 {
                return Err(bad("This theme was deleted while it was being edited."));
            }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO theme (name, kind, schema_version, json) VALUES (?1, ?2, ?3, ?4)", params![name, kind, version, json])?;
            Ok(conn.last_insert_rowid())
        }
    }
}

fn default_id(conn: &Connection, kind: &str) -> Option<i64> {
    conn.query_row("SELECT value FROM setting WHERE key = ?1", [format!("theme_default_{kind}")], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|v| v.parse().ok())
}

pub fn set_default(conn: &Connection, kind: &str, id: i64) -> Result<()> {
    conn.execute("INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2)", params![format!("theme_default_{kind}"), id.to_string()])?;
    Ok(())
}

pub fn list(conn: &Connection) -> Result<Vec<ThemeRow>> {
    let mut st = conn.prepare("SELECT id, name, kind, json FROM theme ORDER BY kind, name")?;
    let rows: Vec<(i64, String, String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
    let mut out = Vec::new();
    for (id, name, kind, json) in rows {
        let is_default = default_id(conn, &kind) == Some(id);
        out.push(ThemeRow { id, name, kind, is_default, theme: serde_json::from_str(&json)? });
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: i64) -> Result<Value> {
    let json: String = conn.query_row("SELECT json FROM theme WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(serde_json::from_str(&json)?)
}

/// The theme to use for a kind: the chosen default, else any theme of that kind,
/// else the built-in one (so output never lacks a theme).
pub fn for_kind(conn: &Connection, kind: &str) -> Result<Value> {
    if let Some(id) = default_id(conn, kind) {
        if let Ok(t) = get(conn, id) {
            return Ok(t);
        }
    }
    if let Ok(json) = conn.query_row("SELECT json FROM theme WHERE kind = ?1 ORDER BY id LIMIT 1", [kind], |r| r.get::<_, String>(0)) {
        return Ok(serde_json::from_str(&json)?);
    }
    let builtin = BUILTINS.iter().filter_map(|s| serde_json::from_str::<Value>(s).ok()).find(|v| v["kind"] == kind);
    Ok(builtin.unwrap_or_else(|| serde_json::from_str(BUILTINS[0]).expect("built-in theme is valid")))
}

pub fn duplicate(conn: &Connection, id: i64) -> Result<i64> {
    let mut t = get(conn, id)?;
    let name = format!("{} copy", t["name"].as_str().unwrap_or("Theme"));
    t["name"] = Value::String(name);
    save(conn, None, &t)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let kind: String = conn.query_row("SELECT kind FROM theme WHERE id = ?1", [id], |r| r.get(0))?;
    let others: i64 = conn.query_row("SELECT count(*) FROM theme WHERE kind = ?1 AND id <> ?2", params![kind, id], |r| r.get(0))?;
    if others == 0 {
        return Err(bad("This is the only theme of its type. Make another one before deleting it."));
    }
    conn.execute("DELETE FROM theme WHERE id = ?1", [id])?;
    if default_id(conn, &kind) == Some(id) {
        let next: i64 = conn.query_row("SELECT id FROM theme WHERE kind = ?1 ORDER BY id LIMIT 1", [&kind], |r| r.get(0))?;
        set_default(conn, &kind, next)?;
    }
    Ok(())
}

// ---------- sharing themes between churches (.wptheme) ----------

pub const FORMAT: &str = "worship-presenter-theme";

fn media_refs(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            if let Some(p) = s.strip_prefix("media:") {
                if !out.iter().any(|x| x == p) {
                    out.push(p.to_string());
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| media_refs(x, out)),
        Value::Object(o) => o.values().for_each(|x| media_refs(x, out)),
        _ => {}
    }
}

/// A .wptheme file: the theme plus every library file it uses, so it works on
/// another computer. Pictures stored inside the theme are already self-contained.
pub fn export(conn: &Connection, media_dir: &std::path::Path, id: i64) -> Result<Vec<u8>> {
    use base64::Engine;
    let theme = get(conn, id)?;
    let mut refs = Vec::new();
    media_refs(&theme, &mut refs);
    let mut files = serde_json::Map::new();
    for r in refs {
        // thumbnails are remade on the other side; a missing file is left out, not fatal
        if let Ok(bytes) = std::fs::read(media_dir.join(&r)) {
            files.insert(r, Value::String(base64::engine::general_purpose::STANDARD.encode(bytes)));
        }
    }
    Ok(serde_json::to_vec(&serde_json::json!({ "format": FORMAT, "version": 1, "theme": theme, "files": files }))?)
}

/// Adds a .wptheme to this library as a new theme and returns (id, name). Its files
/// go into the media library; a file already here with the same bytes is reused.
pub fn import(conn: &Connection, media_dir: &std::path::Path, bytes: &[u8]) -> Result<(i64, String)> {
    use base64::Engine;
    let pkg: Value = serde_json::from_slice(bytes).map_err(|_| bad("This is not a Worship Presenter theme file."))?;
    if pkg.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(bad("This is not a Worship Presenter theme file."));
    }
    let mut json = serde_json::to_string(pkg.get("theme").ok_or_else(|| bad("The theme file is empty."))?)?;
    if let Some(files) = pkg.get("files").and_then(Value::as_object) {
        for (path, data) in files {
            let data = base64::engine::general_purpose::STANDARD
                .decode(data.as_str().unwrap_or_default())
                .map_err(|_| bad("A file inside the theme is damaged."))?;
            let p = std::path::Path::new(path);
            let kind = crate::media::kind_of(p).ok_or_else(|| bad(format!("The theme contains an unsupported file ({path}).")))?;
            // only a plain relative path may come from a theme file: no "..", no drive
            // letter or leading slash (joining an absolute path would replace the media
            // folder and write anywhere on the computer)
            if !crate::media::is_plain_relative(path) {
                return Err(bad("The theme contains an unsafe file path."));
            }
            // reuse an identical file already here (same name or an earlier renamed copy)
            let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let ext = p.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let dir = p.parent().map(|d| d.to_string_lossy().replace('\\', "/")).filter(|d| !d.is_empty()).unwrap_or_else(|| format!("{kind}s"));
            let mut rel = path.clone();
            let mut n = 2;
            let mut reuse = false;
            while media_dir.join(&rel).exists() {
                if std::fs::read(media_dir.join(&rel)).map(|b| b == data).unwrap_or(false) {
                    reuse = true;
                    break;
                }
                rel = format!("{dir}/{stem}-{n}.{ext}");
                n += 1;
            }
            if !reuse {
                std::fs::create_dir_all(media_dir.join(&rel).parent().unwrap_or(media_dir))?;
                std::fs::write(media_dir.join(&rel), &data)?;
                if !path.starts_with("thumbs/") {
                    conn.execute("INSERT INTO media (kind, path, title) VALUES (?1, ?2, ?3)", params![kind, rel, stem.replace(['-', '_'], " ")])?;
                }
            }
            if rel != *path {
                json = json.replace(&format!("\"media:{path}\""), &format!("\"media:{rel}\""));
            }
        }
    }
    let mut theme: Value = serde_json::from_str(&json)?;
    let mut name = theme["name"].as_str().unwrap_or("Imported theme").to_string();
    let taken = |n: &str| conn.query_row("SELECT 1 FROM theme WHERE name = ?1", [n], |_| Ok(())).is_ok();
    if taken(&name) {
        let base = format!("{name} (imported)");
        name = base.clone();
        let mut i = 2;
        while taken(&name) {
            name = format!("{base} {i}");
            i += 1;
        }
    }
    theme["name"] = Value::String(name.clone());
    let id = save(conn, None, &theme)?;
    Ok((id, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_files_cannot_escape_the_media_folder() {
        for evil in ["../x.png", "C:/Users/x/AppData/x.png", "/etc/x.png", "\\\\server\\share\\x.png", "images/../../x.png", "C:x.png"] {
            assert!(!crate::media::is_plain_relative(evil), "{evil}");
        }
        assert!(crate::media::is_plain_relative("images/church logo.png"));
    }

    #[test]
    fn builtins_are_valid_and_seeded_once() {
        for b in BUILTINS {
            validate(&serde_json::from_str(b).unwrap()).unwrap();
        }
        let c = crate::db::open_memory().unwrap();
        seed(&c).unwrap();
        seed(&c).unwrap();
        let l = list(&c).unwrap();
        assert_eq!(l.len(), 4);
        assert!(l.iter().all(|t| t.is_default));
        assert_eq!(for_kind(&c, "lower_third").unwrap()["name"], "Lower third");
    }

    #[test]
    fn duplicate_edit_default_delete() {
        let c = crate::db::open_memory().unwrap();
        seed(&c).unwrap();
        let classic = list(&c).unwrap().into_iter().find(|t| t.kind == "song").unwrap().id;
        assert!(delete(&c, classic).is_err(), "cannot delete the only song theme");
        let copy = duplicate(&c, classic).unwrap();
        let mut t = get(&c, copy).unwrap();
        assert_eq!(t["name"], "Classic lyrics copy");
        t["name"] = "Blue".into();
        t["background"]["color"] = "#0b2a5b".into();
        save(&c, Some(copy), &t).unwrap();
        set_default(&c, "song", copy).unwrap();
        assert_eq!(for_kind(&c, "song").unwrap()["name"], "Blue");
        delete(&c, copy).unwrap();
        assert_eq!(for_kind(&c, "song").unwrap()["name"], "Classic lyrics", "default falls back");
    }

    #[test]
    fn export_import_carries_media_between_libraries() {
        let base = std::env::temp_dir().join(format!("wp-theme-pkg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (ma, mb) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(ma.join("videos")).unwrap();
        std::fs::write(ma.join("videos/loop.mp4"), b"VIDEO-A").unwrap();

        let a = crate::db::open_memory().unwrap();
        let mut t = for_kind(&a, "song").unwrap();
        t["name"] = "Sunrise".into();
        t["background"] = serde_json::json!({"kind":"video","video":"media:videos/loop.mp4","poster":null,"color":"#000","image":null,"fit":"cover","dim":0.3});
        let id = save(&a, None, &t).unwrap();
        let pkg = export(&a, &ma, id).unwrap();

        // library B already has a different file with the same name
        let b = crate::db::open_memory().unwrap();
        std::fs::create_dir_all(mb.join("videos")).unwrap();
        std::fs::write(mb.join("videos/loop.mp4"), b"OTHER").unwrap();
        let (bid, name) = import(&b, &mb, &pkg).unwrap();
        assert_eq!(name, "Sunrise");
        let got = get(&b, bid).unwrap();
        assert_eq!(got["background"]["video"], "media:videos/loop-2.mp4", "renamed, not overwritten");
        assert_eq!(std::fs::read(mb.join("videos/loop-2.mp4")).unwrap(), b"VIDEO-A");
        assert_eq!(std::fs::read(mb.join("videos/loop.mp4")).unwrap(), b"OTHER");
        let n: i64 = b.query_row("SELECT count(*) FROM media", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "the video is in B's media library");

        // importing again reuses the identical file and keeps names unique
        let (id2, name2) = import(&b, &mb, &pkg).unwrap();
        assert_eq!(name2, "Sunrise (imported)");
        assert_eq!(get(&b, id2).unwrap()["background"]["video"], "media:videos/loop-2.mp4", "identical file reused");
        let n: i64 = b.query_row("SELECT count(*) FROM media", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "no duplicate copy of the video");
        assert!(!mb.join("videos/loop-3.mp4").exists());
        assert!(import(&b, &mb, b"{\"format\":\"something-else\"}").is_err());
    }

    #[test]
    fn rejects_broken_themes() {
        let mut t: Value = serde_json::from_str(BUILTINS[0]).unwrap();
        t["layers"][1]["id"] = "lyrics".into();
        assert!(validate(&t).unwrap_err().to_string().contains("share the id"));
        let mut t: Value = serde_json::from_str(BUILTINS[0]).unwrap();
        t["kind"] = "poster".into();
        assert!(validate(&t).is_err());
        let mut t: Value = serde_json::from_str(BUILTINS[0]).unwrap();
        t["layers"][0].as_object_mut().unwrap().remove("w");
        assert!(validate(&t).unwrap_err().to_string().contains("missing its w"));
    }
}
