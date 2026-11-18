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
mod applog;
mod midi;
mod stream;
mod whisper_worker;
mod deepgram;
mod remote;
mod music;
mod fonts;
mod speech;
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
    /// The live song's key, for the musicians' stage view.
    pub song_key: Option<String>,
    /// The theme the theme rules chose for the live item (after its own and the output's).
    pub rule_theme_id: Option<i64>,
    /// The live song's copyright line (for a theme's copyright layer).
    pub song_copyright: Option<String>,
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
    speech: speech::Speech,
    pulpit: Mutex<std::collections::VecDeque<PulpitNote>>,
    /// The latest live subtitle words and when they came (for the NDI renderer).
    caption_now: Mutex<(String, Option<Instant>)>,
    ndi_inputs: ndi::Inputs,
    midi: midi::Midi,
    stream: stream::Stream,
    /// Output windows being built now (one build at a time per output).
    building: Mutex<std::collections::HashSet<String>>,
    /// When each output window was (re)built: the watchdog waits 15 s for it to load.
    born: Mutex<HashMap<String, Instant>>,
    /// Outputs that have drawn at least once since they were built.
    seen: Mutex<std::collections::HashSet<String>>,
}

/// A point sent from the pulpit phone: kept as a slide in today's "Sermon notes" deck,
/// waiting for the operator to preview or show it.
#[derive(Serialize, Clone)]
pub struct PulpitNote {
    id: u64,
    device: String,
    text: String,
    deck_id: i64,
    deck_title: String,
    slide_idx: usize,
}

pub(crate) fn add_pulpit_note(st: &AppState, device: &str, text: &str) -> Res<PulpitNote> {
    let conn = st.db.lock().unwrap();
    let day: String = conn.query_row("SELECT strftime('%d %m %Y', 'now', 'localtime')", [], |r| r.get(0)).map_err(err)?;
    let title = format!("Sermon notes {day}");
    let existing = wp_core::announcements::list(&conn).map_err(err)?.into_iter().find(|d| d.title == title);
    let mut deck = match existing {
        Some(d) => wp_core::announcements::get(&conn, d.id).map_err(err)?,
        None => wp_core::announcements::Deck { id: None, title: title.clone(), looping: false, seconds: 0, slides: vec![] },
    };
    deck.slides.push(wp_core::announcements::DeckSlide { kind: "text".into(), title: String::new(), body: text.to_string(), image: None, seconds: None });
    let slide_idx = deck.slides.len() - 1;
    let deck_id = wp_core::announcements::save(&conn, &deck).map_err(err)?;
    drop(conn);
    let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let n = PulpitNote { id, device: device.to_string(), text: text.to_string(), deck_id, deck_title: title, slide_idx };
    let mut q = st.pulpit.lock().unwrap();
    q.push_front(n.clone());
    q.truncate(30);
    Ok(n)
}

#[tauri::command(async)]
fn pulpit_notes(st: State<'_, AppState>) -> Vec<PulpitNote> {
    st.pulpit.lock().unwrap().iter().cloned().collect()
}

#[tauri::command(async)]
fn pulpit_dismiss(st: State<'_, AppState>, id: Option<u64>) {
    let mut q = st.pulpit.lock().unwrap();
    match id {
        Some(i) => q.retain(|n| n.id != i),
        None => q.clear(),
    }
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
    // every failure the app reports is also written to the log
    let text = e.to_string();
    applog::write("FAIL", &text);
    text
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
            vec![Slide { label: "Media".into(), occurrence: 0, first_key: 0, last_key: 0, primary: vec![], second: vec![], image: None, chords: vec![] }],
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
                    chords: vec![],
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
        song_key: None,
        rule_theme_id: None,
        song_copyright: None,
    };
    let mut payload = payload;
    payload.rule_theme_id = payload.item.as_ref().and_then(|it| rule_theme(&s.db.lock().unwrap(), it));
    if let Some(ProgramItem::Song { song_id, .. }) = &payload.item {
        let row = s.db.lock().unwrap().query_row("SELECT song_key, copyright FROM song WHERE id = ?1", [song_id], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?))).ok();
        if let Some((key, copyright)) = row {
            payload.song_key = key;
            payload.song_copyright = copyright.filter(|c| !c.trim().is_empty());
        }
    }
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

#[tauri::command(async)]
fn search_songs(st: State<'_, AppState>, query: String) -> Res<Vec<library::SongHit>> {
    library::search_songs(&st.db.lock().unwrap(), &query, 60).map_err(err)
}

#[tauri::command(async)]
fn preview_song(st: State<'_, AppState>, song_id: i64, title: String) -> Res<Vec<Slide>> {
    let live = st.live.lock().unwrap().clone();
    build_slides(&st.db.lock().unwrap(), &ProgramItem::Song { song_id, title, theme_id: None, lyrics: None }, &live).map(|r| r.0)
}

#[tauri::command(async)]
fn bible_versions(st: State<'_, AppState>) -> Res<Vec<library::Version>> {
    library::versions(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
fn preview_scripture(st: State<'_, AppState>, reference: String, versions: Vec<String>) -> Res<(Vec<Slide>, Option<String>)> {
    let live = st.live.lock().unwrap().clone();
    build_slides(&st.db.lock().unwrap(), &ProgramItem::Scripture { reference, versions, theme_id: None }, &live)
}

/// Sends an item to air at a slide.
#[tauri::command(async)]
fn go_live(app: AppHandle, st: State<'_, AppState>, item: ProgramItem, slide_idx: usize) -> Res<LiveState> {
    // from Blank, Logo, Clear or a restart, Go live shows the new item (as EasyWorship does)
    let live0 = st.live.lock().unwrap().clone();
    let (slides, caption) = build_slides(&st.db.lock().unwrap(), &item, &live0)?;
    let idx = slide_idx.min(slides.len().saturating_sub(1));
    let pos = slides.get(idx).map(splitter::position_of);
    let mut next = live0.apply(Command::GoLive { item_idx: 0, slide_idx: idx, position: pos });
    next.slide_since_ms = cues::now_ms();
    // Go live is the operator's deliberate "show this": outputs held black after being
    // rebuilt come back with it
    st.held.lock().unwrap().clear();
    // a different song starts in its written key on the stage view
    let same = st.item.lock().unwrap().as_ref().map_or(false, |cur| serde_json::to_value(cur).ok() == serde_json::to_value(&item).ok());
    if !same {
        next.transpose = 0;
        // the operator's quick text change was for the previous item only
        next.text_scale = 1.0;
        next.text_shift = 0.0;
    }
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
#[tauri::command(async)]
fn show_slide(app: AppHandle, st: State<'_, AppState>, slide_idx: Option<usize>, delta: Option<i64>) -> Res<LiveState> {
    let n = st.slides.lock().unwrap().len();
    if n == 0 {
        return Err("Nothing is live. Send an item to Live first.".into());
    }
    // behind Blank or Logo the slide still changes; the screens show it on Resume
    let cur = st.live.lock().unwrap().clone();
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

#[tauri::command(async)]
fn set_mode(app: AppHandle, st: State<'_, AppState>, mode: Mode) -> Res<LiveState> {
    let next = st.live.lock().unwrap().apply(Command::SetMode { mode });
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// Changes lines per slide or language mode and re-splits the live item,
/// keeping the same words on screen.
#[tauri::command(async)]
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

#[tauri::command(async)]
fn undo_go_live(app: AppHandle, st: State<'_, AppState>) -> Res<LiveState> {
    let next = st.live.lock().unwrap().apply(Command::UndoGoLive);
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

/// Re-sends the current state; used by an output window when it (re)connects.
#[tauri::command(async)]
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

/// Async on purpose: asking Windows for the monitors from the main thread can freeze the app.
#[tauri::command]
async fn list_displays(app: AppHandle) -> Res<Vec<Display>> {
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
    // read the settings in their own statements: inside an `if let`, the database would
    // stay locked for the whole block (and saving below would wait for it forever)
    let ndi = wp_core::options::output(&st.db.lock().unwrap(), &role).ok().and_then(|c| c.ndi);
    if let Some(f) = ndi {
        return start_ndi_output(&app, &role, f);
    }
    // remember the monitor by index and name, so it is found again next time
    let cfg = wp_core::options::output(&st.db.lock().unwrap(), &role);
    if let Ok(mut c) = cfg {
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
    let st = app.state::<AppState>();
    // one build at a time per output: startup, Output setup, Identify and the watchdog
    // could otherwise race and collide on the window's name
    if !st.building.lock().unwrap().insert(role.to_string()) {
        return Ok(());
    }
    let r = build_output_now(app, role, display_index);
    st.born.lock().unwrap().insert(role.to_string(), Instant::now());
    st.seen.lock().unwrap().remove(role);
    st.acks.lock().unwrap().insert(role.to_string(), (Instant::now(), 0));
    st.building.lock().unwrap().remove(role);
    r
}

fn build_output_now(app: &AppHandle, role: &str, display_index: Option<usize>) -> Res<()> {
    let label = format!("output-{role}");
    // the output's saved options: custom position and size, alpha channel
    let cfg = app.try_state::<AppState>().and_then(|st| wp_core::options::output(&st.db.lock().unwrap(), role).ok());
    let alpha_wanted = cfg.as_ref().map_or(false, |c| c.alpha);
    if let Some(w) = app.get_webview_window(&label) {
        // a window that is still there is moved and redrawn rather than replaced
        // (transparency is fixed when a window is made, so changing it needs a new one)
        let made_alpha = OUTPUT_ALPHA.lock().unwrap().get(role).copied().unwrap_or(false);
        if made_alpha == alpha_wanted && w.is_visible().is_ok() {
            if let Some(m) = display_index.and_then(|i| app.available_monitors().ok()?.into_iter().nth(i)) {
                let _ = w.set_fullscreen(false);
                let _ = w.set_position(*m.position());
                let _ = w.set_size(*m.size());
                match cfg.as_ref().and_then(|c| c.custom) {
                    Some(r) => {
                        let p = m.position();
                        let _ = w.set_position(tauri::PhysicalPosition::new(p.x + r.x, p.y + r.y));
                        let _ = w.set_size(tauri::PhysicalSize::new(r.w, r.h));
                    }
                    None => {
                        let _ = w.set_fullscreen(true);
                    }
                }
            }
            let _ = w.eval("location.reload()");
            return Ok(());
        }
        let _ = w.destroy();
        // destroying finishes a moment later on Windows; wait until the name is free
        let t = Instant::now();
        while app.get_webview_window(&label).is_some() && t.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(50));
        }
        if app.get_webview_window(&label).is_some() {
            return Err("The previous output window did not close. Try again in a moment.".into());
        }
    }
    OUTPUT_ALPHA.lock().unwrap().insert(role.to_string(), alpha_wanted);
    let alpha = cfg.as_ref().map_or(false, |c| c.alpha);
    let custom = cfg.as_ref().and_then(|c| c.custom);
    let monitors = app.available_monitors().map_err(err)?;
    let mut b = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(format!("index.html#/output/{role}").into()))
            .additional_browser_args(BROWSER_ARGS)
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

/// Whether each output window was made transparent (fixed when the window is made).
static OUTPUT_ALPHA: std::sync::LazyLock<Mutex<HashMap<String, bool>>> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// The operator's Resume for one output that came back after being lost.
#[tauri::command(async)]
fn release_output(app: AppHandle, st: State<'_, AppState>, role: String) {
    st.held.lock().unwrap().remove(&role);
    broadcast(&app, &st);
}

/// Called by each output on every state and heartbeat it renders.
#[tauri::command(async)]
fn output_ack(st: State<'_, AppState>, role: String, revision: u64) {
    st.seen.lock().unwrap().insert(role.clone());
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

#[tauri::command(async)]
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

#[tauri::command(async)]
fn close_output(app: AppHandle, st: State<'_, AppState>, role: String) {
    st.outputs.lock().unwrap().remove(&role);
    st.held.lock().unwrap().remove(&role);
    st.acks.lock().unwrap().remove(&role);
    if let Some(w) = app.get_webview_window(&format!("output-{role}")) {
        let _ = w.destroy();
    }
}

// ---------- service orders ----------

#[tauri::command(async)]
fn list_schedules(st: State<'_, AppState>) -> Res<Vec<wp_core::schedule::Summary>> {
    wp_core::schedule::list(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
fn open_schedule(st: State<'_, AppState>, id: i64) -> Res<wp_core::schedule::Schedule> {
    let s = wp_core::schedule::load(&st.db.lock().unwrap(), id).map_err(err)?;
    *st.schedule_id.lock().unwrap() = Some(id);
    let _ = st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('last_schedule', ?1)", [id.to_string()]);
    Ok(s)
}

/// Creates a service order, optionally filled from a template, and opens it.
#[tauri::command(async)]
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
#[tauri::command(async)]
fn save_schedule(st: State<'_, AppState>, id: i64, items: Vec<wp_core::schedule::Item>) -> Res<()> {
    wp_core::schedule::save_items(&st.db.lock().unwrap(), id, &items).map_err(err)
}

#[tauri::command(async)]
fn rename_schedule(st: State<'_, AppState>, id: i64, name: String, service_date: Option<String>, template: bool) -> Res<()> {
    wp_core::schedule::rename(&st.db.lock().unwrap(), id, &name, service_date.as_deref(), template).map_err(err)
}

#[tauri::command(async)]
fn delete_schedule(st: State<'_, AppState>, id: i64) -> Res<()> {
    let mut cur = st.schedule_id.lock().unwrap();
    if *cur == Some(id) {
        *cur = None;
    }
    wp_core::schedule::delete(&st.db.lock().unwrap(), id).map_err(err)
}

/// The schedule that was open when the app last closed (power-cut recovery).
#[tauri::command(async)]
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
        guard(|| wp_core::import::review::review(&conn, &kind, std::path::Path::new(&path), code.as_deref()))?.map_err(err)
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
        let conn = st.db.lock().unwrap();
        guard(|| wp_core::import::songfiles::review(&conn, &paths))?.map_err(err)
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
        guard(|| wp_core::import::songfiles::import(&conn, &paths, skip.as_ref()))?.map_err(err)
    })
    .await
    .map_err(err)?
}

/// Imports a library file chosen in the Import screen and returns the counts.
/// Runs a file parser so that a damaged or hostile file that makes it panic is
/// reported, not a crash. Locks must be taken outside `f` (so they are not poisoned).
fn guard<T>(f: impl FnOnce() -> T) -> Res<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|_| "The file could not be read: it is damaged or not what it seems to be. Nothing was changed.".to_string())
}

/// Runs off the main thread so outputs keep drawing during a long import.
#[tauri::command]
async fn import_file(app: AppHandle, kind: String, path: String, code: Option<String>, skip: Option<Vec<String>>) -> Res<wp_core::import::Report> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let conn = st.db.lock().unwrap();
        wp_core::backup::create(&conn, &st.backups(), "before-import").map_err(err)?;
        let p = PathBuf::from(&path);
        if !["ewb", "bibleshow", "ew-songs"].contains(&kind.as_str()) {
            return Err(format!("Unknown import type {kind}"));
        }
        let r = guard(|| match kind.as_str() {
            "ewb" => wp_core::import::ewb::import(&conn, &p, code.as_deref()),
            "bibleshow" => wp_core::import::bibleshow::import_file(&conn, &p, code.as_deref()),
            "ew-songs" => {
                // `path` is the EasyWorship folder holding Songs.db and SongWords.db
                let name = p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| "EasyWorship songs".into());
                let skip: Option<std::collections::HashSet<String>> = skip.map(|v| v.iter().map(|t| t.trim().to_lowercase()).collect());
                wp_core::import::ew_songs::import_filtered(&conn, &p.join("Songs.db"), &p.join("SongWords.db"), &name, skip.as_ref())
            }
            _ => unreachable!(),
        })?;
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

#[tauri::command(async)]
fn get_song(st: State<'_, AppState>, id: i64) -> Res<wp_core::songs::SongDoc> {
    wp_core::songs::load(&st.db.lock().unwrap(), id).map_err(err)
}

/// Slides for the editor's live preview, without saving.
#[tauri::command(async)]
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

#[tauri::command(async)]
fn save_song(st: State<'_, AppState>, doc: wp_core::songs::SongDoc) -> Res<SongSaved> {
    let id = wp_core::songs::save(&st.db.lock().unwrap(), &doc).map_err(err)?;
    let live = matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Song { song_id, lyrics: None, .. }) if *song_id == id);
    Ok(SongSaved { id, live })
}

