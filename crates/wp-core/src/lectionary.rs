//! The lectionary: for any date, the Sunday or feast in the Revised Common Lectionary's
//! calendar (its name, Year A, B or C, its season), and the church's own readings for it.
//! The calendar is calculation only; the readings come from the church's lectionary
//! (e.g. the Methodist Church Ghana Diary), imported once a year as a CSV file, by date or
//! by Sunday name.

use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

fn bad(m: impl Into<String>) -> Error {
    Error::Format("Lectionary".into(), "lectionary", m.into())
}

// ---------- dates (days since 1970-01-01; Howard Hinnant's civil-date algorithms) ----------

pub fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 0 = Sunday … 6 = Saturday
fn weekday(z: i64) -> i64 {
    (z + 4).rem_euclid(7)
}

/// The first Sunday on or after a day.
fn sunday_on_or_after(z: i64) -> i64 {
    z + (7 - weekday(z)) % 7
}

/// Easter Day (Western), the anonymous Gregorian algorithm.
pub fn easter(y: i64) -> i64 {
    let a = y % 19;
    let b = y / 100;
    let c = y % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    days(y, month, day)
}

/// Advent Sunday: the Sunday between 27 November and 3 December.
fn advent1(y: i64) -> i64 {
    sunday_on_or_after(days(y, 11, 27))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Day {
    /// "Advent 1", "Proper 12", "Easter Day"…
    pub name: String,
    /// "A", "B" or "C"
    pub year: String,
    /// advent, christmas, epiphany, lent, easter, pentecost (ordinary time)
    pub season: String,
}

/// The lectionary day for a date (a Sunday, or a feast on its own date); None for an
/// ordinary weekday.
pub fn day(z: i64) -> Option<Day> {
    let (y, m, d) = civil(z);
    // the lectionary year starts at Advent
    let cycle = if z >= advent1(y) { y } else { y - 1 };
    let year = ["A", "B", "C"][((cycle - 2022).rem_euclid(3)) as usize].to_string();
    let mk = |name: String, season: &str| Some(Day { name, year: year.clone(), season: season.into() });
    let e_this = easter(y);
    // feasts on their own dates first
    if (m, d) == (12, 24) {
        return mk("Christmas Eve".into(), "christmas");
    }
    if (m, d) == (12, 25) {
        return mk("Christmas Day".into(), "christmas");
    }
    if (m, d) == (1, 6) {
        return mk("Epiphany Day".into(), "epiphany");
    }
    if (m, d) == (11, 1) {
        return mk("All Saints".into(), "pentecost");
    }
    match z - e_this {
        -46 => return mk("Ash Wednesday".into(), "lent"),
        -3 => return mk("Holy Thursday".into(), "lent"),
        -2 => return mk("Good Friday".into(), "lent"),
        39 => return mk("Ascension".into(), "easter"),
        _ => {}
    }
    if weekday(z) != 0 {
        return None;
    }
    // Advent (up to Christmas) and Christmas: the Sunday 26 December – 1 January is
    // Christmas 1; a Sunday 2–5 January keeps the Epiphany (as many churches do)
    let a = advent1(y);
    if (m, d) >= (12, 26) || (m, d) == (1, 1) {
        return mk("Christmas 1".into(), "christmas");
    }
    if z >= a {
        return mk(format!("Advent {}", (z - a) / 7 + 1), "advent");
    }
    if m == 1 && d <= 5 {
        return mk("Epiphany Day".into(), "epiphany");
    }
    // Epiphany to the Transfiguration (the Sunday before Ash Wednesday)
    let baptism = sunday_on_or_after(days(y, 1, 7));
    let transfiguration = e_this - 49;
    if z == baptism {
        return mk("Baptism".into(), "epiphany");
    }
    if z > baptism && z < transfiguration {
        return mk(format!("Epiphany {}", (z - baptism) / 7 + 1), "epiphany");
    }
    if z == transfiguration {
        return mk("Transfiguration".into(), "epiphany");
    }
    // Lent, Holy Week, Easter, Pentecost
    let k = z - e_this;
    if (-42..=-14).contains(&k) {
        return mk(format!("Lent {}", (k + 42) / 7 + 1), "lent");
    }
    if k == -7 {
        return mk("Passion".into(), "lent");
    }
    if k == 0 {
        return mk("Easter Day".into(), "easter");
    }
    if (7..=42).contains(&k) {
        return mk(format!("Easter {}", k / 7 + 1), "easter");
    }
    if k == 49 {
        return mk("Pentecost Day".into(), "pentecost");
    }
    if k == 56 {
        return mk("Trinity".into(), "pentecost");
    }
    // after Trinity: Proper n is the Sunday between 29 May + 7(n − 4) and the six days after
    // (Propers 1–3, 8–28 May, only in years when Easter is early)
    if k > 56 {
        let n = (z - days(y, 5, 29)).div_euclid(7) + 4;
        if (1..=29).contains(&n) {
            return mk(format!("Proper {n}"), "pentecost");
        }
    }
    None
}

/// "2026-11-29" → days
pub fn parse_date(s: &str) -> Option<i64> {
    let p: Vec<i64> = s.trim().split(['-', '/']).filter_map(|x| x.trim().parse().ok()).collect();
    match p.as_slice() {
        [y, m, d] if *y > 1900 => Some(days(*y, *m, *d)),
        // 29/11/2026 (Ghana writes the day first)
        [d, m, y] if *y > 1900 => Some(days(*y, *m, *d)),
        _ => None,
    }
}

pub fn date_text(z: i64) -> String {
    let (y, m, d) = civil(z);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------- the church's readings ----------

fn csv_line(line: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
            c => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    pub by_date: usize,
    pub by_sunday: usize,
    pub skipped: usize,
}

/// Brings in a lectionary from CSV (commas, semicolons or tabs). A header row names the
/// columns: `date` (2026-11-29 or 29/11/2026) or `sunday` ("Advent 1") with `year` (A, B,
/// C or ABC); optional `occasion`; every other column is a reading, in order (first reading,
/// psalm, epistle, gospel…). Replaces what was imported before.
pub fn import(conn: &Connection, text: &str) -> Result<ImportReport> {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head = lines.next().ok_or_else(|| bad("The file is empty."))?;
    let sep = if head.contains('\t') { '\t' } else if head.matches(';').count() > head.matches(',').count() { ';' } else { ',' };
    let cols: Vec<String> = csv_line(head, sep).iter().map(|c| c.to_lowercase()).collect();
    let at = |names: &[&str]| cols.iter().position(|c| names.contains(&c.as_str()));
    let date_c = at(&["date", "day"]);
    let sunday_c = at(&["sunday", "name", "lectionary day"]);
    let year_c = at(&["year", "cycle"]);
    let occ_c = at(&["occasion", "title", "theme"]);
    if date_c.is_none() && sunday_c.is_none() {
        return Err(bad("The first row should name the columns, with “date” or “sunday” among them."));
    }
    let reading_cols: Vec<usize> = (0..cols.len()).filter(|i| Some(*i) != date_c && Some(*i) != sunday_c && Some(*i) != year_c && Some(*i) != occ_c).collect();
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM lectionary", [])?;
    let mut rep = ImportReport::default();
    for line in lines {
        let f = csv_line(line, sep);
        let get = |i: Option<usize>| i.and_then(|i| f.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let readings: Vec<String> = reading_cols.iter().filter_map(|i| f.get(*i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let occasion = get(occ_c);
        let key = if let Some(z) = get(date_c).and_then(|d| parse_date(&d)) {
            rep.by_date += 1;
            format!("date:{}", date_text(z))
        } else if let Some(name) = get(sunday_c) {
            let y = get(year_c).unwrap_or_else(|| "ABC".into()).to_uppercase();
            rep.by_sunday += 1;
            format!("sunday:{y}:{}", name.to_lowercase())
        } else {
            rep.skipped += 1;
            continue;
        };
        tx.execute("INSERT OR REPLACE INTO lectionary (key, occasion, readings) VALUES (?1, ?2, ?3)", params![key, occasion, serde_json::to_string(&readings)?])?;
    }
    tx.commit()?;
    Ok(rep)
}

#[derive(Debug, Clone, Serialize)]
pub struct Readings {
    pub date: String,
    pub day: Option<Day>,
    /// the church's own name for the day, when its lectionary gives one
    pub occasion: Option<String>,
    pub readings: Vec<String>,
    /// how many entries the imported lectionary has (0: none imported yet)
    pub entries: i64,
}

/// The day and the church's readings for a date: by date first, else by the Sunday's name
/// for its year (or for every year).
pub fn readings(conn: &Connection, date: &str) -> Result<Readings> {
    let z = parse_date(date).ok_or_else(|| bad("Not a date."))?;
    let day = day(z);
    let entries: i64 = conn.query_row("SELECT count(*) FROM lectionary", [], |r| r.get(0))?;
    let find = |key: String| -> Result<Option<(Option<String>, String)>> { Ok(conn.query_row("SELECT occasion, readings FROM lectionary WHERE key = ?1", [key], |r| Ok((r.get(0)?, r.get(1)?))).optional()?) };
    let mut hit = find(format!("date:{}", date_text(z)))?;
    if hit.is_none() {
        if let Some(dd) = &day {
            for y in [dd.year.as_str(), "ABC"] {
                hit = find(format!("sunday:{y}:{}", dd.name.to_lowercase()))?;
                if hit.is_some() {
                    break;
                }
            }
        }
    }
    let (occasion, readings) = match hit {
        Some((o, r)) => (o, serde_json::from_str(&r).unwrap_or_default()),
        None => (None, vec![]),
    };
    Ok(Readings { date: date_text(z), day, occasion, readings, entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_easter() {
        assert_eq!(civil(days(2026, 11, 29)), (2026, 11, 29));
        assert_eq!(weekday(days(2026, 11, 29)), 0, "a Sunday");
        for (y, m, d) in [(2024, 3, 31), (2025, 4, 20), (2026, 4, 5), (2027, 3, 28), (2038, 4, 25)] {
            assert_eq!(civil(easter(y)), (y, m, d), "Easter {y}");
        }
    }

    #[test]
    fn the_calendar_agrees_with_an_independent_one_on_every_day_2013_to_2035() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../tests/data/lectionary_names.json")).unwrap();
        let days_ref = fixture["days"].as_object().unwrap();
        // where the reference itself is wrong, by the lectionary's own rules:
        // - 2017, 2023, 2028, 2034: its Propers are one out (2023-06-11 is Proper 5: the
        //   Sunday between 5 and 11 June; it says Proper 6)
        // - a Sunday 31 December or 1 January it calls Epiphany; the lectionary has Christmas 1
        //   there (26 December – 1 January)
        let known_wrong = |date: &str, name: &str| {
            (name.starts_with("Proper") && ["2017", "2023", "2028", "2034"].contains(&&date[..4])) || ((date.ends_with("-01-01") || date.ends_with("-12-31")) && name == "Epiphany Day")
        };
        let mut wrong = Vec::new();
        let mut checked = 0;
        for (date, v) in days_ref {
            if known_wrong(date, v[0].as_str().unwrap()) {
                continue;
            }
            checked += 1;
            let (name, year) = (v[0].as_str().unwrap(), v[1].as_str().unwrap());
            let ours = day(parse_date(date).unwrap());
            let ok = match &ours {
                Some(d) => d.name == name && (year.contains('-') || d.year == year),
                None => false,
            };
            if !ok {
                wrong.push(format!("{date}: {name} {year} — ours {:?}", ours.map(|d| (d.name, d.year))));
            }
        }
        assert!(checked > 1250, "most of the reference is checked ({checked})");
        assert!(wrong.is_empty(), "{} of {checked} differ, e.g. {:?}", wrong.len(), &wrong[..wrong.len().min(8)]);
    }

    #[test]
    fn a_churchs_lectionary_by_date_or_by_sunday() {
        let c = crate::db::open_memory().unwrap();
        let csv = "date,occasion,first reading,psalm,epistle,gospel\n29/11/2026,Advent Sunday,Isaiah 64:1-9,\"Psalm 80:1-7, 17-19\",1 Corinthians 1:3-9,Mark 13:24-37\n";
        let rep = import(&c, csv).unwrap();
        assert_eq!((rep.by_date, rep.by_sunday), (1, 0));
        let r = readings(&c, "2026-11-29").unwrap();
        assert_eq!(r.day.as_ref().map(|d| (d.name.as_str(), d.year.as_str())), Some(("Advent 1", "B")));
        assert_eq!(r.readings, vec!["Isaiah 64:1-9", "Psalm 80:1-7, 17-19", "1 Corinthians 1:3-9", "Mark 13:24-37"], "a quoted comma stays in its reading");
        assert_eq!(r.occasion.as_deref(), Some("Advent Sunday"));
        // by Sunday name and year, for any year that Sunday comes round
        import(&c, "sunday;year;reading 1;gospel\nAdvent 1;B;Isaiah 64:1-9;Mark 13:24-37\nEaster Day;ABC;Acts 10:34-43;John 20:1-18\n").unwrap();
        assert_eq!(readings(&c, "2026-11-29").unwrap().readings[1], "Mark 13:24-37");
        assert_eq!(readings(&c, "2027-03-28").unwrap().readings[0], "Acts 10:34-43", "Easter Day 2027, every year");
        assert!(readings(&c, "2027-03-29").unwrap().day.is_none(), "an ordinary Monday");
        assert!(import(&c, "a,b\n1,2").is_err(), "no date or sunday column");
    }
}

#[cfg(test)]
mod reference_tests {
    #[test]
    fn readings_read_as_references() {
        let c = crate::db::open_memory().unwrap();
        let aliases = crate::library::aliases(&c).unwrap();
        // (a reading with several ranges is added as one item per range)
        for r in ["Isaiah 64:1-9", "Psalm 80:1-7", "Psalm 80:16-18", "Galatians 4:4-7", "1 Corinthians 1:3-9", "Mark 13:24-37", "Isaiah 61:10-62:3", "Psalm 126"] {
            assert!(crate::reference::parse(r, &aliases).is_ok(), "{r} reads as a reference");
        }
    }
}
