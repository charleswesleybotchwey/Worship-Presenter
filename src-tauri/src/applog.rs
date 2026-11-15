//! The app's own log: one file a day in the data folder's Logs (the last 30 days kept).
//! Crashes (with where they happened), errors from any window and important events, so
//! a problem can be sent and fixed. No song words, notes or other content are written.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

fn today() -> (String, String) {
    // local date and time without extra crates: seconds since 1970 adjusted by the offset
    // Windows reports (good enough for a log name; times are also given in UTC)
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = now.div_euclid(86_400);
    let secs = now.rem_euclid(86_400);
    // civil date from days since 1970 (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (format!("{y:04}-{m:02}-{d:02}"), format!("{:02}:{:02}:{:02}Z", secs / 3600, secs % 3600 / 60, secs % 60))
}

/// Starts logging into `dir` and records crashes there.
pub fn init(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    // keep 30 days
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "log")).collect();
        files.sort();
        while files.len() > 30 {
            let _ = std::fs::remove_file(files.remove(0));
        }
    }
    *DIR.lock().unwrap() = Some(dir);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let what = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        write("CRASH", &format!("{what} (at {at}, thread {thread})"));
        previous(info);
    }));
    write("START", &format!("Worship Presenter {} on {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS));
}

pub fn write(kind: &str, text: &str) {
    let Some(dir) = DIR.lock().ok().and_then(|d| d.clone()) else { return };
    let (date, time) = today();
    let line = format!("{date} {time} {kind:<6} {}\n", text.replace('\n', " | ").chars().take(2000).collect::<String>());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{date}.log"))) {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn dir() -> Option<PathBuf> {
    DIR.lock().ok().and_then(|d| d.clone())
}

/// The last lines of the recent logs, for "Copy a diagnostic report".
pub fn recent(lines: usize) -> String {
    let Some(dir) = dir() else { return String::new() };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "log")).collect()).unwrap_or_default();
    files.sort();
    let mut all: Vec<String> = vec![];
    for f in files.iter().rev().take(3).rev() {
        if let Ok(t) = std::fs::read_to_string(f) {
            all.extend(t.lines().map(str::to_string));
        }
    }
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates_and_lines() {
        let (d, t) = super::today();
        assert_eq!((d.len(), &d[4..5], t.len()), (10, "-", 9), "{d} {t}");
        let dir = std::env::temp_dir().join(format!("wp-log-{}", std::process::id()));
        super::init(dir.clone());
        super::write("ERROR", "line one\nline two");
        let r = super::recent(10);
        assert!(r.contains("START") && r.contains("ERROR  line one | line two"), "{r}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
