//! Hymn tunes: a song's tune (a recording of one verse, from the media library), played once
//! per verse with the slides moving in step, for a congregation without an organist.

use crate::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tune {
    pub media_id: i64,
    /// the breath between verses
    pub gap_ms: i64,
}

pub fn get(conn: &Connection, song_id: i64) -> Result<Option<Tune>> {
    Ok(conn.query_row("SELECT media_id, gap_ms FROM song_tune WHERE song_id = ?1", [song_id], |r| Ok(Tune { media_id: r.get(0)?, gap_ms: r.get(1)? })).optional()?)
}

/// Sets (or with None removes) a song's tune.
pub fn set(conn: &Connection, song_id: i64, tune: Option<&Tune>) -> Result<()> {
    match tune {
        Some(t) => conn.execute(
            "INSERT OR REPLACE INTO song_tune (song_id, media_id, gap_ms) VALUES (?1, ?2, ?3)",
            params![song_id, t.media_id, t.gap_ms.clamp(0, 10_000)],
        )?,
        None => conn.execute("DELETE FROM song_tune WHERE song_id = ?1", [song_id])?,
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_keeps_its_tune() {
        let c = crate::db::open_memory().unwrap();
        c.execute("INSERT INTO song (title) VALUES ('Blessed Assurance')", []).unwrap();
        let id = c.last_insert_rowid();
        assert_eq!(get(&c, id).unwrap(), None);
        set(&c, id, Some(&Tune { media_id: 7, gap_ms: 99_999 })).unwrap();
        assert_eq!(get(&c, id).unwrap(), Some(Tune { media_id: 7, gap_ms: 10_000 }), "the breath is at most 10 s");
        set(&c, id, None).unwrap();
        assert_eq!(get(&c, id).unwrap(), None);
    }
}
