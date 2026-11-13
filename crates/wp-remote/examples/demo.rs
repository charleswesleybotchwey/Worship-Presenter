//! Runs the remote with a pretend service, for trying the phone pages without the
//! desktop app:  cargo run -p wp-remote --example demo  (PIN is printed)

use std::sync::{Arc, Mutex};
use wp_remote::{Command, Host, Remote};

struct Pretend {
    s: Mutex<(usize, &'static str, Option<usize>)>, // slide, mode, live item
    remote: Mutex<Option<Arc<Remote>>>,
}

const SLIDES: [(&str, &str, &str); 3] = [
    ("V1", "O for a thousand tongues to sing", "Meenya ngyiramá apem matow ndwom"),
    ("V1", "The glories of my God and King,", "Makamfo me Hen N’enyimnyam,"),
    ("V2", "Jesus! the name that charms our fears,", "Jesus! dzin a ɔma suro yi,"),
];

impl Host for Pretend {
    fn snapshot(&self) -> serde_json::Value {
        let (i, mode, item) = *self.s.lock().unwrap();
        serde_json::json!({
            "server_now_ms": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64,
            "connected": true, "service": "Sunday first service",
            "item": item.map(|_| serde_json::json!({ "kind": "song", "title": "MHB 1, ENG&FANTE" })),
            "mode": mode, "slide_idx": i,
            "slides": if item.is_some() { SLIDES.iter().map(|(l, p, s)| serde_json::json!({ "label": l, "primary": [p], "second": [s] })).collect::<Vec<_>>() } else { vec![] },
            "timer": null, "alert": null, "stage_message": "Two minutes", "repeat": null, "lining_out": false,
            "schedule": [{ "title": "MHB 1, ENG&FANTE" }, { "title": "Yohane 3:16 · TK" }], "live_index": item,
        })
    }
    fn note(&self, device: &str, text: &str) -> Result<(), String> {
        println!("note from {device}: {text}");
        Ok(())
    }
    fn command(&self, cmd: &Command) -> Result<(), String> {
        {
            let mut s = self.s.lock().unwrap();
            // as in the app: slides change behind Blank/Logo, and going live ends them
            match cmd {
                Command::Next => s.0 = (s.0 + 1).min(2),
                Command::Previous => s.0 = s.0.saturating_sub(1),
                Command::Show { index } => s.0 = (*index).min(2),
                Command::Blank => s.1 = "blank",
                Command::Logo => s.1 = "logo",
                Command::Clear => s.1 = "clear",
                Command::Resume => s.1 = "live",
                Command::GoLiveItem { index } => *s = (0, "live", Some(*index)),
                // Preview lives in the service window; the demo has none, so it just goes live
                Command::GoLivePreview => s.1 = "live",
                Command::NextItem => *s = (0, "live", Some(s.2.map_or(0, |i| i + 1))),
                Command::PreviousItem => *s = (0, "live", Some(s.2.map_or(0, |i| i.saturating_sub(1)))),
            }
        }
        if let Some(r) = self.remote.lock().unwrap().as_ref() {
            r.publish();
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let host = Arc::new(Pretend { s: Mutex::new((0, "live", Some(0))), remote: Mutex::new(None) });
    let remote = Remote::new(host.clone());
    remote.set_follow(true);
    *host.remote.lock().unwrap() = Some(remote.clone());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7300);
    let (_tx, rx) = tokio::sync::oneshot::channel();
    let addr = remote.clone().serve(([127, 0, 0, 1], port).into(), rx).await.unwrap();
    println!("PIN {} at http://{addr}/", remote.pin());
    tokio::signal::ctrl_c().await.ok();
}
