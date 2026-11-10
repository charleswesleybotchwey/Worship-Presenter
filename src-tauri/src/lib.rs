//! Worship Presenter desktop shell.
//!
//! Thin layer over wp-core: it owns the database connection and the live state,
//! turns control-window commands into new states, and broadcasts each state (with
//! the slide to draw) to every window. Output windows render only what they are
//! sent, acknowledge the revision they drew, and fall back to a safe image when the
//! heartbeat stops (see ui/src/output).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
mod ndi;
mod remote;
mod music;
use wp_core::cues::{self, TimerShow};
use wp_core::library;
use wp_core::live::{Command, LiveState, Mode};
use wp_core::splitter::{self, LangMode, Options, Slide};

/// What is loaded in the Live pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgramItem {
    Song {
        song_id: i64,
        title: String,
        /// A look for this item only; None uses the theme in use.
        #[serde(default)]
        theme_id: Option<i64>,
        /// Lyrics changed for this service only; None uses the library.
        #[serde(default)]
        lyrics: Option<Vec<wp_core::songs::Lyric>>,
    },
    Scripture {
        reference: String,
        versions: Vec<String>,
        #[serde(default)]
        theme_id: Option<i64>,
    },
    Media { media_id: i64, title: String },
    Announcement { presentation_id: i64, title: String },
}

/// Payload every window receives on "live-state".
#[derive(Debug, Clone, Serialize)]
pub struct Broadcast {
    pub state: LiveState,
    pub item: Option<ProgramItem>,
    pub slides: Vec<Slide>,
    pub caption: Option<String>,
    /// Bumped whenever a theme or default changes; windows refetch themes then.
    pub theme_rev: u64,
    /// The picture or video when a media item is live.
    pub media: Option<wp_core::media::MediaItem>,
    /// Outputs that came back after being lost: they stay black until the operator
    /// chooses Resume for them (design brief §6).
    pub held: Vec<String>,
}

pub struct AppState {
    db: Mutex<Connection>,
    live: Mutex<LiveState>,
    item: Mutex<Option<ProgramItem>>,
    slides: Mutex<Vec<Slide>>,
    caption: Mutex<Option<String>>,
    /// Outputs the operator wants open: role → display index (None = window on control screen).
    outputs: Mutex<HashMap<String, Option<usize>>>,
    /// Last time each output acknowledged; a silent output is rebuilt.
    acks: Mutex<HashMap<String, (Instant, u64)>>,
    /// Schedule currently open, for auto-save and usage logging.
    schedule_id: Mutex<Option<i64>>,
    /// Folder holding every profile, and the active one.
    profiles_root: PathBuf,
    profile: Mutex<(String, PathBuf)>,
    theme_rev: Mutex<u64>,
    held: Mutex<std::collections::HashSet<String>>,
    lines_before_lining: Mutex<u8>,
    ndi: ndi::Ndi,
    remote: Mutex<Option<remote::Running>>,
    music: Mutex<Option<wp_audio::Music>>,
}

impl AppState {
    fn profile_dir(&self) -> PathBuf {
        self.profile.lock().unwrap().1.clone()
    }
    fn backups(&self) -> PathBuf {
        wp_core::backup::default_dir(&self.profile_dir())
    }
}

/// Re-splits whatever is live (after a song edit or restore), keeping the same line on screen.
fn refresh_live(app: &AppHandle, st: &AppState) {
    let item = st.item.lock().unwrap().clone();
    if let Some(item) = item {
        let mut next = st.live.lock().unwrap().clone();
        match build_slides(&st.db.lock().unwrap(), &item, &next) {
            Ok((slides, caption)) => {
                if let Some(pos) = next.position {
                    next.slide_idx = wp_core::splitter::find(&slides, pos);
                }
                *st.slides.lock().unwrap() = slides;
                *st.caption.lock().unwrap() = caption;
            }
            // the live song no longer exists: take it off air
            Err(_) => {
                *st.item.lock().unwrap() = None;
                st.slides.lock().unwrap().clear();
                next = next.apply(Command::SetMode { mode: Mode::Blank });
            }
        }
        next.revision += 1;
        *st.live.lock().unwrap() = next;
        broadcast(app, st);
    }
}

type Res<T> = Result<T, String>;
fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn build_slides(conn: &Connection, item: &ProgramItem, live: &LiveState) -> Res<(Vec<Slide>, Option<String>)> {
    match item {
        ProgramItem::Song { song_id, title, lyrics, .. } => {
            let (sections, langs) = match lyrics {
                // "Apply to this service": this item's own lyrics
                Some(l) if !l.is_empty() => (wp_core::songs::compile(l).map_err(err)?, l.iter().map(|x| x.lang.clone()).collect::<Vec<_>>()),
                _ => (library::song_sections(conn, *song_id).map_err(err)?, library::song_langs(conn, *song_id).map_err(err)?),
            };
            let primary = langs.first().cloned().unwrap_or_else(|| "en".into());
            let opts = Options {
                lines_per_slide: live.lines_per_slide,
                lang_mode: if langs.len() > 1 { live.lang_mode } else { LangMode::Primary },
                primary,
                second: langs.get(1).cloned(),
                sequence: vec![],
                include_title: false,
            };
            Ok((splitter::split(&sections, &opts), Some(title.clone())))
        }
        ProgramItem::Announcement { presentation_id, title } => {
            let d = wp_core::announcements::get(conn, *presentation_id).map_err(err)?;
            Ok((wp_core::announcements::to_slides(&d), Some(title.clone())))
        }
        ProgramItem::Media { title, .. } => Ok((
            vec![Slide { label: "Media".into(), occurrence: 0, first_key: 0, last_key: 0, primary: vec![], second: vec![], image: None }],
            Some(title.clone()),
        )),
        ProgramItem::Scripture { reference, versions, .. } => {
            let codes: Vec<&str> = versions.iter().map(String::as_str).collect();
            let p = library::lookup(conn, reference, &codes).map_err(err)?;
            let first = codes.first().copied().unwrap_or_default();
            let mut slides = Vec::new();
            for v in p.verses.iter().filter(|v| v.version == first) {
                let second: Vec<String> = p
                    .verses
                    .iter()
                    .filter(|o| o.version != first && o.chapter == v.chapter && o.verse == v.verse)
                    .map(|o| o.text.clone())
                    .collect();
                let key = (v.chapter as i64) * 1000 + v.verse as i64;
                slides.push(Slide {
                    label: format!("{}:{}", v.chapter, v.verse),
                    occurrence: 0,
                    first_key: key,
                    last_key: key,
                    primary: vec![v.text.clone()],
                    second,
                    image: None,
                });
            }
            Ok((slides, Some(p.display)))
        }
    }
}

fn broadcast(app: &AppHandle, s: &AppState) {
    let payload = Broadcast {
        state: s.live.lock().unwrap().clone(),
        item: s.item.lock().unwrap().clone(),
        slides: s.slides.lock().unwrap().clone(),
        caption: s.caption.lock().unwrap().clone(),
        theme_rev: *s.theme_rev.lock().unwrap(),
        media: None,
        held: s.held.lock().unwrap().iter().cloned().collect(),
    };
    let mut payload = payload;
    if let Some(ProgramItem::Media { media_id, .. }) = &payload.item {
        payload.media = wp_core::media::get(&s.db.lock().unwrap(), &s.profile_dir().join("media"), *media_id).ok();
    }
    let _ = payload.state.save(&s.profile_dir().join("state.json"));
    let _ = app.emit("live-state", payload);
    // phones and the browser stage display get the same change
    let r = s.remote.lock().unwrap().as_ref().map(|x| x.remote.clone());
    if let Some(r) = r {
        r.publish();
    }
}

#[tauri::command]
fn search_songs(st: State<'_, AppState>, query: String) -> Res<Vec<library::SongHit>> {
    library::search_songs(&st.db.lock().unwrap(), &query, 60).map_err(err)
}

#[tauri::command]
fn preview_song(st: State<'_, AppState>, song_id: i64, title: String) -> Res<Vec<Slide>> {
    let live = st.live.lock().unwrap().clone();
    build_slides(&st.db.lock().unwrap(), &ProgramItem::Song { song_id, title, theme_id: None, lyrics: None }, &live).map(|r| r.0)
}

