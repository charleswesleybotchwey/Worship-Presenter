//! "My collections" for scripture readings and media, as in EasyWorship's sidebars:
//! "Christmas readings", "Easter backgrounds". Each holds program items in order.

use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemCollection {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub count: i64,
}

fn check_kind(kind: &str) -> Result<()> {
    if matches!(kind, "scripture" | "media") {
        Ok(())
    } else {
        Err(Error::Format("Collections".into(), "kind", "Collections are for scripture or media.".into()))
    }
}

pub fn list(conn: &Connection, kind: &str) -> Result<Vec<ItemCollection>> {
    check_kind(kind)?;
    let mut st = conn.prepare(
        "SELECT c.id, c.kind, c.name, (SELECT count(*) FROM item_collection_entry e WHERE e.collection_id = c.id)
         FROM item_collection c WHERE c.kind = ?1 ORDER BY c.name COLLATE NOCASE",
    )?;
    let rows = st.query_map([kind], |r| Ok(ItemCollection { id: r.get(0)?, kind: r.get(1)?, name: r.get(2)?, count: r.get(3)? }))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn create(conn: &Connection, kind: &str, name: &str) -> Result<i64> {
    check_kind(kind)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Format("Collections".into(), "name", "A collection needs a name.".into()));
    }
    conn.execute("INSERT INTO item_collection (kind, name) VALUES (?1, ?2)", params![kind, name])?;
    Ok(conn.last_insert_rowid())
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(Error::Format("Collections".into(), "name", "A collection needs a name.".into()));
    }
    conn.execute("UPDATE item_collection SET name = ?2 WHERE id = ?1", params![id, name.trim()])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM item_collection WHERE id = ?1", [id])?;
    Ok(())
}

/// Adds an item at the end (an item already in the collection is not added twice).
pub fn add(conn: &Connection, id: i64, item: &serde_json::Value) -> Result<()> {
    let text = item.to_string();
    let exists: i64 = conn.query_row("SELECT count(*) FROM item_collection_entry WHERE collection_id = ?1 AND item = ?2", params![id, text], |r| r.get(0))?;
    if exists == 0 {
        let pos: i64 = conn.query_row("SELECT coalesce(max(position), -1) + 1 FROM item_collection_entry WHERE collection_id = ?1", [id], |r| r.get(0))?;
        conn.execute("INSERT INTO item_collection_entry (collection_id, position, item) VALUES (?1, ?2, ?3)", params![id, pos, text])?;
    }
    Ok(())
}

pub fn remove(conn: &Connection, id: i64, item: &serde_json::Value) -> Result<()> {
    conn.execute("DELETE FROM item_collection_entry WHERE collection_id = ?1 AND item = ?2", params![id, item.to_string()])?;
    Ok(())
}

pub fn items(conn: &Connection, id: i64) -> Result<Vec<serde_json::Value>> {
    let mut st = conn.prepare("SELECT item FROM item_collection_entry WHERE collection_id = ?1 ORDER BY position")?;
    let rows = st.query_map([id], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).filter_map(|t| serde_json::from_str(&t).ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_and_media_collections() {
        let c = crate::db::open_memory().unwrap();
        let xmas = create(&c, "scripture", "Christmas readings").unwrap();
        let luke = serde_json::json!({ "kind": "scripture", "reference": "Luke 2:1-14", "versions": ["NKJV"] });
        let isa = serde_json::json!({ "kind": "scripture", "reference": "Isaiah 9:6", "versions": ["NKJV", "FNK"] });
        add(&c, xmas, &isa).unwrap();
        add(&c, xmas, &luke).unwrap();
        add(&c, xmas, &isa).unwrap();
        assert_eq!(items(&c, xmas).unwrap(), vec![isa.clone(), luke.clone()], "in order, no repeats");
        assert_eq!(list(&c, "scripture").unwrap()[0].count, 2);
        remove(&c, xmas, &isa).unwrap();
        assert_eq!(items(&c, xmas).unwrap(), vec![luke]);
        let bg = create(&c, "media", "Easter backgrounds").unwrap();
        assert!(list(&c, "media").unwrap().iter().any(|x| x.id == bg));
        assert!(list(&c, "scripture").unwrap().iter().all(|x| x.id != bg), "kept apart");
        rename(&c, bg, "Easter").unwrap();
        delete(&c, xmas).unwrap();
        assert!(items(&c, xmas).unwrap().is_empty(), "entries go with their collection");
        assert!(create(&c, "songs", "x").is_err() && create(&c, "media", "  ").is_err());
    }
}