/// The operator's explicit "Update the screens" after editing the live song.
#[tauri::command(async)]
fn refresh_live_item(app: AppHandle, st: State<'_, AppState>, title: Option<String>) -> Res<()> {
    if let (Some(t), Some(ProgramItem::Song { title, .. })) = (title, st.item.lock().unwrap().as_mut()) {
        *title = t;
    }
    refresh_live(&app, &st);
    Ok(())
}

#[tauri::command(async)]
fn delete_song(st: State<'_, AppState>, id: i64) -> Res<()> {
    let live_is_this = matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Song { song_id, .. }) if *song_id == id);
    if live_is_this {
        return Err("This song is on the screens. Send something else live first, then delete it.".into());
    }
    wp_core::songs::delete(&st.db.lock().unwrap(), id).map_err(err)
}

// ---------- backups ----------

#[tauri::command(async)]
fn list_backups(st: State<'_, AppState>) -> Res<Vec<wp_core::backup::BackupInfo>> {
    wp_core::backup::list(&st.backups()).map_err(err)
}

#[tauri::command(async)]
fn backup_now(st: State<'_, AppState>) -> Res<wp_core::backup::BackupInfo> {
    wp_core::backup::create(&st.db.lock().unwrap(), &st.backups(), "manual").map_err(err)
}

/// What a backup file holds, shown before the operator confirms a restore.
#[tauri::command(async)]
fn inspect_backup(path: String) -> Res<wp_core::backup::Contents> {
    wp_core::backup::verify(std::path::Path::new(&path)).map_err(err)
}

/// Restores a backup. Returns the safety copy of the library it replaced.
#[tauri::command(async)]
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
#[tauri::command(async)]
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
#[tauri::command(async)]
fn active_themes(st: State<'_, AppState>) -> Res<ActiveThemes> {
    let c = st.db.lock().unwrap();
    let k = |kind: &str| wp_core::themes::for_kind(&c, kind).map_err(err);
    // read the counter on its own: holding it while the database is used could
    // deadlock against code that holds the database and then reads the counter
    let rev = *st.theme_rev.lock().unwrap();
    Ok(ActiveThemes { rev, song: k("song")?, scripture: k("scripture")?, lower_third: k("lower_third")?, announcement: k("announcement")? })
}

#[tauri::command(async)]
fn list_themes(st: State<'_, AppState>) -> Res<Vec<wp_core::themes::ThemeRow>> {
    wp_core::themes::list(&st.db.lock().unwrap()).map_err(err)
}

fn themes_changed(app: &AppHandle, st: &AppState) {
    *st.theme_rev.lock().unwrap() += 1;
    broadcast(app, st);
}

/// Saves a theme (new when id is None); outputs pick up the change at once.
// ---------- sync between church computers ----------

fn setting_of(st: &AppState, key: &str) -> Option<String> {
    st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get::<_, String>(0)).ok().filter(|v| !v.is_empty())
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "this computer".into())
}

fn profile_name(st: &AppState) -> String {
    st.profile.lock().unwrap().0.clone()
}

/// Shares this computer's library into the shared folder.
fn sync_share(st: &AppState) -> Res<usize> {
    let folder = setting_of(st, "sync_folder").ok_or("Choose the shared folder first.")?;
    let n = wp_core::sync::share(&st.db.lock().unwrap(), &st.profile_dir().join("media"), std::path::Path::new(&folder), &profile_name(st), &computer_name()).map_err(err)?;
    let now = cues::now_ms().to_string();
    st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('sync_shared_ms', ?1)", [&now]).map_err(err)?;
    Ok(n)
}

/// Takes the shared library (a backup first; this computer's settings and outputs kept).
fn sync_take(app: &AppHandle, st: &AppState) -> Res<String> {
    let folder = setting_of(st, "sync_folder").ok_or("Choose the shared folder first.")?;
    let db_path = st.profile_dir().join("library.db");
    let mut g = st.db.lock().unwrap();
    wp_core::backup::create(&g, &st.backups(), "before-sync").map_err(err)?;
    let keep = wp_core::sync::local_rows(&g).map_err(err)?;
    // close the library file before it is replaced, then open the new one
    *g = rusqlite::Connection::open_in_memory().map_err(err)?;
    let taken = wp_core::sync::take(std::path::Path::new(&folder), &profile_name(st), &st.profile_dir().join("media"), &db_path);
    *g = wp_core::db::open(&db_path).map_err(err)?;
    let m = taken.map_err(err)?;
    wp_core::sync::restore_local_rows(&g, &keep).map_err(err)?;
    g.execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('sync_taken_ms', ?1)", [m.shared_ms.to_string()]).map_err(err)?;
    drop(g);
    let _ = app.emit("library-updated", &m.computer);
    Ok(m.computer)
}

#[derive(Serialize)]
struct SyncStatus {
    folder: Option<String>,
    role: Option<String>,
    shared_ms: Option<i64>,
    taken_ms: Option<i64>,
    /// An update waiting from another computer: (its name, when).
    waiting: Option<(String, i64)>,
    computer: String,
}

#[tauri::command(async)]
fn sync_status(st: State<'_, AppState>) -> SyncStatus {
    let folder = setting_of(&st, "sync_folder");
    let taken = setting_of(&st, "sync_taken_ms").and_then(|v| v.parse().ok());
    let waiting = folder.as_deref().and_then(|f| wp_core::sync::waiting(std::path::Path::new(f), &profile_name(&st), taken.unwrap_or(0))).filter(|m| m.computer != computer_name()).map(|m| (m.computer, m.shared_ms));
    SyncStatus { folder, role: setting_of(&st, "sync_role"), shared_ms: setting_of(&st, "sync_shared_ms").and_then(|v| v.parse().ok()), taken_ms: taken, waiting, computer: computer_name() }
}

#[tauri::command(async)]
fn sync_set(st: State<'_, AppState>, folder: Option<String>, role: Option<String>) -> Res<()> {
    let conn = st.db.lock().unwrap();
    for (k, v) in [("sync_folder", folder), ("sync_role", role.filter(|r| r == "share" || r == "follow"))] {
        match v {
            Some(v) => conn.execute("INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2)", [k, &v]).map_err(err)?,
            None => conn.execute("DELETE FROM setting WHERE key = ?1", [k]).map_err(err)?,
        };
    }
    Ok(())
}