#[tauri::command]
fn bible_versions(st: State<'_, AppState>) -> Res<Vec<library::Version>> {
    library::versions(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn preview_scripture(st: State<'_, AppState>, reference: String, versions: Vec<String>) -> Res<(Vec<Slide>, Option<String>)> {
    let live = st.live.lock().unwrap().clone();
    build_slides(&st.db.lock().unwrap(), &ProgramItem::Scripture { reference, versions, theme_id: None }, &live)
}

/// Sends an item to air at a slide.
#[tauri::command]
fn go_live(app: AppHandle, st: State<'_, AppState>, item: ProgramItem, slide_idx: usize) -> Res<LiveState> {
    let live0 = st.live.lock().unwrap().clone();
    if live0.latched() {
        return Err("Output is held (blank, logo or text cleared). Choose Resume output first.".into());
    }
    let (slides, caption) = build_slides(&st.db.lock().unwrap(), &item, &live0)?;
    let idx = slide_idx.min(slides.len().saturating_sub(1));
    let pos = slides.get(idx).map(splitter::position_of);
    let mut next = live0.apply(Command::GoLive { item_idx: 0, slide_idx: idx, position: pos });
    next.slide_since_ms = cues::now_ms();
    if let ProgramItem::Announcement { presentation_id, .. } = &item {
        if let Ok(d) = wp_core::announcements::get(&st.db.lock().unwrap(), *presentation_id) {
            let durations_ms = wp_core::announcements::durations_ms(&d);
            if !durations_ms.is_empty() {
                next.auto = Some(wp_core::live::AutoAdvance { durations_ms, looping: d.looping, paused: false });
            }
        }
    }
    if let ProgramItem::Song { song_id, .. } = &item {
        let sid = *st.schedule_id.lock().unwrap();
        let _ = wp_core::options::log_song_use(&st.db.lock().unwrap(), sid, *song_id);
    }
    let kind = serde_json::to_value(&item).ok().and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string)).unwrap_or_default();
    // release the music lock before possibly starting the player (which takes it)
    let told = {
        let g = st.music.lock().unwrap();
        g.as_ref().map(|m| m.send(wp_audio::rodio_backend::Cmd::GoLive(kind.clone()))).is_some()
    };
    if !told && kind == "announcement" {
        music::autostart_for_announcements(&st);
    }
    *st.item.lock().unwrap() = Some(item);
    *st.slides.lock().unwrap() = slides;
    *st.caption.lock().unwrap() = caption;
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// Moves within the live item: absolute index, or +1 / -1 via `delta`.
#[tauri::command]
fn show_slide(app: AppHandle, st: State<'_, AppState>, slide_idx: Option<usize>, delta: Option<i64>) -> Res<LiveState> {
    let n = st.slides.lock().unwrap().len();
    if n == 0 {
        return Err("Nothing is live. Send an item to Live first.".into());
    }
    let cur = st.live.lock().unwrap().clone();
    if cur.latched() {
        return Err("Output is held. Choose Resume output to go back to the live slide.".into());
    }
    let idx = match (slide_idx, delta) {
        (Some(i), _) => i.min(n - 1),
        (None, Some(d)) => (cur.slide_idx as i64 + d).clamp(0, n as i64 - 1) as usize,
        _ => cur.slide_idx,
    };
    let next = show_at(&st, &cur, idx);
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// Moves to a slide: restarts its timer and counts a return to the repeated section.
fn show_at(st: &AppState, cur: &LiveState, idx: usize) -> LiveState {
    let slides = st.slides.lock().unwrap();
    let pos = slides.get(idx).map(splitter::position_of);
    let mut next = cur.apply(Command::Show { slide_idx: idx, position: pos });
    next.slide_since_ms = cues::now_ms();
    if let (Some(r), Some(new), old) = (next.repeat.as_mut(), slides.get(idx), slides.get(cur.slide_idx)) {
        // entering the section's first slide from elsewhere (or jumping back into it)
        let entering = new.label == r.label && old.map_or(true, |o| o.label != r.label || o.occurrence != new.occurrence || idx < cur.slide_idx);
        let first_of_section = idx == 0 || slides.get(idx - 1).map_or(true, |p| p.label != new.label || p.occurrence != new.occurrence);
        if entering && first_of_section {
            r.done += 1;
        }
    }
    next
}

#[tauri::command]
fn set_mode(app: AppHandle, st: State<'_, AppState>, mode: Mode) -> Res<LiveState> {
    let next = st.live.lock().unwrap().apply(Command::SetMode { mode });
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// Changes lines per slide or language mode and re-splits the live item,
/// keeping the same words on screen.
#[tauri::command]
fn set_layout(app: AppHandle, st: State<'_, AppState>, lines: Option<u8>, lang_mode: Option<LangMode>) -> Res<LiveState> {
    let mut next = st.live.lock().unwrap().clone();
    if let Some(l) = lines {
        next = next.apply(Command::SetLinesPerSlide { lines: l });
    }
    if let Some(m) = lang_mode {
        next = next.apply(Command::SetLangMode { lang_mode: m });
    }
    let item = st.item.lock().unwrap().clone();
    if let Some(item) = item {
        let (slides, caption) = build_slides(&st.db.lock().unwrap(), &item, &next)?;
        if let Some(pos) = next.position {
            next.slide_idx = splitter::find(&slides, pos);
        }
        *st.slides.lock().unwrap() = slides;
        *st.caption.lock().unwrap() = caption;
    }
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

#[tauri::command]
fn undo_go_live(app: AppHandle, st: State<'_, AppState>) -> Res<LiveState> {
    let next = st.live.lock().unwrap().apply(Command::UndoGoLive);
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// Re-sends the current state; used by an output window when it (re)connects.
#[tauri::command]
fn resync(app: AppHandle, st: State<'_, AppState>) {
    broadcast(&app, &st);
}

#[derive(Serialize)]
struct Display {
    index: usize,
    name: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    primary: bool,
}

#[tauri::command]
fn list_displays(app: AppHandle) -> Res<Vec<Display>> {
    let primary = app.primary_monitor().map_err(err)?.and_then(|m| m.name().cloned());
    Ok(app
        .available_monitors()
        .map_err(err)?
        .iter()
        .enumerate()
        .map(|(i, m)| Display {
            index: i,
            name: m.name().cloned().unwrap_or_else(|| format!("Display {}", i + 1)),
            width: m.size().width,
            height: m.size().height,
            x: m.position().x,
            y: m.position().y,
            primary: m.name().cloned() == primary,
        })
        .collect())
}

/// Opens (or moves) an output window full-screen on a display. Async on purpose:
/// building a window inside a synchronous command deadlocks on Windows. With no display
/// it opens in a normal window on the control screen, so there is always a picture.
#[tauri::command]
async fn open_output(app: AppHandle, st: State<'_, AppState>, role: String, display_index: Option<usize>) -> Res<()> {
    // remember the monitor by index and name, so it is found again next time
    if let Ok(mut c) = wp_core::options::output(&st.db.lock().unwrap(), &role) {
        let name = display_index.and_then(|i| app.available_monitors().ok().and_then(|m| m.get(i).and_then(|m| m.name().cloned())));
        c.display_index = display_index;
        c.display_name = name;
        let _ = wp_core::options::save_output(&st.db.lock().unwrap(), &c);
    }
    st.outputs.lock().unwrap().insert(role.clone(), display_index);
    st.acks.lock().unwrap().insert(role.clone(), (Instant::now(), 0));
    build_output(&app, &role, display_index)
}

fn build_output(app: &AppHandle, role: &str, display_index: Option<usize>) -> Res<()> {
    let label = format!("output-{role}");
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.destroy();
    }
    // the output's saved options: custom position and size, alpha channel
    let cfg = app.try_state::<AppState>().and_then(|st| wp_core::options::output(&st.db.lock().unwrap(), role).ok());
    let alpha = cfg.as_ref().map_or(false, |c| c.alpha);
    let custom = cfg.as_ref().and_then(|c| c.custom);
    let monitors = app.available_monitors().map_err(err)?;
    let mut b = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(format!("index.html#/output/{role}").into()))
        .title(format!("Worship Presenter – {role} output"))
        .decorations(false)
        .resizable(true)
        .focused(false)
        .transparent(alpha)
        .skip_taskbar(true);
    // A chosen display that is not there is not replaced by the operator's screen.
    if let Some(i) = display_index {
        if monitors.get(i).is_none() {
            return Err(format!("Display {} is not connected.", i + 1));
        }
    }
    match display_index.and_then(|i| monitors.get(i)) {
        Some(m) => {
            let scale = m.scale_factor();
            let pos = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            b = b.position(pos.x, pos.y).inner_size(size.width, size.height).always_on_top(true);
            let w = b.build().map_err(err)?;
            match custom {
                // EasyWorship's Output Position: pixels from the monitor's top-left corner
                Some(r) => {
                    let p = m.position();
                    w.set_position(tauri::PhysicalPosition::new(p.x + r.x, p.y + r.y)).map_err(err)?;
                    w.set_size(tauri::PhysicalSize::new(r.w, r.h)).map_err(err)?;
                }
                None => w.set_fullscreen(true).map_err(err)?,
            }
        }
        // The operator chose "window on this screen".
        None => {
            b.inner_size(960.0, 540.0).decorations(true).skip_taskbar(false).build().map_err(err)?;
        }
    }
    Ok(())
}

/// The operator's Resume for one output that came back after being lost.
#[tauri::command]
fn release_output(app: AppHandle, st: State<'_, AppState>, role: String) {
    st.held.lock().unwrap().remove(&role);
    broadcast(&app, &st);
}

/// Called by each output on every state and heartbeat it renders.
#[tauri::command]
fn output_ack(st: State<'_, AppState>, role: String, revision: u64) {
    st.acks.lock().unwrap().insert(role, (Instant::now(), revision));
}

#[derive(Serialize)]
struct OutputStatus {
    role: String,
    display_index: Option<usize>,
    /// Revision the output last drew, and seconds since it last answered.
    revision: u64,
    silent_secs: f32,
}

#[tauri::command]
fn output_status(st: State<'_, AppState>) -> Vec<OutputStatus> {
    let acks = st.acks.lock().unwrap();
    st.outputs
        .lock()
        .unwrap()
        .iter()
        .map(|(role, d)| {
            let (t, rev) = acks.get(role).copied().unwrap_or((Instant::now(), 0));
            OutputStatus { role: role.clone(), display_index: *d, revision: rev, silent_secs: t.elapsed().as_secs_f32() }
        })
        .collect()
}

#[tauri::command]
fn close_output(app: AppHandle, st: State<'_, AppState>, role: String) {
    st.outputs.lock().unwrap().remove(&role);
    st.held.lock().unwrap().remove(&role);
    st.acks.lock().unwrap().remove(&role);
    if let Some(w) = app.get_webview_window(&format!("output-{role}")) {
        let _ = w.destroy();
    }
}

// ---------- service orders ----------

#[tauri::command]
fn list_schedules(st: State<'_, AppState>) -> Res<Vec<wp_core::schedule::Summary>> {
    wp_core::schedule::list(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn open_schedule(st: State<'_, AppState>, id: i64) -> Res<wp_core::schedule::Schedule> {
    let s = wp_core::schedule::load(&st.db.lock().unwrap(), id).map_err(err)?;
    *st.schedule_id.lock().unwrap() = Some(id);
    let _ = st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('last_schedule', ?1)", [id.to_string()]);
    Ok(s)
}

/// Creates a service order, optionally filled from a template, and opens it.
#[tauri::command]
fn new_schedule(st: State<'_, AppState>, name: String, service_date: Option<String>, template: bool, from_template: Option<i64>) -> Res<wp_core::schedule::Schedule> {
    let id = {
        let c = st.db.lock().unwrap();
        match from_template {
            Some(t) => wp_core::schedule::from_template(&c, t, &name, service_date.as_deref()),
            None => wp_core::schedule::create(&c, &name, service_date.as_deref(), template),
        }
        .map_err(err)?
    };
    open_schedule(st, id)
}

/// Auto-save: the control window calls this on every change to the order.
#[tauri::command]
fn save_schedule(st: State<'_, AppState>, id: i64, items: Vec<wp_core::schedule::Item>) -> Res<()> {
    wp_core::schedule::save_items(&st.db.lock().unwrap(), id, &items).map_err(err)
}

#[tauri::command]
fn rename_schedule(st: State<'_, AppState>, id: i64, name: String, service_date: Option<String>, template: bool) -> Res<()> {
    wp_core::schedule::rename(&st.db.lock().unwrap(), id, &name, service_date.as_deref(), template).map_err(err)
}

#[tauri::command]
fn delete_schedule(st: State<'_, AppState>, id: i64) -> Res<()> {
    let mut cur = st.schedule_id.lock().unwrap();
    if *cur == Some(id) {
        *cur = None;
    }
    wp_core::schedule::delete(&st.db.lock().unwrap(), id).map_err(err)
}

/// The schedule that was open when the app last closed (power-cut recovery).
#[tauri::command]
fn last_schedule(st: State<'_, AppState>) -> Option<i64> {
    st.db
        .lock()
        .unwrap()
        .query_row("SELECT value FROM setting WHERE key = 'last_schedule'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|v| v.parse().ok())
}

/// Imports into a scratch library and reports what would happen; writes nothing.
#[tauri::command]
async fn import_review(app: AppHandle, kind: String, path: String, code: Option<String>) -> Res<wp_core::import::review::Review> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let conn = st.db.lock().unwrap();
        wp_core::import::review::review(&conn, &kind, std::path::Path::new(&path), code.as_deref()).map_err(err)
    })
    .await
    .map_err(err)?
}

/// Song files (OpenLyrics, OpenSong, ProPresenter, SongSelect, ChordPro, text): review.
#[tauri::command]
async fn song_files_review(app: AppHandle, paths: Vec<String>) -> Res<wp_core::import::review::Review> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let r = wp_core::import::songfiles::review(&st.db.lock().unwrap(), &paths).map_err(err);
        r
    })
    .await
    .map_err(err)?
}

