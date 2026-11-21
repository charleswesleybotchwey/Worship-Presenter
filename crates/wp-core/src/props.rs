//! Props (as ProPresenter): graphics that stay on screen independently of the slides. A
//! picture (logo bug, "LIVE" badge), a line of text (sermon title) or a clock, placed in a
//! corner or a band, on the screens, the stream or both. Shown and cleared on their own.

use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    pub id: Option<i64>,
    pub name: String,
    /// image | text | clock
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub media_id: Option<i64>,
    /// top-left | top-right | bottom-left | bottom-right | top | bottom | centre
    pub position: String,
    /// width of a picture, or the text size, in % of the screen
    pub size: f32,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default = "white")]
    pub color: String,
    /// a band behind text ("" = none)
    #[serde(default)]
    pub background: String,
    /// all | screens | stream
    #[serde(default = "all")]
    pub on: String,
}

fn one() -> f32 {
    1.0
}
fn white() -> String {
    "#ffffff".into()
}
fn all() -> String {
    "all".into()
}

fn bad(m: &str) -> Error {
    Error::Format("Prop".into(), "prop", m.into())
}

const POSITIONS: [&str; 7] = ["top-left", "top-right", "bottom-left", "bottom-right", "top", "bottom", "centre"];

pub fn list(conn: &Connection) -> Result<Vec<Prop>> {
    let mut st = conn.prepare("SELECT id, json FROM prop ORDER BY name COLLATE NOCASE")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows
        .filter_map(|r| r.ok())
        .filter_map(|(id, j)| serde_json::from_str::<Prop>(&j).ok().map(|p| Prop { id: Some(id), ..p }))
        .collect())
}

pub fn save(conn: &Connection, p: &Prop) -> Result<i64> {
    if p.name.trim().is_empty() {
        return Err(bad("Give the prop a name."));
    }
    if !["image", "text", "clock"].contains(&p.kind.as_str()) || !POSITIONS.contains(&p.position.as_str()) || !["all", "screens", "stream"].contains(&p.on.as_str()) {
        return Err(bad("Unknown kind, place or screens for the prop."));
    }
    if p.kind == "image" && p.media_id.is_none() {
        return Err(bad("Choose a picture for the prop."));
    }
    if !(1.0..=100.0).contains(&p.size) || !(0.0..=1.0).contains(&p.opacity) {
        return Err(bad("The size is 1–100 % and the opacity 0–1."));
    }
    let json = serde_json::to_string(&Prop { id: None, name: p.name.trim().to_string(), ..p.clone() })?;
    match p.id {
        Some(id) => {
            conn.execute("UPDATE prop SET name = ?2, json = ?3 WHERE id = ?1", params![id, p.name.trim(), json])?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO prop (name, json) VALUES (?1, ?2)", params![p.name.trim(), json])?;
            Ok(conn.last_insert_rowid())
        }
    }
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM prop WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(name: &str) -> Prop {
        Prop { id: None, name: name.into(), kind: "text".into(), text: "Sunday service".into(), media_id: None, position: "bottom".into(), size: 4.0, opacity: 1.0, color: "#ffffff".into(), background: "#000000aa".into(), on: "stream".into() }
    }

    #[test]
    fn props_are_kept_and_checked() {
        let c = crate::db::open_memory().unwrap();
        let id = save(&c, &text("Sermon title")).unwrap();
        save(&c, &Prop { kind: "clock".into(), position: "top-right".into(), ..text("Clock") }).unwrap();
        let all = list(&c).unwrap();
        assert_eq!(all.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Clock", "Sermon title"]);
        assert_eq!(all[1].id, Some(id));
        save(&c, &Prop { id: Some(id), text: "Grace upon grace".into(), ..text("Sermon title") }).unwrap();
        assert_eq!(list(&c).unwrap()[1].text, "Grace upon grace");
        assert!(save(&c, &Prop { kind: "image".into(), ..text("Logo") }).is_err(), "a picture prop needs its picture");
        assert!(save(&c, &Prop { position: "sideways".into(), ..text("x") }).is_err());
        delete(&c, id).unwrap();
        assert_eq!(list(&c).unwrap().len(), 1);
    }
}
