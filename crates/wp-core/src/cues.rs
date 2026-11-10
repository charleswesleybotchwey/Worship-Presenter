//! Timers (countdowns) and alerts (nursery, parking, announcements from the desk).
//!
//! Both live in LiveState, so every window shows the same thing. Times are absolute
//! (milliseconds since 1970), so each output counts down by itself and stays in step
//! with the others on the same computer; pausing stores the time that was left.

use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerShow {
    Off,
    Corner,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    pub label: String,
    /// When it reaches zero. While paused this is meaningless; see `paused_left_ms`.
    pub ends_at_ms: i64,
    pub paused_left_ms: Option<i64>,
    /// Keep counting up after zero (shown in red) instead of stopping at 0:00.
    pub count_over: bool,
    pub on_screens: TimerShow,
    pub on_stage: bool,
}

impl Timer {
    pub fn left_ms(&self, now: i64) -> i64 {
        self.paused_left_ms.unwrap_or(self.ends_at_ms - now)
    }
    pub fn pause(&self, now: i64) -> Timer {
        Timer { paused_left_ms: Some(self.left_ms(now)), ..self.clone() }
    }
    pub fn resume(&self, now: i64) -> Timer {
        match self.paused_left_ms {
            Some(left) => Timer { ends_at_ms: now + left, paused_left_ms: None, ..self.clone() },
            None => self.clone(),
        }
    }
    pub fn add(&self, ms: i64) -> Timer {
        match self.paused_left_ms {
            Some(left) => Timer { paused_left_ms: Some(left + ms), ..self.clone() },
            None => Timer { ends_at_ms: self.ends_at_ms + ms, ..self.clone() },
        }
    }
}