#[tauri::command]
async fn song_files_import(app: AppHandle, paths: Vec<String>, skip: Option<Vec<String>>) -> Res<wp_core::import::Report> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let conn = st.db.lock().unwrap();
        wp_core::backup::create(&conn, &st.backups(), "before-import").map_err(err)?;
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let skip: Option<std::collections::HashSet<String>> = skip.map(|v| v.iter().map(|t| t.trim().to_lowercase()).collect());
        wp_core::import::songfiles::import(&conn, &paths, skip.as_ref()).map_err(err)
    })
    .await
    .map_err(err)?
}

/// Imports a library file chosen in the Import screen and returns the counts.
/// Runs off the main thread so outputs keep drawing during a long import.
#[tauri::command]
async fn import_file(app: AppHandle, kind: String, path: String, code: Option<String>, skip: Option<Vec<String>>) -> Res<wp_core::import::Report> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let conn = st.db.lock().unwrap();
        wp_core::backup::create(&conn, &st.backups(), "before-import").map_err(err)?;
        let p = PathBuf::from(&path);
        let r = match kind.as_str() {
            "ewb" => wp_core::import::ewb::import(&conn, &p, code.as_deref()),
            "bibleshow" => wp_core::import::bibleshow::import_file(&conn, &p, code.as_deref()),
            "ew-songs" => {
                // `path` is the EasyWorship folder holding Songs.db and SongWords.db
                let name = p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| "EasyWorship songs".into());
                let skip: Option<std::collections::HashSet<String>> = skip.map(|v| v.iter().map(|t| t.trim().to_lowercase()).collect());
                wp_core::import::ew_songs::import_filtered(&conn, &p.join("Songs.db"), &p.join("SongWords.db"), &name, skip.as_ref())
            }
            other => return Err(format!("Unknown import type {other}")),
        };
        r.map_err(|e| match e {
            // two Bibles with the same short code: tell the operator what to do
            wp_core::Error::Db(ref d) if d.to_string().contains("UNIQUE") => {
                "A Bible with this short code is already in the library. Give this one a different code.".to_string()
            }
            e => e.to_string(),
        })
    })
    .await
    .map_err(err)?
}

// ---------- song editor ----------

#[tauri::command]
fn get_song(st: State<'_, AppState>, id: i64) -> Res<wp_core::songs::SongDoc> {
    wp_core::songs::load(&st.db.lock().unwrap(), id).map_err(err)
}

/// Slides for the editor's live preview, without saving.
#[tauri::command]
fn preview_song_doc(st: State<'_, AppState>, doc: wp_core::songs::SongDoc) -> Res<Vec<Slide>> {
    let lines = st.live.lock().unwrap().lines_per_slide;
    wp_core::songs::preview(&doc, lines).map_err(err)
}

/// Saves to the library. Never changes the screens: if this song is live, the
/// result says so and the operator chooses "Update the screens" (brief §5).
#[derive(Serialize)]
struct SongSaved {
    id: i64,
    live: bool,
}

#[tauri::command]
fn save_song(st: State<'_, AppState>, doc: wp_core::songs::SongDoc) -> Res<SongSaved> {
    let id = wp_core::songs::save(&st.db.lock().unwrap(), &doc).map_err(err)?;
    let live = matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Song { song_id, lyrics: None, .. }) if *song_id == id);
    Ok(SongSaved { id, live })
}

/// The operator's explicit "Update the screens" after editing the live song.
#[tauri::command]
fn refresh_live_item(app: AppHandle, st: State<'_, AppState>, title: Option<String>) -> Res<()> {
    if let (Some(t), Some(ProgramItem::Song { title, .. })) = (title, st.item.lock().unwrap().as_mut()) {
        *title = t;
    }
    refresh_live(&app, &st);
    Ok(())
}