#[tauri::command]
async fn sync_share_now(app: AppHandle) -> Res<usize> {
    tauri::async_runtime::spawn_blocking(move || sync_share(&app.state::<AppState>())).await.map_err(err)?
}

#[tauri::command]
async fn sync_take_now(app: AppHandle) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        if st.item.lock().unwrap().is_some() && st.live.lock().unwrap().mode == Mode::Live {
            return Err("Something is live: update after the service (or press Blank first).".into());
        }
        sync_take(&app, &st)
    })
    .await
    .map_err(err)?
}

/// Set when this build carries the update key (the updater is switched on); without it,
/// asking the updater crashes, so the update commands check this first.
static UPDATER_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Serialize)]
struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

/// Is a newer Worship Presenter published? (None: up to date.)
#[tauri::command]
async fn update_check(app: AppHandle) -> Res<Option<UpdateInfo>> {
    use tauri_plugin_updater::UpdaterExt;
    if !UPDATER_ON.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Automatic updates are not set up in this build.".into());
    }
    let updater = app.updater().map_err(|_| "Automatic updates are not set up in this build.".to_string())?;
    let u = updater.check().await.map_err(|e| format!("Could not check for updates ({e})."))?;
    Ok(u.map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone() }))
}

/// Downloads the update, checks its signature, installs it and restarts.
#[tauri::command]
async fn update_install(app: AppHandle, st: State<'_, AppState>) -> Res<()> {
    use tauri_plugin_updater::UpdaterExt;
    if !UPDATER_ON.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Automatic updates are not set up in this build.".into());
    }
    if st.item.lock().unwrap().is_some() && st.live.lock().unwrap().mode == Mode::Live {
        return Err("Something is live: install the update after the service.".into());
    }
    let u = app.updater().map_err(err)?.check().await.map_err(err)?.ok_or("Already up to date.")?;
    applog::write("UPDATE", &format!("installing {}", u.version));
    u.download_and_install(|_, _| {}, || {}).await.map_err(|e| format!("The update could not be installed ({e})."))?;
    app.restart();
}

/// An error from a window (the interface), for the log.
#[tauri::command(async)]
fn log_error(window: String, text: String) {
    applog::write("ERROR", &format!("[{window}] {text}"));
}

/// Opens the folder with the log files.
#[tauri::command(async)]
fn open_logs() -> Res<()> {
    let dir = applog::dir().ok_or("No log folder yet.")?;
    let r = if cfg!(windows) {
        std::process::Command::new("explorer").arg(&dir).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&dir).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&dir).spawn()
    };
    r.map(drop).map_err(|e| format!("The folder could not be opened ({e})."))
}

/// A report to paste into a message: version, system, outputs and the recent log.
#[tauri::command(async)]
fn diagnostic_report(app: AppHandle, st: State<'_, AppState>) -> String {
    let outputs: Vec<String> = wp_core::options::outputs(&st.db.lock().unwrap())
        .unwrap_or_default()
        .iter()
        .map(|c| format!("{}: {}", c.role, if c.ndi.is_some() { "NDI stream".to_string() } else { c.display_name.clone().unwrap_or_else(|| "no monitor".into()) }))
        .collect();
    let monitors = app.available_monitors().map(|m| m.len()).unwrap_or(0);
    format!(
        "Worship Presenter {}\nSystem: {} {}\nMonitors: {monitors}\nOutputs: {}\nNDI: {}\n\nRecent log:\n{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        outputs.join("; "),
        st.ndi.jobs.lock().unwrap().keys().cloned().collect::<Vec<_>>().join(", "),
        applog::recent(150)
    )
}

// ---------- finding lyrics online (Hymnary.org, LRCLIB) ----------

fn web_text(url: &str) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(12)).user_agent(concat!("Worship Presenter/", env!("CARGO_PKG_VERSION"), " (github.com/charleswesleybotchwey/Worship-Presenter)")).build();
    let r = agent.get(url).call().map_err(|e| format!("No answer from the website ({e}). Check the internet connection."))?;
    r.into_string().map_err(err)
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Songs matching a title (and artist) on Hymnary.org and LRCLIB, hymns first.
#[tauri::command]
async fn lyrics_search(title: String, artist: String) -> Res<Vec<wp_core::lyrics_online::Found>> {
    tauri::async_runtime::spawn_blocking(move || {
        if title.trim().is_empty() {
            return Err("Type the song's title.".into());
        }
        let hymns = web_text(&format!("https://hymnary.org/search?qu=%20tuneTitle%3A{}%20media%3Atext%20in%3Atexts&export=csv", enc(title.trim())))
            .map(|csv| wp_core::lyrics_online::hymnary_results(&csv))
            .unwrap_or_default();
        let mut url = format!("https://lrclib.net/api/search?track_name={}", enc(title.trim()));
        if !artist.trim().is_empty() {
            url.push_str(&format!("&artist_name={}", enc(artist.trim())));
        }
        let songs = web_text(&url).map(|j| wp_core::lyrics_online::lrclib_results(&j));
        if hymns.is_empty() {
            if let Err(e) = &songs {
                return Err(e.clone());
            }
        }
        let mut all = hymns;
        all.extend(songs.unwrap_or_default());
        Ok(all)
    })
    .await
    .map_err(err)?
}

/// The words of one result (Hymnary gives them on the hymn's page).
#[tauri::command]
async fn lyrics_words(source: String, key: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err("Unknown song.".into());
        }
        match source.as_str() {
            "Hymnary" => {
                let html = web_text(&format!("https://hymnary.org/text/{key}"))?;
                wp_core::lyrics_online::hymnary_words(&html).ok_or_else(|| "Hymnary does not show the full words of this hymn (it may still be under copyright).".into())
            }
            "LRCLIB" => {
                let json = web_text(&format!("https://lrclib.net/api/get/{key}"))?;
                let v: serde_json::Value = serde_json::from_str(&json).map_err(err)?;
                v["plainLyrics"].as_str().map(str::to_string).ok_or_else(|| "No words for this song.".into())
            }
            _ => Err("Unknown source.".into()),
        }
    })
    .await
    .map_err(err)?
}

/// Adds the words as a new song (verses and chorus marked), ready to check in the editor.
#[tauri::command(async)]
fn lyrics_add(st: State<'_, AppState>, title: String, author: String, words: String, source: String) -> Res<i64> {
    let body = wp_core::lyrics_online::to_song_body(&words);
    if body.is_empty() {
        return Err("There are no words to add.".into());
    }
    let doc = wp_core::songs::SongDoc {
        id: None,
        title: title.trim().to_string(),
        author: Some(author.trim().to_string()).filter(|a| !a.is_empty()),
        copyright: Some(format!("Words from {source}")),
        ccli: None,
        numbers: vec![],
        lyrics: vec![wp_core::songs::Lyric { lang: "en".into(), body }],
        sequence: String::new(),
        lines_per_slide: None,
        key: None,
    };
    wp_core::songs::save(&st.db.lock().unwrap(), &doc).map_err(err)
}

/// eBible.org's freely shareable Bibles (needs internet).
#[tauri::command]
async fn bible_catalogue() -> Res<Vec<wp_core::import::vpl::Offer>> {
    tauri::async_runtime::spawn_blocking(|| {
        let bytes = speech::download("https://ebible.org/Scriptures/translations.csv", &|_| {}, "the list of Bibles")?;
        let list = wp_core::import::vpl::catalogue(&String::from_utf8_lossy(&bytes));
        if list.is_empty() {
            return Err("The list of Bibles could not be read. Try again later.".into());
        }
        Ok(list)
    })
    .await
    .map_err(err)?
}

/// Downloads one Bible from eBible.org and adds it to the library (a backup first).
#[tauri::command]
async fn bible_download(app: AppHandle, id: String, code: String, title: String, lang: String) -> Res<usize> {
    tauri::async_runtime::spawn_blocking(move || {
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || id.is_empty() {
            return Err("Unknown Bible.".to_string());
        }
        let st = app.state::<AppState>();
        let progress = |m: String| *st.speech.install.lock().unwrap() = Some(m);
        let result = (|| -> Result<usize, String> {
            let bytes = speech::download(&format!("https://ebible.org/Scriptures/{id}_vpl.zip"), &progress, &title)?;
            let text = wp_core::import::vpl::text_of(&bytes).map_err(err)?;
            let conn = st.db.lock().unwrap();
            wp_core::backup::create(&conn, &st.backups(), "before-import").map_err(err)?;
            let r = wp_core::import::vpl::import(&conn, &text, &code, &title, &lang, &format!("{id}_vpl.zip")).map_err(err)?;
            Ok(r.read)
        })();
        *st.speech.install.lock().unwrap() = None;
        result
    })
    .await
    .map_err(err)?
}

/// "My collections" for scripture readings and media.
#[tauri::command(async)]
fn item_collections(st: State<'_, AppState>, kind: String) -> Res<Vec<wp_core::item_collections::ItemCollection>> {
    wp_core::item_collections::list(&st.db.lock().unwrap(), &kind).map_err(err)
}
#[tauri::command(async)]
fn item_collection_create(st: State<'_, AppState>, kind: String, name: String) -> Res<i64> {
    wp_core::item_collections::create(&st.db.lock().unwrap(), &kind, &name).map_err(err)
}
#[tauri::command(async)]
fn item_collection_rename(st: State<'_, AppState>, id: i64, name: String) -> Res<()> {
    wp_core::item_collections::rename(&st.db.lock().unwrap(), id, &name).map_err(err)
}
#[tauri::command(async)]
fn item_collection_delete(st: State<'_, AppState>, id: i64) -> Res<()> {
    wp_core::item_collections::delete(&st.db.lock().unwrap(), id).map_err(err)
}
#[tauri::command(async)]
fn item_collection_add(st: State<'_, AppState>, id: i64, item: serde_json::Value) -> Res<()> {
    wp_core::item_collections::add(&st.db.lock().unwrap(), id, &item).map_err(err)
}
#[tauri::command(async)]
fn item_collection_remove(st: State<'_, AppState>, id: i64, item: serde_json::Value) -> Res<()> {
    wp_core::item_collections::remove(&st.db.lock().unwrap(), id, &item).map_err(err)
}
#[tauri::command(async)]
fn item_collection_items(st: State<'_, AppState>, id: i64) -> Res<Vec<serde_json::Value>> {
    wp_core::item_collections::items(&st.db.lock().unwrap(), id).map_err(err)
}

/// A sermon outline turned into slides (not saved: the operator checks them first).
#[tauri::command(async)]
fn outline_deck(st: State<'_, AppState>, text: String, version: String) -> Res<wp_core::announcements::Deck> {
    wp_core::outline::to_deck(&st.db.lock().unwrap(), &text, &version).map_err(err)
}

/// The quick box's hymn numbers ("555", "MHB 555").
#[tauri::command(async)]
fn quick_song(st: State<'_, AppState>, typed: String) -> Res<Vec<wp_core::library::SongHit>> {
    wp_core::library::quick_song(&st.db.lock().unwrap(), &typed).map_err(err)
}

