//! Announcement decks: slides with a title and text, or a picture from the media
//! library, each shown for a set time. A deck can loop (before the service) or stop
//! on its last slide. The offering pack gives ready-made decks to fill in.

use crate::splitter::Slide;
use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckSlide {
    /// "text" or "picture"
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// Media library path for picture slides.
    #[serde(default)]
    pub image: Option<String>,
    /// How long it shows when advancing by itself; None uses the deck's length.
    #[serde(default)]
    pub seconds: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub id: Option<i64>,
    pub title: String,
    pub looping: bool,
    /// Seconds per slide when advancing by itself; 0 = only when the operator presses Next.
    pub seconds: u32,
    pub slides: Vec<DeckSlide>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeckSummary {
    pub id: i64,
    pub title: String,
    pub slides: i64,
    pub looping: bool,
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Announcements".into(), "announcement", msg.into())
}

pub fn list(conn: &Connection) -> Result<Vec<DeckSummary>> {
    let mut st = conn.prepare(
        "SELECT p.id, p.title, (SELECT count(*) FROM presentation_slide s WHERE s.presentation_id = p.id), coalesce(p.settings_json, '{}')
         FROM presentation p WHERE p.kind = 'builtin' ORDER BY p.title",
    )?;
    let v = st
        .query_map([], |r| {
            let settings: String = r.get(3)?;
            let j: serde_json::Value = serde_json::from_str(&settings).unwrap_or_default();
            Ok(DeckSummary { id: r.get(0)?, title: r.get(1)?, slides: r.get(2)?, looping: j["looping"].as_bool().unwrap_or(false) })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

pub fn get(conn: &Connection, id: i64) -> Result<Deck> {
    let (title, settings): (String, Option<String>) =
        conn.query_row("SELECT title, settings_json FROM presentation WHERE id = ?1 AND kind = 'builtin'", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let j: serde_json::Value = settings.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut st = conn.prepare("SELECT json FROM presentation_slide WHERE presentation_id = ?1 ORDER BY idx")?;
    let slides = st
        .query_map([id], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .filter_map(|s| serde_json::from_str::<DeckSlide>(&s).ok())
        .collect();
    Ok(Deck { id: Some(id), title, looping: j["looping"].as_bool().unwrap_or(false), seconds: j["seconds"].as_u64().unwrap_or(10) as u32, slides })
}

pub fn save(conn: &Connection, d: &Deck) -> Result<i64> {
    let title = d.title.trim();
    if title.is_empty() {
        return Err(bad("Announcements need a name."));
    }
    if d.slides.is_empty() {
        return Err(bad("Add at least one slide."));
    }
    for (i, s) in d.slides.iter().enumerate() {
        match s.kind.as_str() {
            "text" if s.title.trim().is_empty() && s.body.trim().is_empty() => return Err(bad(format!("Slide {} is empty.", i + 1))),
            "picture" if s.image.is_none() => return Err(bad(format!("Slide {} needs a picture.", i + 1))),
            "text" | "picture" => {}
            k => return Err(bad(format!("Unknown slide type {k}."))),
        }
    }
    let settings = serde_json::json!({ "looping": d.looping, "seconds": d.seconds }).to_string();
    let tx = conn.unchecked_transaction()?;
    let id = match d.id {
        Some(id) => {
            tx.execute("UPDATE presentation SET title = ?2, settings_json = ?3 WHERE id = ?1 AND kind = 'builtin'", params![id, title, settings])?;
            tx.execute("DELETE FROM presentation_slide WHERE presentation_id = ?1", [id])?;
            id
        }
        None => {
            tx.execute("INSERT INTO presentation (title, kind, settings_json) VALUES (?1, 'builtin', ?2)", params![title, settings])?;
            tx.last_insert_rowid()
        }
    };
    for (i, s) in d.slides.iter().enumerate() {
        tx.execute(
            "INSERT INTO presentation_slide (presentation_id, idx, json, image_path, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, i as i64, serde_json::to_string(s)?, s.image, s.seconds.map(|x| x as i64 * 1000)],
        )?;
    }
    tx.commit()?;
    Ok(id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM presentation WHERE id = ?1 AND kind = 'builtin'", [id])?;
    Ok(())
}

/// Slides for Preview, Live and the outputs. Body lines become the slide's lines.
pub fn to_slides(d: &Deck) -> Vec<Slide> {
    d.slides
        .iter()
        .enumerate()
        .map(|(i, s)| Slide {
            label: if s.title.trim().is_empty() { format!("Slide {}", i + 1) } else { s.title.trim().to_string() },
            occurrence: i,
            first_key: i as i64,
            last_key: i as i64,
            primary: if s.kind == "text" { s.body.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).map(str::to_string).collect() } else { vec![] },
            second: vec![],
            image: if s.kind == "picture" { s.image.clone() } else { None },
        })
        .collect()
}

/// How long each slide shows when advancing by itself (ms); empty when the deck
/// only moves on Next.
pub fn durations_ms(d: &Deck) -> Vec<i64> {
    if d.seconds == 0 && d.slides.iter().all(|s| s.seconds.is_none()) {
        return vec![];
    }
    d.slides.iter().map(|s| s.seconds.unwrap_or(d.seconds).max(2) as i64 * 1000).collect()
}

/// The offering pack: ready-made decks with blanks to fill in.
pub fn templates() -> Vec<Deck> {
    let t = |title: &str, body: &str| DeckSlide { kind: "text".into(), title: title.into(), body: body.into(), image: None, seconds: None };
    vec![
        Deck {
            id: None,
            title: "Offering and tithes".into(),
            looping: false,
            seconds: 0,
            slides: vec![
                t("Offering", "“God loves a cheerful giver.” 2 Corinthians 9:7"),
                t("Give by Mobile Money", "MTN MoMo: 024 000 0000\nName: Your Church Name\nReference: Offering"),
                t("Tithes", "Bring the whole tithe into the storehouse. Malachi 3:10\nMoMo reference: Tithe and your name"),
                t("Bank transfer", "Bank: Your Bank, Branch\nAccount name: Your Church Name\nAccount number: 0000000000"),
            ],
        },
        Deck {
            id: None,
            title: "Birthdays this week".into(),
            looping: false,
            seconds: 0,
            slides: vec![t("Happy birthday", "Name One\nName Two\nName Three"), t("Ayɛkoo!", "We celebrate you. May God bless your new year.")],
        },
        Deck {
            id: None,
            title: "Welcome before service".into(),
            looping: true,
            seconds: 12,
            slides: vec![
                t("Akwaaba! Welcome", "We are glad you are here.\nThe service begins shortly."),
                t("Please silence your phone", "Thank you."),
                t("This week", "Bible study: Wednesday 6:30 pm\nPrayer meeting: Friday 6:30 pm"),
                t("Visiting today?", "Please stay after the service so we can welcome you."),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_slides_and_timing() {
        let c = crate::db::open_memory().unwrap();
        let mut d = templates().into_iter().find(|d| d.looping).unwrap();
        d.slides.push(DeckSlide { kind: "picture".into(), title: "".into(), body: "".into(), image: Some("images/flyer.png".into()), seconds: Some(20) });
        let id = save(&c, &d).unwrap();
        let back = get(&c, id).unwrap();
        assert_eq!((back.looping, back.seconds, back.slides.len()), (true, 12, 5));
        let s = to_slides(&back);
        assert_eq!(s[0].label, "Akwaaba! Welcome");
        assert_eq!(s[0].primary, vec!["We are glad you are here.", "The service begins shortly."]);
        assert_eq!(s[4].image.as_deref(), Some("images/flyer.png"));
        assert_eq!(durations_ms(&back), vec![12000, 12000, 12000, 12000, 20000]);
        assert_eq!(list(&c).unwrap()[0].slides, 5);
        delete(&c, id).unwrap();
        assert!(list(&c).unwrap().is_empty());
    }

    #[test]
    fn manual_decks_and_checks() {
        let offering = &templates()[0];
        assert!(durations_ms(offering).is_empty(), "offering moves only on Next");
        let c = crate::db::open_memory().unwrap();
        let mut d = offering.clone();
        d.slides.push(DeckSlide { kind: "picture".into(), title: "".into(), body: "".into(), image: None, seconds: None });
        assert!(save(&c, &d).unwrap_err().to_string().contains("needs a picture"));
        d.slides.clear();
        assert!(save(&c, &d).is_err());
    }
}