#[tauri::command]
fn delete_song(st: State<'_, AppState>, id: i64) -> Res<()> {
    let live_is_this = matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Song { song_id, .. }) if *song_id == id);
    if live_is_this {
        return Err("This song is on the screens. Send something else live first, then delete it.".into());
    }
    wp_core::songs::delete(&st.db.lock().unwrap(), id).map_err(err)
}

// ---------- backups ----------

#[tauri::command]
fn list_backups(st: State<'_, AppState>) -> Res<Vec<wp_core::backup::BackupInfo>> {
    wp_core::backup::list(&st.backups()).map_err(err)
}

#[tauri::command]
fn backup_now(st: State<'_, AppState>) -> Res<wp_core::backup::BackupInfo> {
    wp_core::backup::create(&st.db.lock().unwrap(), &st.backups(), "manual").map_err(err)
}

/// What a backup file holds, shown before the operator confirms a restore.
#[tauri::command]
fn inspect_backup(path: String) -> Res<wp_core::backup::Contents> {
    wp_core::backup::verify(std::path::Path::new(&path)).map_err(err)
}

/// Restores a backup. Returns the safety copy of the library it replaced.
#[tauri::command]
fn restore_backup(app: AppHandle, st: State<'_, AppState>, path: String) -> Res<wp_core::backup::BackupInfo> {
    let safety = {
        let mut conn = st.db.lock().unwrap();
        wp_core::backup::restore(&mut conn, std::path::Path::new(&path), &st.backups()).map_err(err)?
    };
    *st.schedule_id.lock().unwrap() = None;
    *st.theme_rev.lock().unwrap() += 1;
    refresh_live(&app, &st);
    broadcast(&app, &st);
    let _ = app.emit("library-restored", ());
    Ok(safety)
}

/// Opens the backups folder in Explorer so backups can be copied to a USB stick.
#[tauri::command]
fn backups_folder(st: State<'_, AppState>) -> Res<String> {
    let dir = st.backups();
    std::fs::create_dir_all(&dir).map_err(err)?;
    Ok(dir.to_string_lossy().into_owned())
}

// ---------- themes ----------

#[derive(Serialize)]
struct ActiveThemes {
    rev: u64,
    song: serde_json::Value,
    scripture: serde_json::Value,
    lower_third: serde_json::Value,
    announcement: serde_json::Value,
}

/// The default theme of each kind, for outputs, Preview, Live and thumbnails.
#[tauri::command]
fn active_themes(st: State<'_, AppState>) -> Res<ActiveThemes> {
    let c = st.db.lock().unwrap();
    let k = |kind: &str| wp_core::themes::for_kind(&c, kind).map_err(err);
    Ok(ActiveThemes { rev: *st.theme_rev.lock().unwrap(), song: k("song")?, scripture: k("scripture")?, lower_third: k("lower_third")?, announcement: k("announcement")? })
}

#[tauri::command]
fn list_themes(st: State<'_, AppState>) -> Res<Vec<wp_core::themes::ThemeRow>> {
    wp_core::themes::list(&st.db.lock().unwrap()).map_err(err)
}

fn themes_changed(app: &AppHandle, st: &AppState) {
    *st.theme_rev.lock().unwrap() += 1;
    broadcast(app, st);
}

/// Saves a theme (new when id is None); outputs pick up the change at once.
#[tauri::command]
fn save_theme(app: AppHandle, st: State<'_, AppState>, id: Option<i64>, theme: serde_json::Value) -> Res<i64> {
    let id = wp_core::themes::save(&st.db.lock().unwrap(), id, &theme).map_err(err)?;
    themes_changed(&app, &st);
    Ok(id)
}

#[tauri::command]
fn duplicate_theme(st: State<'_, AppState>, id: i64) -> Res<i64> {
    wp_core::themes::duplicate(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
fn delete_theme(app: AppHandle, st: State<'_, AppState>, id: i64) -> Res<()> {
    wp_core::themes::delete(&st.db.lock().unwrap(), id).map_err(err)?;
    themes_changed(&app, &st);
    Ok(())
}

#[tauri::command]
fn set_default_theme(app: AppHandle, st: State<'_, AppState>, kind: String, id: i64) -> Res<()> {
    wp_core::themes::set_default(&st.db.lock().unwrap(), &kind, id).map_err(err)?;
    themes_changed(&app, &st);
    Ok(())
}

// ---------- media ----------

#[tauri::command]
fn media_root(st: State<'_, AppState>) -> String {
    st.profile_dir().join("media").to_string_lossy().into_owned()
}

#[tauri::command]
fn list_media(st: State<'_, AppState>, kind: Option<String>, query: String) -> Res<Vec<wp_core::media::MediaItem>> {
    wp_core::media::list(&st.db.lock().unwrap(), &st.profile_dir().join("media"), kind.as_deref(), &query).map_err(err)
}

#[derive(Serialize)]
struct MediaImport {
    file: String,
    item: Option<wp_core::media::MediaItem>,
    error: Option<String>,
}

/// Copies files into the library. Videos can be large, so this runs off the main thread.
#[tauri::command]
async fn import_media(app: AppHandle, paths: Vec<String>) -> Res<Vec<MediaImport>> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let dir = st.profile_dir().join("media");
        paths
            .into_iter()
            .map(|p| {
                let conn = st.db.lock().unwrap();
                match wp_core::media::import(&conn, &dir, std::path::Path::new(&p)) {
                    Ok(item) => MediaImport { file: p, item: Some(item), error: None },
                    Err(e) => MediaImport { file: p, item: None, error: Some(e.to_string()) },
                }
            })
            .collect()
    })
    .await
    .map_err(err)
}

#[tauri::command]
fn relink_media(st: State<'_, AppState>, id: i64, path: String) -> Res<wp_core::media::MediaItem> {
    wp_core::media::relink(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id, std::path::Path::new(&path)).map_err(err)
}

/// Shows a test pattern on one output for a few seconds. The UI asks first when it is Main.
#[tauri::command]
fn identify_output(app: AppHandle, role: String) {
    let _ = app.emit("identify", role);
}

/// Settings the core uses; only these keys can be written.
const SETTINGS: &[&str] = &["project_on_restart", "low_spec_mode", "lines_per_slide", "advance_after_go_live", "font_fallback"];

#[tauri::command]
fn get_settings(st: State<'_, AppState>) -> Res<std::collections::HashMap<String, String>> {
    let c = st.db.lock().unwrap();
    let mut out = std::collections::HashMap::new();
    for k in SETTINGS {
        if let Ok(v) = c.query_row("SELECT value FROM setting WHERE key = ?1", [k], |r| r.get::<_, String>(0)) {
            out.insert(k.to_string(), v);
        }
    }
    Ok(out)
}

#[tauri::command]
fn set_setting(app: AppHandle, st: State<'_, AppState>, key: String, value: String) -> Res<()> {
    if !SETTINGS.contains(&key.as_str()) {
        return Err(format!("Unknown setting {key}"));
    }
    st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2)", [&key, &value]).map_err(err)?;
    let _ = app.emit("settings-changed", (key, value));
    Ok(())
}

#[tauri::command]
fn set_media_meta(st: State<'_, AppState>, id: i64, duration_ms: Option<i64>, width: Option<i64>, height: Option<i64>) -> Res<()> {
    wp_core::media::set_meta(&st.db.lock().unwrap(), id, duration_ms, width, height).map_err(err)
}

#[tauri::command]
fn save_media_thumb(st: State<'_, AppState>, id: i64, jpeg: Vec<u8>) -> Res<String> {
    wp_core::media::save_thumb(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id, &jpeg).map_err(err)
}

#[tauri::command]
fn rename_media(st: State<'_, AppState>, id: i64, title: String, tags: Option<String>) -> Res<()> {
    wp_core::media::rename(&st.db.lock().unwrap(), id, &title, tags.as_deref()).map_err(err)
}

#[tauri::command]
fn media_used_by(st: State<'_, AppState>, id: i64) -> Res<Vec<String>> {
    wp_core::media::used_by(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id).map_err(err)
}

#[tauri::command]
fn delete_media(st: State<'_, AppState>, id: i64) -> Res<()> {
    let live_is_this = matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Media { media_id, .. }) if *media_id == id);
    if live_is_this {
        return Err("This is on the screens. Send something else live first, then delete it.".into());
    }
    wp_core::media::delete(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id).map_err(err)
}

