//! Worship Presenter LAN remote.
//!
//! A small web server on the church network: phones and tablets join with a PIN
//! (or by scanning the QR code), as an operator (can drive the screens), a stage
//! viewer (the stage display in a browser) or read-only (sees what is live).
//! Off by default; every control action is logged; sessions can be revoked.
//!
//! The server knows nothing about songs or outputs: the desktop app implements
//! `Host`, and calls `publish` whenever the live state changes.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::broadcast;

const REMOTE_HTML: &str = include_str!("../web/remote.html");
const STAGE_HTML: &str = include_str!("../web/stage.html");
const ICON_180: &[u8] = include_bytes!("../web/icon-180.png");
const ICON_64: &[u8] = include_bytes!("../web/icon-64.png");

fn png(bytes: &'static [u8]) -> Response {
    ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "max-age=86400")], bytes).into_response()
}

/// What the phones may ask for. Every one goes through the desktop app's own
/// checks (held output, rehearsal…), exactly as if pressed at the desk.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Next,
    Previous,
    Show { index: usize },
    Blank,
    Logo,
    Clear,
    Resume,
    /// Send service-order item `index` live from its first slide.
    GoLiveItem { index: usize },
}

pub trait Host: Send + Sync + 'static {
    /// Current state for phones: live item, slides, mode, service order, timers.
    fn snapshot(&self) -> serde_json::Value;
    fn command(&self, cmd: &Command) -> Result<(), String>;
    /// A note sent from the pulpit; it waits for the operator (never shown by itself).
    fn note(&self, device: &str, text: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Operator,
    Stage,
    Viewer,
    /// Sends sermon points to the operator.
    Preacher,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub role: Role,
    pub device: String,
    pub ip: String,
    pub joined_secs_ago: u64,
    pub idle_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    /// Seconds since 1970.
    pub at: u64,
    pub device: String,
    pub action: String,
    pub ok: bool,
}

struct Session {
    role: Role,
    device: String,
    ip: IpAddr,
    joined: Instant,
    seen: Instant,
    last_cmd: Option<Instant>,
}

/// Sessions end after this long without contact (a service, with room to spare).
const IDLE_LIMIT: Duration = Duration::from_secs(6 * 3600);
/// Wrong PINs allowed per address before a pause.
const MAX_FAILURES: u32 = 5;
/// Wrong PINs from all devices together per minute before joining pauses for everyone
/// (stops guessing spread over many addresses).
const MAX_FAILURES_ALL: u32 = 20;
/// Devices joined at once.
const MAX_SESSIONS: usize = 30;
const LOCKOUT: Duration = Duration::from_secs(60);
/// At most this many commands per second from one phone.
const MIN_GAP: Duration = Duration::from_millis(150);

