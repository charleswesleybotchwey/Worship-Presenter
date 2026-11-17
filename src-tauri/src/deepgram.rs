//! Deepgram: online speech recognition, streamed live (needs internet and a Deepgram API
//! key; Deepgram charges per minute of audio). The microphone goes to Deepgram's
//! WebSocket as 16 kHz 16-bit mono; interim words become the live caption and each
//! finished phrase goes through the same scripture finding as Whisper.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, stream::MaybeTlsStream};

/// Book names that speech engines often mishear, sent to Deepgram as key terms.
const KEYTERMS: [&str; 30] = [
    "Genesis", "Exodus", "Leviticus", "Deuteronomy", "Ecclesiastes", "Lamentations", "Ezekiel", "Hosea", "Obadiah", "Micah", "Nahum", "Habakkuk", "Zephaniah", "Haggai", "Zechariah", "Malachi",
    "Corinthians", "Galatians", "Ephesians", "Philippians", "Colossians", "Thessalonians", "Timothy", "Titus", "Philemon", "Hebrews", "Revelation", "Psalms", "Proverbs", "Nehemiah",
];

pub fn listen_url(base: &str, model: &str) -> String {
    let mut u = format!(
        "{base}/v1/listen?model={model}&language=en&encoding=linear16&sample_rate=16000&channels=1&interim_results=true&smart_format=true&endpointing=300"
    );
    // Nova-3 takes "keyterm" hints; older models take "keywords" instead
    let param = if model.starts_with("nova-3") { "keyterm" } else { "keywords" };
    for k in KEYTERMS {
        u.push_str(&format!("&{param}={k}"));
    }
    u
}

/// (words, finished?, confidence) from one Deepgram message; None for anything else.
pub fn parse(msg: &str) -> Option<(String, bool, Option<f32>)> {
    let v: serde_json::Value = serde_json::from_str(msg).ok()?;
    if v["type"].as_str() != Some("Results") {
        return None;
    }
    let t = v["channel"]["alternatives"][0]["transcript"].as_str()?.trim().to_string();
    if t.is_empty() {
        return None;
    }
    let conf = v["channel"]["alternatives"][0]["confidence"].as_f64().map(|c| c as f32);
    Some((t, v["is_final"].as_bool().unwrap_or(false), conf))
}

pub enum End {
    /// Listening was stopped by the operator.
    Stopped,
    /// The key was refused: no point trying again.
    BadKey(String),
    /// The connection failed or dropped (try again).
    Lost(String),
}

