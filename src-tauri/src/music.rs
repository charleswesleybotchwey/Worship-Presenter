//! Background music commands. The player starts on first use, so a computer
//! with no sound output can still run a service; the playlist (media ids) and
//! the rules are kept in the profile's settings.

use crate::{err, AppState, Res};
use serde::Serialize;
use tauri::State;
use wp_audio::rodio_backend::Cmd;
use wp_audio::{Music, Rules, Status, Track};

fn setting(st: &AppState, key: &str) -> Option<String> {
    st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get::<_, String>(0)).ok()
}

fn save_setting(st: &AppState, key: &str, value: &str) {
    let _ = st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2)", [key, value]);
}

fn rules(st: &AppState) -> Rules {
    setting(st, "music_rules").and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
}

fn playlist(st: &AppState) -> Vec<Track> {
    let ids: Vec<i64> = setting(st, "music_playlist").and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
    let dir = st.profile_dir().join("media");
    let conn = st.db.lock().unwrap();
    ids.into_iter()
        .filter_map(|id| wp_core::media::get(&conn, &dir, id).ok())
        .filter(|m| m.kind == "audio" && !m.missing)
        .map(|m| Track { id: m.id, title: m.title, path: dir.join(&m.path) })
        .collect()
}

/// Starts the player if needed and runs `f` on it.
fn with<T>(st: &AppState, f: impl FnOnce(&Music) -> T) -> Res<T> {
    let mut g = st.music.lock().unwrap();
    if g.is_none() {
        let m = Music::start(rules(st))?;
        m.send(Cmd::Playlist(playlist(st)));
        if let Some(v) = setting(st, "music_volume").and_then(|v| v.parse::<f32>().ok()) {
            m.send(Cmd::Volume(v));
        }
        *g = Some(m);
    }
    Ok(f(g.as_ref().unwrap()))
}

/// Announcements went live before the player was ever used: start it if the rule says so.
pub fn autostart_for_announcements(st: &AppState) {
    if rules(st).with_announcements && !playlist(st).is_empty() {
        let _ = with(st, |m| m.send(Cmd::GoLive("announcement".into())));
    }
}

#[derive(Serialize)]
pub struct MusicStatus {
    started: bool,
    status: Status,
    rules: Rules,
    /// media ids in playlist order (also when the player has not started)
    playlist_ids: Vec<i64>,
}

#[tauri::command]
pub fn music_status(st: State<'_, AppState>) -> MusicStatus {
    let g = st.music.lock().unwrap();
    let status = g.as_ref().map(|m| m.status()).unwrap_or_default();
    let started = g.is_some();
    drop(g);
    MusicStatus { started, status, rules: rules(&st), playlist_ids: playlist(&st).iter().map(|t| t.id).collect() }
}

#[tauri::command]
pub fn music_set_playlist(st: State<'_, AppState>, media_ids: Vec<i64>) -> Res<()> {
    save_setting(&st, "music_playlist", &serde_json::to_string(&media_ids).map_err(err)?);
    let tracks = playlist(&st);
    if let Some(m) = st.music.lock().unwrap().as_ref() {
        m.send(Cmd::Playlist(tracks));
    }
    Ok(())
}

#[tauri::command]
pub fn music_play(st: State<'_, AppState>, index: Option<usize>) -> Res<()> {
    if playlist(&st).is_empty() {
        return Err("Add some music to the playlist first.".into());
    }
    with(&st, |m| m.send(Cmd::Play(index)))
}

#[tauri::command]
pub fn music_pause(st: State<'_, AppState>, paused: bool) -> Res<()> {
    with(&st, |m| m.send(Cmd::Pause(paused)))
}

#[tauri::command]
pub fn music_next(st: State<'_, AppState>) -> Res<()> {
    with(&st, |m| m.send(Cmd::Next))
}

#[tauri::command]
pub fn music_previous(st: State<'_, AppState>) -> Res<()> {
    with(&st, |m| m.send(Cmd::Previous))
}

/// Fades out over 2.5 s, then stops.
#[tauri::command]
pub fn music_stop(st: State<'_, AppState>) {
    if let Some(m) = st.music.lock().unwrap().as_ref() {
        m.send(Cmd::Stop);
    }
}

#[tauri::command]
pub fn music_volume(st: State<'_, AppState>, volume: f32) -> Res<()> {
    save_setting(&st, "music_volume", &volume.to_string());
    with(&st, |m| m.send(Cmd::Volume(volume)))
}

#[tauri::command]
pub fn music_unduck(st: State<'_, AppState>) {
    if let Some(m) = st.music.lock().unwrap().as_ref() {
        m.send(Cmd::Unduck);
    }
}

#[tauri::command]
pub fn music_set_rules(st: State<'_, AppState>, rules: Rules) -> Res<()> {
    save_setting(&st, "music_rules", &serde_json::to_string(&rules).map_err(err)?);
    if let Some(m) = st.music.lock().unwrap().as_ref() {
        m.send(Cmd::Rules(rules));
    }
    Ok(())
}