pub struct Remote {
    host: Arc<dyn Host>,
    pin: Mutex<String>,
    sessions: Mutex<HashMap<String, Session>>,
    failures: Mutex<HashMap<IpAddr, (u32, Instant)>>,
    failures_all: Mutex<(u32, Instant)>,
    log: Mutex<VecDeque<LogLine>>,
    tx: broadcast::Sender<String>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn random_pin() -> String {
    use rand::Rng;
    format!("{:06}", rand::thread_rng().gen_range(0..1_000_000))
}

/// Compares without stopping at the first different character (no timing hint).
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Remote {
    pub fn new(host: Arc<dyn Host>) -> Arc<Remote> {
        let (tx, _) = broadcast::channel(64);
        Arc::new(Remote {
            host,
            pin: Mutex::new(random_pin()),
            sessions: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
            failures_all: Mutex::new((0, Instant::now())),
            log: Mutex::new(VecDeque::new()),
            tx,
        })
    }

    pub fn pin(&self) -> String {
        self.pin.lock().unwrap().clone()
    }

    /// New PIN; phones already joined stay joined.
    pub fn new_pin(&self) -> String {
        let p = random_pin();
        *self.pin.lock().unwrap() = p.clone();
        p
    }

    /// Tell every connected phone the state changed.
    pub fn publish(&self) {
        let _ = self.tx.send(self.host.snapshot().to_string());
    }

    pub fn sessions(&self) -> Vec<SessionInfo> {
        let mut v: Vec<SessionInfo> = self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .map(|(id, s)| SessionInfo {
                id: id[..8].to_string(),
                role: s.role,
                device: s.device.clone(),
                ip: s.ip.to_string(),
                joined_secs_ago: s.joined.elapsed().as_secs(),
                idle_secs: s.seen.elapsed().as_secs(),
            })
            .collect();
        v.sort_by_key(|s| s.joined_secs_ago);
        v
    }

    /// Revokes by the short id shown in the desktop app.
    pub fn revoke(&self, short_id: &str) {
        self.sessions.lock().unwrap().retain(|k, _| !k.starts_with(short_id));
        self.publish();
    }

    /// "End of service": every phone must join again.
    pub fn revoke_all(&self) {
        self.sessions.lock().unwrap().clear();
        self.publish();
    }

    pub fn log(&self) -> Vec<LogLine> {
        self.log.lock().unwrap().iter().cloned().collect()
    }

    fn record(&self, device: &str, action: String, ok: bool) {
        let mut l = self.log.lock().unwrap();
        l.push_back(LogLine { at: now_secs(), device: device.to_string(), action, ok });
        while l.len() > 200 {
            l.pop_front();
        }
    }

    fn session(&self, token: &str) -> Option<(Role, String)> {
        let mut s = self.sessions.lock().unwrap();
        s.retain(|_, x| x.seen.elapsed() < IDLE_LIMIT);
        let x = s.get_mut(token)?;
        x.seen = Instant::now();
        Some((x.role, x.device.clone()))
    }

    pub fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/", get(|| async { Html(REMOTE_HTML) }))
            .route("/stage", get(|| async { Html(STAGE_HTML) }))
            .route("/icon-180.png", get(|| async { png(ICON_180) }))
            .route("/icon-64.png", get(|| async { png(ICON_64) }))
            .route("/api/join", post(join))
            .route("/api/snapshot", get(snapshot))
            .route("/api/command", post(command))
            .route("/api/note", post(note))
            .route("/ws", get(ws))
            .with_state(self.clone())
    }

    /// Serves on `addr` until `stop` resolves. Returns the bound address.
    pub async fn serve(self: Arc<Self>, addr: SocketAddr, stop: tokio::sync::oneshot::Receiver<()>) -> std::io::Result<SocketAddr> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let bound = listener.local_addr()?;
        let app = self.router().into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = stop.await;
                })
                .await;
        });
        Ok(bound)
    }
}

/// QR code (SVG) for the join address, shown in the desktop app.
pub fn qr_svg(url: &str) -> String {
    match qrcode::QrCode::new(url.as_bytes()) {
        Ok(code) => code.render::<qrcode::render::svg::Color>().min_dimensions(200, 200).quiet_zone(true).build(),
        Err(_) => String::new(),
    }
}

/// Browsers send Origin on POST and WebSocket; it must be this server, so another
/// web page open on a phone cannot drive the screens.
fn same_origin(h: &HeaderMap) -> bool {
    let Some(origin) = h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else { return true };
    let Some(host) = h.get(header::HOST).and_then(|v| v.to_str().ok()) else { return false };
    origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")) == Some(host)
}

fn bearer(h: &HeaderMap) -> String {
    h.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("").to_string()
}

fn fail(code: StatusCode, msg: &str) -> Response {
    (code, Json(serde_json::json!({ "error": msg }))).into_response()
}

#[derive(Deserialize)]
struct JoinReq {
    pin: String,
    role: Role,
    #[serde(default)]
    device: String,
}