/// The starter gallery of ready-made themes.
#[tauri::command(async)]
fn theme_gallery() -> Vec<serde_json::Value> {
    wp_core::themes::gallery()
}

#[tauri::command(async)]
fn save_theme(app: AppHandle, st: State<'_, AppState>, id: Option<i64>, theme: serde_json::Value) -> Res<i64> {
    let id = wp_core::themes::save(&st.db.lock().unwrap(), id, &theme).map_err(err)?;
    themes_changed(&app, &st);
    Ok(id)
}

#[tauri::command(async)]
fn duplicate_theme(st: State<'_, AppState>, id: i64) -> Res<i64> {
    wp_core::themes::duplicate(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command(async)]
fn delete_theme(app: AppHandle, st: State<'_, AppState>, id: i64) -> Res<()> {
    wp_core::themes::delete(&st.db.lock().unwrap(), id).map_err(err)?;
    themes_changed(&app, &st);
    Ok(())
}

#[tauri::command(async)]
fn set_default_theme(app: AppHandle, st: State<'_, AppState>, kind: String, id: i64) -> Res<()> {
    wp_core::themes::set_default(&st.db.lock().unwrap(), &kind, id).map_err(err)?;
    themes_changed(&app, &st);
    Ok(())
}

// ---------- media ----------

#[tauri::command(async)]
fn media_root(st: State<'_, AppState>) -> String {
    st.profile_dir().join("media").to_string_lossy().into_owned()
}

#[tauri::command(async)]
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

#[tauri::command(async)]
fn relink_media(st: State<'_, AppState>, id: i64, path: String) -> Res<wp_core::media::MediaItem> {
    wp_core::media::relink(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id, std::path::Path::new(&path)).map_err(err)
}

/// EasyWorship's "Identify": a large number and the monitor's name on every connected
/// monitor for 4 seconds, so the operator knows which is Monitor 1, 2, 3. Built off the
/// main thread; the windows close themselves.
#[tauri::command]
async fn identify_monitors(app: AppHandle) -> Res<usize> {
    let monitors = app.available_monitors().map_err(err)?;
    let mut labels = vec![];
    for (i, m) in monitors.iter().enumerate() {
        let label = format!("identify-{i}");
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.destroy();
        }
        let name: String = m.name().cloned().unwrap_or_default().chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
        let url = format!("index.html#/identify/{}/{}x{}/{}", i + 1, m.size().width, m.size().height, name.replace(' ', "%20"));
        let scale = m.scale_factor();
        let pos = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        let w = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
            .additional_browser_args(BROWSER_ARGS)
            .title("Identify")
            .decorations(false)
            .skip_taskbar(true)
            .focused(false)
            .always_on_top(true)
            .position(pos.x, pos.y)
            .inner_size(size.width, size.height)
            .build()
            .map_err(err)?;
        let _ = w.set_position(tauri::PhysicalPosition::new(m.position().x, m.position().y));
        let _ = w.set_size(tauri::PhysicalSize::new(m.size().width, m.size().height));
        labels.push(label);
    }
    let h = app.clone();
    let n = labels.len();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        for l in labels {
            if let Some(w) = h.get_webview_window(&l) {
                let _ = w.destroy();
            }
        }
    });
    Ok(n)
}

/// Shows a test pattern on one output for a few seconds. The UI asks first when it is Main.
#[tauri::command(async)]
fn identify_output(app: AppHandle, role: String) {
    let _ = app.emit("identify", role);
}

/// Settings the core uses; only these keys can be written.
const SETTINGS: &[&str] = &["project_on_restart", "low_spec_mode", "lines_per_slide", "advance_after_go_live", "font_fallback", "speech_engine", "speech_mic", "speech_refs_only", "speech_whisper_model", "deepgram_key", "deepgram_model", "quick_hymnal", "logo_media_id", "brand_colors", "brand_font", "brand_heading_font"];

#[tauri::command(async)]
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

#[tauri::command(async)]
fn set_setting(app: AppHandle, st: State<'_, AppState>, key: String, value: String) -> Res<()> {
    if !SETTINGS.contains(&key.as_str()) {
        return Err(format!("Unknown setting {key}"));
    }
    st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2)", [&key, &value]).map_err(err)?;
    let _ = app.emit("settings-changed", (key, value));
    Ok(())
}

/// EasyWorship's Repeating and Aspect Ratio for a picture or video.
#[tauri::command(async)]
fn set_media_options(app: AppHandle, st: State<'_, AppState>, id: i64, repeat: Option<String>, fit: Option<String>) -> Res<()> {
    wp_core::media::set_options(&st.db.lock().unwrap(), id, repeat.as_deref(), fit.as_deref()).map_err(err)?;
    // a live picture or video takes the new choice at once
    if matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Media { media_id, .. }) if *media_id == id) {
        broadcast(&app, &st);
    }
    let _ = app.emit("settings-changed", ("media", id.to_string()));
    Ok(())
}

/// EasyWorship's Open File Location: Explorer with the file selected.
#[tauri::command(async)]
fn reveal_media(st: State<'_, AppState>, id: i64) -> Res<()> {
    let item = wp_core::media::get(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id).map_err(err)?;
    let path = st.profile_dir().join("media").join(&item.path);
    if !path.exists() {
        return Err(format!("“{}” is missing from the media folder.", item.title));
    }
    let r = if cfg!(windows) {
        std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg("-R").arg(&path).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(path.parent().unwrap_or(&path)).spawn()
    };
    r.map(drop).map_err(|e| format!("The folder could not be opened ({e})."))
}

#[tauri::command(async)]
fn set_media_meta(st: State<'_, AppState>, id: i64, duration_ms: Option<i64>, width: Option<i64>, height: Option<i64>) -> Res<()> {
    wp_core::media::set_meta(&st.db.lock().unwrap(), id, duration_ms, width, height).map_err(err)
}

#[tauri::command(async)]
fn save_media_thumb(st: State<'_, AppState>, id: i64, jpeg: Vec<u8>) -> Res<String> {
    wp_core::media::save_thumb(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id, &jpeg).map_err(err)
}

#[tauri::command(async)]
fn rename_media(st: State<'_, AppState>, id: i64, title: String, tags: Option<String>) -> Res<()> {
    wp_core::media::rename(&st.db.lock().unwrap(), id, &title, tags.as_deref()).map_err(err)
}

#[tauri::command(async)]
fn media_used_by(st: State<'_, AppState>, id: i64) -> Res<Vec<String>> {
    wp_core::media::used_by(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id).map_err(err)
}

#[tauri::command(async)]
fn delete_media(st: State<'_, AppState>, id: i64) -> Res<()> {
    let live_is_this = matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Media { media_id, .. }) if *media_id == id);
    if live_is_this {
        return Err("This is on the screens. Send something else live first, then delete it.".into());
    }
    wp_core::media::delete(&st.db.lock().unwrap(), &st.profile_dir().join("media"), id).map_err(err)
}

/// Play/pause or restart the live video.
#[tauri::command(async)]
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
#[tauri::command(async)]
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
        let conn = st.db.lock().unwrap();
        let (id, name) = guard(|| wp_core::themes::import(&conn, &st.profile_dir().join("media"), &bytes))?.map_err(err)?;
        drop(conn);
        Ok(ImportedTheme { id, name })
    })
    .await
    .map_err(err)?
}

/// EasyWorship (.ewtx) or ProPresenter (.proTheme) themes: every design, with a preview.
#[tauri::command]
async fn foreign_theme_review(path: String) -> Res<Vec<wp_core::import::foreign_themes::Design>> {
    tauri::async_runtime::spawn_blocking(move || {
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 400_000_000 {
            return Err("This theme file is over 400 MB.".to_string());
        }
        guard(|| wp_core::import::foreign_themes::review(std::path::Path::new(&path)))?.map_err(err)
    })
    .await
    .map_err(err)?
}

/// Imports the chosen designs as themes.
#[tauri::command]
async fn foreign_theme_import(app: AppHandle, path: String, chosen: Vec<usize>) -> Res<Vec<ImportedTheme>> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let conn = st.db.lock().unwrap();
        let out = guard(|| wp_core::import::foreign_themes::import(&conn, &st.profile_dir().join("media"), std::path::Path::new(&path), &chosen))?.map_err(err)?;
        drop(conn);
        Ok(out.into_iter().map(|(id, name)| ImportedTheme { id, name }).collect())
    })
    .await
    .map_err(err)?
}

/// A note for the stage display only. Empty clears it.
#[tauri::command(async)]
fn set_stage_message(app: AppHandle, st: State<'_, AppState>, message: Option<String>) -> Res<LiveState> {
    let next = st.live.lock().unwrap().apply(Command::SetStageMessage { message });
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
}

// ---------- fonts ----------

