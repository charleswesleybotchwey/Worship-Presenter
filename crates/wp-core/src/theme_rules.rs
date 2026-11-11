//! Theme rules: pick a theme automatically for songs (by hymnbook, language, one or two
//! languages, service interval) and scripture (by Bible version, service interval).
//! The first matching rule wins. An item's own theme and an output's own theme still
//! come before any rule; with no rule matching, the default theme is used.

use crate::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub id: i64,
    pub kind: String,
    #[serde(rename = "match")]
    pub matcher: String,
    pub value: String,
    pub theme_id: i64,
}

pub fn list(conn: &Connection) -> Result<Vec<Rule>> {
    let mut st = conn.prepare("SELECT id, kind, match, value, theme_id FROM theme_rule ORDER BY position, id")?;
    let rows = st.query_map([], |r| Ok(Rule { id: r.get(0)?, kind: r.get(1)?, matcher: r.get(2)?, value: r.get(3)?, theme_id: r.get(4)? }))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Replaces every rule (the list order is the priority).
pub fn save_all(conn: &Connection, rules: &[Rule]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM theme_rule", [])?;
    for (i, r) in rules.iter().enumerate() {
        let ok_kind = matches!(r.kind.as_str(), "song" | "scripture");
        let ok_match = matches!(r.matcher.as_str(), "hymnal" | "language" | "bilingual" | "interval" | "version");
        if !ok_kind || !ok_match || r.value.trim().is_empty() {
            return Err(crate::Error::Format("Theme rules".into(), "rule", format!("Rule {} is incomplete.", i + 1)));
        }
        tx.execute(
            "INSERT INTO theme_rule (position, kind, match, value, theme_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![i as i64, r.kind, r.matcher, r.value.trim(), r.theme_id],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn interval_now(conn: &Connection) -> Option<String> {
    crate::options::current_interval(conn).ok().flatten()
}

/// The theme the rules choose for a song, if any.
pub fn for_song(conn: &Connection, song_id: i64) -> Result<Option<i64>> {
    let rules: Vec<Rule> = list(conn)?.into_iter().filter(|r| r.kind == "song").collect();
    if rules.is_empty() {
        return Ok(None);
    }
    let hymnals: Vec<String> = {
        let mut st = conn.prepare("SELECT h.code FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE n.song_id = ?1")?;
        let v = st.query_map([song_id], |r| r.get::<_, String>(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    let langs: Vec<String> = {
        let mut st = conn.prepare("SELECT lang FROM song_lyric WHERE song_id = ?1 AND trim(body) <> ''")?;
        let v = st.query_map([song_id], |r| r.get::<_, String>(0))?.collect::<std::result::Result<_, _>>()?;
        v
    };
    let interval = interval_now(conn);
    for r in rules {
        let v = r.value.trim();
        let hit = match r.matcher.as_str() {
            "hymnal" => hymnals.iter().any(|h| h.eq_ignore_ascii_case(v)),
            "language" => langs.iter().any(|l| l.eq_ignore_ascii_case(v)),
            "bilingual" => (langs.len() > 1) == matches!(v.to_lowercase().as_str(), "yes" | "true" | "1"),
            "interval" => interval.as_deref().is_some_and(|i| i.eq_ignore_ascii_case(v)),
            _ => false,
        };
        if hit && exists(conn, r.theme_id)? {
            return Ok(Some(r.theme_id));
        }
    }
    Ok(None)
}

/// The theme the rules choose for a reading in these Bible versions, if any.
pub fn for_scripture(conn: &Connection, versions: &[String]) -> Result<Option<i64>> {
    let rules: Vec<Rule> = list(conn)?.into_iter().filter(|r| r.kind == "scripture").collect();
    if rules.is_empty() {
        return Ok(None);
    }
    let interval = interval_now(conn);
    for r in rules {
        let v = r.value.trim();
        let hit = match r.matcher.as_str() {
            "version" => versions.first().is_some_and(|x| x.eq_ignore_ascii_case(v)),
            "bilingual" => (versions.len() > 1) == matches!(v.to_lowercase().as_str(), "yes" | "true" | "1"),
            "interval" => interval.as_deref().is_some_and(|i| i.eq_ignore_ascii_case(v)),
            _ => false,
        };
        if hit && exists(conn, r.theme_id)? {
            return Ok(Some(r.theme_id));
        }
    }
    Ok(None)
}

fn exists(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.query_row("SELECT 1 FROM theme WHERE id = ?1", [id], |_| Ok(())).optional()?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(c: &Connection, title: &str, hymnal: &str, num: &str, langs: &[&str]) -> i64 {
        c.execute("INSERT OR IGNORE INTO hymnal (code, name) VALUES (?1, ?1)", [hymnal]).unwrap();
        c.execute("INSERT INTO song (title) VALUES (?1)", [title]).unwrap();
        let id = c.last_insert_rowid();
        c.execute("INSERT INTO song_number (song_id, hymnal_id, number) SELECT ?1, id, ?2 FROM hymnal WHERE code = ?3", params![id, num, hymnal]).unwrap();
        for l in langs {
            c.execute("INSERT INTO song_lyric (song_id, lang, body) VALUES (?1, ?2, '[V1]\nx')", params![id, l]).unwrap();
        }
        id
    }

    #[test]
    fn first_matching_rule_wins() {
        let c = crate::db::open_memory().unwrap();
        crate::themes::seed(&c).unwrap();
        let theme = |name: &str| -> i64 {
            c.execute("INSERT INTO theme (name, kind, json) VALUES (?1, 'song', '{}')", [name]).unwrap();
            c.last_insert_rowid()
        };
        let (mhb, bilingual, youth) = (theme("MHB look"), theme("Two languages"), theme("Youth"));
        let a = song(&c, "MHB 1", "MHB", "1", &["en", "fat"]);
        let b = song(&c, "NCAN 5", "NCAN", "5", &["en", "fat"]);
        let d = song(&c, "NCAN 6", "NCAN", "6", &["en"]);
        assert_eq!(for_song(&c, a).unwrap(), None, "no rules: the default theme");
        save_all(&c, &[
            Rule { id: 0, kind: "song".into(), matcher: "hymnal".into(), value: "mhb".into(), theme_id: mhb },
            Rule { id: 0, kind: "song".into(), matcher: "bilingual".into(), value: "yes".into(), theme_id: bilingual },
            Rule { id: 0, kind: "song".into(), matcher: "interval".into(), value: "Youth Service".into(), theme_id: youth },
        ]).unwrap();
        assert_eq!(for_song(&c, a).unwrap(), Some(mhb), "the hymnbook rule comes first");
        assert_eq!(for_song(&c, b).unwrap(), Some(bilingual), "two languages");
        assert_eq!(for_song(&c, d).unwrap(), None, "English only, no interval running");
        c.execute("DELETE FROM theme WHERE id = ?1", [mhb]).unwrap();
        assert_eq!(for_song(&c, a).unwrap(), Some(bilingual), "a deleted theme's rule goes with it");
        assert_eq!(list(&c).unwrap().len(), 2);
        save_all(&c, &[Rule { id: 0, kind: "scripture".into(), matcher: "version".into(), value: "TK2012".into(), theme_id: youth }]).unwrap();
        assert_eq!(for_scripture(&c, &["TK2012".into()]).unwrap(), Some(youth));
        assert_eq!(for_scripture(&c, &["NKJV".into()]).unwrap(), None);
        assert!(save_all(&c, &[Rule { id: 0, kind: "song".into(), matcher: "hymnal".into(), value: " ".into(), theme_id: youth }]).is_err());
    }
}