async fn join(State(r): State<Arc<Remote>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, h: HeaderMap, Json(req): Json<JoinReq>) -> Response {
    if !same_origin(&h) {
        return fail(StatusCode::FORBIDDEN, "Open the remote from the address shown on the computer.");
    }
    let ip = addr.ip();
    {
        let mut f = r.failures.lock().unwrap();
        if let Some((n, since)) = f.get(&ip) {
            if *n >= MAX_FAILURES && since.elapsed() < LOCKOUT {
                return fail(StatusCode::TOO_MANY_REQUESTS, "Too many wrong PINs. Wait a minute, then try again.");
            }
            if since.elapsed() >= LOCKOUT {
                f.remove(&ip);
            }
        }
    }
    {
        let mut all = r.failures_all.lock().unwrap();
        if all.1.elapsed() >= LOCKOUT {
            *all = (0, Instant::now());
        }
        if all.0 >= MAX_FAILURES_ALL {
            return fail(StatusCode::TOO_MANY_REQUESTS, "Too many wrong PINs on this network. Joining is paused for a minute.");
        }
    }
    if !same_secret(req.pin.trim(), &r.pin()) {
        r.failures_all.lock().unwrap().0 += 1;
        let mut f = r.failures.lock().unwrap();
        let e = f.entry(ip).or_insert((0, Instant::now()));
        e.0 += 1;
        e.1 = Instant::now();
        drop(f);
        r.record(&req.device, format!("wrong PIN from {ip}"), false);
        return fail(StatusCode::UNAUTHORIZED, "That PIN is not right. It is shown on the computer under Settings, Remote.");
    }
    r.failures.lock().unwrap().remove(&ip);
    if r.sessions.lock().unwrap().len() >= MAX_SESSIONS {
        return fail(StatusCode::TOO_MANY_REQUESTS, "Too many devices are joined. Sign some out on the computer first.");
    }
    let token = random_token();
    let device: String = req.device.chars().filter(|c| !c.is_control()).take(40).collect();
    let device = if device.trim().is_empty() { format!("Phone at {ip}") } else { device.trim().to_string() };
    r.sessions.lock().unwrap().insert(token.clone(), Session { role: req.role, device: device.clone(), ip, joined: Instant::now(), seen: Instant::now(), last_cmd: None });
    r.record(&device, format!("joined as {:?}", req.role).to_lowercase(), true);
    Json(serde_json::json!({ "token": token, "role": req.role })).into_response()
}

async fn snapshot(State(r): State<Arc<Remote>>, h: HeaderMap) -> Response {
    match r.session(&bearer(&h)) {
        Some(_) => Json(r.host.snapshot()).into_response(),
        None => fail(StatusCode::UNAUTHORIZED, "Join again with the PIN."),
    }
}

async fn command(State(r): State<Arc<Remote>>, h: HeaderMap, Json(cmd): Json<Command>) -> Response {
    if !same_origin(&h) {
        return fail(StatusCode::FORBIDDEN, "Open the remote from the address shown on the computer.");
    }
    let token = bearer(&h);
    let Some((role, device)) = r.session(&token) else { return fail(StatusCode::UNAUTHORIZED, "Join again with the PIN.") };
    if role != Role::Operator {
        return fail(StatusCode::FORBIDDEN, "This phone can watch but not control. Join as an operator.");
    }
    {
        let mut s = r.sessions.lock().unwrap();
        let Some(x) = s.get_mut(&token) else { return fail(StatusCode::UNAUTHORIZED, "Join again with the PIN.") };
        if x.last_cmd.map_or(false, |t| t.elapsed() < MIN_GAP) {
            return fail(StatusCode::TOO_MANY_REQUESTS, "Too fast. Press once.");
        }
        x.last_cmd = Some(Instant::now());
    }
    let label = serde_json::to_value(&cmd).ok().and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(str::to_string)).unwrap_or_default();
    let detail = match &cmd {
        Command::Show { index } | Command::GoLiveItem { index } => format!("{label} {}", index + 1),
        _ => label,
    };
    match r.host.command(&cmd) {
        Ok(()) => {
            r.record(&device, detail, true);
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => {
            r.record(&device, format!("{detail}: {e}"), false);
            fail(StatusCode::CONFLICT, &e)
        }
    }
}

#[derive(Deserialize)]
struct NoteReq {
    text: String,
}