/// One connection: streams `audio` until stopped or the connection ends.
pub fn session(url: &str, key: &str, audio: &Receiver<Vec<i16>>, stop: &AtomicBool, on_open: impl FnOnce(), mut on_text: impl FnMut(&str, bool, Option<f32>)) -> End {
    let Ok(mut req) = url.into_client_request() else { return End::Lost("bad address".into()) };
    match format!("Token {key}").parse() {
        Ok(h) => {
            req.headers_mut().insert("Authorization", h);
        }
        Err(_) => return End::BadKey("The Deepgram key has characters that cannot be used.".into()),
    }
    let mut ws = match tungstenite::connect(req) {
        Ok((ws, _)) => ws,
        Err(tungstenite::Error::Http(r)) if r.status() == 401 || r.status() == 403 => {
            return End::BadKey("Deepgram refused the API key. Check it in Settings → Scripture recognition.".into());
        }
        Err(tungstenite::Error::Http(r)) if r.status() == 402 => return End::BadKey("Deepgram says the account has no credit left.".into()),
        Err(e) => return End::Lost(e.to_string()),
    };
    // short reads, so sending and receiving share one thread
    let timeout = Some(Duration::from_millis(40));
    match ws.get_mut() {
        MaybeTlsStream::Plain(s) => drop(s.set_read_timeout(timeout)),
        MaybeTlsStream::Rustls(s) => drop(s.sock.set_read_timeout(timeout)),
        _ => {}
    }
    on_open();
    let mut last_send = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = ws.send(Message::text(r#"{"type":"CloseStream"}"#));
            let _ = ws.close(None);
            return End::Stopped;
        }
        // the microphone's latest sound
        while let Ok(chunk) = audio.try_recv() {
            let bytes: Vec<u8> = chunk.iter().flat_map(|s| s.to_le_bytes()).collect();
            if let Err(e) = ws.send(Message::binary(bytes)) {
                return End::Lost(e.to_string());
            }
            last_send = Instant::now();
        }
        // pauses: tell Deepgram we are still here
        if last_send.elapsed() > Duration::from_secs(5) {
            if let Err(e) = ws.send(Message::text(r#"{"type":"KeepAlive"}"#)) {
                return End::Lost(e.to_string());
            }
            last_send = Instant::now();
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                if let Some((words, done, conf)) = parse(t.as_str()) {
                    on_text(&words, done, conf);
                }
            }
            Ok(Message::Close(_)) => return End::Lost("Deepgram closed the connection.".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return End::Lost(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_and_messages() {
        let u = listen_url("wss://api.deepgram.com", "nova-3");
        assert!(u.starts_with("wss://api.deepgram.com/v1/listen?model=nova-3&language=en&encoding=linear16&sample_rate=16000"));
        assert!(u.contains("&keyterm=Habakkuk") && u.contains("&keyterm=Thessalonians"));
        let old = listen_url("wss://api.deepgram.com", "nova-2");
        assert!(old.contains("&keywords=Habakkuk") && !old.contains("keyterm"), "Nova-2 takes keywords");
        let fin = r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":"turn to John chapter three","confidence":0.98}]}}"#;
        assert_eq!(parse(fin), Some(("turn to John chapter three".into(), true, Some(0.98))));
        let interim = r#"{"type":"Results","is_final":false,"channel":{"alternatives":[{"transcript":"turn to"}]}}"#;
        assert_eq!(parse(interim), Some(("turn to".into(), false, None)));
        assert_eq!(parse(r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":""}]}}"#), None, "silence");
        assert_eq!(parse(r#"{"type":"Metadata","request_id":"x"}"#), None);
    }

    /// The whole session against a local stand-in for Deepgram: the key is sent, the
    /// audio arrives as 16-bit samples, words come back, and Stop closes politely.
    #[test]
    fn a_session_with_a_stand_in_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (tcp, _) = listener.accept().unwrap();
            let mut auth = String::new();
            let mut ws = tungstenite::accept_hdr(tcp, |req: &tungstenite::handshake::server::Request, resp| {
                auth = req.headers().get("Authorization").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
                Ok(resp)
            })
            .unwrap();
            let mut audio_bytes = 0;
            let mut closed_politely = false;
            loop {
                match ws.read() {
                    Ok(Message::Binary(b)) => {
                        audio_bytes += b.len();
                        if audio_bytes >= 3200 && audio_bytes - b.len() < 3200 {
                            let _ = ws.send(Message::text(r#"{"type":"Results","is_final":false,"channel":{"alternatives":[{"transcript":"turn to"}]}}"#));
                            let _ = ws.send(Message::text(r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":"turn to John chapter three verse sixteen"}]}}"#));
                        }
                    }
                    Ok(Message::Text(t)) if t.as_str().contains("CloseStream") => closed_politely = true,
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
            (auth, audio_bytes, closed_politely)
        });
        let (tx, rx) = std::sync::mpsc::channel::<Vec<i16>>();
        for _ in 0..4 {
            tx.send(vec![1000i16; 800]).unwrap(); // 4 × 50 ms of sound
        }
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let mut heard: Vec<(String, bool)> = vec![];
        let mut opened = false;
        let end = session(&format!("ws://127.0.0.1:{port}/v1/listen?model=nova-3"), "secret-key", &rx, &stop, || opened = true, |t, done, _| {
            heard.push((t.to_string(), done));
            if done {
                stop2.store(true, Ordering::Relaxed);
            }
        });
        assert!(matches!(end, End::Stopped));
        assert!(opened, "connected is reported (the panel's spinner stops)");
        assert_eq!(heard, vec![("turn to".to_string(), false), ("turn to John chapter three verse sixteen".to_string(), true)]);
        let (auth, bytes, polite) = server.join().unwrap();
        assert_eq!(auth, "Token secret-key");
        assert_eq!(bytes, 4 * 800 * 2, "16-bit samples");
        assert!(polite, "Stop sends CloseStream");
    }
}
