//! MIDI pads, keyboards and mixers as buttons: press "Learn" beside an action, press a
//! pad, and from then on that pad does that action (Go live, Next, Blank…), the same way
//! the phone remote and Companion do.

use crate::remote::Desk;
use crate::{err, AppState, Res};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use wp_remote::{Command, Host};

pub const ACTIONS: [&str; 9] = ["go-live", "next", "previous", "next-item", "previous-item", "blank", "logo", "clear", "resume"];

#[derive(Default)]
pub struct Midi {
    conn: Mutex<Option<midir::MidiInputConnection<()>>>,
    port: Mutex<Option<String>>,
    learning: Mutex<Option<String>>,
}

fn command_for(action: &str) -> Option<Command> {
    Some(match action {
        "go-live" => Command::GoLivePreview,
        "next" => Command::Next,
        "previous" => Command::Previous,
        "next-item" => Command::NextItem,
        "previous-item" => Command::PreviousItem,
        "blank" => Command::Blank,
        "logo" => Command::Logo,
        "clear" => Command::Clear,
        "resume" => Command::Resume,
        _ => return None,
    })
}

/// "note 60" or "cc 21" for a pressed pad, knob or key (releases are ignored).
pub fn control_of(msg: &[u8]) -> Option<String> {
    let (status, data1, data2) = (*msg.first()?, *msg.get(1)?, *msg.get(2).unwrap_or(&0));
    match status & 0xF0 {
        0x90 if data2 > 0 => Some(format!("note {data1}")),
        0xB0 if data2 > 0 => Some(format!("cc {data1}")),
        _ => None,
    }
}

fn load_map(st: &AppState) -> HashMap<String, String> {
    st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = 'midi_map'", [], |r| r.get::<_, String>(0)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn on_message(app: &AppHandle, msg: &[u8]) {
    let Some(control) = control_of(msg) else { return };
    let st = app.state::<AppState>();
    let learning = st.midi.learning.lock().unwrap().take();
    if let Some(action) = learning {
        let mut map = load_map(&st);
        map.retain(|_, c| *c != control); // one pad, one action
        map.insert(action.clone(), control.clone());
        let _ = st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('midi_map', ?1)", [serde_json::to_string(&map).unwrap_or_default()]);
        let _ = app.emit("midi-learned", serde_json::json!({ "action": action, "control": control }));
        return;
    }
    if let Some((action, _)) = load_map(&st).into_iter().find(|(_, c)| *c == control) {
        if let Some(cmd) = command_for(&action) {
            let _ = Desk(app.clone()).command(&cmd);
        }
    }
}

#[tauri::command]
pub fn midi_ports() -> Res<Vec<String>> {
    let input = midir::MidiInput::new("Worship Presenter").map_err(err)?;
    Ok(input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect())
}

/// Listens to a MIDI device (None stops); remembered for the next start.
#[tauri::command]
pub fn midi_use(app: AppHandle, st: State<'_, AppState>, port: Option<String>) -> Res<()> {
    *st.midi.conn.lock().unwrap() = None;
    *st.midi.port.lock().unwrap() = None;
    let conn = st.db.lock().unwrap();
    match &port {
        Some(p) => conn.execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('midi_port', ?1)", [p]).map_err(err)?,
        None => conn.execute("DELETE FROM setting WHERE key = 'midi_port'", []).map_err(err)?,
    };
    drop(conn);
    if let Some(name) = port {
        connect(&app, &name)?;
    }
    Ok(())
}

pub fn connect(app: &AppHandle, name: &str) -> Result<(), String> {
    let input = midir::MidiInput::new("Worship Presenter").map_err(err)?;
    let port = input.ports().into_iter().find(|p| input.port_name(p).ok().as_deref() == Some(name)).ok_or_else(|| format!("“{name}” is not connected."))?;
    let h = app.clone();
    let c = input.connect(&port, "worship-presenter", move |_, msg, _| on_message(&h, msg), ()).map_err(|e| format!("The MIDI device could not be opened ({e})."))?;
    let st = app.state::<AppState>();
    *st.midi.conn.lock().unwrap() = Some(c);
    *st.midi.port.lock().unwrap() = Some(name.to_string());
    Ok(())
}

/// The next pad pressed is learnt for this action.
#[tauri::command]
pub fn midi_learn(st: State<'_, AppState>, action: String) -> Res<()> {
    if !ACTIONS.contains(&action.as_str()) {
        return Err("Unknown action.".into());
    }
    *st.midi.learning.lock().unwrap() = Some(action);
    Ok(())
}

#[derive(serde::Serialize)]
pub struct MidiStatus {
    port: Option<String>,
    learning: Option<String>,
    map: HashMap<String, String>,
}

#[tauri::command]
pub fn midi_status(st: State<'_, AppState>) -> MidiStatus {
    MidiStatus { port: st.midi.port.lock().unwrap().clone(), learning: st.midi.learning.lock().unwrap().clone(), map: load_map(&st) }
}

#[tauri::command]
pub fn midi_forget(st: State<'_, AppState>, action: String) -> Res<()> {
    let mut map = load_map(&st);
    map.remove(&action);
    st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('midi_map', ?1)", [serde_json::to_string(&map).unwrap_or_default()]).map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn pads_knobs_and_releases() {
        assert_eq!(super::control_of(&[0x90, 60, 100]).as_deref(), Some("note 60"));
        assert_eq!(super::control_of(&[0x93, 36, 1]).as_deref(), Some("note 36"), "any channel");
        assert_eq!(super::control_of(&[0x90, 60, 0]), None, "note-on with no velocity is a release");
        assert_eq!(super::control_of(&[0x80, 60, 64]), None, "note-off");
        assert_eq!(super::control_of(&[0xB0, 21, 127]).as_deref(), Some("cc 21"));
        assert_eq!(super::control_of(&[0xB0, 21, 0]), None, "a button let go");
        assert_eq!(super::control_of(&[0xF8]), None, "clock ticks");
    }
}