/// Play/pause or restart the live video.
#[tauri::command]
fn media_control(app: AppHandle, st: State<'_, AppState>, playing: Option<bool>, restart: Option<bool>) -> Res<LiveState> {
    let mut next = st.live.lock().unwrap().clone();
    if restart == Some(true) {
        next = next.apply(Command::RestartMedia);
    }
    if let Some(p) = playing {
        next = next.apply(Command::SetMediaPlaying { playing: p });
    }
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// One theme by id (a per-item look), for windows that need more than the defaults.
#[tauri::command]
fn get_theme(st: State<'_, AppState>, id: i64) -> Res<serde_json::Value> {
    wp_core::themes::get(&st.db.lock().unwrap(), id).map_err(err)
}

/// Saves a theme and its library files as one .wptheme file.
#[tauri::command]
async fn export_theme(app: AppHandle, id: i64, path: String) -> Res<u64> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let bytes = wp_core::themes::export(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id).map_err(err)?;
        std::fs::write(&path, &bytes).map_err(err)?;
        Ok(bytes.len() as u64)
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
struct ImportedTheme {
    id: i64,
    name: String,
}

#[tauri::command]
async fn import_theme(app: AppHandle, path: String) -> Res<ImportedTheme> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let bytes = std::fs::read(&path).map_err(err)?;
        if bytes.len() > 400_000_000 {
            return Err("This theme file is over 400 MB.".to_string());
        }
        let (id, name) = wp_core::themes::import(&st.db.lock().unwrap(), &st.profile_dir().join("media"), &bytes).map_err(err)?;
        Ok(ImportedTheme { id, name })
    })
    .await
    .map_err(err)?
}

/// A note for the stage display only. Empty clears it.
#[tauri::command]
fn set_stage_message(app: AppHandle, st: State<'_, AppState>, message: Option<String>) -> Res<LiveState> {
    let next = st.live.lock().unwrap().apply(Command::SetStageMessage { message });
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

// ---------- PowerPoint and PDF ----------

#[derive(Serialize)]
struct PresentationPlan {
    /// "done" (a deck was made) or "render_pdf" (the window draws the PDF's pages)
    stage: String,
    /// "powerpoint", "libreoffice", "pdf" or "text_only"
    method: String,
    title: String,
    deck_id: Option<i64>,
    /// the PDF to draw, relative to the media folder
    pdf: Option<String>,
    token: Option<String>,
    warnings: Vec<String>,
}

/// Runs a program and gives up after `secs` (PowerPoint or LibreOffice can hang on a bad file).
fn run_with_timeout(mut cmd: std::process::Command, secs: u64) -> Result<bool, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let mut child = cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status.success());
        }
        if start.elapsed().as_secs() > secs {
            let _ = child.kill();
            return Err("took too long".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Imports a PowerPoint (.pptx, .ppt, .odp) or PDF into Presentations.
#[tauri::command]
async fn presentation_import(app: AppHandle, path: String) -> Res<PresentationPlan> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let media = st.profile_dir().join("media");
        let src = PathBuf::from(&path);
        let title = src.file_stem().map(|s| s.to_string_lossy().replace(['_', '-'], " ")).unwrap_or_else(|| "Presentation".into());
        let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let token = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let work = media.join("tmp").join(&token);
        std::fs::create_dir_all(&work).map_err(err)?;
        let plan = |stage: &str, method: &str, deck_id: Option<i64>, pdf: Option<String>, warnings: Vec<String>| PresentationPlan {
            stage: stage.into(),
            method: method.into(),
            title: title.clone(),
            deck_id,
            pdf,
            token: Some(token.clone()),
            warnings,
        };
        if ext == "pdf" {
            std::fs::copy(&src, work.join("source.pdf")).map_err(err)?;
            return Ok(plan("render_pdf", "pdf", None, Some(format!("tmp/{token}/source.pdf")), vec![]));
        }
        if !["pptx", "ppt", "pps", "ppsx", "odp"].contains(&ext.as_str()) {
            return Err("Choose a PowerPoint (.pptx, .ppt) or PDF file.".into());
        }
        let mut warnings = Vec::new();
        // 1. PowerPoint itself (Windows): exact pictures of every slide
        if cfg!(windows) {
            let mut cmd = std::process::Command::new("powershell");
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", &wp_core::presentations::powerpoint_script(&src, &work)]);
            match run_with_timeout(cmd, 180) {
                Ok(true) => {
                    let mut files: Vec<PathBuf> = std::fs::read_dir(&work).map_err(err)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |e| e == "png")).collect();
                    files.sort();
                    if !files.is_empty() {
                        let pics: Vec<(String, Vec<u8>)> = files.iter().filter_map(|f| std::fs::read(f).ok().map(|b| ("png".to_string(), b))).collect();
                        let id = wp_core::presentations::deck_from_pictures(&st.db.lock().unwrap(), &media, &title, &pics).map_err(err)?;
                        let _ = std::fs::remove_dir_all(&work);
                        return Ok(plan("done", "powerpoint", Some(id), None, vec!["Animations and slide transitions inside the PowerPoint are not kept; each slide is a picture.".into()]));
                    }
                }
                Ok(false) => warnings.push("PowerPoint is not installed or could not open the file.".into()),
                Err(e) => warnings.push(format!("PowerPoint {e}.")),
            }
        }
        // 2. LibreOffice: convert to PDF, then the window draws the pages
        for soffice in wp_core::presentations::soffice_candidates() {
            let mut cmd = std::process::Command::new(&soffice);
            cmd.args(["--headless", "--norestore", "--convert-to", "pdf", "--outdir"]).arg(&work).arg(&src);
            if let Ok(true) = run_with_timeout(cmd, 180) {
                let pdf = std::fs::read_dir(&work).ok().and_then(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().map_or(false, |e| e == "pdf")));
                if let Some(pdf) = pdf {
                    let _ = std::fs::rename(&pdf, work.join("source.pdf"));
                    return Ok(plan("render_pdf", "libreoffice", None, Some(format!("tmp/{token}/source.pdf")), vec![]));
                }
            }
        }
        // 3. Read the .pptx here: the words of every slide, and what was left out
        if ext != "pptx" && ext != "ppsx" {
            return Err("This older PowerPoint file needs PowerPoint or LibreOffice on this computer. Save it as .pptx or PDF, or install LibreOffice (free).".into());
        }
        let slides = wp_core::presentations::read_pptx(&src).map_err(err)?;
        let id = wp_core::presentations::deck_from_text(&st.db.lock().unwrap(), &title, &slides).map_err(err)?;
        let _ = std::fs::remove_dir_all(&work);
        warnings.push("Neither PowerPoint nor LibreOffice is on this computer, so only the words of each slide were kept. Install LibreOffice (free) for full slides.".into());
        warnings.extend(wp_core::presentations::loss_report(&slides));
        Ok(plan("done", "text_only", Some(id), None, warnings))
    })
    .await
    .map_err(err)?
}

/// One drawn PDF page, sent as raw bytes. Headers: x-token, x-index, x-ext.
#[tauri::command]
fn presentation_page(st: State<'_, AppState>, request: tauri::ipc::Request<'_>) -> Res<()> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err("Expected picture bytes.".into()) };
    let h = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    let token = h("x-token").filter(|t| t.chars().all(|c| c.is_ascii_hexdigit())).ok_or("Missing import token.")?;
    let index: usize = h("x-index").and_then(|i| i.parse().ok()).ok_or("Missing page number.")?;
    let ext = if h("x-ext").as_deref() == Some("png") { "png" } else { "jpg" };
    let dir = st.profile_dir().join("media").join("tmp").join(&token);
    std::fs::create_dir_all(&dir).map_err(err)?;
    std::fs::write(dir.join(format!("page-{index:04}.{ext}")), bytes).map_err(err)
}

/// All pages are in: make the deck and tidy up.
#[tauri::command]
fn presentation_finish(st: State<'_, AppState>, token: String, title: String) -> Res<i64> {
    if !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Bad import token.".into());
    }
    let media = st.profile_dir().join("media");
    let dir = media.join("tmp").join(&token);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(err)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.file_name().map_or(false, |n| n.to_string_lossy().starts_with("page-"))).collect();
    files.sort();
    let pics: Vec<(String, Vec<u8>)> = files
        .iter()
        .filter_map(|f| Some((f.extension()?.to_string_lossy().into_owned(), std::fs::read(f).ok()?)))
        .collect();
    let id = wp_core::presentations::deck_from_pictures(&st.db.lock().unwrap(), &media, &title, &pics).map_err(err)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(id)
}

// ---------- EasyWorship Options ----------

