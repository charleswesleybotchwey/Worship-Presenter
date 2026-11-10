//! The LAN remote's link to the desktop app. Phones get the same live state as the
//! output windows, and their commands go through the same functions as the desk's
//! buttons, so held output, latching and every other rule applies unchanged.

use crate::{err, AppState, ProgramItem, Res};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use wp_core::live::Mode;
use wp_remote::{Command, Host, Remote};

pub struct Running {
    pub remote: Arc<Remote>,
    stop: tokio::sync::oneshot::Sender<()>,
    pub addr: SocketAddr,
}

struct Desk(AppHandle);

impl Desk {
    fn service_items(&self) -> (Option<String>, Vec<wp_core::schedule::Item>) {
        let st = self.0.state::<AppState>();
        let Some(id) = *st.schedule_id.lock().unwrap() else { return (None, vec![]) };
        let loaded = wp_core::schedule::load(&st.db.lock().unwrap(), id);
        match loaded {
            Ok(s) => (Some(s.name), s.items),
            Err(_) => (None, vec![]),
        }
    }
}

fn title_of(i: &wp_core::schedule::Item) -> String {
    use wp_core::schedule::Item::*;
    match i {
        Song { title, .. } | Media { title, .. } | Announcement { title, .. } => title.clone(),
        Scripture { reference, versions, .. } => format!("{reference} · {}", versions.join(" + ")),
    }
}

/// Schedule items and live items share their JSON shape.
fn to_program(i: &wp_core::schedule::Item) -> Option<ProgramItem> {
    serde_json::to_value(i).ok().and_then(|v| serde_json::from_value(v).ok())
}

impl Host for Desk {
    fn snapshot(&self) -> serde_json::Value {
        let st = self.0.state::<AppState>();
        let live = st.live.lock().unwrap().clone();
        let item = st.item.lock().unwrap().clone();
        let slides = st.slides.lock().unwrap().clone();
        let caption = st.caption.lock().unwrap().clone();
        let (service, items) = self.service_items();
        let live_json = item.as_ref().and_then(|i| serde_json::to_value(i).ok());
        let live_index = live_json.as_ref().and_then(|lj| {
            items.iter().position(|it| {
                let v = serde_json::to_value(it).unwrap_or_default();
                // same item: same kind and same id or reference (looks and edits aside)
                ["kind", "song_id", "reference", "media_id", "presentation_id"].iter().all(|k| v.get(k) == lj.get(k))
            })
        });
        let title = item.as_ref().map(|i| match i {
            ProgramItem::Scripture { .. } => caption.clone().unwrap_or_default(),
            ProgramItem::Song { title, .. } | ProgramItem::Media { title, .. } | ProgramItem::Announcement { title, .. } => title.clone(),
        });
        serde_json::json!({
            "server_now_ms": wp_core::cues::now_ms(),
            "connected": true,
            "service": service,
            "item": item.as_ref().map(|i| serde_json::json!({ "kind": serde_json::to_value(i).ok().and_then(|v| v.get("kind").cloned()), "title": title })),
            "mode": live.mode,
            "slide_idx": live.slide_idx,
            "slides": slides.iter().map(|s| serde_json::json!({ "label": s.label, "primary": s.primary, "second": s.second })).collect::<Vec<_>>(),
            "timer": live.timer,
            "alert": live.alert,
            "stage_message": live.stage_message,
            "repeat": live.repeat,
            "lining_out": live.lining_out,
            "schedule": items.iter().map(|i| serde_json::json!({ "title": title_of(i) })).collect::<Vec<_>>(),
            "live_index": live_index,
        })
    }

