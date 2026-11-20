//! Smart song collections, as EasyWorship's "Edit collection rules": Match all / any of
//! rules such as "Title contains grace" or "Author is Charles Wesley". Text is compared
//! without case or accents ("nyankopon" matches "Nyankopɔn").

use crate::text::fold;
use crate::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// title | author | copyright | ccli | words | any
    pub field: String,
    /// contains | not_contains | is | is_not | starts | ends
    pub op: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    /// true: every rule must match; false: any one is enough
    pub all: bool,
    pub rules: Vec<Rule>,
}

fn test(op: &str, have: &str, want: &str) -> bool {
    match op {
        "contains" => have.contains(want),
        "not_contains" => !have.contains(want),
        "is" => have == want,
        "is_not" => have != want,
        "starts" => have.starts_with(want),
        "ends" => have.ends_with(want),
        _ => false,
    }
}

fn matches(r: &Rule, title: &str, author: &str, copyright: &str, ccli: &str, words: &str) -> bool {
    let want = fold(r.value.trim());
    let check = |v: &str| test(&r.op, &fold(v.trim()), &want);
    match r.field.as_str() {
        "title" => check(title),
        "author" => check(author),
        "copyright" => check(copyright),
        "ccli" => check(ccli),
        "words" => check(words),
        // any field: a negative rule must hold for every field, a positive one for one
        _ => {
            let fields = [title, author, copyright, ccli, words];
            if r.op.starts_with("not") || r.op == "is_not" { fields.iter().all(|f| check(f)) } else { fields.iter().any(|f| check(f)) }
        }
    }
}

/// The songs matching the rules.
pub fn songs(conn: &Connection, rules: &Rules) -> Result<Vec<i64>> {
    let rs: Vec<&Rule> = rules.rules.iter().filter(|r| !r.value.trim().is_empty()).collect();
    if rs.is_empty() {
        return Ok(vec![]);
    }
    let words_needed = rs.iter().any(|r| r.field == "words" || r.field == "any");
    let mut st = conn.prepare(
        "SELECT s.id, s.title, coalesce(s.author, ''), coalesce(s.copyright, ''), coalesce(s.ccli, ''),
                CASE WHEN ?1 THEN coalesce((SELECT group_concat(body, ' ') FROM song_lyric l WHERE l.song_id = s.id), '') ELSE '' END
         FROM song s",
    )?;
    let rows = st.query_map(params![words_needed], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?)))?;
    let mut out = vec![];
    for row in rows {
        let (id, title, author, copyright, ccli, words) = row?;
        // section marks ("[V1]") are not words
        let words = if words_needed { words.split('\n').filter(|l| !l.trim_start().starts_with('[')).collect::<Vec<_>>().join(" ") } else { words };
        let hit = |r: &&Rule| matches(r, &title, &author, &copyright, &ccli, &words);
        if if rules.all { rs.iter().all(hit) } else { rs.iter().any(hit) } {
            out.push(id);
        }
    }
    Ok(out)
}

pub fn get(conn: &Connection, collection: i64) -> Result<Option<Rules>> {
    let raw: Option<String> = conn.query_row("SELECT rules FROM my_collection WHERE id = ?1", [collection], |r| r.get(0))?;
    Ok(raw.and_then(|t| serde_json::from_str(&t).ok()))
}

/// Sets (or, with None, removes) a collection's rules.
pub fn set(conn: &Connection, collection: i64, rules: Option<&Rules>) -> Result<()> {
    let raw = rules.map(|r| serde_json::to_string(r).unwrap_or_default());
    conn.execute("UPDATE my_collection SET rules = ?2 WHERE id = ?1", params![collection, raw])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::songs::{Lyric, Number, SongDoc};

    fn song(c: &Connection, title: &str, author: &str, words: &str) {
        let d = SongDoc { id: None, title: title.into(), author: Some(author.into()), copyright: Some("Public Domain".into()), ccli: None, numbers: vec![Number { hymnal: "MHB".into(), number: "1".into() }], lyrics: vec![Lyric { lang: "en".into(), body: format!("[V1]\n{words}") }], sequence: String::new(), lines_per_slide: None, key: None };
        crate::songs::save(c, &d).unwrap();
    }

    #[test]
    fn rules_like_easyworship() {
        let c = crate::db::open_memory().unwrap();
        song(&c, "O for a thousand tongues", "Charles Wesley", "My great Redeemer's praise");
        song(&c, "Amazing Grace", "John Newton", "How sweet the sound");
        song(&c, "And can it be", "Charles Wesley", "Amazing love! how can it be");
        song(&c, "Nyame ye", "Fante hymn", "Nyankopɔn ye");
        let titles = |r: Rules| -> Vec<String> {
            let mut t: Vec<String> = songs(&c, &r).unwrap().into_iter().map(|id| c.query_row("SELECT title FROM song WHERE id = ?1", [id], |x| x.get(0)).unwrap()).collect();
            t.sort();
            t
        };
        let rule = |f: &str, o: &str, v: &str| Rule { field: f.into(), op: o.into(), value: v.into() };
        assert_eq!(titles(Rules { all: true, rules: vec![rule("author", "is", "charles wesley")] }), vec!["And can it be", "O for a thousand tongues"]);
        assert_eq!(titles(Rules { all: true, rules: vec![rule("author", "is", "Charles Wesley"), rule("title", "starts", "and")] }), vec!["And can it be"]);
        assert_eq!(titles(Rules { all: false, rules: vec![rule("title", "contains", "grace"), rule("words", "contains", "amazing")] }), vec!["Amazing Grace", "And can it be"]);
        assert_eq!(titles(Rules { all: true, rules: vec![rule("words", "contains", "nyankopon")] }), vec!["Nyame ye"], "no accents needed");
        assert_eq!(titles(Rules { all: true, rules: vec![rule("any", "not_contains", "wesley")] }), vec!["Amazing Grace", "Nyame ye"]);
        assert_eq!(titles(Rules { all: true, rules: vec![rule("words", "contains", "v1")] }), Vec::<String>::new(), "section marks are not words");
        assert!(titles(Rules { all: true, rules: vec![rule("title", "contains", "  ")] }).is_empty(), "empty rules match nothing");
    }

    #[test]
    fn a_collection_keeps_its_rules() {
        let c = crate::db::open_memory().unwrap();
        let id = crate::browse::create_collection(&c, "Wesley hymns").unwrap();
        assert_eq!(get(&c, id).unwrap(), None);
        let r = Rules { all: true, rules: vec![Rule { field: "author".into(), op: "contains".into(), value: "Wesley".into() }] };
        set(&c, id, Some(&r)).unwrap();
        assert_eq!(get(&c, id).unwrap(), Some(r));
        set(&c, id, None).unwrap();
        assert_eq!(get(&c, id).unwrap(), None);
    }
}
