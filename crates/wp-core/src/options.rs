//! EasyWorship's Options dialog: per-output settings (monitor, full screen or a
//! custom position and size, screen margins, alpha channel, the output's own song
//! and scripture themes, logo/blank behaviour, show on startup), service intervals
//! and the song usage report for CCLI.

use crate::{Error, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputConfig {
    pub role: String,
    pub name: String,
    /// Index and name of the monitor; the name lets it be found again if Windows
    /// numbers the monitors differently next time.
    pub display_index: Option<usize>,
    pub display_name: Option<String>,
    /// None = fill the monitor. Otherwise pixels from the monitor's top-left.
    pub custom: Option<Rect>,
    pub margins: Margins,
    /// See-through background (for keying hardware or capture); otherwise black.
    pub alpha: bool,
    pub song_theme_id: Option<i64>,
    pub scripture_theme_id: Option<i64>,
    /// logo | blank | transparent | ignore
    pub on_logo: String,
    /// blank | transparent | ignore
    pub on_blank: String,
    pub show_on_startup: bool,
    /// EasyWorship's "NDI Stream" output monitor: this output is sent over NDI in this
    /// format instead of opening on a monitor (None = a monitor or custom position).
    #[serde(default)]
    pub ndi: Option<NdiFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NdiFormat {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Serialize, Deserialize, Default)]
struct Extra {
    #[serde(default)]
    ndi: Option<NdiFormat>,
    display_index: Option<usize>,
    custom: Option<Rect>,
    #[serde(default)]
    margins: Margins,
    #[serde(default)]
    alpha: bool,
    song_theme_id: Option<i64>,
    scripture_theme_id: Option<i64>,
    #[serde(default)]
    show_on_startup: bool,
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Output".into(), "output", msg.into())
}

pub fn outputs(conn: &Connection) -> Result<Vec<OutputConfig>> {
    let mut st = conn.prepare("SELECT role, name, display_name, bounds_json, on_logo, on_blank FROM output WHERE role IN ('main','alternate','foldback') ORDER BY CASE role WHEN 'main' THEN 0 WHEN 'alternate' THEN 1 ELSE 2 END")?;
    let v = st
        .query_map([], |r| {
            let extra: Extra = r.get::<_, Option<String>>(3)?.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
            Ok(OutputConfig {
                role: r.get(0)?,
                name: r.get(1)?,
                display_name: r.get(2)?,
                display_index: extra.display_index,
                custom: extra.custom,
                margins: extra.margins,
                alpha: extra.alpha,
                song_theme_id: extra.song_theme_id,
                scripture_theme_id: extra.scripture_theme_id,
                on_logo: r.get(4)?,
                on_blank: r.get(5)?,
                show_on_startup: extra.show_on_startup,
                ndi: extra.ndi,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

pub fn output(conn: &Connection, role: &str) -> Result<OutputConfig> {
    outputs(conn)?.into_iter().find(|o| o.role == role).ok_or_else(|| bad(format!("No output called {role}.")))
}

pub fn save_output(conn: &Connection, c: &OutputConfig) -> Result<()> {
    if let Some(r) = c.custom {
        if r.w < 64 || r.h < 64 {
            return Err(bad("The output must be at least 64 × 64 pixels."));
        }
    }
    let m = c.margins;
    if m.left + m.right > 2000 || m.top + m.bottom > 2000 {
        return Err(bad("Those margins leave no room for the slides."));
    }
    if !["logo", "blank", "transparent", "ignore"].contains(&c.on_logo.as_str()) || !["blank", "transparent", "ignore"].contains(&c.on_blank.as_str()) {
        return Err(bad("Unknown logo or blank behaviour."));
    }
    if let Some(f) = c.ndi {
        let ok = matches!((f.width, f.height), (1280, 720) | (1920, 1080) | (1024, 768) | (3840, 2160)) && matches!(f.fps, 24 | 25 | 30 | 50 | 60);
        if !ok {
            return Err(bad("Choose one of the listed NDI formats."));
        }
    }
    let extra = Extra {
        ndi: c.ndi,
        display_index: c.display_index,
        custom: c.custom,
        margins: c.margins,
        alpha: c.alpha,
        song_theme_id: c.song_theme_id,
        scripture_theme_id: c.scripture_theme_id,
        show_on_startup: c.show_on_startup,
    };
    let n = conn.execute(
        "UPDATE output SET name = ?2, display_name = ?3, bounds_json = ?4, on_logo = ?5, on_blank = ?6 WHERE role = ?1",
        params![c.role, c.name.trim(), c.display_name, serde_json::to_string(&extra)?, c.on_logo, c.on_blank],
    )?;
    if n == 0 {
        return Err(bad(format!("No output called {}.", c.role)));
    }
    Ok(())
}

/// Which monitor to use: the saved name if that monitor is still connected
/// (Windows may renumber monitors), else the saved index if it exists.
pub fn pick_display(c: &OutputConfig, connected: &[String]) -> Option<usize> {
    if let Some(name) = &c.display_name {
        if let Some(i) = connected.iter().position(|n| n == name) {
            return Some(i);
        }
    }
    c.display_index.filter(|i| *i < connected.len())
}

// ---------- service intervals ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    pub id: Option<i64>,
    pub name: String,
    /// 0 Sunday … 6 Saturday, -1 every day
    pub weekday: i32,
    pub start: String,
    pub end: String,
}

fn hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub fn intervals(conn: &Connection) -> Result<Vec<Interval>> {
    let mut st = conn.prepare("SELECT id, name, weekday, start, end FROM service_interval ORDER BY weekday, start")?;
    let v = st.query_map([], |r| Ok(Interval { id: r.get(0)?, name: r.get(1)?, weekday: r.get(2)?, start: r.get(3)?, end: r.get(4)? }))?.collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

pub fn save_interval(conn: &Connection, i: &Interval) -> Result<i64> {
    let (Some(a), Some(b)) = (hhmm(&i.start), hhmm(&i.end)) else { return Err(bad("Times are like 07:00 and 09:30.")) };
    if a >= b {
        return Err(bad("The service must end after it starts."));
    }
    if i.name.trim().is_empty() {
        return Err(bad("Give the service a name, e.g. First service."));
    }
    match i.id {
        Some(id) => {
            conn.execute("UPDATE service_interval SET name=?2, weekday=?3, start=?4, end=?5 WHERE id=?1", params![id, i.name.trim(), i.weekday, i.start.trim(), i.end.trim()])?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO service_interval (name, weekday, start, end) VALUES (?1, ?2, ?3, ?4)", params![i.name.trim(), i.weekday, i.start.trim(), i.end.trim()])?;
            Ok(conn.last_insert_rowid())
        }
    }
}

pub fn delete_interval(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM service_interval WHERE id = ?1", [id])?;
    Ok(())
}

/// The interval covering a local weekday and minute of the day.
pub fn interval_at(list: &[Interval], weekday: i32, minute: u32) -> Option<String> {
    list.iter()
        .find(|i| (i.weekday == -1 || i.weekday == weekday) && hhmm(&i.start).map_or(false, |a| minute >= a) && hhmm(&i.end).map_or(false, |b| minute < b))
        .map(|i| i.name.clone())
}

/// The interval happening now, by this computer's local time.
pub fn current_interval(conn: &Connection) -> Result<Option<String>> {
    let (wd, hm): (String, String) = conn.query_row("SELECT strftime('%w','now','localtime'), strftime('%H:%M','now','localtime')", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(interval_at(&intervals(conn)?, wd.parse().unwrap_or(0), hhmm(&hm).unwrap_or(0)))
}

/// Records a song going live, with the service interval it happened in.
pub fn log_song_use(conn: &Connection, schedule_id: Option<i64>, song_id: i64) -> Result<()> {
    let interval = current_interval(conn)?;
    conn.execute("INSERT INTO usage_log (song_id, schedule_id, interval_name) VALUES (?1, ?2, ?3)", params![song_id, schedule_id, interval])?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageRow {
    pub title: String,
    pub numbers: String,
    pub author: String,
    pub copyright: String,
    pub ccli: String,
    pub interval: String,
    pub uses: i64,
    pub first: String,
    pub last: String,
}

/// Songs used between two dates (inclusive, YYYY-MM-DD), grouped by service interval.
/// A song shown several times in one service counts once.
pub fn usage_report(conn: &Connection, from: &str, to: &str) -> Result<Vec<UsageRow>> {
    let mut st = conn.prepare(
        "SELECT s.title,
                coalesce((SELECT group_concat(h.code || ' ' || n.number, ', ') FROM song_number n JOIN hymnal h ON h.id = n.hymnal_id WHERE n.song_id = s.id), ''),
                coalesce(s.author, ''), coalesce(s.copyright, ''), coalesce(s.ccli, ''),
                coalesce(u.interval_name, 'Other times'),
                count(DISTINCT date(u.used_at, 'localtime') || coalesce(u.interval_name, '') || coalesce(u.schedule_id, '')),
                min(date(u.used_at, 'localtime')), max(date(u.used_at, 'localtime'))
         FROM usage_log u JOIN song s ON s.id = u.song_id
         WHERE date(u.used_at, 'localtime') BETWEEN ?1 AND ?2
         GROUP BY s.id, u.interval_name ORDER BY 6, 7 DESC, s.title",
    )?;
    let v = st
        .query_map(params![from, to], |r| {
            Ok(UsageRow {
                title: r.get(0)?,
                numbers: r.get(1)?,
                author: r.get(2)?,
                copyright: r.get(3)?,
                ccli: r.get(4)?,
                interval: r.get(5)?,
                uses: r.get(6)?,
                first: r.get(7)?,
                last: r.get(8)?,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

/// The report as CSV, ready for a spreadsheet or the CCLI website.
pub fn usage_csv(rows: &[UsageRow]) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out = String::from("Service,Title,Hymn numbers,Author,Copyright,CCLI song number,Times used,First used,Last used\r\n");
    for r in rows {
        out.push_str(&format!("{},{},{},{},{},{},{},{},{}\r\n", q(&r.interval), q(&r.title), q(&r.numbers), q(&r.author), q(&r.copyright), q(&r.ccli), r.uses, r.first, r.last));
    }
    out
}

#[cfg(test)]
mod ndi_tests {
    #[test]
    fn ndi_stream_is_kept_with_the_output() {
        let c = crate::db::open_memory().unwrap();
        let mut main = super::output(&c, "main").unwrap();
        assert_eq!(main.ndi, None);
        main.ndi = Some(super::NdiFormat { width: 1280, height: 720, fps: 60 });
        super::save_output(&c, &main).unwrap();
        assert_eq!(super::output(&c, "main").unwrap().ndi, Some(super::NdiFormat { width: 1280, height: 720, fps: 60 }));
        main.ndi = Some(super::NdiFormat { width: 777, height: 720, fps: 60 });
        assert!(super::save_output(&c, &main).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_settings_round_trip() {
        let c = crate::db::open_memory().unwrap();
        let all = outputs(&c).unwrap();
        assert_eq!(all.iter().map(|o| o.role.as_str()).collect::<Vec<_>>(), vec!["main", "alternate", "foldback"]);
        let mut m = output(&c, "main").unwrap();
        m.display_index = Some(1);
        m.display_name = Some("EPSON PJ".into());
        m.custom = Some(Rect { x: 0, y: 0, w: 1920, h: 1080 });
        m.margins = Margins { left: 40, right: 40, top: 20, bottom: 60 };
        m.song_theme_id = Some(3);
        m.show_on_startup = true;
        save_output(&c, &m).unwrap();
        assert_eq!(output(&c, "main").unwrap(), m);
        m.custom = Some(Rect { x: 0, y: 0, w: 10, h: 10 });
        assert!(save_output(&c, &m).is_err());
        m.on_logo = "sparkle".into();
        assert!(save_output(&c, &output(&c, "main").unwrap()).is_ok());
        assert!(save_output(&c, &m).is_err());
    }

    #[test]
    fn monitor_found_again_by_name() {
        let mut c = OutputConfig {
            role: "main".into(), name: "Main".into(), display_index: Some(1), display_name: Some("EPSON PJ".into()), custom: None,
            margins: Margins::default(), alpha: false, song_theme_id: None, scripture_theme_id: None, on_logo: "logo".into(), on_blank: "blank".into(), show_on_startup: true,
            ndi: None,
        };
        let now = vec!["EPSON PJ".to_string(), "Laptop".to_string()];
        assert_eq!(pick_display(&c, &now), Some(0), "renumbered: found by name");
        assert_eq!(pick_display(&c, &["Laptop".to_string()]), None, "projector unplugged: not the laptop screen");
        c.display_name = None;
        assert_eq!(pick_display(&c, &now), Some(1));
    }

    #[test]
    fn intervals_and_usage_report() {
        let c = crate::db::open_memory().unwrap();
        let first = Interval { id: None, name: "First service".into(), weekday: 0, start: "07:00".into(), end: "09:30".into() };
        save_interval(&c, &first).unwrap();
        save_interval(&c, &Interval { id: None, name: "Bible study".into(), weekday: 3, start: "18:30".into(), end: "20:00".into() }).unwrap();
        assert!(save_interval(&c, &Interval { start: "10:00".into(), end: "09:00".into(), ..first.clone() }).is_err());
        let list = intervals(&c).unwrap();
        assert_eq!(interval_at(&list, 0, 8 * 60), Some("First service".into()));
        assert_eq!(interval_at(&list, 0, 10 * 60), None);
        assert_eq!(interval_at(&list, 3, 19 * 60), Some("Bible study".into()));

        c.execute("INSERT INTO song (id, title, author, copyright, ccli) VALUES (1, 'MHB 1', 'Charles Wesley', 'Public Domain', NULL), (2, 'Way Maker', 'Sinach', '© 2016 Integrity', '7115744')", []).unwrap();
        for (song, sched, when, iv) in [(1, 10, "2026-11-08 07:30:00", "First service"), (1, 10, "2026-11-08 07:45:00", "First service"), (2, 10, "2026-11-08 08:00:00", "First service"), (2, 11, "2026-11-11 19:00:00", "Bible study")] {
            c.execute("INSERT INTO usage_log (song_id, schedule_id, used_at, interval_name) VALUES (?1, ?2, ?3, ?4)", params![song, sched, when, iv]).unwrap();
        }
        let rows = usage_report(&c, "2026-11-01", "2026-11-30").unwrap();
        let mhb = rows.iter().find(|r| r.title == "MHB 1").unwrap();
        assert_eq!(mhb.uses, 1, "shown twice in one service counts once");
        assert_eq!(rows.iter().filter(|r| r.title == "Way Maker").count(), 2, "one row per service interval");
        let csv = usage_csv(&rows);
        assert!(csv.starts_with("Service,Title") && csv.contains("\"7115744\""));
    }
}