    fn command(&self, cmd: &Command) -> Result<(), String> {
        let app = self.0.clone();
        let st = || app.state::<AppState>();
        match cmd {
            Command::Next => crate::show_slide(app.clone(), st(), None, Some(1)).map(drop),
            Command::Previous => crate::show_slide(app.clone(), st(), None, Some(-1)).map(drop),
            Command::Show { index } => crate::show_slide(app.clone(), st(), Some(*index), None).map(drop),
            Command::Blank => crate::set_mode(app.clone(), st(), Mode::Blank).map(drop),
            Command::Logo => crate::set_mode(app.clone(), st(), Mode::Logo).map(drop),
            Command::Clear => crate::set_mode(app.clone(), st(), Mode::Clear).map(drop),
            Command::Resume => crate::set_mode(app.clone(), st(), Mode::Live).map(drop),
            Command::GoLiveItem { index } => {
                let (_, items) = self.service_items();
                let it = items.get(*index).ok_or("That item is no longer in the service order.")?;
                let p = to_program(it).ok_or("That item cannot be sent live.")?;
                crate::go_live(app.clone(), st(), p, 0).map(drop)
            }
        }
    }
}

/// The computer's address on the church network (no traffic is sent).
fn lan_ip() -> Option<IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:80").ok()?;
    s.local_addr().ok().map(|a| a.ip()).filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
}

#[derive(Serialize)]
pub struct RemoteStatus {
    running: bool,
    /// Addresses phones can use, e.g. http://192.168.1.10:7300/
    url: Option<String>,
    stage_url: Option<String>,
    pin: Option<String>,
    qr_svg: Option<String>,
    lan_ip: Option<String>,
    sessions: Vec<wp_remote::SessionInfo>,
    log: Vec<wp_remote::LogLine>,
}

#[tauri::command]
pub fn remote_status(st: tauri::State<'_, AppState>) -> RemoteStatus {
    let r = st.remote.lock().unwrap();
    let ip = lan_ip();
    match r.as_ref() {
        None => RemoteStatus { running: false, url: None, stage_url: None, pin: None, qr_svg: None, lan_ip: ip.map(|i| i.to_string()), sessions: vec![], log: vec![] },
        Some(x) => {
            let host = if x.addr.ip().is_unspecified() { ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)) } else { x.addr.ip() };
            let url = format!("http://{host}:{}/", x.addr.port());
            RemoteStatus {
                running: true,
                stage_url: Some(format!("{url}stage")),
                pin: Some(x.remote.pin()),
                qr_svg: Some(wp_remote::qr_svg(&url)),
                url: Some(url),
                lan_ip: ip.map(|i| i.to_string()),
                sessions: x.remote.sessions(),
                log: x.remote.log().into_iter().rev().take(30).collect(),
            }
        }
    }
}

/// Starts the remote on this computer's network address (or every address).
#[tauri::command]
pub async fn remote_start(app: AppHandle, everywhere: bool, port: u16) -> Res<()> {
    let st = app.state::<AppState>();
    if st.remote.lock().unwrap().is_some() {
        return Ok(());
    }
    let ip = if everywhere { IpAddr::V4(Ipv4Addr::UNSPECIFIED) } else { lan_ip().ok_or("This computer is not on a network. Connect to the church Wi-Fi first.")? };
    let remote = Remote::new(Arc::new(Desk(app.clone())));
    let (tx, rx) = tokio::sync::oneshot::channel();
    let addr = remote
        .clone()
        .serve(SocketAddr::new(ip, port), rx)
        .await
        .map_err(|e| format!("The remote could not start on port {port} ({e}). Try another port."))?;
    *st.remote.lock().unwrap() = Some(Running { remote, stop: tx, addr });
    Ok(())
}

#[tauri::command]
pub fn remote_stop(st: tauri::State<'_, AppState>) {
    if let Some(r) = st.remote.lock().unwrap().take() {
        r.remote.revoke_all();
        let _ = r.stop.send(());
    }
}

#[tauri::command]
pub fn remote_new_pin(st: tauri::State<'_, AppState>) -> Res<String> {
    st.remote.lock().unwrap().as_ref().map(|r| r.remote.new_pin()).ok_or_else(|| err("The remote is off."))
}

#[tauri::command]
pub fn remote_revoke(st: tauri::State<'_, AppState>, id: String) {
    if let Some(r) = st.remote.lock().unwrap().as_ref() {
        r.remote.revoke(&id);
    }
}

/// End of service: every phone and tablet must join again.
#[tauri::command]
pub fn remote_end_all(st: tauri::State<'_, AppState>) {
    if let Some(r) = st.remote.lock().unwrap().as_ref() {
        r.remote.revoke_all();
    }
}
