//! Service orders: saved by date, reusable as templates, auto-saved on every change.

use crate::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// One entry in a service order. Mirrors the control window's ProgramItem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    /// `theme_id`: a look for this item only; None uses the theme in use.
    Song {
        song_id: i64,
        title: String,
        #[serde(default)]
        theme_id: Option<i64>,
        /// Lyrics changed for this service only ("Apply to this service"); None uses the library.
        #[serde(default)]
        lyrics: Option<Vec<crate::songs::Lyric>>,
    },
    Scripture {
        reference: String,
        versions: Vec<String>,
        #[serde(default)]
        theme_id: Option<i64>,
    },
    Media { media_id: i64, title: String },
    Announcement { presentation_id: i64, title: String },
}

fn theme_override(json: &Option<String>) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(json.as_deref()?).ok()?;
    v.get("theme_id")?.as_i64()
}

fn lyrics_override(json: &Option<String>) -> Option<Vec<crate::songs::Lyric>> {
    let v: serde_json::Value = serde_json::from_str(json.as_deref()?).ok()?;
    serde_json::from_value(v.get("lyrics")?.clone()).ok()
}

fn override_json(theme_id: Option<i64>) -> Option<String> {
    override_full(theme_id, &None)
}

fn override_full(theme_id: Option<i64>, lyrics: &Option<Vec<crate::songs::Lyric>>) -> Option<String> {
    if theme_id.is_none() && lyrics.is_none() {
        return None;
    }
    Some(serde_json::json!({ "theme_id": theme_id, "lyrics": lyrics }).to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: i64,
    pub name: String,
    pub service_date: Option<String>,
    pub template: bool,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub id: i64,
    pub name: String,
    pub service_date: Option<String>,
    pub template: bool,
    pub items: i64,
}

pub fn list(conn: &Connection) -> Result<Vec<Summary>> {
    let mut st = conn.prepare(
        "SELECT s.id, s.name, s.service_date, s.template, (SELECT count(*) FROM schedule_item i WHERE i.schedule_id = s.id)
         FROM schedule s ORDER BY s.template DESC, coalesce(s.service_date, s.created_at) DESC, s.id DESC",
    )?;
    let v = st
        .query_map([], |r| Ok(Summary { id: r.get(0)?, name: r.get(1)?, service_date: r.get(2)?, template: r.get::<_, i64>(3)? != 0, items: r.get(4)? }))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(v)
}

pub fn create(conn: &Connection, name: &str, service_date: Option<&str>, template: bool) -> Result<i64> {
    conn.execute("INSERT INTO schedule (name, service_date, template) VALUES (?1, ?2, ?3)", params![name, service_date, template])?;
    Ok(conn.last_insert_rowid())
}

/// A new dated service order filled from a template.
pub fn from_template(conn: &Connection, template_id: i64, name: &str, service_date: Option<&str>) -> Result<i64> {
    let t = load(conn, template_id)?;
    let id = create(conn, name, service_date, false)?;
    save_items(conn, id, &t.items)?;
    Ok(id)
}

pub fn load(conn: &Connection, id: i64) -> Result<Schedule> {
    let (name, service_date, template): (String, Option<String>, i64) =
        conn.query_row("SELECT name, service_date, template FROM schedule WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut st = conn.prepare("SELECT kind, ref_id, ref_text, override_json FROM schedule_item WHERE schedule_id = ?1 ORDER BY idx")?;
    let mut items = Vec::new();
    for row in st.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?)))? {
        let (kind, ref_id, ref_text, ov) = row?;
        // a look deleted since saving falls back to the theme in use
        let theme_id = theme_override(&ov).filter(|t| conn.query_row("SELECT 1 FROM theme WHERE id = ?1", [t], |_| Ok(())).is_ok());
        match kind.as_str() {
            "song" => {
                let Some(song_id) = ref_id else { continue };
                // a song deleted from the library since saving is skipped, not an error
                if let Ok(title) = conn.query_row("SELECT title FROM song WHERE id = ?1", [song_id], |r| r.get::<_, String>(0)) {
                    items.push(Item::Song { song_id, title, theme_id, lyrics: lyrics_override(&ov) });
                }
            }
            "scripture" => {
                let text = ref_text.unwrap_or_default();
                let (reference, versions) = text.split_once('|').unwrap_or((&text, ""));
                items.push(Item::Scripture {
                    reference: reference.to_string(),
                    versions: versions.split(',').filter(|v| !v.is_empty()).map(str::to_string).collect(),
                    theme_id,
                });
            }
            "presentation" => {
                let Some(pid) = ref_id else { continue };
                if let Ok(title) = conn.query_row("SELECT title FROM presentation WHERE id = ?1", [pid], |r| r.get::<_, String>(0)) {
                    items.push(Item::Announcement { presentation_id: pid, title });
                }
            }
            "media" => {
                let Some(media_id) = ref_id else { continue };
                // a picture or video deleted from the library since saving is skipped
                if let Ok(title) = conn.query_row("SELECT title FROM media WHERE id = ?1", [media_id], |r| r.get::<_, String>(0)) {
                    items.push(Item::Media { media_id, title });
                }
            }
            _ => {}
        }
    }
    Ok(Schedule { id, name, service_date, template: template != 0, items })
}

