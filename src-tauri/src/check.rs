//! Before and during the service: the output dashboard (every screen, NDI stream and the
//! recording at a glance) and "Check system" (everything that should be ready, each
//! problem with where to fix it).

use crate::AppState;
use wp_core::live::Mode;
use serde::Serialize;
use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, State};

#[derive(Serialize)]
pub struct OutputTile {
    pub role: String,
    pub name: String,
    /// "monitor", "ndi" or "recording"
    pub kind: String,
    /// live | blank | logo | clear | idle | held | silent | off | recording | streaming
    pub state: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct Dashboard {
    pub tiles: Vec<OutputTile>,
    pub screens_off: bool,
}

/// An output that has not answered for this long is shown as not answering.
const SILENT: f32 = 5.0;

#[tauri::command(async)]
pub fn dashboard(app: AppHandle, st: State<'_, AppState>) -> Dashboard {
    let screens_off = st.screens_off.lock().unwrap().is_some();
    let mode = st.live.lock().unwrap().mode;
    let has_item = st.item.lock().unwrap().is_some();
    let showing = match mode {
        Mode::Blank => "blank",
        Mode::Logo => "logo",
        Mode::Clear => "clear",
        Mode::Live if has_item => "live",
        _ => "idle",
    };
    let configs = wp_core::options::outputs(&st.db.lock().unwrap()).unwrap_or_default();
    let open = st.outputs.lock().unwrap().clone();
    let held = st.held.lock().unwrap().clone();
    let acks = st.acks.lock().unwrap().clone();
    let ndi: Vec<String> = st.ndi.jobs.lock().unwrap().keys().filter_map(|k| k.strip_prefix("out-").map(str::to_string)).collect();
    let mut tiles = vec![];
    for c in &configs {
        let in_use = c.display_index.is_some() || c.ndi.is_some() || c.custom.is_some() || open.contains_key(&c.role);
        if !in_use {
            continue;
        }
        let is_ndi = c.ndi.is_some();
        let (state, detail) = if screens_off {
            ("off", "Screens are switched off".to_string())
        } else if is_ndi {
            if ndi.contains(&c.role) { (showing, "Sending over NDI".to_string()) } else { ("off", "NDI stream not running".to_string()) }
        } else if !open.contains_key(&c.role) {
            ("off", "Not open on a monitor".to_string())
        } else if held.contains(&c.role) {
            ("held", "Waiting for the screen to be ready again".to_string())
        } else {
            let silent = acks.get(&c.role).map(|(t, _)| t.elapsed().as_secs_f32()).unwrap_or(0.0);
            if silent > SILENT {
                ("silent", format!("Has not answered for {} s", silent as u64))
            } else {
                (showing, c.display_name.clone().map(|n| format!("On {n}")).unwrap_or_else(|| "On its monitor".into()))
            }
        };
        tiles.push(OutputTile { role: c.role.clone(), name: c.name.clone(), kind: if is_ndi { "ndi".into() } else { "monitor".into() }, state: state.into(), detail });
    }
    // the recording and the stream online
    if let Some(r) = crate::stream::running_summary(&app) {
        tiles.push(r);
    }
    Dashboard { tiles, screens_off }
}

#[derive(Serialize)]
pub struct Check {
    pub id: String,
    pub label: String,
    /// ok | warn | bad
    pub status: String,
    pub detail: String,
    /// where to fix it (a Settings page, or "screens")
    pub fix: Option<String>,
}

fn check(id: &str, label: &str, status: &str, detail: impl Into<String>, fix: Option<&str>) -> Check {
    Check { id: id.into(), label: label.into(), status: status.into(), detail: detail.into(), fix: fix.map(str::to_string) }
}

/// Free space on the drive holding `p`, in bytes.
pub fn free_space(p: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut free: u64 = 0;
        let ok = unsafe { windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
        if ok != 0 { Some(free) } else { None }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0 { Some(s.f_bavail as u64 * s.f_frsize as u64) } else { None }
    }
}

fn newest_file_age(dir: &Path) -> Option<Duration> {
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()?.metadata().ok()?.modified().ok()).max().and_then(|t| t.elapsed().ok())
}

