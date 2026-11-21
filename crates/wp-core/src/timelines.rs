//! Timelines (as ProPresenter): a song or deck that runs in time with a backing track —
//! recorded by going through the slides while the track plays, then played back by itself.

use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub item_kind: String,
    pub item_id: i64,
    /// the audio track (media library)
    pub media_id: i64,
    /// (ms into the track, slide index), in time order
    pub cues: Vec<(u64, usize)>,
}

fn bad(m: &str) -> Error {
    Error::Format("Timeline".into(), "timeline", m.into())
}

pub fn get(conn: &Connection, item_kind: &str, item_id: i64) -> Result<Option<Timeline>> {
    let row: Option<(i64, String)> = conn
        .query_row("SELECT media_id, cues FROM timeline WHERE item_kind = ?1 AND item_id = ?2", params![item_kind, item_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(row.and_then(|(media_id, cues)| serde_json::from_str(&cues).ok().map(|cues| Timeline { item_kind: item_kind.into(), item_id, media_id, cues })))
}

pub fn save(conn: &Connection, t: &Timeline) -> Result<()> {
    if !["song", "announcement"].contains(&t.item_kind.as_str()) {
        return Err(bad("Timelines are for songs and announcements."));
    }
    if t.cues.is_empty() {
        return Err(bad("Nothing was recorded: go through the slides while the track plays."));
    }
    // in time order (a slide changed twice at the same moment keeps the last)
    let mut cues = t.cues.clone();
    cues.sort_by_key(|c| c.0);
    cues.dedup_by(|b, a| a.0 == b.0 && {
        a.1 = b.1;
        true
    });
    conn.execute(
        "INSERT OR REPLACE INTO timeline (item_kind, item_id, media_id, cues) VALUES (?1, ?2, ?3, ?4)",
        params![t.item_kind, t.item_id, t.media_id, serde_json::to_string(&cues)?],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, item_kind: &str, item_id: i64) -> Result<()> {
    conn.execute("DELETE FROM timeline WHERE item_kind = ?1 AND item_id = ?2", params![item_kind, item_id])?;
    Ok(())
}

/// The slide for a moment in the track: the last cue at or before it (None before the first).
pub fn slide_at(cues: &[(u64, usize)], ms: u64) -> Option<usize> {
    cues.iter().take_while(|c| c.0 <= ms).last().map(|c| c.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timelines_are_kept_in_time_order() {
        let c = crate::db::open_memory().unwrap();
        assert_eq!(get(&c, "song", 3).unwrap(), None);
        let t = Timeline { item_kind: "song".into(), item_id: 3, media_id: 9, cues: vec![(0, 0), (31_500, 2), (14_200, 1), (31_500, 3)] };
        save(&c, &t).unwrap();
        let back = get(&c, "song", 3).unwrap().unwrap();
        assert_eq!(back.cues, vec![(0, 0), (14_200, 1), (31_500, 3)], "in order; a doubled moment keeps the later slide");
        assert_eq!(slide_at(&back.cues, 20_000), Some(1));
        assert_eq!(slide_at(&back.cues, 40_000), Some(3));
        assert!(save(&c, &Timeline { cues: vec![], ..t.clone() }).is_err());
        delete(&c, "song", 3).unwrap();
        assert_eq!(get(&c, "song", 3).unwrap(), None);
    }
}
