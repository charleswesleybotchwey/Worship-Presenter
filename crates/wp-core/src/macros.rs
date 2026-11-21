//! Macros (as ProPresenter): one button that runs several actions in order — e.g. "Walk-in":
//! logo on, the welcome prop, the countdown, the music. And slide actions: a macro run when a
//! particular slide goes live (a song's section, or a deck's slide).

use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// One step: what to do, and its value (a prop id, minutes, a screen and look…).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub kind: String,
    #[serde(default)]
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Macro {
    pub id: Option<i64>,
    pub name: String,
    #[serde(default = "grey")]
    pub color: String,
    pub actions: Vec<Action>,
}

fn grey() -> String {
    "#64748b".into()
}

/// The actions a macro can take.
pub const KINDS: [&str; 15] = [
    "blank", "logo", "clear", "resume", "media_off", "media_on", "prop_on", "prop_off", "props_clear", "next_item", "previous_item", "countdown", "timer_stop", "music_play", "music_stop",
];
const MORE_KINDS: [&str; 3] = ["look", "clear_all", "wait"];

fn bad(m: &str) -> Error {
    Error::Format("Macro".into(), "macro", m.into())
}

pub fn list(conn: &Connection) -> Result<Vec<Macro>> {
    let mut st = conn.prepare("SELECT id, json FROM macro ORDER BY name COLLATE NOCASE")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.filter_map(|r| r.ok()).filter_map(|(id, j)| serde_json::from_str::<Macro>(&j).ok().map(|m| Macro { id: Some(id), ..m })).collect())
}

pub fn save(conn: &Connection, m: &Macro) -> Result<i64> {
    if m.name.trim().is_empty() {
        return Err(bad("Give the macro a name."));
    }
    if m.actions.is_empty() {
        return Err(bad("Add at least one action."));
    }
    if let Some(a) = m.actions.iter().find(|a| !KINDS.contains(&a.kind.as_str()) && !MORE_KINDS.contains(&a.kind.as_str())) {
        return Err(bad(&format!("Unknown action: {}.", a.kind)));
    }
    let json = serde_json::to_string(&Macro { id: None, name: m.name.trim().to_string(), ..m.clone() })?;
    match m.id {
        Some(id) => {
            conn.execute("UPDATE macro SET name = ?2, json = ?3 WHERE id = ?1", params![id, m.name.trim(), json])?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO macro (name, json) VALUES (?1, ?2)", params![m.name.trim(), json])?;
            Ok(conn.last_insert_rowid())
        }
    }
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM slide_action WHERE macro_id = ?1", [id])?;
    conn.execute("DELETE FROM macro WHERE id = ?1", [id])?;
    Ok(())
}

/// A slide's action: run this macro when it goes live (None: no action).
pub fn set_slide_action(conn: &Connection, item_kind: &str, item_id: i64, slide_key: &str, macro_id: Option<i64>) -> Result<()> {
    match macro_id {
        Some(m) => conn.execute(
            "INSERT OR REPLACE INTO slide_action (item_kind, item_id, slide_key, macro_id) VALUES (?1, ?2, ?3, ?4)",
            params![item_kind, item_id, slide_key, m],
        )?,
        None => conn.execute("DELETE FROM slide_action WHERE item_kind = ?1 AND item_id = ?2 AND slide_key = ?3", params![item_kind, item_id, slide_key])?,
    };
    Ok(())
}

/// The slide actions of an item: (slide key, macro id).
pub fn slide_actions(conn: &Connection, item_kind: &str, item_id: i64) -> Result<Vec<(String, i64)>> {
    let mut st = conn.prepare("SELECT slide_key, macro_id FROM slide_action WHERE item_kind = ?1 AND item_id = ?2")?;
    let rows = st.query_map(params![item_kind, item_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk_in() -> Macro {
        Macro {
            id: None,
            name: "Walk-in".into(),
            color: "#22c55e".into(),
            actions: vec![
                Action { kind: "logo".into(), value: serde_json::Value::Null },
                Action { kind: "prop_on".into(), value: serde_json::json!(1) },
                Action { kind: "countdown".into(), value: serde_json::json!(5) },
                Action { kind: "music_play".into(), value: serde_json::Value::Null },
            ],
        }
    }

    #[test]
    fn macros_and_slide_actions() {
        let c = crate::db::open_memory().unwrap();
        let id = save(&c, &walk_in()).unwrap();
        let all = list(&c).unwrap();
        assert_eq!((all.len(), all[0].id, all[0].actions.len()), (1, Some(id), 4));
        assert!(save(&c, &Macro { actions: vec![], ..walk_in() }).is_err(), "no actions");
        assert!(save(&c, &Macro { actions: vec![Action { kind: "launch_rockets".into(), value: serde_json::Value::Null }], ..walk_in() }).is_err());
        set_slide_action(&c, "song", 7, "C", Some(id)).unwrap();
        set_slide_action(&c, "song", 7, "V2", Some(id)).unwrap();
        assert_eq!(slide_actions(&c, "song", 7).unwrap().len(), 2);
        set_slide_action(&c, "song", 7, "V2", None).unwrap();
        assert_eq!(slide_actions(&c, "song", 7).unwrap(), vec![("C".to_string(), id)]);
        delete(&c, id).unwrap();
        assert!(slide_actions(&c, "song", 7).unwrap().is_empty(), "a deleted macro leaves no slide actions");
    }
}