/// The preacher (or an operator) sends a point; it goes to the desk to be shown or not.
async fn note(State(r): State<Arc<Remote>>, h: HeaderMap, Json(req): Json<NoteReq>) -> Response {
    if !same_origin(&h) {
        return fail(StatusCode::FORBIDDEN, "Open the remote from the address shown on the computer.");
    }
    let token = bearer(&h);
    let Some((role, device)) = r.session(&token) else { return fail(StatusCode::UNAUTHORIZED, "Join again with the PIN.") };
    if !matches!(role, Role::Preacher | Role::Operator) {
        return fail(StatusCode::FORBIDDEN, "Join as Pastor's notes to send points.");
    }
    let text: String = req.text.chars().filter(|c| !c.is_control() || *c == '\n').take(600).collect();
    if text.trim().is_empty() {
        return fail(StatusCode::BAD_REQUEST, "Write something first.");
    }
    {
        let mut s = r.sessions.lock().unwrap();
        if let Some(x) = s.get_mut(&token) {
            if x.last_cmd.map_or(false, |t| t.elapsed() < Duration::from_secs(2)) {
                return fail(StatusCode::TOO_MANY_REQUESTS, "Sent already. Wait a moment.");
            }
            x.last_cmd = Some(Instant::now());
        }
    }
    match r.host.note(&device, text.trim()) {
        Ok(()) => {
            r.record(&device, format!("sent a note ({} letters)", text.trim().chars().count()), true);
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => fail(StatusCode::CONFLICT, &e),
    }
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws(State(r): State<Arc<Remote>>, h: HeaderMap, Query(q): Query<WsQuery>, up: WebSocketUpgrade) -> Response {
    if !same_origin(&h) {
        return fail(StatusCode::FORBIDDEN, "Wrong origin.");
    }
    if r.session(&q.token).is_none() {
        return fail(StatusCode::UNAUTHORIZED, "Join again with the PIN.");
    }
    up.on_upgrade(move |socket| push(r, q.token, socket))
}

/// Sends a full snapshot on connect and on every change; ends when the session is revoked.
async fn push(r: Arc<Remote>, token: String, mut socket: WebSocket) {
    let mut rx = r.tx.subscribe();
    if socket.send(Message::Text(r.host.snapshot().to_string().into())).await.is_err() {
        return;
    }
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(text) => {
                    if r.session(&token).is_none() || socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // a slow phone skips to the newest state
                    if socket.send(Message::Text(r.host.snapshot().to_string().into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            },
            _ = tick.tick() => {
                if r.session(&token).is_none() || socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => if !matches!(incoming, Some(Ok(_))) { break },
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    struct Fake(Mutex<Vec<Command>>);
    impl Host for Fake {
        fn snapshot(&self) -> serde_json::Value {
            serde_json::json!({ "title": "MHB 1", "slide": 0 })
        }
        fn note(&self, _device: &str, text: &str) -> Result<(), String> {
            self.0.lock().unwrap().push(Command::Show { index: text.len() });
            Ok(())
        }
        fn command(&self, cmd: &Command) -> Result<(), String> {
            if *cmd == Command::Next && self.0.lock().unwrap().contains(&Command::Blank) {
                return Err("Output is held. Choose Resume output first.".into());
            }
            self.0.lock().unwrap().push(cmd.clone());
            Ok(())
        }
    }

    fn app() -> (Arc<Remote>, Arc<Fake>, Router) {
        let fake = Arc::new(Fake(Mutex::new(vec![])));
        let r = Remote::new(fake.clone());
        let router = r.router().layer(MockConnectInfo(SocketAddr::from(([192, 168, 1, 20], 5555))));
        (r, fake, router)
    }

    async fn call(app: &Router, method: &str, uri: &str, token: Option<&str>, origin: Option<&str>, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
        let mut b = Request::builder().method(method).uri(uri).header(header::HOST, "192.168.1.10:7300").header(header::CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        if let Some(o) = origin {
            b = b.header(header::ORIGIN, o);
        }
        let res = app.clone().oneshot(b.body(Body::from(body.to_string())).unwrap()).await.unwrap();
        let code = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (code, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    #[tokio::test]
    async fn join_roles_and_commands() {
        let (r, fake, app) = app();
        let pin = r.pin();
        let (c, _) = call(&app, "POST", "/api/join", None, None, serde_json::json!({ "pin": "000000x", "role": "operator" })).await;
        assert_eq!(c, StatusCode::UNAUTHORIZED);
        let (c, v) = call(&app, "POST", "/api/join", None, Some("http://192.168.1.10:7300"), serde_json::json!({ "pin": pin, "role": "operator", "device": "Kofi's phone" })).await;
        assert_eq!(c, StatusCode::OK);
        let op = v["token"].as_str().unwrap().to_string();
        let (_, v) = call(&app, "POST", "/api/join", None, None, serde_json::json!({ "pin": pin, "role": "viewer" })).await;
        let viewer = v["token"].as_str().unwrap().to_string();

        let (c, v) = call(&app, "GET", "/api/snapshot", Some(&viewer), None, serde_json::json!(null)).await;
        assert_eq!((c, v["title"].as_str()), (StatusCode::OK, Some("MHB 1")));
        let (c, _) = call(&app, "POST", "/api/command", Some(&viewer), None, serde_json::json!({ "cmd": "next" })).await;
        assert_eq!(c, StatusCode::FORBIDDEN, "read-only cannot control");
        let (c, _) = call(&app, "POST", "/api/command", Some(&op), None, serde_json::json!({ "cmd": "show", "index": 2 })).await;
        assert_eq!(c, StatusCode::OK);
        let (c, _) = call(&app, "POST", "/api/command", Some(&op), None, serde_json::json!({ "cmd": "next" })).await;
        assert_eq!(c, StatusCode::TOO_MANY_REQUESTS, "two presses within 150 ms");
        tokio::time::sleep(MIN_GAP).await;
        call(&app, "POST", "/api/command", Some(&op), None, serde_json::json!({ "cmd": "blank" })).await;
        tokio::time::sleep(MIN_GAP).await;
        let (c, v) = call(&app, "POST", "/api/command", Some(&op), None, serde_json::json!({ "cmd": "next" })).await;
        assert_eq!(c, StatusCode::CONFLICT, "the desktop's own rules still apply");
        assert!(v["error"].as_str().unwrap().contains("Resume"));
        assert_eq!(*fake.0.lock().unwrap(), vec![Command::Show { index: 2 }, Command::Blank]);
        let log = r.log();
        assert!(log.iter().any(|l| l.device == "Kofi's phone" && l.action == "show 3" && l.ok));
        assert!(log.iter().any(|l| !l.ok && l.action.starts_with("wrong PIN")));

        r.revoke_all();
        let (c, _) = call(&app, "GET", "/api/snapshot", Some(&op), None, serde_json::json!(null)).await;
        assert_eq!(c, StatusCode::UNAUTHORIZED, "End of service signs every phone out");
    }

    #[tokio::test]
    async fn preacher_sends_notes_but_cannot_drive() {
        let (r, fake, app) = app();
        let (_, v) = call(&app, "POST", "/api/join", None, None, serde_json::json!({ "pin": r.pin(), "role": "preacher", "device": "Pastor" })).await;
        let t = v["token"].as_str().unwrap().to_string();
        let (c, _) = call(&app, "POST", "/api/note", Some(&t), None, serde_json::json!({ "text": "Point 1: God is faithful" })).await;
        assert_eq!(c, StatusCode::OK);
        let (c, _) = call(&app, "POST", "/api/note", Some(&t), None, serde_json::json!({ "text": "again" })).await;
        assert_eq!(c, StatusCode::TOO_MANY_REQUESTS, "two notes within 2 s");
        let (c, _) = call(&app, "POST", "/api/command", Some(&t), None, serde_json::json!({ "cmd": "next" })).await;
        assert_eq!(c, StatusCode::FORBIDDEN, "notes only, no driving the screens");
        assert_eq!(fake.0.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn wrong_pins_lock_out_and_foreign_pages_are_refused() {
        let (r, _f, app) = app();
        for _ in 0..MAX_FAILURES {
            call(&app, "POST", "/api/join", None, None, serde_json::json!({ "pin": "1", "role": "operator" })).await;
        }
        let (c, _) = call(&app, "POST", "/api/join", None, None, serde_json::json!({ "pin": r.pin(), "role": "operator" })).await;
        assert_eq!(c, StatusCode::TOO_MANY_REQUESTS, "even the right PIN waits after 5 wrong ones");
        let (c, _) = call(&app, "POST", "/api/join", None, Some("http://evil.example"), serde_json::json!({ "pin": r.pin(), "role": "operator" })).await;
        assert_eq!(c, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn guessing_from_many_addresses_is_paused() {
        let fake = Arc::new(Fake(Mutex::new(vec![])));
        let r = Remote::new(fake);
        for n in 0..MAX_FAILURES_ALL {
            let app = r.router().layer(MockConnectInfo(SocketAddr::from(([10, 0, 0, n as u8 + 1], 5555))));
            call(&app, "POST", "/api/join", None, None, serde_json::json!({ "pin": "999999x", "role": "operator" })).await;
        }
        let app = r.router().layer(MockConnectInfo(SocketAddr::from(([10, 0, 1, 1], 5555))));
        let (c, _) = call(&app, "POST", "/api/join", None, None, serde_json::json!({ "pin": r.pin(), "role": "operator" })).await;
        assert_eq!(c, StatusCode::TOO_MANY_REQUESTS);
        assert!(same_secret("123456", "123456") && !same_secret("123456", "123457") && !same_secret("12345", "123456"));
    }

    #[test]
    fn pins_and_qr() {
        let r = Remote::new(Arc::new(Fake(Mutex::new(vec![]))));
        let a = r.pin();
        assert_eq!(a.len(), 6);
        assert!(a.chars().all(|c| c.is_ascii_digit()));
        let svg = qr_svg("http://192.168.1.10:7300/");
        assert!(svg.starts_with("<?xml") || svg.contains("<svg"));
    }
}
