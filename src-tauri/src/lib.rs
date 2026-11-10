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
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use wp_core::library;
use wp_core::live::{Command, LiveState, Mode};
use wp_core::splitter::{self, LangMode, Options, Slide};

/// What is loaded in the Live pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgramItem {
    Song { song_id: i64, title: String },
    Scripture { reference: String, versions: Vec<String> },
}

/// Payload every window receives on "live-state".
#[derive(Debug, Clone, Serialize)]
pub struct Broadcast {
    pub state: LiveState,
    pub item: Option<ProgramItem>,
    pub slides: Vec<Slide>,
    pub caption: Option<String>,
}

pub struct AppState {
    db: Mutex<Connection>,
    live: Mutex<LiveState>,
    item: Mutex<Option<ProgramItem>>,
    slides: Mutex<Vec<Slide>>,
    caption: Mutex<Option<String>>,
    state_path: PathBuf,
}

type Res<T> = Result<T, String>;
fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn build_slides(conn: &Connection, item: &ProgramItem, live: &LiveState) -> Res<(Vec<Slide>, Option<String>)> {
    match item {
        ProgramItem::Song { song_id, title } => {
            let sections = library::song_sections(conn, *song_id).map_err(err)?;
            let langs = library::song_langs(conn, *song_id).map_err(err)?;
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
        ProgramItem::Scripture { reference, versions } => {
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
    };
    let _ = payload.state.save(&s.state_path);
    let _ = app.emit("live-state", payload);
}

#[tauri::command]
fn search_songs(st: State<'_, AppState>, query: String) -> Res<Vec<library::SongHit>> {
    library::search_songs(&st.db.lock().unwrap(), &query, 60).map_err(err)
}

#[tauri::command]
fn preview_song(st: State<'_, AppState>, song_id: i64, title: String) -> Res<Vec<Slide>> {
    let live = st.live.lock().unwrap().clone();
    build_slides(&st.db.lock().unwrap(), &ProgramItem::Song { song_id, title }, &live).map(|r| r.0)
}

#[tauri::command]
fn bible_versions(st: State<'_, AppState>) -> Res<Vec<library::Version>> {
    library::versions(&st.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
fn preview_scripture(st: State<'_, AppState>, reference: String, versions: Vec<String>) -> Res<(Vec<Slide>, Option<String>)> {
    let live = st.live.lock().unwrap().clone();
    build_slides(&st.db.lock().unwrap(), &ProgramItem::Scripture { reference, versions }, &live)
}

/// Sends an item to air at a slide.
#[tauri::command]
fn go_live(app: AppHandle, st: State<'_, AppState>, item: ProgramItem, slide_idx: usize) -> Res<LiveState> {
    let live0 = st.live.lock().unwrap().clone();
    let (slides, caption) = build_slides(&st.db.lock().unwrap(), &item, &live0)?;
    let idx = slide_idx.min(slides.len().saturating_sub(1));
    let pos = slides.get(idx).map(splitter::position_of);
    let next = live0.apply(Command::GoLive { item_idx: 0, slide_idx: idx, position: pos });
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
    let idx = match (slide_idx, delta) {
        (Some(i), _) => i.min(n - 1),
        (None, Some(d)) => (cur.slide_idx as i64 + d).clamp(0, n as i64 - 1) as usize,
        _ => cur.slide_idx,
    };
    let pos = st.slides.lock().unwrap().get(idx).map(splitter::position_of);
    let next = cur.apply(Command::Show { slide_idx: idx, position: pos });
    *st.live.lock().unwrap() = next.clone();
    broadcast(&app, &st);
    Ok(next)
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

/// Opens (or moves) an output window full-screen on a display. With no display
/// it opens in a normal window on the control screen, so there is always a picture.
#[tauri::command]
fn open_output(app: AppHandle, role: String, display_index: Option<usize>) -> Res<()> {
    let label = format!("output-{role}");
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.close();
    }
    let monitors = app.available_monitors().map_err(err)?;
    let mut b = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(format!("index.html#/output/{role}").into()))
        .title(format!("Worship Presenter – {role} output"))
        .decorations(false)
        .resizable(true)
        .focused(false)
        .skip_taskbar(true);
    match display_index.and_then(|i| monitors.get(i)) {
        Some(m) => {
            let scale = m.scale_factor();
            let pos = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            b = b.position(pos.x, pos.y).inner_size(size.width, size.height).always_on_top(true);
            let w = b.build().map_err(err)?;
            w.set_fullscreen(true).map_err(err)?;
        }
        None => {
            b.inner_size(960.0, 540.0).decorations(true).skip_taskbar(false).build().map_err(err)?;
        }
    }
    Ok(())
}

#[tauri::command]
fn close_output(app: AppHandle, role: String) {
    if let Some(w) = app.get_webview_window(&format!("output-{role}")) {
        let _ = w.close();
    }
}

/// Imports a library file chosen in the Import dialog and returns the counts.
#[tauri::command]
fn import_file(st: State<'_, AppState>, kind: String, path: String, code: Option<String>) -> Res<wp_core::import::Report> {
    let p = PathBuf::from(&path);
    let conn = st.db.lock().unwrap();
    let r = match kind.as_str() {
        "ewb" => wp_core::import::ewb::import(&conn, &p, code.as_deref()),
        "bibleshow" => wp_core::import::bibleshow::import_file(&conn, &p, code.as_deref()),
        "ew-songs" => {
            // `path` is the EasyWorship Songs folder holding Songs.db and SongWords.db
            let name = p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| "EasyWorship songs".into());
            wp_core::import::ew_songs::import(&conn, &p.join("Songs.db"), &p.join("SongWords.db"), &name)
        }
        other => return Err(format!("Unknown import type {other}")),
    };
    r.map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?.join("profiles").join("default");
            std::fs::create_dir_all(dir.join("media"))?;
            let conn = wp_core::db::open(&dir.join("library.db"))?;
            if !wp_core::db::integrity_ok(&conn)? {
                // Keep the damaged file; the control window shows recovery options.
                let _ = app.emit("library-damaged", dir.join("library.db"));
            }
            let project_on_restart = conn
                .query_row("SELECT value FROM setting WHERE key = 'project_on_restart'", [], |r| r.get::<_, String>(0))
                .map(|v| v == "true")
                .unwrap_or(false);
            let state_path = dir.join("state.json");
            let live = LiveState::load(&state_path, project_on_restart);
            app.manage(AppState {
                db: Mutex::new(conn),
                live: Mutex::new(live),
                item: Mutex::new(None),
                slides: Mutex::new(Vec::new()),
                caption: Mutex::new(None),
                state_path,
            });
            // Heartbeat: outputs switch to their fallback image if this stops.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                let rev = handle.try_state::<AppState>().map(|s| s.live.lock().unwrap().revision).unwrap_or(0);
                let _ = handle.emit("heartbeat", rev);
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
            import_file,
        ])
        .run(tauri::generate_context!())
        .expect("Worship Presenter failed to start");
}