#[tauri::command(async)]
pub fn system_check(app: AppHandle, st: State<'_, AppState>) -> Vec<Check> {
    let mut out = vec![];
    let dash = dashboard(app.clone(), st.clone());
    // screens
    let main = dash.tiles.iter().find(|t| t.role == "main");
    out.push(match main {
        _ if dash.screens_off => check("screens", "Screens", "bad", "Every screen is switched off (the Screens button).", Some("screens")),
        None => check("main", "Main screen", "bad", "No main screen is set up: choose its monitor (or NDI) in Outputs.", Some("outputs")),
        Some(t) if t.state == "off" => check("main", "Main screen", "bad", format!("{}.", t.detail), Some("outputs")),
        Some(t) if t.state == "silent" || t.state == "held" => check("main", "Main screen", "bad", format!("{}.", t.detail), Some("outputs")),
        Some(t) => check("main", "Main screen", "ok", format!("{}.", t.detail), None),
    });
    for t in dash.tiles.iter().filter(|t| t.role != "main" && t.kind != "recording") {
        let bad = matches!(t.state.as_str(), "off" | "silent" | "held");
        out.push(check(&format!("out-{}", t.role), &t.name, if bad { "warn" } else { "ok" }, format!("{}.", t.detail), bad.then_some("outputs")));
    }
    // the library
    {
        let conn = st.db.lock().unwrap();
        let n = |q: &str| conn.query_row(q, [], |r| r.get::<_, i64>(0)).unwrap_or(0);
        let (songs, bibles) = (n("SELECT count(*) FROM song"), n("SELECT count(*) FROM bible_version"));
        out.push(if bibles == 0 {
            check("library", "Songs and Bibles", "warn", format!("{songs} songs, but no Bible is installed."), Some("import"))
        } else {
            check("library", "Songs and Bibles", "ok", format!("{songs} songs, {bibles} Bibles."), None)
        });
    }
    // disk space (recordings and media live here)
    let dir = st.profile_dir();
    match free_space(&dir) {
        Some(b) if b < 500 * 1024 * 1024 => out.push(check("disk", "Disk space", "bad", format!("Only {} MB free: recording will fail and the library cannot grow.", b / 1024 / 1024), None)),
        Some(b) if b < 3 * 1024 * 1024 * 1024 => out.push(check("disk", "Disk space", "warn", format!("{:.1} GB free: enough for about {} hours of recording.", b as f64 / 1e9, b / (1_500_000_000)), None)),
        Some(b) => out.push(check("disk", "Disk space", "ok", format!("{:.0} GB free.", b as f64 / 1e9), None)),
        None => {}
    }
    // backups
    out.push(match newest_file_age(&st.backups()) {
        Some(a) if a < Duration::from_secs(8 * 86400) => check("backup", "Backup", "ok", format!("Last backup {} day(s) ago.", a.as_secs() / 86400), None),
        Some(a) => check("backup", "Backup", "warn", format!("Last backup {} days ago: make one now.", a.as_secs() / 86400), Some("backups")),
        None => check("backup", "Backup", "warn", "No backup yet: make one now.", Some("backups")),
    });
    // scripture recognition, if it is used
    let setting = |k: &str| st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = ?1", [k], |r| r.get::<_, String>(0)).ok().unwrap_or_default();
    let engine = setting("speech_engine");
    if !engine.is_empty() {
        let sd = crate::speech::speech_dir(&st);
        let (ok, why) = match engine.as_str() {
            // as listening does: the chosen model, else small.en or base.en if downloaded
            "whisper" => {
                let m = setting("speech_whisper_model");
                let found = [m.as_str(), "small.en", "base.en"].iter().filter(|x| !x.is_empty()).any(|x| crate::speech::whisper_path(&sd, x).exists());
                (found, "no Whisper model is downloaded".to_string())
            }
            "vosk" => (crate::speech::vosk_ready(&sd), "the Vosk model is not downloaded".to_string()),
            "deepgram" => (!setting("deepgram_key").is_empty(), "no Deepgram key is saved".to_string()),
            _ => (true, String::new()),
        };
        out.push(if ok {
            check("speech", "Scripture recognition", "ok", format!("Ready ({engine})."), None)
        } else {
            check("speech", "Scripture recognition", "warn", format!("Not ready: {why}."), Some("speech"))
        });
    }
    // recording and streaming
    let ffmpeg = crate::stream::ffmpeg_path(&app).exists();
    out.push(if ffmpeg {
        check("record", "Recording and streaming", "ok", "FFmpeg is installed.", None)
    } else {
        check("record", "Recording and streaming", "warn", "FFmpeg is not installed yet (needed only to record or go live online; the Record panel installs it).", None)
    });
    // the phone remote
    if let Some(r) = st.remote.lock().unwrap().as_ref() {
        out.push(check("remote", "Phone remote", "ok", format!("On at http://{}.", r.addr), None));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn free_space_is_measured() {
        let free = super::free_space(&std::env::temp_dir()).expect("free space");
        assert!(free > 0);
    }
}