#[tauri::command]
fn output_configs(st: State<'_, AppState>) -> Res<Vec<wp_core::options::OutputConfig>> {
    wp_core::options::outputs(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn output_config(st: State<'_, AppState>, role: String) -> Res<wp_core::options::OutputConfig> {
    wp_core::options::output(&st.db.lock().unwrap(), &role).map_err(err)
}

/// Saves an output's options; an open output is rebuilt with them straight away.
#[tauri::command]
async fn save_output_config(app: AppHandle, config: wp_core::options::OutputConfig) -> Res<()> {
    let st = app.state::<AppState>();
    wp_core::options::save_output(&st.db.lock().unwrap(), &config).map_err(err)?;
    let open = st.outputs.lock().unwrap().get(&config.role).copied();
    if let Some(display) = open {
        build_output(&app, &config.role, display)?;
    }
    let _ = app.emit("output-config-changed", config.role.clone());
    Ok(())
}

#[tauri::command]
fn service_intervals(st: State<'_, AppState>) -> Res<Vec<wp_core::options::Interval>> {
    wp_core::options::intervals(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn save_service_interval(st: State<'_, AppState>, interval: wp_core::options::Interval) -> Res<i64> {
    wp_core::options::save_interval(&st.db.lock().unwrap(), &interval).map_err(err)
}

#[tauri::command]
fn delete_service_interval(st: State<'_, AppState>, id: i64) -> Res<()> {
    wp_core::options::delete_interval(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
fn current_service_interval(st: State<'_, AppState>) -> Option<String> {
    wp_core::options::current_interval(&st.db.lock().unwrap()).ok().flatten()
}

#[tauri::command]
fn usage_report(st: State<'_, AppState>, from: String, to: String) -> Res<Vec<wp_core::options::UsageRow>> {
    wp_core::options::usage_report(&st.db.lock().unwrap(), &from, &to).map_err(err)
}

/// Writes the song usage report as CSV (for a spreadsheet or CCLI's website).
#[tauri::command]
fn save_usage_report(st: State<'_, AppState>, from: String, to: String, path: String) -> Res<usize> {
    let rows = wp_core::options::usage_report(&st.db.lock().unwrap(), &from, &to).map_err(err)?;
    std::fs::write(&path, wp_core::options::usage_csv(&rows)).map_err(err)?;
    Ok(rows.len())
}

// ---------- EasyWorship-style library ----------

#[tauri::command]
fn song_tree(st: State<'_, AppState>) -> Res<wp_core::browse::SongTree> {
    wp_core::browse::song_tree(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn browse_songs(st: State<'_, AppState>, scope: wp_core::browse::Scope, query: String) -> Res<Vec<wp_core::browse::SongRow>> {
    wp_core::browse::browse_songs(&st.db.lock().unwrap(), &scope, &query).map_err(err)
}

#[tauri::command]
fn collection_create(st: State<'_, AppState>, name: String) -> Res<i64> {
    wp_core::browse::create_collection(&st.db.lock().unwrap(), &name).map_err(err)
}

#[tauri::command]
fn collection_rename(st: State<'_, AppState>, id: i64, name: String) -> Res<()> {
    wp_core::browse::rename_collection(&st.db.lock().unwrap(), id, &name).map_err(err)
}

#[tauri::command]
fn collection_delete(st: State<'_, AppState>, id: i64) -> Res<()> {
    wp_core::browse::delete_collection(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
fn collection_add(st: State<'_, AppState>, id: i64, song_ids: Vec<i64>) -> Res<usize> {
    wp_core::browse::add_to_collection(&st.db.lock().unwrap(), id, &song_ids).map_err(err)
}

#[tauri::command]
fn collection_remove(st: State<'_, AppState>, id: i64, song_ids: Vec<i64>) -> Res<()> {
    wp_core::browse::remove_from_collection(&st.db.lock().unwrap(), id, &song_ids).map_err(err)
}

#[tauri::command]
fn browse_chapter(st: State<'_, AppState>, version: String, typed: String) -> Res<wp_core::browse::ChapterView> {
    wp_core::browse::browse_chapter(&st.db.lock().unwrap(), &version, &typed).map_err(err)
}

#[tauri::command]
fn rebuild_search(st: State<'_, AppState>) -> Res<usize> {
    wp_core::browse::rebuild_search(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn reset_song_looks(st: State<'_, AppState>) -> Res<usize> {
    wp_core::browse::reset_song_looks(&st.db.lock().unwrap()).map_err(err)
}

// ---------- NDI (Phase 4 prototype) ----------

#[derive(Serialize)]
struct NdiSource {
    source: String,
    name: String,
    seconds: u64,
    fps: u32,
    captured: u64,
    sent: u64,
    process_ms: f64,
    connections: u64,
    width: u64,
    height: u64,
}

#[derive(Serialize)]
struct NdiStatus {
    runtime: Option<String>,
    problem: Option<String>,
    capture_supported: bool,
    sources: Vec<NdiSource>,
}

#[tauri::command]
fn ndi_status(st: State<'_, AppState>) -> NdiStatus {
    let (runtime, problem) = match st.ndi.runtime() {
        Ok(rt) => (Some(format!("{} ({})", rt.version, rt.path.display())), None),
        Err(e) => (None, Some(e)),
    };
    let sources = st
        .ndi
        .jobs
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, j)| !j.stop.load(std::sync::atomic::Ordering::Relaxed))
        .map(|(k, j)| {
            let s = &j.stats;
            let ld = |a: &std::sync::atomic::AtomicU64| a.load(std::sync::atomic::Ordering::Relaxed);
            NdiSource {
                source: k.clone(),
                name: j.name.clone(),
                seconds: j.started.elapsed().as_secs(),
                fps: j.fps,
                captured: ld(&s.captured),
                sent: ld(&s.sent),
                process_ms: ld(&s.process_us) as f64 / 1000.0,
                connections: ld(&s.connections),
                width: ld(&s.width),
                height: ld(&s.height),
            }
        })
        .collect();
    NdiStatus { runtime, problem, capture_supported: cfg!(windows), sources }
}

/// Sends a moving test pattern (with a see-through half) for `seconds`.
#[tauri::command]
fn ndi_test_pattern(st: State<'_, AppState>, name: String, seconds: Option<u64>) -> Res<()> {
    let (slot, _stats, stop) = st.ndi.start_pump("test", &name, 30)?;
    ndi::run_test_pattern(slot, stop, seconds.unwrap_or(30));
    Ok(())
}

/// Starts an NDI source. "lower_third": an off-screen keyed window; "main": the
/// open main output window as it is.
#[tauri::command]
async fn ndi_start(app: AppHandle, source: String, name: String, fps: Option<u32>) -> Res<()> {
    let st = app.state::<AppState>();
    if !cfg!(windows) {
        return Err("Capturing a window for NDI works on Windows only in this version. The test pattern works everywhere.".into());
    }
    let fps = fps.unwrap_or(30).clamp(10, 60);
    let label = match source.as_str() {
        "lower_third" => {
            // 1920×1080 window drawn on the key colour, kept off the visible desktop
            let w = WebviewWindowBuilder::new(&app, "output-ndi", WebviewUrl::App("index.html#/output/ndi".into()))
                .title("Worship Presenter – NDI lower third")
                .decorations(false)
                .skip_taskbar(true)
                .focused(false)
                .inner_size(1920.0, 1080.0)
                .position(-4000.0, 0.0)
                .build()
                .or_else(|_| app.get_webview_window("output-ndi").ok_or_else(|| "NDI window could not open".to_string()))
                .map_err(err)?;
            let _ = w.set_size(tauri::PhysicalSize::new(1920u32, 1080u32));
            "output-ndi"
        }
        "main" => "output-main",
        other => return Err(format!("Unknown NDI source {other}")),
    };
    let default_name = if source == "main" { "Worship Presenter Main" } else { "Worship Presenter Lower Third" };
    let name = if name.trim().is_empty() { default_name.to_string() } else { name.trim().to_string() };
    #[cfg(windows)]
    {
        let win = app.get_webview_window(label).ok_or_else(|| "Open the main output first (Output setup).".to_string())?;
        let hwnd = win.hwnd().map_err(err)?.0 as *mut std::ffi::c_void;
        let (slot, stats, _stop) = st.ndi.start_pump(&source, &name, fps)?;
        let running = ndi::capture::start(hwnd, slot, stats, source == "lower_third")?;
        if let Some(j) = st.ndi.jobs.lock().unwrap().get_mut(&source) {
            j.capture = Some(running);
        }
    }
    #[cfg(not(windows))]
    let _ = (label, name, fps, &st);
    Ok(())
}

#[tauri::command]
fn ndi_stop(app: AppHandle, st: State<'_, AppState>, source: String) {
    st.ndi.stop(&source);
    if source == "lower_third" {
        if let Some(w) = app.get_webview_window("output-ndi") {
            let _ = w.destroy();
        }
    }
}

// ---------- announcements, repeat counter, lining out ----------

#[tauri::command]
fn list_announcements(st: State<'_, AppState>) -> Res<Vec<wp_core::announcements::DeckSummary>> {
    wp_core::announcements::list(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn get_announcement(st: State<'_, AppState>, id: i64) -> Res<wp_core::announcements::Deck> {
    wp_core::announcements::get(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
fn announcement_templates() -> Vec<wp_core::announcements::Deck> {
    wp_core::announcements::templates()
}

/// Saves a deck. Like songs, never changes what is on the screens.
#[tauri::command]
fn save_announcement(st: State<'_, AppState>, deck: wp_core::announcements::Deck) -> Res<i64> {
    wp_core::announcements::save(&st.db.lock().unwrap(), &deck).map_err(err)
}

#[tauri::command]
fn delete_announcement(st: State<'_, AppState>, id: i64) -> Res<()> {
    if matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Announcement { presentation_id, .. }) if *presentation_id == id) {
        return Err("These announcements are on the screens. Send something else live first.".into());
    }
    wp_core::announcements::delete(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
fn preview_announcement(st: State<'_, AppState>, id: i64) -> Res<Vec<Slide>> {
    let d = wp_core::announcements::get(&st.db.lock().unwrap(), id).map_err(err)?;
    Ok(wp_core::announcements::to_slides(&d))
}

/// Pause or resume a deck that advances by itself.
#[tauri::command]
fn set_auto_paused(app: AppHandle, st: State<'_, AppState>, paused: bool) -> Res<LiveState> {
    let cur = st.live.lock().unwrap().clone();
    let Some(mut a) = cur.auto.clone() else { return Err("Nothing is advancing by itself.".into()) };
    a.paused = paused;
    let mut next = cur.apply(Command::SetAuto { auto: Some(a) });
    next.slide_since_ms = cues::now_ms(); // resuming gives the slide its full time
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// Count a section (usually the chorus) for the band: total times to sing it.
#[tauri::command]
fn set_repeat(app: AppHandle, st: State<'_, AppState>, label: Option<String>, total: u32, done: Option<u32>) -> Res<LiveState> {
    let cur = st.live.lock().unwrap().clone();
    let repeat = label.map(|l| {
        let on_it = st.slides.lock().unwrap().get(cur.slide_idx).map_or(false, |s| s.label == l);
        wp_core::live::Repeat { label: l, total: total.max(1), done: done.unwrap_or(if on_it { 1 } else { 0 }) }
    });
    Ok(apply(&app, &st, Command::SetRepeat { repeat }))
}

/// Lining out: one line per slide, remembering the previous setting to restore.
#[tauri::command]
fn set_lining_out(app: AppHandle, st: State<'_, AppState>, on: bool) -> Res<LiveState> {
    let prev = st.live.lock().unwrap().lines_per_slide;
    let restore = { *st.lines_before_lining.lock().unwrap() };
    if on && !st.live.lock().unwrap().lining_out {
        *st.lines_before_lining.lock().unwrap() = prev;
    }
    let lines = if on { 1 } else { restore.max(1) };
    let mut next = set_layout(app.clone(), st.clone(), Some(lines), None)?;
    next = next.apply(Command::SetLiningOut { on });
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

// ---------- timers and alerts ----------

fn apply(app: &AppHandle, st: &AppState, c: Command) -> LiveState {
    let next = st.live.lock().unwrap().apply(c);
    *st.live.lock().unwrap() = next.clone();
    broadcast(app, st);
    next
}

/// Starts a timer: mode "countdown" with value "5:00", or "to_time" with "09:00".
#[tauri::command]
fn timer_start(app: AppHandle, st: State<'_, AppState>, label: String, mode: String, value: String, on_screens: TimerShow, on_stage: bool) -> Res<LiveState> {
    let now = cues::now_ms();
    let ends = match mode.as_str() {
        "countdown" => cues::parse_length(&value).map(|ms| now + ms),
        _ => cues::parse_clock(&value, now, cues::utc_offset_ms(&st.db.lock().unwrap())),
    }
    .ok_or_else(|| "Use a length like 5:00, or a time like 09:00 or 9:30 am.".to_string())?;
    let timer = cues::Timer { label: label.trim().to_string(), ends_at_ms: ends, paused_left_ms: None, count_over: true, on_screens, on_stage };
    Ok(apply(&app, &st, Command::SetTimer { timer: Some(timer) }))
}

/// pause, resume, stop, or add/subtract milliseconds; or change where it shows.
#[tauri::command]
fn timer_control(app: AppHandle, st: State<'_, AppState>, action: String, ms: Option<i64>, on_screens: Option<TimerShow>, on_stage: Option<bool>) -> Res<LiveState> {
    let now = cues::now_ms();
    let Some(t) = st.live.lock().unwrap().timer.clone() else { return Err("No timer is running.".into()) };
    let t = match action.as_str() {
        "pause" => Some(t.pause(now)),
        "resume" => Some(t.resume(now)),
        "add" => Some(t.add(ms.unwrap_or(60_000))),
        "stop" => None,
        "show" => Some(cues::Timer { on_screens: on_screens.unwrap_or(t.on_screens), on_stage: on_stage.unwrap_or(t.on_stage), ..t }),
        other => return Err(format!("Unknown timer action {other}")),
    };
    Ok(apply(&app, &st, Command::SetTimer { timer: t }))
}

/// Shows an alert. `values` fill the {blanks} in the text.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn alert_show(
    app: AppHandle,
    st: State<'_, AppState>,
    text: String,
    values: Vec<(String, String)>,
    style: cues::AlertStyle,
    position: String,
    seconds: Option<i64>,
    on_screens: bool,
    on_stream: bool,
    on_stage: bool,
) -> Res<LiveState> {
    let text = cues::fill(&text, &values).map_err(err)?;
    if !(on_screens || on_stream || on_stage) {
        return Err("Choose at least one place to show the alert.".into());
    }
    let alert = cues::Alert {
        text,
        style,
        position: if position == "top" { "top".into() } else { "bottom".into() },
        until_ms: seconds.filter(|s| *s > 0).map(|s| cues::now_ms() + s * 1000),
        on_screens,
        on_stream,
        on_stage,
    };
    Ok(apply(&app, &st, Command::SetAlert { alert: Some(alert) }))
}

#[tauri::command]
fn alert_clear(app: AppHandle, st: State<'_, AppState>) -> LiveState {
    apply(&app, &st, Command::SetAlert { alert: None })
}

#[derive(Serialize)]
struct CuePresets {
    timers: Vec<cues::TimerPreset>,
    alerts: Vec<cues::AlertPreset>,
}

#[tauri::command]
fn cue_presets(st: State<'_, AppState>) -> Res<CuePresets> {
    let c = st.db.lock().unwrap();
    Ok(CuePresets { timers: cues::timer_presets(&c).map_err(err)?, alerts: cues::alert_presets(&c).map_err(err)? })
}

#[tauri::command]
fn save_timer_preset(st: State<'_, AppState>, preset: cues::TimerPreset) -> Res<i64> {
    cues::save_timer_preset(&st.db.lock().unwrap(), &preset).map_err(err)
}

#[tauri::command]
fn save_alert_preset(st: State<'_, AppState>, preset: cues::AlertPreset) -> Res<i64> {
    cues::save_alert_preset(&st.db.lock().unwrap(), &preset).map_err(err)
}

#[tauri::command]
fn delete_cue_preset(st: State<'_, AppState>, kind: String, id: i64) -> Res<()> {
    cues::delete_preset(&st.db.lock().unwrap(), &kind, id).map_err(err)
}

// ---------- profiles ----------

#[derive(Serialize)]
struct CurrentProfile {
    slug: String,
    name: String,
}

#[tauri::command]
fn list_profiles(st: State<'_, AppState>) -> Res<Vec<wp_core::profiles::Profile>> {
    wp_core::profiles::list(&st.profiles_root).map_err(err)
}

#[tauri::command]
fn current_profile(st: State<'_, AppState>) -> Res<CurrentProfile> {
    let slug = st.profile.lock().unwrap().0.clone();
    let name = wp_core::profiles::list(&st.profiles_root).map_err(err)?.into_iter().find(|p| p.slug == slug).map(|p| p.name).unwrap_or_else(|| slug.clone());
    Ok(CurrentProfile { slug, name })
}

/// New profile, empty or starting as a copy of the current library.
#[tauri::command]
fn create_profile(st: State<'_, AppState>, name: String, copy_current: bool) -> Res<String> {
    let conn = st.db.lock().unwrap();
    wp_core::profiles::create(&st.profiles_root, &name, copy_current.then_some(&*conn)).map_err(err)
}

#[tauri::command]
fn rename_profile(st: State<'_, AppState>, slug: String, name: String) -> Res<()> {
    wp_core::profiles::rename(&st.profiles_root, &slug, &name).map_err(err)
}

#[tauri::command]
fn delete_profile(st: State<'_, AppState>, slug: String) -> Res<String> {
    wp_core::profiles::delete(&st.profiles_root, &slug).map(|p| p.to_string_lossy().into_owned()).map_err(err)
}

/// Swaps the whole library to another profile. What is live is taken off air
/// (outputs go blank) because it belongs to the old library.
#[tauri::command]
fn switch_profile(app: AppHandle, st: State<'_, AppState>, slug: String) -> Res<()> {
    if st.profile.lock().unwrap().0 == slug {
        return Ok(());
    }
    let dir = st.profiles_root.join(&slug);
    let conn = wp_core::db::open(&dir.join("library.db")).map_err(err)?; // fails before anything changes
    wp_core::profiles::set_active(&st.profiles_root, &slug).map_err(err)?;
    let _ = wp_core::backup::weekly_if_due(&conn, &wp_core::backup::default_dir(&dir));
    let _ = wp_core::schedule::prune_empty_untitled(&conn);
    *st.db.lock().unwrap() = conn;
    *st.profile.lock().unwrap() = (slug, dir.clone());
    *st.item.lock().unwrap() = None;
    st.slides.lock().unwrap().clear();
    *st.caption.lock().unwrap() = None;
    *st.schedule_id.lock().unwrap() = None;
    let old_rev = st.live.lock().unwrap().revision;
    let mut next = LiveState::load(&dir.join("state.json"), false);
    // outputs ignore anything older than what they last drew, so keep counting up
    next.revision = next.revision.max(old_rev) + 1;
    next.item_idx = None;
    *st.live.lock().unwrap() = next;
    *st.theme_rev.lock().unwrap() += 1;
    broadcast(&app, &st);
    let _ = app.emit("profile-changed", ());
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_data_dir()?.join("profiles");
            std::fs::create_dir_all(&root)?;
            let slug = wp_core::profiles::ensure_default(&root)?;
            let dir = root.join(&slug);
            std::fs::create_dir_all(dir.join("media"))?;
            let conn = wp_core::db::open(&dir.join("library.db"))?;
            if !wp_core::db::integrity_ok(&conn)? {
                // Keep the damaged file; the control window shows recovery options.
                let _ = app.emit("library-damaged", dir.join("library.db"));
            }
            let _ = wp_core::schedule::prune_empty_untitled(&conn);
            let project_on_restart = conn
                .query_row("SELECT value FROM setting WHERE key = 'project_on_restart'", [], |r| r.get::<_, String>(0))
                .map(|v| v == "true")
                .unwrap_or(false);
            let live = LiveState::load(&dir.join("state.json"), project_on_restart);
            app.manage(AppState {
                db: Mutex::new(conn),
                live: Mutex::new(live),
                item: Mutex::new(None),
                slides: Mutex::new(Vec::new()),
                caption: Mutex::new(None),
                outputs: Mutex::new(HashMap::new()),
                acks: Mutex::new(HashMap::new()),
                schedule_id: Mutex::new(None),
                profiles_root: root.clone(),
                profile: Mutex::new((slug.clone(), dir.clone())),
                theme_rev: Mutex::new(1),
                held: Mutex::new(Default::default()),
                lines_before_lining: Mutex::new(2),
                ndi: ndi::Ndi::default(),
                remote: Mutex::new(None),
                music: Mutex::new(None),
            });
            // weekly automatic backup, taken quietly at start-up
            if let Some(s) = app.try_state::<AppState>() {
                let _ = wp_core::backup::weekly_if_due(&s.db.lock().unwrap(), &wp_core::backup::default_dir(&dir));
            }
            // Heartbeat: outputs switch to their fallback image if this stops.
            let handle = app.handle().clone();
            // EasyWorship's "Show live screen on startup": open each output marked so, on
            // its saved monitor only; if that monitor is not connected, it stays closed
            let h2 = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1200));
                let Some(st) = h2.try_state::<AppState>() else { return };
                let cfgs = wp_core::options::outputs(&st.db.lock().unwrap()).unwrap_or_default();
                let names: Vec<String> = h2.available_monitors().unwrap_or_default().iter().map(|m| m.name().cloned().unwrap_or_default()).collect();
                for c in cfgs.into_iter().filter(|c| c.show_on_startup) {
                    if let Some(i) = wp_core::options::pick_display(&c, &names) {
                        st.outputs.lock().unwrap().insert(c.role.clone(), Some(i));
                        st.acks.lock().unwrap().insert(c.role.clone(), (Instant::now(), 0));
                        let (h3, role) = (h2.clone(), c.role.clone());
                        let _ = h2.run_on_main_thread(move || {
                            let _ = build_output(&h3, &role, Some(i));
                        });
                    }
                }
            });
            // Watchdog: an output that has not acknowledged for 4 s (crashed or hung
            // renderer, closed by accident) is rebuilt on the same display.
            std::thread::spawn(move || loop {
                let rev = handle.try_state::<AppState>().map(|s| s.live.lock().unwrap().revision).unwrap_or(0);
                let _ = handle.emit("heartbeat", rev);
                if let Some(s) = handle.try_state::<AppState>() {
                    // decks that advance by themselves
                    let cur = s.live.lock().unwrap().clone();
                    let n = s.slides.lock().unwrap().len();
                    if let Some(to) = cur.auto_due(cues::now_ms(), n) {
                        let next = show_at(&s, &cur, to);
                        *s.live.lock().unwrap() = next;
                        broadcast(&handle, &s);
                    }
                    // alerts with a time limit clear themselves
                    let expired = s.live.lock().unwrap().alert.as_ref().map_or(false, |a| a.expired(cues::now_ms()));
                    if expired {
                        apply(&handle, &s, Command::SetAlert { alert: None });
                    }
                    let wanted: Vec<(String, Option<usize>)> = s.outputs.lock().unwrap().iter().map(|(r, d)| (r.clone(), *d)).collect();
                    for (role, display) in wanted {
                        let silent = s.acks.lock().unwrap().get(&role).map(|(t, _)| t.elapsed() > Duration::from_secs(4)).unwrap_or(true);
                        let missing = handle.get_webview_window(&format!("output-{role}")).is_none();
                        if silent || missing {
                            s.acks.lock().unwrap().insert(role.clone(), (Instant::now(), 0));
                            // when it comes back it stays black until the operator resumes it
                            if s.held.lock().unwrap().insert(role.clone()) {
                                broadcast(&handle, &s);
                            }
                            let h = handle.clone();
                            // window work must happen on the main thread
                            let _ = handle.run_on_main_thread(move || {
                                let _ = build_output(&h, &role, display);
                            });
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(1000));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_songs,
            preview_song,
            bible_versions,
            preview_scripture,
            go_live,
            show_slide,
            set_mode,
            set_layout,
            undo_go_live,
            resync,
            list_displays,
            open_output,
            close_output,
            output_ack,
            release_output,
            output_status,
            import_file,
            list_schedules,
            open_schedule,
            new_schedule,
            save_schedule,
            rename_schedule,
            delete_schedule,
            last_schedule,
            get_song,
            preview_song_doc,
            save_song,
            refresh_live_item,
            import_review,
            song_files_review,
            song_files_import,
            relink_media,
            identify_output,
            get_settings,
            set_setting,
            delete_song,
            list_backups,
            backup_now,
            inspect_backup,
            restore_backup,
            backups_folder,
            list_profiles,
            presentation_import,
            presentation_page,
            presentation_finish,
            output_configs,
            output_config,
            save_output_config,
            service_intervals,
            save_service_interval,
            delete_service_interval,
            current_service_interval,
            usage_report,
            save_usage_report,
            song_tree,
            browse_songs,
            collection_create,
            collection_rename,
            collection_delete,
            collection_add,
            collection_remove,
            browse_chapter,
            rebuild_search,
            reset_song_looks,
            music::music_status,
            music::music_set_playlist,
            music::music_play,
            music::music_pause,
            music::music_next,
            music::music_previous,
            music::music_stop,
            music::music_volume,
            music::music_unduck,
            music::music_set_rules,
            remote::remote_status,
            remote::remote_start,
            remote::remote_stop,
            remote::remote_new_pin,
            remote::remote_revoke,
            remote::remote_end_all,
            ndi_status,
            ndi_test_pattern,
            ndi_start,
            ndi_stop,
            list_announcements,
            get_announcement,
            announcement_templates,
            save_announcement,
            delete_announcement,
            preview_announcement,
            set_auto_paused,
            set_repeat,
            set_lining_out,
            timer_start,
            timer_control,
            alert_show,
            alert_clear,
            cue_presets,
            save_timer_preset,
            save_alert_preset,
            delete_cue_preset,
            get_theme,
            export_theme,
            import_theme,
            set_stage_message,
            media_root,
            list_media,
            import_media,
            set_media_meta,
            save_media_thumb,
            rename_media,
            media_used_by,
            delete_media,
            media_control,
            active_themes,
            list_themes,
            save_theme,
            duplicate_theme,
            delete_theme,
            set_default_theme,
            current_profile,
            create_profile,
            rename_profile,
            delete_profile,
            switch_profile,
        ])
        .run(tauri::generate_context!())
        .expect("Worship Presenter failed to start");
}