/// Fonts installed on this computer (read once, then remembered).
#[tauri::command]
async fn list_fonts() -> Vec<fonts::FontFamily> {
    static CACHE: std::sync::OnceLock<Vec<fonts::FontFamily>> = std::sync::OnceLock::new();
    if let Some(c) = CACHE.get() {
        return c.clone();
    }
    let list = tauri::async_runtime::spawn_blocking(fonts::installed).await.unwrap_or_default();
    CACHE.get_or_init(|| list).clone()
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

/// Windows PowerShell by its full path, so a program named "powershell" earlier in
/// PATH cannot stand in for it.
pub(crate) fn powershell_exe() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(|r| PathBuf::from(r).join("System32").join("WindowsPowerShell").join("v1.0").join("powershell.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("powershell"))
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
            let mut cmd = std::process::Command::new(powershell_exe());
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
        let slides = guard(|| wp_core::presentations::read_pptx(&src))?.map_err(err)?;
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
#[tauri::command(async)]
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
#[tauri::command(async)]
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

#[tauri::command(async)]
fn output_configs(st: State<'_, AppState>) -> Res<Vec<wp_core::options::OutputConfig>> {
    wp_core::options::outputs(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
fn output_config(st: State<'_, AppState>, role: String) -> Res<wp_core::options::OutputConfig> {
    wp_core::options::output(&st.db.lock().unwrap(), &role).map_err(err)
}

/// Saves an output's options; an open output is rebuilt with them straight away.
#[tauri::command]
async fn save_output_config(app: AppHandle, config: wp_core::options::OutputConfig) -> Res<()> {
    let st = app.state::<AppState>();
    wp_core::options::save_output(&st.db.lock().unwrap(), &config).map_err(err)?;
    if let Some(f) = config.ndi {
        // EasyWorship's "NDI Stream" output monitor: no window, sent over NDI
        start_ndi_output(&app, &config.role, f)?;
    } else {
        stop_ndi_output(&app, &config.role);
        let open = st.outputs.lock().unwrap().get(&config.role).copied();
        if let Some(display) = open {
            build_output(&app, &config.role, display)?;
        }
    }
    let _ = app.emit("output-config-changed", config.role.clone());
    Ok(())
}

#[tauri::command(async)]
fn service_intervals(st: State<'_, AppState>) -> Res<Vec<wp_core::options::Interval>> {
    wp_core::options::intervals(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
fn save_service_interval(st: State<'_, AppState>, interval: wp_core::options::Interval) -> Res<i64> {
    wp_core::options::save_interval(&st.db.lock().unwrap(), &interval).map_err(err)
}

#[tauri::command(async)]
fn delete_service_interval(st: State<'_, AppState>, id: i64) -> Res<()> {
    wp_core::options::delete_interval(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command(async)]
fn current_service_interval(st: State<'_, AppState>) -> Option<String> {
    wp_core::options::current_interval(&st.db.lock().unwrap()).ok().flatten()
}

#[tauri::command(async)]
fn usage_report(st: State<'_, AppState>, from: String, to: String) -> Res<Vec<wp_core::options::UsageRow>> {
    wp_core::options::usage_report(&st.db.lock().unwrap(), &from, &to).map_err(err)
}

/// Writes the song usage report as CSV (for a spreadsheet or CCLI's website).
#[tauri::command(async)]
fn save_usage_report(st: State<'_, AppState>, from: String, to: String, path: String) -> Res<usize> {
    let rows = wp_core::options::usage_report(&st.db.lock().unwrap(), &from, &to).map_err(err)?;
    std::fs::write(&path, wp_core::options::usage_csv(&rows)).map_err(err)?;
    Ok(rows.len())
}

// ---------- EasyWorship-style library ----------

#[tauri::command(async)]
fn song_tree(st: State<'_, AppState>) -> Res<wp_core::browse::SongTree> {
    wp_core::browse::song_tree(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
fn browse_songs(st: State<'_, AppState>, scope: wp_core::browse::Scope, query: String) -> Res<Vec<wp_core::browse::SongRow>> {
    wp_core::browse::browse_songs(&st.db.lock().unwrap(), &scope, &query).map_err(err)
}

#[tauri::command(async)]
fn collection_create(st: State<'_, AppState>, name: String) -> Res<i64> {
    wp_core::browse::create_collection(&st.db.lock().unwrap(), &name).map_err(err)
}

#[tauri::command(async)]
fn collection_rename(st: State<'_, AppState>, id: i64, name: String) -> Res<()> {
    wp_core::browse::rename_collection(&st.db.lock().unwrap(), id, &name).map_err(err)
}

#[tauri::command(async)]
fn collection_delete(st: State<'_, AppState>, id: i64) -> Res<()> {
    wp_core::browse::delete_collection(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command(async)]
fn collection_add(st: State<'_, AppState>, id: i64, song_ids: Vec<i64>) -> Res<usize> {
    wp_core::browse::add_to_collection(&st.db.lock().unwrap(), id, &song_ids).map_err(err)
}

#[tauri::command(async)]
fn collection_remove(st: State<'_, AppState>, id: i64, song_ids: Vec<i64>) -> Res<()> {
    wp_core::browse::remove_from_collection(&st.db.lock().unwrap(), id, &song_ids).map_err(err)
}

#[tauri::command(async)]
fn browse_chapter(st: State<'_, AppState>, version: String, typed: String) -> Res<wp_core::browse::ChapterView> {
    wp_core::browse::browse_chapter(&st.db.lock().unwrap(), &version, &typed).map_err(err)
}

/// Verses containing the words typed (Bible word search).
#[tauri::command]
async fn search_bible(app: AppHandle, versions: Vec<String>, query: String) -> Res<Vec<wp_core::browse::VerseHit>> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let conn = st.db.lock().unwrap();
        // one Bible chosen: its own words, else English words shown in that Bible
        if versions.len() == 1 {
            wp_core::browse::search_bible_in(&conn, &versions[0], &query, 200).map_err(err)
        } else {
            wp_core::browse::search_bible(&conn, &versions, &query, 200).map_err(err)
        }
    })
    .await
    .map_err(err)?
}

#[tauri::command(async)]
fn rebuild_search(st: State<'_, AppState>) -> Res<usize> {
    wp_core::browse::rebuild_search(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
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
    audio: bool,
}

#[derive(Serialize)]
struct NdiStatus {
    runtime: Option<String>,
    problem: Option<String>,
    capture_supported: bool,
    sources: Vec<NdiSource>,
}

#[tauri::command(async)]
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
                audio: j.audio.is_some(),
            }
        })
        .collect();
    NdiStatus { runtime, problem, capture_supported: cfg!(windows), sources }
}

/// Sends a moving test pattern (with a see-through half) for `seconds`.
#[tauri::command(async)]
fn ndi_test_pattern(st: State<'_, AppState>, name: String, seconds: Option<u64>) -> Res<()> {
    let (slot, _stats, stop) = st.ndi.start_pump("test", &name, None, 30)?;
    ndi::run_test_pattern(slot, stop, seconds.unwrap_or(30));
    Ok(())
}

/// Starts an NDI source. "lower_third": an off-screen keyed window; "main": the
/// open main output window as it is.
#[tauri::command]
async fn ndi_start(app: AppHandle, source: String, name: String, fps: Option<u32>, groups: Option<String>, renderer: Option<String>) -> Res<()> {
    let st = app.state::<AppState>();
    let fps = fps.unwrap_or(30).clamp(10, 60);
    let default_name = if source == "main" { "Worship Presenter Main" } else { "Worship Presenter Lower Third" };
    let name = if name.trim().is_empty() { default_name.to_string() } else { name.trim().to_string() };
    let groups = groups.filter(|g| !g.trim().is_empty());
    // the lower third is drawn by the built-in renderer unless window capture is chosen
    if source == "lower_third" && renderer.as_deref() != Some("capture") {
        let (slot, stats, stop) = st.ndi.start_pump(&source, &name, groups.as_deref(), fps)?;
        ndi::run_builtin(app.clone(), slot, stats, stop);
        return Ok(());
    }
    if !cfg!(windows) {
        return Err("Window capture for NDI works on Windows only. Use the built-in renderer for the lower third.".into());
    }
    let label = match source.as_str() {
        "lower_third" => {
            // 1920×1080 window drawn on the key colour, kept off the visible desktop
            let w = WebviewWindowBuilder::new(&app, "output-ndi", WebviewUrl::App("index.html#/output/ndi".into()))
                .additional_browser_args(BROWSER_ARGS)
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
    #[cfg(windows)]
    {
        let win = app.get_webview_window(label).ok_or_else(|| "Open the main output first (Output setup).".to_string())?;
        let hwnd = win.hwnd().map_err(err)?.0 as *mut std::ffi::c_void;
        let (slot, stats, _stop) = st.ndi.start_pump(&source, &name, groups.as_deref(), fps)?;
        let running = ndi::capture::start(hwnd, slot, stats, source == "lower_third")?;
        if let Some(j) = st.ndi.jobs.lock().unwrap().get_mut(&source) {
            j.capture = Some(running);
        }
    }
    #[cfg(not(windows))]
    let _ = (label, name, fps, &st, groups);
    Ok(())
}

/// What the NDI lower third shows now (the same rules as the lower-third output):
/// song and scripture words; nothing for media, announcements, Blank, Logo or held.
pub(crate) fn ndi_content(st: &AppState) -> (wp_render::Content, serde_json::Value, String, bool) {
    let live = st.live.lock().unwrap().clone();
    let item = st.item.lock().unwrap().clone();
    let slides = st.slides.lock().unwrap().clone();
    let caption = st.caption.lock().unwrap().clone();
    let rev = *st.theme_rev.lock().unwrap();
    // the theme is read again only when themes change (this runs 20 times a second)
    static CACHE: std::sync::LazyLock<Mutex<Option<(u64, serde_json::Value, bool)>>> = std::sync::LazyLock::new(|| Mutex::new(None));
    let brand = {
        let conn = st.db.lock().unwrap();
        let get = |k: &str| conn.query_row("SELECT value FROM setting WHERE key = ?1", [k], |r| r.get::<_, String>(0)).ok();
        wp_render::Brand::from_settings(get("brand_colors").as_deref(), get("brand_font").as_deref(), get("brand_heading_font").as_deref())
    };
    // the cache key includes the brand, so a brand change re-reads the theme
    let rev = rev.wrapping_mul(1_000_003).wrapping_add(brand.key());
    let cached = CACHE.lock().unwrap().clone().filter(|(r, _, _)| *r == rev);
    let (theme, has_clock) = match cached {
        Some((_, t, c)) => (t, c),
        None => {
            let t = {
                let conn = st.db.lock().unwrap();
                wp_core::themes::list(&conn).ok().and_then(|rows| rows.into_iter().filter(|r| r.kind == "lower_third").max_by_key(|r| r.is_default).map(|r| r.theme))
            }
            .unwrap_or_else(|| serde_json::json!({ "background": { "kind": "transparent" }, "layers": [] }));
            let t = brand.resolve(&t);
            let c = t.to_string().contains("\"clock\"");
            *CACHE.lock().unwrap() = Some((rev, t.clone(), c));
            (t, c)
        }
    };
    let clock_now = if has_clock {
        st.db.lock().unwrap().query_row("SELECT strftime('%H:%M', 'now', 'localtime')", [], |r| r.get::<_, String>(0)).unwrap_or_default()
    } else {
        String::new()
    };
    let mut c = wp_render::Content { clock: clock_now, ..Default::default() };
    let words = matches!(item, Some(ProgramItem::Song { .. }) | Some(ProgramItem::Scripture { .. }));
    let showing = matches!(live.mode, Mode::Live | Mode::Clear);
    if words && showing {
        c.first = live.slide_idx == 0;
        c.last = live.slide_idx + 1 >= slides.len();
        if let Some(s) = slides.get(live.slide_idx) {
            c.primary = s.primary.clone();
            c.second = s.second.clone();
            if let Some(ProgramItem::Song { song_id, .. }) = item {
                c.section = Some(section_name(&s.label));
                c.title = caption.clone();
                c.copyright = st.db.lock().unwrap().query_row("SELECT copyright FROM song WHERE id = ?1", [song_id], |r| r.get::<_, Option<String>>(0)).ok().flatten();
            } else {
                c.reference = caption.clone();
            }
        }
        c.hide_text = live.mode == Mode::Clear;
        c.text_scale = live.text_scale;
        c.text_shift = live.text_shift;
    }
    c.alert = live.alert.as_ref().filter(|a| a.on_stream).map(|a| a.text.clone());
    if matches!(live.subtitles.as_deref(), Some("stream") | Some("both")) {
        let (text, at) = st.caption_now.lock().unwrap().clone();
        if at.map_or(false, |t| t.elapsed().as_secs() < 6) {
            c.subtitle = Some(text);
        }
    }
    let clock = has_clock;
    let sig = format!("{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{}|{}", live.revision, rev, c.primary, c.alert, c.subtitle, c.reference, c.hide_text, c.text_scale, c.text_shift);
    (c, theme, sig, clock)
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a full-screen output sent over NDI shows (the same choices as its window would
/// make): the item's theme, else the output's, else the rules', else the default; Blank,
/// Logo and Clear; pictures, presentation slides, and a video's preview picture.
pub(crate) fn full_content(st: &AppState, role: &str) -> (wp_render::Content, serde_json::Value, String, bool) {
    let (mut c, _lt, _sig, _clock) = ndi_content(st);
    let live = st.live.lock().unwrap().clone();
    let item = st.item.lock().unwrap().clone();
    let slides = st.slides.lock().unwrap().clone();
    let caption = st.caption.lock().unwrap().clone();
    let rev = *st.theme_rev.lock().unwrap();
    let conn = st.db.lock().unwrap();
    let cfg = wp_core::options::output(&conn, role).ok();
    let setting = |k: &str| conn.query_row("SELECT value FROM setting WHERE key = ?1", [k], |r| r.get::<_, String>(0)).ok();
    let brand = wp_render::Brand::from_settings(setting("brand_colors").as_deref(), setting("brand_font").as_deref(), setting("brand_heading_font").as_deref());
    let picture = |path: &str, fit: &str| serde_json::json!({ "background": { "kind": "image", "color": "#000000", "image": format!("media:{path}"), "fit": fit, "dim": 0 }, "layers": [] });
    let black = serde_json::json!({ "background": { "kind": "color", "color": "#000000" }, "layers": [] });
    let clear = serde_json::json!({ "background": { "kind": "transparent" }, "layers": [] });
    let alpha = cfg.as_ref().map_or(false, |c| c.alpha);
    let mut theme = match live.mode {
        Mode::Blank => {
            if cfg.as_ref().map_or(false, |c| c.on_blank == "transparent") || alpha { clear.clone() } else { black.clone() }
        }
        Mode::Logo => {
            let logo = setting("logo_media_id").and_then(|id| id.parse::<i64>().ok()).and_then(|id| wp_core::media::get(&conn, &st.profile_dir().join("media"), id).ok());
            match logo {
                Some(m) if m.kind == "image" => picture(&m.path, "cover"),
                Some(m) => m.thumb.as_deref().map(|t| picture(t, "cover")).unwrap_or_else(|| black.clone()),
                None => black.clone(),
            }
        }
        _ => match &item {
            Some(ProgramItem::Song { song_id, theme_id, .. }) => {
                let id = theme_id.or(cfg.as_ref().and_then(|c| c.song_theme_id)).or_else(|| wp_core::theme_rules::for_song(&conn, *song_id).ok().flatten());
                id.and_then(|i| wp_core::themes::get(&conn, i).ok()).or_else(|| wp_core::themes::for_kind(&conn, "song").ok()).unwrap_or(black.clone())
            }
            Some(ProgramItem::Scripture { versions, theme_id, .. }) => {
                let id = theme_id.or(cfg.as_ref().and_then(|c| c.scripture_theme_id)).or_else(|| wp_core::theme_rules::for_scripture(&conn, versions).ok().flatten());
                id.and_then(|i| wp_core::themes::get(&conn, i).ok()).or_else(|| wp_core::themes::for_kind(&conn, "scripture").ok()).unwrap_or(black.clone())
            }
            Some(ProgramItem::Announcement { .. }) => {
                let s = slides.get(live.slide_idx);
                match s.and_then(|s| s.image.clone()) {
                    Some(img) => picture(&img, "contain"),
                    None => {
                        if let Some(s) = s {
                            c.primary = s.primary.clone();
                            c.title = caption.clone();
                        }
                        wp_core::themes::for_kind(&conn, "announcement").unwrap_or(black.clone())
                    }
                }
            }
            Some(ProgramItem::Media { media_id, .. }) => match wp_core::media::get(&conn, &st.profile_dir().join("media"), *media_id) {
                Ok(m) => {
                    let fit = if m.fit.as_deref() == Some("fill") { "cover" } else { "contain" };
                    match m.kind.as_str() {
                        "image" => picture(&m.path, fit),
                        // the built-in renderer sends still pictures: a video shows its preview
                        "video" => m.thumb.as_deref().map(|t| picture(t, fit)).unwrap_or(black.clone()),
                        _ => black.clone(),
                    }
                }
                Err(_) => black.clone(),
            },
            None => black.clone(),
        },
    };
    drop(conn);
    // the full-screen look shows song and scripture words (the lower-third rules aside)
    if matches!(live.mode, Mode::Live | Mode::Clear) {
        if let (Some(ProgramItem::Song { .. }) | Some(ProgramItem::Scripture { .. }), Some(s)) = (&item, slides.get(live.slide_idx)) {
            c.primary = s.primary.clone();
            c.second = s.second.clone();
        }
    }
    if alpha {
        theme["background"] = serde_json::json!({ "kind": "transparent" });
    }
    let theme = brand.resolve(&theme);
    let has_clock = theme.to_string().contains("\"clock\"");
    let sig = format!("{}|{}|{:?}|{}|{:?}|{:?}|{}|{}|{}", live.revision, rev, live.mode, brand.key(), c.primary, c.alert, c.text_scale, c.text_shift, theme.to_string().len());
    (c, theme, sig, has_clock)
}

/// Starts (or restarts) an output as an NDI stream, closing its window if it had one.
fn start_ndi_output(app: &AppHandle, role: &str, f: wp_core::options::NdiFormat) -> Res<()> {
    let st = app.state::<AppState>();
    st.outputs.lock().unwrap().remove(role);
    st.acks.lock().unwrap().remove(role);
    if let Some(w) = app.get_webview_window(&format!("output-{role}")) {
        let _ = w.destroy();
    }
    let name = match role {
        "main" => "Worship Presenter Main",
        "alternate" => "Worship Presenter Alternate",
        _ => "Worship Presenter Foldback",
    };
    let key = format!("out-{role}");
    let (slot, stats, stop) = st.ndi.start_pump(&key, name, None, f.fps)?;
    // the alternate output is the lower-third look: built-in renderer, exact transparency
    if role == "alternate" {
        ndi::run_builtin_as(app.clone(), slot, stats, stop, None, f.width, f.height);
        return Ok(());
    }
    // full screen: like FreeShow, send the real output page (videos, animations, camera)
    #[cfg(windows)]
    {
        match ndi_window(app, role, f) {
            Ok(hwnd) => match ndi::capture::start(hwnd, slot.clone(), stats.clone(), false) {
                Ok(running) => {
                    if let Some(j) = st.ndi.jobs.lock().unwrap().get_mut(&key) {
                        j.capture = Some(running);
                    }
                    return Ok(());
                }
                Err(e) => eprintln!("NDI window capture failed, using the built-in renderer: {e}"),
            },
            Err(e) => eprintln!("NDI window failed, using the built-in renderer: {e}"),
        }
    }
    ndi::run_builtin_as(app.clone(), slot, stats, stop, Some(role.to_string()), f.width, f.height);
    Ok(())
}

/// The output page for an NDI stream: a real output window at the NDI size, off the
/// visible desktop and out of the taskbar (WebView2 has no off-screen painting, so it is
/// captured instead; the flags in BROWSER_ARGS keep it from being throttled).
#[cfg(windows)]
fn ndi_window(app: &AppHandle, role: &str, f: wp_core::options::NdiFormat) -> Res<*mut std::ffi::c_void> {
    let label = format!("output-ndi-{role}");
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.destroy();
        let t = Instant::now();
        while app.get_webview_window(&label).is_some() && t.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let w = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(format!("index.html#/output/{role}").into()))
        .additional_browser_args(BROWSER_ARGS)
        .title(format!("Worship Presenter – NDI {role}"))
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .inner_size(f.width as f64, f.height as f64)
        .position(-20000.0, 0.0)
        .build()
        .map_err(err)?;
    let _ = w.set_size(tauri::PhysicalSize::new(f.width, f.height));
    let _ = w.set_position(tauri::PhysicalPosition::new(-20000, 0));
    // give the page a moment to load before frames are taken
    std::thread::sleep(Duration::from_millis(800));
    Ok(w.hwnd().map_err(err)?.0 as *mut std::ffi::c_void)
}

fn stop_ndi_output(app: &AppHandle, role: &str) {
    app.state::<AppState>().ndi.stop(&format!("out-{role}"));
    if let Some(w) = app.get_webview_window(&format!("output-ndi-{role}")) {
        let _ = w.destroy();
    }
}

/// "V2" → "Verse 2", "C" → "Chorus" (as the screens label sections).
fn section_name(label: &str) -> String {
    let letters: String = label.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let num = &label[letters.len()..];
    let word = match letters.as_str() {
        "V" => "Verse",
        "C" => "Chorus",
        "B" => "Bridge",
        "PC" => "Pre-chorus",
        "T" => "Tag",
        "E" => "Ending",
        "I" => "Intro",
        "O" => "Outro",
        _ => return label.to_string(),
    };
    if num.is_empty() { word.to_string() } else { format!("{word} {num}") }
}

/// NDI sources on the network, for the Camera panel.
#[tauri::command]
async fn ndi_sources(app: AppHandle, groups: Option<String>) -> Res<Vec<wp_core::ndi::sdk::Source>> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let rt = st.ndi.runtime()?;
        let own: Vec<String> = st.ndi.jobs.lock().unwrap().values().map(|j| j.name.clone()).collect();
        // leave out this computer's own outputs
        Ok(rt.find_sources(groups.as_deref(), 1500).into_iter().filter(|s| !own.iter().any(|o| s.name.ends_with(&format!("({o})")))).collect())
    })
    .await
    .map_err(err)?
}

/// The theme the rules choose for an item (None: the default theme).
fn rule_theme(conn: &rusqlite::Connection, item: &ProgramItem) -> Option<i64> {
    match item {
        ProgramItem::Song { song_id, .. } => wp_core::theme_rules::for_song(conn, *song_id).ok().flatten(),
        ProgramItem::Scripture { versions, .. } => wp_core::theme_rules::for_scripture(conn, versions).ok().flatten(),
        _ => None,
    }
}

#[tauri::command(async)]
fn theme_rules(st: State<'_, AppState>) -> Res<Vec<wp_core::theme_rules::Rule>> {
    wp_core::theme_rules::list(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
fn save_theme_rules(app: AppHandle, st: State<'_, AppState>, rules: Vec<wp_core::theme_rules::Rule>) -> Res<()> {
    wp_core::theme_rules::save_all(&st.db.lock().unwrap(), &rules).map_err(err)?;
    // the live item may now use another theme
    broadcast(&app, &st);
    let _ = app.emit("settings-changed", ("theme_rules", ""));
    Ok(())
}

/// For Preview: the theme the rules choose for this item.
#[tauri::command(async)]
fn resolve_theme(st: State<'_, AppState>, item: ProgramItem) -> Res<Option<i64>> {
    Ok(rule_theme(&st.db.lock().unwrap(), &item))
}

/// Sends (or stops sending) the computer's sound with an NDI source.
#[tauri::command(async)]
fn ndi_audio(st: State<'_, AppState>, source: String, on: bool) -> Res<()> {
    let mut jobs = st.ndi.jobs.lock().unwrap();
    let job = jobs.get_mut(&source).ok_or("Start that NDI source first.")?;
    if let Some(a) = job.audio.take() {
        a.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if on {
        job.audio = Some(ndi::start_audio(job.sender.clone())?);
    }
    Ok(())
}

/// The Windows title bar follows the app's Dark/Light choice; on Windows 11 the
/// window gets the Mica backdrop in the same shade (Windows 10 ignores it).
#[tauri::command(async)]
fn set_window_look(app: AppHandle, dark: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_theme(Some(if dark { tauri::Theme::Dark } else { tauri::Theme::Light }));
        let effect = if dark { tauri::window::Effect::MicaDark } else { tauri::window::Effect::MicaLight };
        let _ = w.set_effects(tauri::window::EffectsBuilder::new().effect(effect).build());
        let _ = w.set_background_color(Some(if dark { tauri::window::Color(25, 25, 24, 255) } else { tauri::window::Color(247, 247, 245, 255) }));
    }
}

/// The control window starts hidden and appears once its first frame is drawn.
#[tauri::command(async)]
fn show_main_window(app: AppHandle) {
    applog::write("START", "the service window is drawn: showing it");
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// The Windows menu bar of the control window. Shortcuts are shown beside each item
/// but handled by the app's own keyboard code, so a key never acts twice.
fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
    let item = |id: &str, text: &str, keys: &str| {
        let label = if keys.is_empty() { text.to_string() } else { format!("{text}\t{keys}") };
        MenuItemBuilder::with_id(id, label).build(app)
    };
    let file = SubmenuBuilder::new(app, "&File")
        .item(&item("file.service", "&Open or start a service order…", "")?)
        .item(&item("file.import", "&Import songs or Bibles…", "")?)
        .item(&item("file.backups", "&Backups…", "")?)
        .item(&item("file.print-order", "&Print the service order…", "Ctrl+P")?)
        .separator()
        .item(&item("file.outputs", "Output &setup…", "")?)
        .item(&item("file.settings", "Se&ttings", "")?)
        .separator()
        .item(&item("file.exit", "E&xit", "Alt+F4")?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "&Edit")
        .item(&PredefinedMenuItem::cut(app, Some("Cu&t\tCtrl+X"))?)
        .item(&PredefinedMenuItem::copy(app, Some("&Copy\tCtrl+C"))?)
        .item(&PredefinedMenuItem::paste(app, Some("&Paste\tCtrl+V"))?)
        .item(&PredefinedMenuItem::select_all(app, Some("Select &all\tCtrl+A"))?)
        .separator()
        .item(&item("edit.find", "&Find a song", "Ctrl+F")?)
        .item(&item("edit.quick", "&Quick scripture", "Ctrl+G")?)
        .item(&item("edit.newsong", "&New song", "")?)
        .separator()
        .item(&item("edit.palette", "C&ommands…", "Ctrl+K")?)
        .build()?;
    let live = SubmenuBuilder::new(app, "&Live")
        .item(&item("live.go", "&Go live", "F5")?)
        .item(&item("live.next", "&Next slide", "→")?)
        .item(&item("live.previous", "&Previous slide", "←")?)
        .item(&item("live.nextitem", "Next &item", "PgDn")?)
        .item(&item("live.previousitem", "Previous i&tem", "PgUp")?)
        .separator()
        .item(&item("live.blank", "&Blank", "B")?)
        .item(&item("live.logo", "&Logo", "L")?)
        .item(&item("live.clear", "&Clear text", "C")?)
        .item(&item("live.resume", "&Resume", "")?)
        .separator()
        .item(&item("live.timer", "&Timer…", "")?)
        .item(&item("live.alert", "&Alert…", "")?)
        .item(&item("live.music", "Background &music…", "")?)
        .separator()
        .item(&item("live.rehearsal", "Re&hearsal mode", "")?)
        .build()?;
    let profiles = SubmenuBuilder::new(app, "&Profiles").item(&item("profiles.manage", "&Switch or manage profiles…", "")?).build()?;
    let view = SubmenuBuilder::new(app, "&View")
        .item(&item("view.dark", "&Dark", "")?)
        .item(&item("view.light", "&Light", "")?)
        .item(&item("view.system", "&Follow Windows", "")?)
        .separator()
        .item(&item("view.preview-list", "Preview as &list", "")?)
        .item(&item("view.preview-pictures", "Preview as &pictures", "")?)
        .item(&item("view.live-list", "Live as l&ist", "")?)
        .item(&item("view.live-pictures", "Live as pi&ctures", "")?)
        .separator()
        .item(&item("view.themes", "&Themes", "")?)
        .item(&item("view.media", "&Media library", "")?)
        .item(&item("view.reset", "&Reset pane sizes", "")?)
        .build()?;
    let help = SubmenuBuilder::new(app, "&Help")
        .item(&item("help.shortcuts", "&Keyboard shortcuts", "")?)
        .separator()
        .item(&item("help.about", "&About Worship Presenter", "")?)
        .build()?;
    MenuBuilder::new(app).items(&[&file, &edit, &live, &profiles, &view, &help]).build()
}

/// Opens NDI's official runtime download in the browser (this fixed address only).
#[tauri::command(async)]
fn open_ndi_runtime_page() -> Res<()> {
    let url = if cfg!(target_os = "macos") { "https://ndi.link/NDIRedistV6Apple" } else { "https://ndi.link/NDIRedistV6" };
    let r = if cfg!(windows) {
        std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    r.map(drop).map_err(|e| format!("Could not open the browser ({e}). The address is {url}"))
}

#[tauri::command(async)]
fn ndi_stop(app: AppHandle, st: State<'_, AppState>, source: String) {
    st.ndi.stop(&source);
    if source == "lower_third" {
        if let Some(w) = app.get_webview_window("output-ndi") {
            let _ = w.destroy();
        }
    }
}

// ---------- announcements, repeat counter, lining out ----------

#[tauri::command(async)]
fn list_announcements(st: State<'_, AppState>) -> Res<Vec<wp_core::announcements::DeckSummary>> {
    wp_core::announcements::list(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command(async)]
fn get_announcement(st: State<'_, AppState>, id: i64) -> Res<wp_core::announcements::Deck> {
    wp_core::announcements::get(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command(async)]
fn announcement_templates() -> Vec<wp_core::announcements::Deck> {
    wp_core::announcements::templates()
}

/// Saves a deck. Like songs, never changes what is on the screens.
#[tauri::command(async)]
fn save_announcement(st: State<'_, AppState>, deck: wp_core::announcements::Deck) -> Res<i64> {
    wp_core::announcements::save(&st.db.lock().unwrap(), &deck).map_err(err)
}

#[tauri::command(async)]
fn delete_announcement(st: State<'_, AppState>, id: i64) -> Res<()> {
    if matches!(&*st.item.lock().unwrap(), Some(ProgramItem::Announcement { presentation_id, .. }) if *presentation_id == id) {
        return Err("These announcements are on the screens. Send something else live first.".into());
    }
    wp_core::announcements::delete(&st.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command(async)]
fn preview_announcement(st: State<'_, AppState>, id: i64) -> Res<Vec<Slide>> {
    let d = wp_core::announcements::get(&st.db.lock().unwrap(), id).map_err(err)?;
    Ok(wp_core::announcements::to_slides(&d))
}

/// Pause or resume a deck that advances by itself.
#[tauri::command(async)]
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
#[tauri::command(async)]
fn set_repeat(app: AppHandle, st: State<'_, AppState>, label: Option<String>, total: u32, done: Option<u32>) -> Res<LiveState> {
    let cur = st.live.lock().unwrap().clone();
    let repeat = label.map(|l| {
        let on_it = st.slides.lock().unwrap().get(cur.slide_idx).map_or(false, |s| s.label == l);
        wp_core::live::Repeat { label: l, total: total.max(1), done: done.unwrap_or(if on_it { 1 } else { 0 }) }
    });
    Ok(apply(&app, &st, Command::SetRepeat { repeat }))
}

/// Browser arguments for every window (they must match across windows). Tauri's
/// defaults, plus camera access for the app's own pages without a prompt that would
/// appear on the projector (only the app's code runs there: see the CSP).
/// The same for every window (WebView2 windows share one engine). As FreeShow does for its
/// NDI windows (backgroundThrottling off, autoplay without a click): output windows that
/// are covered, minimised or off the desktop keep playing videos and animations.
pub(crate) const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --use-fake-ui-for-media-stream --disable-pinch --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows --autoplay-policy=no-user-gesture-required";

/// The Live pane's quick change: words larger/smaller and up/down for this item only.
#[tauri::command(async)]
fn set_text_adjust(app: AppHandle, st: State<'_, AppState>, scale: f32, shift: f32) -> Res<LiveState> {
    Ok(apply(&app, &st, Command::SetTextAdjust { scale, shift }))
}

/// Moves the musicians' chords up or down (semitones) on the stage view.
#[tauri::command(async)]
fn set_transpose(app: AppHandle, st: State<'_, AppState>, semitones: i8) -> Res<LiveState> {
    Ok(apply(&app, &st, Command::SetTranspose { semitones }))
}

/// Shows a camera on the main screen (or takes it off with None).
#[tauri::command(async)]
fn set_camera(app: AppHandle, st: State<'_, AppState>, camera: Option<wp_core::live::Camera>) -> Res<LiveState> {
    Ok(apply(&app, &st, Command::SetCamera { camera }))
}

/// Live sermon subtitles on the stream, the screens, both, or off.
#[tauri::command(async)]
fn set_subtitles(app: AppHandle, st: State<'_, AppState>, target: Option<String>) -> Res<LiveState> {
    Ok(apply(&app, &st, Command::SetSubtitles { target }))
}

/// Lining out: one line per slide, remembering the previous setting to restore.
#[tauri::command(async)]
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
#[tauri::command(async)]
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
#[tauri::command(async)]
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

#[tauri::command(async)]
fn alert_clear(app: AppHandle, st: State<'_, AppState>) -> LiveState {
    apply(&app, &st, Command::SetAlert { alert: None })
}

#[derive(Serialize)]
struct CuePresets {
    timers: Vec<cues::TimerPreset>,
    alerts: Vec<cues::AlertPreset>,
}

#[tauri::command(async)]
fn cue_presets(st: State<'_, AppState>) -> Res<CuePresets> {
    let c = st.db.lock().unwrap();
    Ok(CuePresets { timers: cues::timer_presets(&c).map_err(err)?, alerts: cues::alert_presets(&c).map_err(err)? })
}

#[tauri::command(async)]
fn save_timer_preset(st: State<'_, AppState>, preset: cues::TimerPreset) -> Res<i64> {
    cues::save_timer_preset(&st.db.lock().unwrap(), &preset).map_err(err)
}

#[tauri::command(async)]
fn save_alert_preset(st: State<'_, AppState>, preset: cues::AlertPreset) -> Res<i64> {
    cues::save_alert_preset(&st.db.lock().unwrap(), &preset).map_err(err)
}

#[tauri::command(async)]
fn delete_cue_preset(st: State<'_, AppState>, kind: String, id: i64) -> Res<()> {
    cues::delete_preset(&st.db.lock().unwrap(), &kind, id).map_err(err)
}

// ---------- profiles ----------

#[derive(Serialize)]
struct CurrentProfile {
    slug: String,
    name: String,
}

#[tauri::command(async)]
fn list_profiles(st: State<'_, AppState>) -> Res<Vec<wp_core::profiles::Profile>> {
    wp_core::profiles::list(&st.profiles_root).map_err(err)
}

#[tauri::command(async)]
fn current_profile(st: State<'_, AppState>) -> Res<CurrentProfile> {
    let slug = st.profile.lock().unwrap().0.clone();
    let name = wp_core::profiles::list(&st.profiles_root).map_err(err)?.into_iter().find(|p| p.slug == slug).map(|p| p.name).unwrap_or_else(|| slug.clone());
    Ok(CurrentProfile { slug, name })
}

/// New profile, empty or starting as a copy of the current library.
#[tauri::command(async)]
fn create_profile(st: State<'_, AppState>, name: String, copy_current: bool) -> Res<String> {
    let conn = st.db.lock().unwrap();
    wp_core::profiles::create(&st.profiles_root, &name, copy_current.then_some(&*conn)).map_err(err)
}

#[tauri::command(async)]
fn rename_profile(st: State<'_, AppState>, slug: String, name: String) -> Res<()> {
    wp_core::profiles::rename(&st.profiles_root, &slug, &name).map_err(err)
}

#[tauri::command(async)]
fn delete_profile(st: State<'_, AppState>, slug: String) -> Res<String> {
    wp_core::profiles::delete(&st.profiles_root, &slug).map(|p| p.to_string_lossy().into_owned()).map_err(err)
}

/// Swaps the whole library to another profile. What is live is taken off air
/// (outputs go blank) because it belongs to the old library.
#[tauri::command(async)]
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
/// The Whisper helper process's entry (see whisper_worker.rs).
pub fn whisper_worker(model: &str) -> ! {
    whisper_worker::worker_main(model, speech::WHISPER_PROMPT)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // the Windows menu bar on the control window only (outputs stay bare)
            if let Some(main) = app.get_webview_window("main") {
                if let Ok(menu) = build_menu(app.handle()) {
                    let _ = main.set_menu(menu);
                }
                let h = app.handle().clone();
                main.on_menu_event(move |_w, e| {
                    let _ = h.emit_to("main", "menu", e.id().0.clone());
                });
                // never stay invisible: shown when the first frame is drawn, or after 4 s
                let m2 = main.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(4));
                    if !m2.is_visible().unwrap_or(true) {
                        let _ = m2.show();
                    }
                });
            }
            // automatic updates, when the build carries the church's public key
            let has_key = app.config().plugins.0.get("updater").and_then(|u| u.get("pubkey")).and_then(|k| k.as_str()).is_some_and(|k| !k.trim().is_empty());
            if has_key {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
                UPDATER_ON.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            // the app's own log (crashes, errors, important events)
            applog::init(app.path().app_data_dir()?.join("Logs"));
            // the main thread must keep answering: if it is stuck for 5 s, say so in the log
            {
                let h = app.handle().clone();
                std::thread::spawn(move || {
                    let beat = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
                    let mut stuck_since: Option<Instant> = None;
                    let mut n = 0u64;
                    loop {
                        n += 1;
                        let b = beat.clone();
                        let _ = h.run_on_main_thread(move || b.store(n, std::sync::atomic::Ordering::Relaxed));
                        std::thread::sleep(Duration::from_secs(2));
                        if beat.load(std::sync::atomic::Ordering::Relaxed) < n {
                            let since = *stuck_since.get_or_insert_with(Instant::now);
                            if since.elapsed() >= Duration::from_secs(4) {
                                applog::write("STUCK", &format!("the main thread has not answered for {} s", since.elapsed().as_secs() + 2));
                            }
                        } else if let Some(since) = stuck_since.take() {
                            applog::write("STUCK", &format!("the main thread answers again after {} s", since.elapsed().as_secs() + 2));
                        }
                    }
                });
            }
            applog::write("START", "setting up");
            // the window stays hidden until the interface has drawn; if that never comes,
            // show it anyway after 15 s (and say so in the log) rather than look dead
            {
                let h = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(15));
                    if let Some(w) = h.get_webview_window("main") {
                        if !w.is_visible().unwrap_or(true) {
                            applog::write("STUCK", "the interface did not finish loading in 15 s: showing the window anyway");
                            let _ = w.show();
                        }
                    }
                });
            }
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
                speech: speech::Speech::default(),
                pulpit: Mutex::new(Default::default()),
                caption_now: Mutex::new((String::new(), None)),
                ndi_inputs: ndi::Inputs::default(),
                midi: midi::Midi::default(),
                stream: stream::Stream::default(),
                building: Mutex::new(Default::default()),
                born: Mutex::new(HashMap::new()),
                seen: Mutex::new(Default::default()),
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
                let mut cfgs = wp_core::options::outputs(&st.db.lock().unwrap()).unwrap_or_default();
                let monitors = h2.available_monitors().unwrap_or_default();
                let names: Vec<String> = monitors.iter().map(|m| m.name().cloned().unwrap_or_default()).collect();
                // First run with a projector or TV connected (as EasyWorship does): the main
                // output goes on the first monitor that is not the operator's own screen,
                // and is remembered and shown at every start from then on.
                let primary = h2.primary_monitor().ok().flatten().map(|m| *m.position());
                if let Some(main) = cfgs.iter_mut().find(|c| c.role == "main") {
                    let never_set = main.display_index.is_none() && main.display_name.is_none();
                    if never_set && monitors.len() > 1 {
                        if let Some(i) = monitors.iter().position(|m| Some(*m.position()) != primary) {
                            main.display_index = Some(i);
                            main.display_name = monitors[i].name().cloned();
                            main.show_on_startup = true;
                            let _ = wp_core::options::save_output(&st.db.lock().unwrap(), main);
                        }
                    }
                }
                // sync: a following computer takes a waiting update before anything is shown;
                // a sharing computer shares now and every 30 minutes
                match setting_of(&st, "sync_role").as_deref() {
                    Some("follow") => {
                        let taken = setting_of(&st, "sync_taken_ms").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let waiting = setting_of(&st, "sync_folder").and_then(|f| wp_core::sync::waiting(std::path::Path::new(&f), &profile_name(&st), taken));
                        if waiting.is_some_and(|m| m.computer != computer_name()) {
                            let _ = sync_take(&h2, &st);
                        }
                    }
                    Some("share") => {
                        let h = h2.clone();
                        std::thread::spawn(move || loop {
                            std::thread::sleep(Duration::from_secs(20));
                            let st = h.state::<AppState>();
                            if setting_of(&st, "sync_role").as_deref() != Some("share") {
                                break;
                            }
                            let _ = sync_share(&st);
                            std::thread::sleep(Duration::from_secs(30 * 60));
                        });
                    }
                    _ => {}
                }
                // the MIDI pads chosen last time
                let midi_port: Option<String> = st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = 'midi_port'", [], |r| r.get(0)).ok();
                if let Some(p) = midi_port {
                    let _ = midi::connect(&h2, &p);
                }
                for c in cfgs.iter().filter(|c| c.ndi.is_some()) {
                    let _ = start_ndi_output(&h2, &c.role, c.ndi.unwrap());
                }
                for c in cfgs.into_iter().filter(|c| c.show_on_startup && c.ndi.is_none()) {
                    if let Some(i) = wp_core::options::pick_display(&c, &names) {
                        st.outputs.lock().unwrap().insert(c.role.clone(), Some(i));
                        st.acks.lock().unwrap().insert(c.role.clone(), (Instant::now(), 0));
                        let (h3, role) = (h2.clone(), c.role.clone());
                        // never build windows on the main thread (WebView2 freezes)
                        tauri::async_runtime::spawn(async move {
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
                        // a window being built, or still loading (15 s), is left alone
                        if s.building.lock().unwrap().contains(&role) || s.born.lock().unwrap().get(&role).map_or(false, |t| t.elapsed() < Duration::from_secs(15)) {
                            continue;
                        }
                        let silent = s.acks.lock().unwrap().get(&role).map(|(t, _)| t.elapsed() > Duration::from_secs(4)).unwrap_or(true);
                        let missing = handle.get_webview_window(&format!("output-{role}")).is_none();
                        if silent || missing {
                            s.acks.lock().unwrap().insert(role.clone(), (Instant::now(), 0));
                            // an output that was working and was lost comes back black until
                            // the operator resumes it; one that never started is not held
                            let had_worked = s.seen.lock().unwrap().contains(&role);
                            if had_worked && s.held.lock().unwrap().insert(role.clone()) {
                                broadcast(&handle, &s);
                            }
                            let h = handle.clone();
                            // off the main thread: building a window there freezes WebView2
                            tauri::async_runtime::spawn(async move {
                                let _ = build_output(&h, &role, display);
                            });
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(1000));
            });
            applog::write("START", "setup finished");
            Ok(())
        })
        // closing the control window ends Worship Presenter: outputs, music, the phone
        // remote and the watchdog go with it (they used to keep running with no window)
        // screens fetch the newest picture of an NDI source here (ndi://localhost/frame/…)
        .register_asynchronous_uri_scheme_protocol("ndi", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || {
                let uri = request.uri();
                let path = uri.path().trim_start_matches('/');
                let name = path.strip_prefix("frame/").map(pct_decode).unwrap_or_default();
                let url = uri.query().and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("url="))).map(pct_decode).unwrap_or_default();
                let st = app.state::<AppState>();
                let body = if name.is_empty() { None } else { st.ndi_inputs.frame(&st.ndi, &name, &url) };
                let resp = match body {
                    Some(jpeg) => tauri::http::Response::builder().status(200).header("Content-Type", "image/jpeg").header("Cache-Control", "no-store").body(jpeg),
                    None => tauri::http::Response::builder().status(404).header("Cache-Control", "no-store").body(Vec::new()),
                };
                responder.respond(resp.unwrap_or_default());
            });
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::Destroyed = event {
                    let app = window.app_handle().clone();
                    if let Some(st) = app.try_state::<AppState>() {
                        st.outputs.lock().unwrap().clear();
                        if let Some(r) = st.remote.lock().unwrap().take() {
                            r.remote.revoke_all();
                        }
                        st.music.lock().unwrap().take();
                    }
                    app.exit(0);
                }
            }
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
            identify_monitors,
            get_settings,
            set_setting,
            delete_song,
            list_backups,
            backup_now,
            inspect_backup,
            restore_backup,
            backups_folder,
            list_profiles,
            list_fonts,
            pulpit_notes,
            set_subtitles,
            set_camera,
            set_transpose,
            set_text_adjust,
            foreign_theme_review,
            foreign_theme_import,
            pulpit_dismiss,
            speech::speech_status,
            speech::speech_start,
            speech::speech_stop,
            speech::speech_dismiss,
            speech::speech_mics,
            speech::speech_install_vosk,
            speech::speech_install_whisper,
            speech::speech_test_recording,
            speech::speech_save_notes,
            speech::speech_set_gain,
            speech::speech_default_mic,
            speech::speech_history,
            speech::speech_history_read,
            speech::speech_history_delete,
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
            search_bible,
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
            remote::remote_follow,
            remote::remote_control_key,
            midi::midi_ports,
            midi::midi_use,
            midi::midi_learn,
            midi::midi_status,
            midi::midi_forget,
            remote::remote_control_key_get,
            remote::remote_new_pin,
            remote::remote_revoke,
            remote::remote_end_all,
            ndi_status,
            ndi_test_pattern,
            ndi_start,
            open_ndi_runtime_page,
            ndi_audio,
            set_window_look,
            show_main_window,
            ndi_sources,
            set_media_options,
            theme_rules,
            theme_gallery,
            quick_song,
            outline_deck,
            item_collections,
            bible_catalogue,
            lyrics_search,
            lyrics_words,
            lyrics_add,
            log_error,
            update_check,
            update_install,
            stream::stream_install_ffmpeg,
            stream::stream_start,
            stream::stream_stop,
            stream::stream_status,
            open_logs,
            diagnostic_report,
            sync_status,
            sync_set,
            sync_share_now,
            sync_take_now,
            bible_download,
            item_collection_create,
            item_collection_rename,
            item_collection_delete,
            item_collection_add,
            item_collection_remove,
            item_collection_items,
            save_theme_rules,
            resolve_theme,
            reveal_media,
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