/// Parses "5", "5:00", "1:30:00" (minutes or h:mm:ss) into milliseconds.
pub fn parse_length(s: &str) -> Option<i64> {
    let parts: Vec<i64> = s.trim().split(':').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let secs = match parts.as_slice() {
        [m] => m * 60,
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    (secs > 0 && secs < 24 * 3600).then_some(secs * 1000)
}

/// "09:00" or "9:30 am" today, as an absolute time, given the local offset from UTC.
/// A time already passed today stays today, so the timer shows how late it is.
pub fn parse_clock(s: &str, now: i64, utc_offset_ms: i64) -> Option<i64> {
    let t = s.trim().to_lowercase();
    let pm = t.ends_with("pm");
    let am = t.ends_with("am");
    let t = t.trim_end_matches("am").trim_end_matches("pm").trim();
    let (h, m) = t.split_once(':').or_else(|| t.split_once('.')).unwrap_or((t, "0"));
    let (mut h, m): (i64, i64) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    if !(0..=23).contains(&h) || !(0..=59).contains(&m) || ((am || pm) && !(1..=12).contains(&h)) {
        return None;
    }
    if pm && h < 12 {
        h += 12;
    }
    if am && h == 12 {
        h = 0;
    }
    let day = 86_400_000;
    let local_midnight = (now + utc_offset_ms).div_euclid(day) * day - utc_offset_ms;
    Some(local_midnight + h * 3_600_000 + m * 60_000)
}

/// Local offset from UTC, read from SQLite's own idea of local time.
pub fn utc_offset_ms(conn: &Connection) -> i64 {
    conn.query_row("SELECT round((julianday('now','localtime') - julianday('now')) * 86400000)", [], |r| r.get::<_, f64>(0))
        .map(|v| v as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertStyle {
    Band,
    Ticker,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub text: String,
    pub style: AlertStyle,
    /// "top" or "bottom"
    pub position: String,
    /// When it disappears by itself; None = until cleared.
    pub until_ms: Option<i64>,
    pub on_screens: bool,
    pub on_stream: bool,
    pub on_stage: bool,
}

impl Alert {
    pub fn expired(&self, now: i64) -> bool {
        self.until_ms.map_or(false, |u| now >= u)
    }
}

/// Names inside braces: "Car {registration} is blocking" → ["registration"].
pub fn blanks(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(a) = rest.find('{') {
        let Some(b) = rest[a..].find('}') else { break };
        let name = rest[a + 1..a + b].trim().to_string();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
        rest = &rest[a + b + 1..];
    }
    out
}

pub fn fill(template: &str, values: &[(String, String)]) -> Result<String> {
    let mut s = template.to_string();
    for name in blanks(template) {
        let v = values.iter().find(|(k, _)| *k == name).map(|(_, v)| v.trim()).unwrap_or_default();
        if v.is_empty() {
            return Err(Error::Format("Alert".into(), "alert", format!("Fill in “{name}” first.")));
        }
        s = s.replace(&format!("{{{name}}}"), v);
    }
    Ok(s.split_whitespace().collect::<Vec<_>>().join(" "))
}

// ---------- presets ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimerPreset {
    pub id: Option<i64>,
    pub name: String,
    /// "countdown" (value "5:00") or "to_time" (value "09:00")
    pub mode: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertPreset {
    pub id: Option<i64>,
    pub text: String,
    pub position: String,
    pub style: AlertStyle,
    pub seconds: Option<i64>,
}

/// Starter presets for a new library.
pub fn seed(conn: &Connection) -> Result<()> {
    let n: i64 = conn.query_row("SELECT (SELECT count(*) FROM timer_preset) + (SELECT count(*) FROM alert_preset)", [], |r| r.get(0))?;
    if n > 0 {
        return Ok(());
    }
    for (name, mode, value) in [("5-minute countdown", "countdown", "5:00"), ("Sermon, 30 minutes", "countdown", "30:00"), ("Service starts", "to_time", "09:00")] {
        save_timer_preset(conn, &TimerPreset { id: None, name: name.into(), mode: mode.into(), value: value.into() })?;
    }
    for (text, style, secs) in [
        ("Parent of child number {number}, please come to the children's room.", AlertStyle::Band, Some(45)),
        ("The owner of car {registration} is asked to see an usher, please.", AlertStyle::Band, Some(45)),
        ("Welcome to our service. Please switch your phone to silent.", AlertStyle::Ticker, Some(60)),
    ] {
        save_alert_preset(conn, &AlertPreset { id: None, text: text.into(), position: "bottom".into(), style, seconds: secs })?;
    }
    Ok(())
}

pub fn timer_presets(conn: &Connection) -> Result<Vec<TimerPreset>> {
    let mut st = conn.prepare("SELECT id, name, mode, value FROM timer_preset ORDER BY id")?;
    let v = st.query_map([], |r| Ok(TimerPreset { id: r.get(0)?, name: r.get(1)?, mode: r.get(2)?, value: r.get(3)? }))?.collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

pub fn save_timer_preset(conn: &Connection, p: &TimerPreset) -> Result<i64> {
    let ok = match p.mode.as_str() {
        "countdown" => parse_length(&p.value).is_some(),
        "to_time" => parse_clock(&p.value, 0, 0).is_some(),
        _ => false,
    };
    if !ok || p.name.trim().is_empty() {
        return Err(Error::Format("Timer".into(), "timer", "Give the timer a name and a length like 5:00, or a time like 09:00.".into()));
    }
    match p.id {
        Some(id) => {
            conn.execute("UPDATE timer_preset SET name=?2, mode=?3, value=?4 WHERE id=?1", params![id, p.name.trim(), p.mode, p.value.trim()])?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO timer_preset (name, mode, value) VALUES (?1, ?2, ?3)", params![p.name.trim(), p.mode, p.value.trim()])?;
            Ok(conn.last_insert_rowid())
        }
    }
}

pub fn alert_presets(conn: &Connection) -> Result<Vec<AlertPreset>> {
    let mut st = conn.prepare("SELECT id, text, position, style_json FROM alert_preset ORDER BY id")?;
    let v = st
        .query_map([], |r| {
            let style: Option<String> = r.get(3)?;
            let j: serde_json::Value = style.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
            Ok(AlertPreset {
                id: r.get(0)?,
                text: r.get(1)?,
                position: r.get::<_, Option<String>>(2)?.unwrap_or_else(|| "bottom".into()),
                style: if j["style"] == "ticker" { AlertStyle::Ticker } else { AlertStyle::Band },
                seconds: j["seconds"].as_i64(),
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

pub fn save_alert_preset(conn: &Connection, p: &AlertPreset) -> Result<i64> {
    if p.text.trim().is_empty() {
        return Err(Error::Format("Alert".into(), "alert", "An alert needs some text.".into()));
    }
    let style = serde_json::json!({ "style": p.style, "seconds": p.seconds }).to_string();
    match p.id {
        Some(id) => {
            conn.execute("UPDATE alert_preset SET text=?2, position=?3, style_json=?4 WHERE id=?1", params![id, p.text.trim(), p.position, style])?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO alert_preset (text, position, style_json) VALUES (?1, ?2, ?3)", params![p.text.trim(), p.position, style])?;
            Ok(conn.last_insert_rowid())
        }
    }
}

pub fn delete_preset(conn: &Connection, kind: &str, id: i64) -> Result<()> {
    let table = if kind == "timer" { "timer_preset" } else { "alert_preset" };
    conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_and_clock_times() {
        assert_eq!(parse_length("5"), Some(300_000));
        assert_eq!(parse_length("5:30"), Some(330_000));
        assert_eq!(parse_length("1:00:00"), Some(3_600_000));
        assert_eq!(parse_length("0"), None);
        assert_eq!(parse_length("abc"), None);
        // 2026-11-01 08:00:00 UTC; Ghana is UTC+0
        let now = 1_793_520_000_000;
        assert_eq!(parse_clock("09:00", now, 0), Some(now + 3_600_000));
        assert_eq!(parse_clock("9:30 am", now, 0), Some(now + 5_400_000));
        assert_eq!(parse_clock("2 pm", now, 0).map(|t| (t - now) / 3_600_000), Some(6));
        assert_eq!(parse_clock("7:00", now, 0), Some(now - 3_600_000), "already passed today shows how late");
        assert_eq!(parse_clock("12:00 am", now, 0), Some(now - 8 * 3_600_000));
        assert_eq!(parse_clock("25:00", now, 0), None);
        // a church one hour ahead of UTC
        assert_eq!(parse_clock("09:00", now, 3_600_000), Some(now));
    }

    #[test]
    fn pause_resume_add() {
        let t = Timer { label: "x".into(), ends_at_ms: 10_000, paused_left_ms: None, count_over: true, on_screens: TimerShow::Corner, on_stage: true };
        assert_eq!(t.left_ms(4_000), 6_000);
        let p = t.pause(4_000);
        assert_eq!(p.left_ms(9_999_999), 6_000, "time stands still while paused");
        let p = p.add(60_000);
        let r = p.resume(100_000);
        assert_eq!(r.left_ms(100_000), 66_000);
        assert_eq!(r.left_ms(200_000), -34_000, "negative means over time");
    }

    #[test]
    fn alert_blanks() {
        let t = "Car {registration} is blocking. {registration}, see {who}.";
        assert_eq!(blanks(t), vec!["registration", "who"]);
        let v = vec![("registration".to_string(), " GR 1234-20 ".to_string()), ("who".to_string(), "an usher".to_string())];
        assert_eq!(fill(t, &v).unwrap(), "Car GR 1234-20 is blocking. GR 1234-20, see an usher.");
        assert!(fill(t, &v[..1]).unwrap_err().to_string().contains("who"));
    }

    #[test]
    fn presets_seed_and_edit() {
        let c = crate::db::open_memory().unwrap();
        seed(&c).unwrap();
        seed(&c).unwrap();
        assert_eq!(timer_presets(&c).unwrap().len(), 3);
        let a = alert_presets(&c).unwrap();
        assert_eq!((a.len(), a[2].style), (3, AlertStyle::Ticker));
        assert!(save_timer_preset(&c, &TimerPreset { id: None, name: "Bad".into(), mode: "countdown".into(), value: "soon".into() }).is_err());
        delete_preset(&c, "alert", a[0].id.unwrap()).unwrap();
        assert_eq!(alert_presets(&c).unwrap().len(), 2);
    }
}