/// Replaces a schedule's items in one transaction (called on every edit).
pub fn save_items(conn: &Connection, id: i64, items: &[Item]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM schedule_item WHERE schedule_id = ?1", [id])?;
    {
        let mut st = tx.prepare("INSERT INTO schedule_item (schedule_id, idx, kind, ref_id, ref_text, override_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
        for (i, it) in items.iter().enumerate() {
            match it {
                Item::Song { song_id, theme_id, lyrics, .. } => st.execute(params![id, i as i64, "song", song_id, Option::<String>::None, override_full(*theme_id, lyrics)])?,
                Item::Scripture { reference, versions, theme_id } => st.execute(params![
                    id,
                    i as i64,
                    "scripture",
                    Option::<i64>::None,
                    format!("{reference}|{}", versions.join(",")),
                    override_json(*theme_id)
                ])?,
                Item::Media { media_id, .. } => st.execute(params![id, i as i64, "media", media_id, Option::<String>::None, Option::<String>::None])?,
                Item::Announcement { presentation_id, .. } => st.execute(params![id, i as i64, "presentation", presentation_id, Option::<String>::None, Option::<String>::None])?,
            };
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn rename(conn: &Connection, id: i64, name: &str, service_date: Option<&str>, template: bool) -> Result<()> {
    conn.execute("UPDATE schedule SET name = ?2, service_date = ?3, template = ?4 WHERE id = ?1", params![id, name, service_date, template])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM schedule WHERE id = ?1", [id])?;
    Ok(())
}

/// Removes empty "Untitled service" orders left by an earlier version that created
/// one each time the app opened. Named or non-empty orders are never touched.
pub fn prune_empty_untitled(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM schedule
         WHERE template = 0
           AND (name = 'Untitled service' OR name GLOB 'Service [A-Z][a-z][a-z], *')
           AND NOT EXISTS (SELECT 1 FROM schedule_item i WHERE i.schedule_id = schedule.id)",
        [],
    )?)
}

/// Records that songs in a schedule were used (feeds CCLI usage reports).
pub fn log_usage(conn: &Connection, schedule_id: Option<i64>, song_id: i64) -> Result<()> {
    conn.execute("INSERT INTO usage_log (song_id, schedule_id) VALUES (?1, ?2)", params![song_id, schedule_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prunes_only_empty_auto_named_orders() {
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO song (id, title) VALUES (1, 'MHB 1')", []).unwrap();
        let empty1 = create(&c, "Untitled service", Some("2026-11-01"), false).unwrap();
        let _empty2 = create(&c, "Service Sun, Nov 1", Some("2026-11-01"), false).unwrap();
        let kept_items = create(&c, "Untitled service", Some("2026-11-01"), false).unwrap();
        save_items(&c, kept_items, &[Item::Song { song_id: 1, title: "MHB 1".into(), theme_id: None, lyrics: None }]).unwrap();
        let _kept_named = create(&c, "Service of lessons and carols", None, false).unwrap();
        let _kept_template = create(&c, "Untitled service", None, true).unwrap();
        assert_eq!(prune_empty_untitled(&c).unwrap(), 2);
        assert!(load(&c, empty1).is_err());
        assert_eq!(list(&c).unwrap().len(), 3);
    }

    #[test]
    fn save_load_template_and_missing_song() {
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO song (id, title) VALUES (7, 'MHB 1'), (8, 'MHB 50')", []).unwrap();
        c.execute("INSERT INTO media (id, kind, path, title) VALUES (3, 'video', 'videos/a.mp4', 'Countdown')", []).unwrap();
        let theme: i64 = c.query_row("SELECT id FROM theme WHERE kind = 'song'", [], |r| r.get(0)).unwrap();
        let t = create(&c, "Sunday first service", None, true).unwrap();
        c.execute("INSERT INTO presentation (id, title, kind) VALUES (5, 'Offering', 'builtin')", []).unwrap();
        let items = vec![
            Item::Media { media_id: 3, title: "Countdown".into() },
            Item::Announcement { presentation_id: 5, title: "Offering".into() },
            Item::Song { song_id: 7, title: "MHB 1".into(), theme_id: Some(theme), lyrics: Some(vec![crate::songs::Lyric { lang: "en".into(), body: "[V1]\nJust for today".into() }]) },
            Item::Scripture { reference: "Jn 3:16-18".into(), versions: vec!["NKJV".into(), "TK".into()], theme_id: None },
            Item::Song { song_id: 8, title: "MHB 50".into(), theme_id: None, lyrics: None },
        ];
        save_items(&c, t, &items).unwrap();
        assert_eq!(load(&c, t).unwrap().items, items);

        let s = from_template(&c, t, "Sunday 8 Nov", Some("2026-11-08")).unwrap();
        assert_eq!(load(&c, s).unwrap().items.len(), 5);
        let l = list(&c).unwrap();
        assert!(l[0].template && l[1].service_date.as_deref() == Some("2026-11-08"));

        c.execute("DELETE FROM song WHERE id = 8", []).unwrap();
        assert_eq!(load(&c, s).unwrap().items.len(), 4, "deleted song skipped");
        // per-item look survives saving; a deleted look falls back to the default
        assert!(matches!(load(&c, s).unwrap().items[2], Item::Song { theme_id: Some(t), .. } if t == theme));
        let copy = crate::themes::duplicate(&c, theme).unwrap();
        save_items(&c, s, &[Item::Song { song_id: 7, title: "MHB 1".into(), theme_id: Some(copy), lyrics: None }]).unwrap();
        crate::themes::delete(&c, copy).unwrap();
        assert!(matches!(load(&c, s).unwrap().items[0], Item::Song { theme_id: None, .. }));
        delete(&c, s).unwrap();
        assert_eq!(list(&c).unwrap().len(), 1);
    }
}
