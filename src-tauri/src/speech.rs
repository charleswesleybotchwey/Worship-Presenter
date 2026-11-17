//! Scripture recognition (experimental). Listens to the preacher's microphone and
//! offers the references it hears as suggestions; it never puts anything on the screens.
//!
//! Engines (Settings → Scripture recognition):
//! - Vosk (default): light and offline. Its engine and English model (about 60 MB) are
//!   downloaded once on request. "References only" limits its vocabulary to book names
//!   and numbers, which is far more accurate in a noisy church.
//! - Windows Speech: offline, built into Windows; nothing to download.
//! Whisper and cloud services are listed in Settings as not available yet.

use crate::{err, AppState, Res};
use serde::Serialize;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use wp_core::speech_ref::{self, Heard};

#[derive(Serialize, Clone)]
pub struct Detection {
    #[serde(flatten)]
    pub heard: Heard,
    /// Seconds since 1970.
    pub at: u64,
}

#[derive(Default)]
pub struct Speech {
    stop: Mutex<Option<Arc<AtomicBool>>>,
    engine: Mutex<Option<String>>,
    caption: Mutex<String>,
    recent: Mutex<VecDeque<Detection>>,
    problem: Mutex<Option<String>>,
    pub(crate) install: Mutex<Option<String>>,
    /// Follows the preacher: book, chapter and verse so far.
    tracker: Mutex<speech_ref::Tracker>,
    /// What was said in the last ~15 seconds (for recognising a verse read aloud).
    said: Mutex<VecDeque<(std::time::Instant, String)>>,
    last_quote_check: Mutex<Option<std::time::Instant>>,
    /// Everything heard this session (time, words, confidence 0–1 if the engine gives it).
    log: Mutex<Vec<(String, String, Option<f32>)>>,
    /// Starting up (loading a model, connecting): the panel shows a spinner.
    starting: AtomicBool,
    /// Paused after 10 minutes without speech.
    idle_paused: AtomicBool,
    /// When words last came back from the engine (ms), 0 before the first.
    last_text_ms: AtomicU64,
    /// Every scripture suggested since then (time, reference).
    log_refs: Mutex<Vec<(String, String)>>,
}

impl Speech {
    fn ready(&self) {
        self.starting.store(false, Ordering::Relaxed);
    }

    fn heard(&self, app: &AppHandle, text: &str, aliases: &[wp_core::reference::Alias]) {
        self.heard_with(app, text, None, aliases)
    }

    fn heard_with(&self, app: &AppHandle, text: &str, conf: Option<f32>, aliases: &[wp_core::reference::Alias]) {
        self.last_text_ms.store(now_ms(), Ordering::Relaxed);
        VOICED_SINCE_TEXT_MS.store(0, Ordering::Relaxed);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let clock = app.state::<AppState>().db.lock().unwrap().query_row("SELECT strftime('%H:%M:%S', 'now', 'localtime')", [], |r| r.get::<_, String>(0)).unwrap_or_default();
        {
            let mut log = self.log.lock().unwrap();
            if log.len() < 20_000 {
                log.push((clock.clone(), text.trim().to_string(), conf));
            }
        }
        // references, following the preacher ("verse seventeen", "the next verse", a
        // reference spread over two sentences)
        let mut found = self.tracker.lock().unwrap().feed(text, aliases);
        // a verse read aloud without its reference (at most every 3 seconds)
        {
            let mut said = self.said.lock().unwrap();
            said.push_back((std::time::Instant::now(), text.to_string()));
            while said.front().is_some_and(|(t, _)| t.elapsed().as_secs() > 15) {
                said.pop_front();
            }
            let due = self.last_quote_check.lock().unwrap().map_or(true, |t| t.elapsed().as_millis() > 3000);
            if due && found.is_empty() {
                *self.last_quote_check.lock().unwrap() = Some(std::time::Instant::now());
                let window = said.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join(" ");
                drop(said);
                let st = app.state::<AppState>();
                // the Bibles of the reading on the screens, else every Bible
                let versions: Vec<String> = match &*st.item.lock().unwrap() {
                    Some(crate::ProgramItem::Scripture { versions, .. }) => versions.clone(),
                    _ => vec![],
                };
                let quote = wp_core::speech_quote::find(&st.db.lock().unwrap(), &versions, &window).ok().flatten();
                if let Some(q) = quote {
                    let reference = format!("{} {}:{}", speech_ref::book_display(q.book_no), q.chapter, q.verse);
                    found.push(speech_ref::Heard {
                        reference,
                        book_no: q.book_no,
                        chapter: q.chapter,
                        verse_from: Some(q.verse),
                        verse_to: None,
                        heard: format!("heard the verse read ({}%, {})", q.score, q.version),
                    });
                }
            }
        }
        let mut recent = self.recent.lock().unwrap();
        for h in found {
            // the same reference again within half a minute is the preacher repeating it
            if recent.iter().any(|d| d.heard.reference == h.reference && now.saturating_sub(d.at) < 30) {
                continue;
            }
            self.log_refs.lock().unwrap().push((clock.clone(), h.reference.clone()));
            recent.push_front(Detection { heard: h, at: now });
            recent.truncate(20);
        }
    }

    /// Sends what is being said to the outputs as subtitles, when they are switched on.
    fn subtitle(app: &AppHandle, text: &str, done: bool) {
        use tauri::Emitter;
        let st = app.state::<AppState>();
        let on = st.live.lock().unwrap().subtitles.is_some();
        if on && !text.trim().is_empty() {
            *st.caption_now.lock().unwrap() = (text.to_string(), Some(std::time::Instant::now()));
            let _ = app.emit("caption", serde_json::json!({ "text": text, "final": done }));
        }
    }

    fn caption(&self, text: &str) {
        let t: String = text.chars().rev().take(140).collect::<Vec<_>>().into_iter().rev().collect();
        *self.caption.lock().unwrap() = t;
    }

    fn fail(&self, msg: impl Into<String>) {
        *self.problem.lock().unwrap() = Some(msg.into());
        *self.engine.lock().unwrap() = None;
    }
}

fn speech_dir(st: &AppState) -> PathBuf {
    st.profile_dir().parent().map(|p| p.join("speech")).unwrap_or_else(|| st.profile_dir().join("speech"))
}

fn vosk_lib_name() -> &'static str {
    if cfg!(windows) {
        "libvosk.dll"
    } else if cfg!(target_os = "macos") {
        "libvosk.dylib"
    } else {
        "libvosk.so"
    }
}

fn vosk_ready(dir: &Path) -> bool {
    dir.join("vosk").join(vosk_lib_name()).exists() && dir.join("vosk").join("model").join("am").exists()
}

#[derive(Serialize)]
pub struct SpeechStatus {
    listening: Option<String>,
    caption: String,
    recent: Vec<Detection>,
    problem: Option<String>,
    vosk_ready: bool,
    installing: Option<String>,
    windows_speech: bool,
    /// Whisper models downloaded ("base.en", "small.en").
    whisper_models: Vec<String>,
    /// Loading a model or connecting.
    starting: bool,
    /// The microphone level as the engine hears it (0–1, after the gain).
    level: f32,
    /// This session's transcript (time, words, confidence 0–1), the last 300 lines.
    lines: Vec<(String, String, Option<f32>)>,
    /// Speech is coming in but no words come back (a slow connection, a stuck engine).
    warning: Option<String>,
    /// Paused after 10 minutes without speech.
    idle_paused: bool,
    gain: u32,
}

#[tauri::command(async)]
pub fn speech_status(st: State<'_, AppState>) -> SpeechStatus {
    let s = &st.speech;
    SpeechStatus {
        listening: s.engine.lock().unwrap().clone(),
        caption: s.caption.lock().unwrap().clone(),
        recent: s.recent.lock().unwrap().iter().cloned().collect(),
        problem: s.problem.lock().unwrap().clone(),
        vosk_ready: vosk_ready(&speech_dir(&st)),
        installing: s.install.lock().unwrap().clone(),
        windows_speech: cfg!(windows),
        whisper_models: WHISPER_MODELS.iter().filter(|(m, _, _)| whisper_path(&speech_dir(&st), m).exists()).map(|(m, _, _)| m.to_string()).collect(),
        starting: s.starting.load(Ordering::Relaxed) && s.engine.lock().unwrap().is_some(),
        level: if s.engine.lock().unwrap().is_some() { f32::from_bits(LEVEL.load(Ordering::Relaxed)) } else { 0.0 },
        lines: {
            let l = s.log.lock().unwrap();
            l[l.len().saturating_sub(300)..].to_vec()
        },
        warning: {
            // after words had been coming: 30 s of speech and nothing back for 45 s
            let last = s.last_text_ms.load(Ordering::Relaxed);
            let engine = s.engine.lock().unwrap().clone();
            if engine.is_some() && last > 0 && now_ms().saturating_sub(last) > 45_000 && VOICED_SINCE_TEXT_MS.load(Ordering::Relaxed) > 30_000 {
                Some(if engine.as_deref() == Some("Deepgram") { "Connection seems unstable: transcription may be delayed.".into() } else { "Speech is coming in but no words are being recognised: check the microphone and the engine.".into() })
            } else {
                None
            }
        },
        idle_paused: s.idle_paused.load(Ordering::Relaxed),
        gain: st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = 'speech_gain'", [], |r| r.get::<_, String>(0)).ok().and_then(|v| v.parse().ok()).unwrap_or(50),
    }
}

#[tauri::command(async)]
pub fn speech_dismiss(st: State<'_, AppState>, reference: Option<String>) {
    let mut r = st.speech.recent.lock().unwrap();
    match reference {
        Some(x) => r.retain(|d| d.heard.reference != x),
        None => r.clear(),
    }
}

/// Microphones for Vosk (Windows Speech uses Windows' default microphone).
#[tauri::command]
pub async fn speech_mics() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(|| {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let mut v: Vec<String> = host.input_devices().map(|d| d.filter_map(|x| x.description().ok().map(|d| d.name().to_string())).collect()).unwrap_or_default();
        // the system's default first
        if let Some(def) = host.default_input_device().and_then(|d| d.description().ok().map(|d| d.name().to_string())) {
            v.retain(|n| *n != def);
            v.insert(0, def);
        }
        v
    })
    .await
    .unwrap_or_default()
}

#[tauri::command(async)]
pub fn speech_stop(app: AppHandle, st: State<'_, AppState>) {
    stop_listening(&st);
    save_session(&app, &st);
}

fn stop_listening(st: &AppState) {
    if let Some(s) = st.speech.stop.lock().unwrap().take() {
        s.store(true, Ordering::Relaxed);
    }
    *st.speech.engine.lock().unwrap() = None;
    st.speech.caption.lock().unwrap().clear();
    st.speech.starting.store(false, Ordering::Relaxed);
}

fn history_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("Transcripts")
}

/// The session's transcript into the history (Transcripts folder), then a fresh start.
fn save_session(app: &AppHandle, st: &AppState) {
    let log = std::mem::take(&mut *st.speech.log.lock().unwrap());
    let refs = std::mem::take(&mut *st.speech.log_refs.lock().unwrap());
    if log.is_empty() {
        return;
    }
    let (date, stamp) = st.db.lock().unwrap().query_row("SELECT strftime('%d %m %Y', 'now', 'localtime'), strftime('%Y-%m-%d %H%M', 'now', 'localtime')", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).unwrap_or_default();
    let dir = history_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    let mut path = dir.join(format!("{stamp}.txt"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stamp} ({n}).txt"));
        n += 1;
    }
    let _ = std::fs::write(&path, notes_text(&date, &log, &refs).as_bytes());
}

fn notes_text(date: &str, log: &[(String, String, Option<f32>)], refs: &[(String, String)]) -> String {
    let mut out = format!("Sermon transcript, {date}\r\n\r\n");
    if !refs.is_empty() {
        out.push_str("Scriptures\r\n");
        let mut seen: Vec<&str> = vec![];
        for (t, r) in refs {
            if !seen.contains(&r.as_str()) {
                seen.push(r);
                out.push_str(&format!("  {t}  {r}\r\n"));
            }
        }
        out.push_str("\r\n");
    }
    out.push_str("What was said\r\n");
    for (t, line, _) in log {
        out.push_str(&format!("[{t}] {line}\r\n"));
    }
    out
}

#[tauri::command]
pub async fn speech_start(app: AppHandle, engine: String, mic: Option<String>, references_only: bool) -> Res<()> {
    let st = app.state::<AppState>();
    stop_listening(&st);
    *st.speech.problem.lock().unwrap() = None;
    st.speech.starting.store(true, Ordering::Relaxed);
    st.speech.idle_paused.store(false, Ordering::Relaxed);
    st.speech.last_text_ms.store(0, Ordering::Relaxed);
    LAST_VOICE_MS.store(now_ms(), Ordering::Relaxed);
    VOICED_SINCE_TEXT_MS.store(0, Ordering::Relaxed);
    let pct: u32 = st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = 'speech_gain'", [], |r| r.get::<_, String>(0)).ok().and_then(|v| v.parse().ok()).unwrap_or(50);
    GAIN.store(gain_factor(pct).to_bits(), Ordering::Relaxed);
    let aliases = wp_core::library::aliases(&st.db.lock().unwrap()).map_err(err)?;
    let stop = Arc::new(AtomicBool::new(false));
    let watch_stop = stop.clone(); // for the idle watcher
    *st.speech.stop.lock().unwrap() = Some(stop.clone());
    let label = match engine.as_str() {
        "vosk" => {
            let dir = speech_dir(&st).join("vosk");
            if !vosk_ready(&speech_dir(&st)) {
                return Err("The Vosk engine is not downloaded yet. Settings → Scripture recognition → Download.".into());
            }
            let h = app.clone();
            std::thread::Builder::new()
                .name("speech-vosk".into())
                .spawn(move || {
                    if let Err(e) = run_vosk(&h, &dir, mic.as_deref(), references_only, &aliases, &stop) {
                        h.state::<AppState>().speech.fail(e);
                    }
                })
                .map_err(err)?;
            "Vosk"
        }
        "windows" => {
            if !cfg!(windows) {
                return Err("Windows Speech works on Windows only.".into());
            }
            let h = app.clone();
            std::thread::Builder::new()
                .name("speech-windows".into())
                .spawn(move || {
                    if let Err(e) = run_windows(&h, &aliases, &stop) {
                        h.state::<AppState>().speech.fail(e);
                    }
                })
                .map_err(err)?;
            "Windows Speech"
        }
        "whisper" => {
            // the best downloaded model (small is more accurate, base is lighter)
            let dir = speech_dir(&st);
            let wanted = st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = 'speech_whisper_model'", [], |r| r.get::<_, String>(0)).ok();
            let model = wanted
                .iter()
                .map(String::as_str)
                .chain(["small.en", "base.en"])
                .map(|m| whisper_path(&dir, m))
                .find(|p| p.exists())
                .ok_or("No Whisper model is downloaded yet. Settings → Scripture recognition → Whisper → Download.")?;
            let h = app.clone();
            std::thread::Builder::new()
                .name("speech-whisper".into())
                .spawn(move || {
                    if let Err(e) = run_whisper(&h, &model, mic.as_deref(), &aliases, &stop) {
                        h.state::<AppState>().speech.fail(e);
                    }
                })
                .map_err(err)?;
            "Whisper"
        }
        // Deepgram (online): needs internet and the church's Deepgram API key
        "deepgram" | "cloud" => {
            let get = |k: &str| st.db.lock().unwrap().query_row("SELECT value FROM setting WHERE key = ?1", [k], |r| r.get::<_, String>(0)).ok().filter(|v| !v.trim().is_empty());
            let key = get("deepgram_key").ok_or("Paste your Deepgram API key first: Settings → Scripture recognition → Deepgram.")?;
            let model = get("deepgram_model").unwrap_or_else(|| "nova-3".into());
            // Whisper takes over if the internet goes, when it is downloaded
            let dir = speech_dir(&st);
            let fallback = ["small.en", "base.en"].iter().map(|m| whisper_path(&dir, m)).find(|p| p.exists());
            let h = app.clone();
            std::thread::Builder::new()
                .name("speech-deepgram".into())
                .spawn(move || {
                    if let Err(e) = run_deepgram(&h, &key, &model, mic.as_deref(), &aliases, &stop, fallback) {
                        h.state::<AppState>().speech.fail(e);
                    }
                })
                .map_err(err)?;
            "Deepgram"
        }
        other => return Err(format!("Unknown speech engine {other}.")),
    };
    *st.speech.engine.lock().unwrap() = Some(label.into());
    if label != "Whisper" && label != "Deepgram" {
        st.speech.ready(); // these start at once; Whisper loads a model, Deepgram connects
    }
    // pause after 10 minutes without speech (saves the transcript, offers Resume)
    {
        let h = app.clone();
        let stop2 = watch_stop.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            if stop2.load(Ordering::Relaxed) {
                break;
            }
            if now_ms().saturating_sub(LAST_VOICE_MS.load(Ordering::Relaxed)) > 10 * 60 * 1000 {
                let st = h.state::<AppState>();
                stop_listening(&st);
                save_session(&h, &st);
                st.speech.idle_paused.store(true, Ordering::Relaxed);
                crate::applog::write("SPEECH", "paused after 10 minutes without speech");
                break;
            }
        });
    }
    Ok(())
}

// ---------- Windows Speech (System.Speech, offline) ----------

const WINDOWS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Add-Type -AssemblyName System.Speech
$r = New-Object System.Speech.Recognition.SpeechRecognitionEngine
$r.SetInputToDefaultAudioDevice()
$r.LoadGrammar((New-Object System.Speech.Recognition.DictationGrammar))
Register-ObjectEvent -InputObject $r -EventName SpeechRecognized -SourceIdentifier wpR | Out-Null
Register-ObjectEvent -InputObject $r -EventName SpeechHypothesized -SourceIdentifier wpH | Out-Null
$r.RecognizeAsync([System.Speech.Recognition.RecognizeMode]::Multiple)
[Console]::Out.WriteLine('READY'); [Console]::Out.Flush()
while ($true) {
  $e = Wait-Event
  $t = $e.SourceEventArgs.Result.Text
  if ($e.SourceIdentifier -eq 'wpR') { [Console]::Out.WriteLine("R`t" + $t) } else { [Console]::Out.WriteLine("H`t" + $t) }
  [Console]::Out.Flush()
  Remove-Event -EventIdentifier $e.EventIdentifier
}
"#;

fn run_windows(app: &AppHandle, aliases: &[wp_core::reference::Alias], stop: &AtomicBool) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    let mut cmd = std::process::Command::new(crate::powershell_exe());
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", WINDOWS_SCRIPT]).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| format!("Windows Speech could not start ({e})."))?;
    let out = child.stdout.take().ok_or("no output")?;
    let speech = &app.state::<AppState>().speech;
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines() {
            if tx.send(line.ok()).is_err() {
                break;
            }
        }
        let _ = tx.send(None);
    });
    let mut ready = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill();
            return Ok(());
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(line)) => {
                if line == "READY" {
                    ready = true;
                } else if let Some(t) = line.strip_prefix("R\t") {
                    speech.caption(t);
                    Speech::subtitle(app, t, true);
                    speech.heard(app, t, aliases);
                } else if let Some(t) = line.strip_prefix("H\t") {
                    speech.caption(t);
                    Speech::subtitle(app, t, false);
                }
            }
            Ok(None) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let mut msg = String::new();
                if let Some(mut e) = child.stderr.take() {
                    use std::io::Read;
                    let _ = e.read_to_string(&mut msg);
                }
                let _ = child.kill();
                return Err(if ready {
                    "Windows Speech stopped.".into()
                } else if msg.contains("recognizer") || msg.contains("Recognizer") {
                    "Windows has no English speech recognizer installed. Windows Settings → Time & language → Speech, or use Vosk.".into()
                } else if msg.contains("audio") || msg.contains("Audio") {
                    "No microphone was found for Windows Speech. Check the default recording device in Windows sound settings.".into()
                } else {
                    format!("Windows Speech could not start. {}", msg.lines().find(|l| !l.trim().is_empty()).unwrap_or(""))
                });
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

// ---------- Vosk (offline, light) ----------

type VModel = *mut std::ffi::c_void;
type VRec = *mut std::ffi::c_void;

struct Vosk {
    _lib: libloading::Library,
    model_new: unsafe extern "C" fn(*const std::ffi::c_char) -> VModel,
    model_free: unsafe extern "C" fn(VModel),
    rec_new: unsafe extern "C" fn(VModel, f32) -> VRec,
    rec_new_grm: unsafe extern "C" fn(VModel, f32, *const std::ffi::c_char) -> VRec,
    rec_free: unsafe extern "C" fn(VRec),
    accept: unsafe extern "C" fn(VRec, *const i16, i32) -> i32,
    result: unsafe extern "C" fn(VRec) -> *const std::ffi::c_char,
    partial: unsafe extern "C" fn(VRec) -> *const std::ffi::c_char,
    set_log: unsafe extern "C" fn(i32),
}

impl Vosk {
    fn load(dir: &Path) -> Result<Vosk, String> {
        let path = dir.join(vosk_lib_name());
        // load its companion DLLs from the same folder
        #[cfg(windows)]
        let lib = unsafe { libloading::os::windows::Library::load_with_flags(&path, 0x0000_0008).map(libloading::Library::from) };
        #[cfg(not(windows))]
        let lib = unsafe { libloading::Library::new(&path) };
        let lib = lib.map_err(|e| format!("The Vosk engine could not be loaded ({e}). Download it again in Settings."))?;
        unsafe {
            let f = |n: &[u8]| lib.get::<*mut std::ffi::c_void>(n).map(|s| *s).map_err(|e| format!("The Vosk engine is incomplete ({e})."));
            Ok(Vosk {
                model_new: std::mem::transmute(f(b"vosk_model_new\0")?),
                model_free: std::mem::transmute(f(b"vosk_model_free\0")?),
                rec_new: std::mem::transmute(f(b"vosk_recognizer_new\0")?),
                rec_new_grm: std::mem::transmute(f(b"vosk_recognizer_new_grm\0")?),
                rec_free: std::mem::transmute(f(b"vosk_recognizer_free\0")?),
                accept: std::mem::transmute(f(b"vosk_recognizer_accept_waveform_s\0")?),
                result: std::mem::transmute(f(b"vosk_recognizer_result\0")?),
                partial: std::mem::transmute(f(b"vosk_recognizer_partial_result\0")?),
                set_log: std::mem::transmute(f(b"vosk_set_log_level\0")?),
                _lib: lib,
            })
        }
    }
}

fn json_text(p: *const std::ffi::c_char, key: &str) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned();
    serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|v| v.get(key).and_then(|t| t.as_str()).map(str::to_string)).unwrap_or_default().replace("[unk]", "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The words a reference can be made of, for Vosk's "references only" mode.
fn reference_vocabulary() -> String {
    let mut words: Vec<String> = vec![];
    for b in wp_core::reference::BOOKS {
        for w in b.english[0].split_whitespace() {
            if w.chars().all(char::is_alphabetic) {
                words.push(w.to_lowercase());
            }
        }
    }
    for w in [
        "psalm", "revelations", "songs", "solomon", "first", "second", "third", "chapter", "chapters", "verse", "verses", "to", "through", "and", "till", "zero", "one", "two",
        "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety", "hundred", "a", "saint", "the", "of", "[unk]",
    ] {
        words.push(w.into());
    }
    words.sort();
    words.dedup();
    serde_json::to_string(&words).unwrap_or_else(|_| "[]".into())
}

fn run_vosk(app: &AppHandle, dir: &Path, mic: Option<&str>, references_only: bool, aliases: &[wp_core::reference::Alias], stop: &AtomicBool) -> Result<(), String> {
    let v = Vosk::load(dir)?;
    unsafe { (v.set_log)(-1) };
    let model_path = std::ffi::CString::new(dir.join("model").to_string_lossy().as_bytes()).map_err(err)?;
    let model = unsafe { (v.model_new)(model_path.as_ptr()) };
    if model.is_null() {
        return Err("The Vosk speech model could not be opened. Download it again in Settings.".into());
    }
    let rec = if references_only {
        let g = std::ffi::CString::new(reference_vocabulary()).map_err(err)?;
        unsafe { (v.rec_new_grm)(model, 16000.0, g.as_ptr()) }
    } else {
        unsafe { (v.rec_new)(model, 16000.0) }
    };
    if rec.is_null() {
        unsafe { (v.model_free)(model) };
        return Err("Vosk could not start listening.".into());
    }
    let (stream, rx) = open_mic(mic)?;
    let speech = &app.state::<AppState>().speech;
    let mut last_partial = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
        let done = unsafe { (v.accept)(rec, chunk.as_ptr(), chunk.len() as i32) };
        if done == 1 {
            let t = json_text(unsafe { (v.result)(rec) }, "text");
            if !t.is_empty() {
                speech.caption(&t);
                Speech::subtitle(app, &t, true);
                speech.heard(app, &t, aliases);
            }
        } else if last_partial.elapsed() > Duration::from_millis(250) {
            let t = json_text(unsafe { (v.partial)(rec) }, "partial");
            if !t.is_empty() {
                speech.caption(&t);
                Speech::subtitle(app, &t, false);
            }
            last_partial = Instant::now();
        }
    }
    drop(stream);
    unsafe {
        (v.rec_free)(rec);
        (v.model_free)(model);
    }
    Ok(())
}

// ---------- download (once, on request) ----------

const VOSK_VERSION: &str = "0.3.45";

/// SHA-256 of the official release downloads. The engine is program code that gets
/// loaded into Worship Presenter, so a download that does not match is refused.

/// The microphone (chosen by name, or Windows' default) as 16 kHz mono samples.
// The microphone as every engine hears it: input gain (Settings, live), the level meter
// (Live Transcript panel) and when speech was last heard (idle pause, connection health).
static GAIN: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0
static LEVEL: AtomicU32 = AtomicU32::new(0);
static LAST_VOICE_MS: AtomicU64 = AtomicU64::new(0);
/// Milliseconds of speech heard since the last words came back from the engine.
static VOICED_SINCE_TEXT_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// 0–100 % on the slider: 50 % leaves the sound as it is, 100 % is four times louder.
pub fn gain_factor(percent: u32) -> f32 {
    let p = percent.min(100) as f32;
    if p <= 50.0 { p / 50.0 } else { 1.0 + (p - 50.0) / 50.0 * 3.0 }
}

fn open_mic(mic: Option<&str>) -> Result<(cpal::Stream, std::sync::mpsc::Receiver<Vec<i16>>), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = match mic {
        Some(name) => host.input_devices().map_err(err)?.find(|d| d.description().map(|x| x.name() == name).unwrap_or(false)),
        None => host.default_input_device(),
    }
    .ok_or_else(|| "The chosen microphone is not connected. Choose another in Settings → Scripture recognition.".to_string())?;
    let cfg = device.default_input_config().map_err(|e| format!("The microphone cannot be used ({e})."))?;
    let rate = cfg.sample_rate() as f64;
    let channels = cfg.channels() as usize;
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<i16>>(64);
    // downmix to mono and resample to 16 kHz
    let mut pos = 0.0f64;
    let step = rate / 16000.0;
    let mut smooth = 0.0f32;
    let mut convert = move |mono: &mut dyn Iterator<Item = f32>| -> Vec<i16> {
        let g = f32::from_bits(GAIN.load(Ordering::Relaxed));
        let frames: Vec<f32> = mono.map(|x| (x * g).clamp(-1.0, 1.0)).collect();
        // the meter: the loudest moment, falling back smoothly
        let peak = frames.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        smooth = if peak > smooth { peak } else { smooth * 0.85 + peak * 0.15 };
        LEVEL.store(smooth.to_bits(), Ordering::Relaxed);
        let rms = (frames.iter().map(|x| x * x).sum::<f32>() / frames.len().max(1) as f32).sqrt();
        if rms > 0.02 {
            LAST_VOICE_MS.store(now_ms(), Ordering::Relaxed);
            VOICED_SINCE_TEXT_MS.fetch_add((frames.len() as f64 / rate * 1000.0) as u64, Ordering::Relaxed);
        }
        let mut out = Vec::with_capacity((frames.len() as f64 / step) as usize + 1);
        while (pos as usize) < frames.len() {
            out.push((frames[pos as usize] * 32767.0) as i16);
            pos += step;
        }
        pos -= frames.len() as f64;
        out
    };
    let errf = |e: cpal::Error| eprintln!("microphone: {e}");
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
            cfg.config(),
            move |data: &[f32], _| {
                let mut it = data.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32);
                let _ = tx.try_send(convert(&mut it));
            },
            errf,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream::<i16, _, _>(
            cfg.config(),
            move |data: &[i16], _| {
                let mut it = data.chunks(channels).map(|c| c.iter().map(|s| *s as f32 / 32768.0).sum::<f32>() / channels as f32);
                let _ = tx.try_send(convert(&mut it));
            },
            errf,
            None,
        ),
        other => return Err(format!("This microphone's sample format ({other:?}) is not supported.")),
    }
    .map_err(|e| format!("The microphone could not be opened ({e})."))?;
    stream.play().map_err(|e| format!("The microphone could not start ({e})."))?;
    Ok((stream, rx))
}

fn vosk_engine_sha256() -> Option<&'static str> {
    if cfg!(windows) {
        Some("f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2")
    } else {
        None
    }
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}
const MODEL_URL: &str = "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip";

fn vosk_engine_url() -> String {
    let platform = if cfg!(windows) { "win64" } else if cfg!(target_os = "macos") { "osx" } else { "linux-x86_64" };
    format!("https://github.com/alphacep/vosk-api/releases/download/v{VOSK_VERSION}/vosk-{platform}-{VOSK_VERSION}.zip")
}

pub(crate) fn download(url: &str, progress: &dyn Fn(String), what: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let resp = ureq::get(url).timeout(Duration::from_secs(600)).call().map_err(|e| format!("Could not download the {what} ({e}). Check the internet connection."))?;
    let total: Option<usize> = resp.header("Content-Length").and_then(|v| v.parse().ok());
    let mut r = resp.into_reader();
    let mut buf = Vec::with_capacity(total.unwrap_or(1 << 20));
    let mut chunk = [0u8; 1 << 16];
    loop {
        let n = r.read(&mut chunk).map_err(|e| format!("The {what} download stopped ({e})."))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        let mb = buf.len() as f64 / 1_048_576.0;
        progress(match total {
            Some(t) => format!("Downloading the {what}: {mb:.0} of {:.0} MB", t as f64 / 1_048_576.0),
            None => format!("Downloading the {what}: {mb:.0} MB"),
        });
    }
    Ok(buf)
}

/// Unzips, dropping the archive's top folder (vosk-win64-0.3.45/…, vosk-model-…/…).
fn unzip_flat(bytes: &[u8], into: &Path) -> Result<(), String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("The download is damaged ({e})."))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(err)?;
        let Some(name) = f.enclosed_name() else { continue };
        let rel: PathBuf = name.components().skip(1).collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = into.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(err)?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(err)?;
            }
            let mut w = std::fs::File::create(&out).map_err(err)?;
            std::io::copy(&mut f, &mut w).map_err(err)?;
        }
    }
    Ok(())
}

/// Downloads the Vosk engine and the small English model (about 60 MB together).
#[tauri::command]
pub async fn speech_install_vosk(app: AppHandle) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let dir = speech_dir(&st).join("vosk");
        let progress = |m: String| *st.speech.install.lock().unwrap() = Some(m);
        let result = (|| -> Result<(), String> {
            std::fs::create_dir_all(&dir).map_err(err)?;
            let engine = download(&vosk_engine_url(), &progress, "speech engine")?;
            match vosk_engine_sha256() {
                Some(want) if sha256_hex(&engine) != want => {
                    return Err("The downloaded speech engine is not the official Vosk release (its fingerprint does not match), so it was not installed. Try again later or on another network.".into())
                }
                None => return Err("The speech engine is not available for this kind of computer.".into()),
                _ => {}
            }
            progress("Unpacking the speech engine…".into());
            unzip_flat(&engine, &dir)?;
            let model = download(MODEL_URL, &progress, "English speech model")?;
            progress("Unpacking the English model…".into());
            let _ = std::fs::remove_dir_all(dir.join("model"));
            unzip_flat(&model, &dir.join("model"))?;
            // the model is data, not code: check it has the parts Vosk needs
            if !dir.join("model").join("am").join("final.mdl").exists() || !dir.join("model").join("conf").exists() {
                let _ = std::fs::remove_dir_all(dir.join("model"));
                return Err("The downloaded speech model is incomplete. Try again.".into());
            }
            Ok(())
        })();
        *st.speech.install.lock().unwrap() = None;
        result
    })
    .await
    .map_err(err)?
}

#[cfg(test)]
mod tests {
    #[test]
    fn vocabulary_has_books_and_numbers() {
        let v: Vec<String> = serde_json::from_str(&super::reference_vocabulary()).unwrap();
        for w in ["genesis", "corinthians", "john", "psalm", "sixteen", "twenty", "verse", "[unk]"] {
            assert!(v.contains(&w.to_string()), "{w}");
        }
    }

    #[test]
    fn engine_fingerprint_is_checked() {
        let bytes = std::fs::read("/tmp/vosk/v.zip").unwrap_or_default();
        if !bytes.is_empty() && cfg!(target_os = "linux") {
            assert_eq!(Some(super::sha256_hex(&bytes).as_str()), super::vosk_engine_sha256());
        }
        assert_ne!(Some(super::sha256_hex(b"tampered").as_str()), super::vosk_engine_sha256());
    }

    #[test]
    fn vosk_json() {
        let s = std::ffi::CString::new(r#"{"text" : "john three [unk] sixteen"}"#).unwrap();
        assert_eq!(super::json_text(s.as_ptr(), "text"), "john three sixteen");
    }

    /// With WP_VOSK set to an unpacked Vosk engine folder, check it loads.
    #[test]
    fn vosk_engine_loads_when_present() {
        let Some(d) = std::env::var_os("WP_VOSK") else { return };
        let v = super::Vosk::load(std::path::Path::new(&d)).unwrap();
        let p = std::ffi::CString::new("/no/such/model").unwrap();
        unsafe { (v.set_log)(-1) };
        assert!(unsafe { (v.model_new)(p.as_ptr()) }.is_null(), "a missing model is refused, not a crash");
    }
}

// ---------- Whisper (whisper.cpp, offline) ----------
//
// OpenAI's Whisper model run on this computer: far better than Vosk or Windows Speech
// with accents and a live room. It hears whole phrases: the microphone is listened to
// continuously and each phrase is transcribed when the speaker pauses (or every few
// seconds in a long run), then goes through the same reference and verse finding.

/// (name, file, approximate size in MB)
const WHISPER_MODELS: [(&str, &str, u64); 2] = [("base.en", "ggml-base.en.bin", 148), ("small.en", "ggml-small.en.bin", 488)];

fn whisper_path(dir: &Path, model: &str) -> PathBuf {
    let file = WHISPER_MODELS.iter().find(|(m, _, _)| *m == model).map(|(_, f, _)| *f).unwrap_or("ggml-base.en.bin");
    dir.join("whisper").join(file)
}

/// Bible book names, so Whisper expects them ("First Corinthians", "Habakkuk").
pub(crate) const WHISPER_PROMPT: &str = "Sermon with Bible readings: Genesis, Exodus, Leviticus, Deuteronomy, Joshua, Judges, Ruth, Samuel, Kings, Chronicles, Nehemiah, Esther, Job, Psalms, Proverbs, Ecclesiastes, Isaiah, Jeremiah, Lamentations, Ezekiel, Daniel, Hosea, Habakkuk, Zechariah, Malachi, Matthew, Mark, Luke, John, Acts, Romans, First Corinthians, Galatians, Ephesians, Philippians, Colossians, Thessalonians, Timothy, Hebrews, James, Peter, Jude, Revelation. John chapter three verse sixteen.";

/// Whisper's habit on silence or music: invented phrases. These are thrown away.
fn whisper_junk(t: &str) -> bool {
    let l = t.trim().to_lowercase();
    l.is_empty() || l.starts_with('[') || l.starts_with('(') || matches!(l.trim_end_matches(['.', '!']), "thank you" | "thanks for watching" | "you" | "amen" | "bye")
}

fn run_whisper(app: &AppHandle, model: &Path, mic: Option<&str>, aliases: &[wp_core::reference::Alias], stop: &AtomicBool) -> Result<(), String> {
    let speech = &app.state::<AppState>().speech;
    speech.caption("Loading the Whisper model…");
    // Whisper runs in its own process: a crash there never closes the presentation
    let mut worker = crate::whisper_worker::Worker::start(model)?;
    let mut restarts = 0u32;
    let (_stream, rx) = open_mic(mic)?;
    speech.caption("Listening…");
    speech.ready();
    let mut buf: Vec<f32> = Vec::new();
    let mut voiced = 0usize; // samples of speech in buf
    let mut quiet = 0usize; // samples of silence since the last speech
    let mut floor = 0.004f32; // background noise level, learnt as we go
    while !stop.load(Ordering::Relaxed) {
        let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
        let f: Vec<f32> = chunk.iter().map(|s| *s as f32 / 32768.0).collect();
        let rms = (f.iter().map(|x| x * x).sum::<f32>() / f.len().max(1) as f32).sqrt();
        let speaking = rms > (floor * 3.0).max(0.008);
        if speaking {
            voiced += f.len();
            quiet = 0;
        } else {
            quiet += f.len();
            floor = floor * 0.98 + rms * 0.02;
        }
        buf.extend_from_slice(&f);
        // nothing said for a while: forget the silence
        if voiced == 0 && buf.len() > 16_000 * 3 {
            buf.drain(..buf.len() - 16_000);
        }
        // transcribe at a pause (0.6 s) after enough speech, or every 7 s in a long run
        let pause = quiet > 16_000 * 6 / 10 && voiced > 16_000 * 8 / 10;
        if !(pause || buf.len() > 16_000 * 7) {
            continue;
        }
        let audio = std::mem::take(&mut buf);
        voiced = 0;
        let (text, conf) = match worker.transcribe(&audio) {
            Ok(t) => t,
            Err(e) => {
                // the helper ended (a crash in Whisper): the app carries on; start it again
                restarts += 1;
                crate::applog::write("CRASH", &format!("Whisper helper ended ({e}); restart {restarts}"));
                if restarts > 3 {
                    return Err("Whisper keeps stopping on this computer, so listening has stopped. The log has the details (Settings → About); Vosk may work better here.".into());
                }
                speech.caption("Whisper stopped; starting it again…");
                worker = crate::whisper_worker::Worker::start(model)?;
                speech.caption("Listening…");
                continue;
            }
        };
        if whisper_junk(&text) {
            continue;
        }
        speech.caption(&text);
        Speech::subtitle(app, &text, true);
        speech.heard_with(app, &text, conf, aliases);
    }
    Ok(())
}

/// Deepgram, live over the internet: interim words as the caption, finished phrases to
/// the scripture finding. Reconnects if the line drops; after repeated failures Whisper
/// takes over (if downloaded).
fn run_deepgram(app: &AppHandle, key: &str, model: &str, mic: Option<&str>, aliases: &[wp_core::reference::Alias], stop: &AtomicBool, fallback: Option<PathBuf>) -> Result<(), String> {
    use crate::deepgram::{listen_url, session, End};
    let speech = &app.state::<AppState>().speech;
    let (stream, rx) = open_mic(mic)?;
    let url = listen_url("wss://api.deepgram.com", model);
    let mut fails = 0u32;
    speech.caption("Connecting to Deepgram…");
    loop {
        let began = std::time::Instant::now();
        let end = session(&url, key, &rx, stop, || speech.ready(), |text, done, conf| {
            speech.caption(text);
            Speech::subtitle(app, text, done);
            if done {
                speech.heard_with(app, text, conf, aliases);
            }
        });
        match end {
            End::Stopped => return Ok(()),
            End::BadKey(m) => return Err(m),
            End::Lost(e) => {
                if began.elapsed() > Duration::from_secs(60) {
                    fails = 0; // it had been working
                }
                fails += 1;
                crate::applog::write("SPEECH", &format!("Deepgram connection lost ({e}); attempt {fails}"));
                if fails >= 3 {
                    if let Some(m) = &fallback {
                        drop(stream);
                        speech.caption("No connection to Deepgram: Whisper is listening instead.");
                        return run_whisper(app, m, mic, aliases, stop);
                    }
                }
                speech.caption("Deepgram connection lost; reconnecting…");
                while rx.try_recv().is_ok() {} // old sound is no use now
                for _ in 0..20 {
                    if stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
}

/// Downloads a Whisper model from the official whisper.cpp models (Hugging Face).
#[tauri::command]
pub async fn speech_install_whisper(app: AppHandle, model: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let Some((name, file, mb)) = WHISPER_MODELS.iter().find(|(m, _, _)| *m == model).copied() else {
            return Err(format!("Unknown Whisper model {model}."));
        };
        let progress = |m: String| *st.speech.install.lock().unwrap() = Some(m);
        let result = (|| -> Result<(), String> {
            let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}");
            let bytes = download(&url, &progress, &format!("Whisper {name} model (about {mb} MB)"))?;
            // a real model file: whisper.cpp's "ggml" signature and about the right size
            let sig_ok = bytes.len() > 8 && &bytes[0..4] == b"lmgg";
            let size_ok = (bytes.len() as u64) > mb * 1_000_000 * 9 / 10 && (bytes.len() as u64) < mb * 1_000_000 * 11 / 10;
            if !sig_ok || !size_ok {
                return Err("The downloaded file is not a Whisper model, so it was not installed. Try again later or on another network.".into());
            }
            let path = whisper_path(&speech_dir(&st), name);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(err)?;
            let tmp = path.with_extension("part");
            std::fs::write(&tmp, &bytes).map_err(err)?;
            std::fs::rename(&tmp, &path).map_err(err)?;
            Ok(())
        })();
        *st.speech.install.lock().unwrap() = None;
        result
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
pub struct RecordingTest {
    /// (seconds into the recording, what Whisper heard)
    transcript: Vec<(u32, String)>,
    /// (seconds, reference, how it was found)
    found: Vec<(u32, String, String)>,
    model: String,
    seconds: u32,
    took_seconds: u32,
}

/// The test bench: runs a sermon recording through Whisper and the same reference and
/// verse finding as live listening, and reports what would have been suggested.
#[tauri::command]
pub async fn speech_test_recording(app: AppHandle, path: String) -> Res<RecordingTest> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let dir = speech_dir(&st);
        let (model_name, model) = ["small.en", "base.en"].iter().map(|m| (m.to_string(), whisper_path(&dir, m))).find(|(_, p)| p.exists()).ok_or("Download a Whisper model first (Settings → Scripture recognition).")?;
        let audio = wp_audio::decode_mono_16k(std::path::Path::new(&path))?;
        let started = std::time::Instant::now();
        // in its own process, as when listening live
        let mut worker = crate::whisper_worker::Worker::start(&model)?;
        let aliases = wp_core::library::aliases(&st.db.lock().unwrap()).map_err(err)?;
        // phrases: split at pauses, as when listening live (at most 7 s each)
        let mut phrases: Vec<(usize, usize)> = vec![];
        let (mut start, mut quiet, mut floor) = (0usize, 0usize, 0.004f32);
        for (i, block) in audio.chunks(3200).enumerate() {
            let rms = (block.iter().map(|x| x * x).sum::<f32>() / block.len().max(1) as f32).sqrt();
            if rms > (floor * 3.0).max(0.008) {
                quiet = 0;
            } else {
                quiet += block.len();
                floor = floor * 0.98 + rms * 0.02;
            }
            let end = (i + 1) * 3200;
            if (quiet > 9600 && end - start > 12_800) || end - start > 16_000 * 7 {
                phrases.push((start, end.min(audio.len())));
                start = end;
            }
        }
        if start < audio.len() {
            phrases.push((start, audio.len()));
        }
        let mut tracker = wp_core::speech_ref::Tracker::new();
        let mut window: VecDeque<(u32, String)> = VecDeque::new();
        let (mut transcript, mut found) = (vec![], vec![]);
        for (k, (a, b)) in phrases.iter().enumerate() {
            *st.speech.install.lock().unwrap() = Some(format!("Testing the recording… {}%", k * 100 / phrases.len().max(1)));
            let (text, _) = worker.transcribe(&audio[*a..*b]).map_err(|e| format!("{e} (the log has the details: Settings → About)"))?;
            if whisper_junk(&text) {
                continue;
            }
            let at = (*a / 16_000) as u32;
            transcript.push((at, text.clone()));
            let refs = tracker.feed(&text, &aliases);
            for h in &refs {
                found.push((at, h.reference.clone(), format!("said: “{}”", h.heard)));
            }
            window.push_back((at, text.clone()));
            while window.front().is_some_and(|(t, _)| at.saturating_sub(*t) > 15) {
                window.pop_front();
            }
            if refs.is_empty() {
                let w = window.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join(" ");
                if let Ok(Some(q)) = wp_core::speech_quote::find(&st.db.lock().unwrap(), &[], &w) {
                    let r = format!("{} {}:{}", speech_ref::book_display(q.book_no), q.chapter, q.verse);
                    if !found.iter().any(|f: &(u32, String, String)| f.1 == r && at.saturating_sub(f.0) < 30) {
                        found.push((at, r, format!("verse read aloud ({}%, {})", q.score, q.version)));
                    }
                }
            }
        }
        *st.speech.install.lock().unwrap() = None;
        Ok(RecordingTest { transcript, found, model: model_name, seconds: (audio.len() / 16_000) as u32, took_seconds: started.elapsed().as_secs() as u32 })
    })
    .await
    .map_err(err)?
}

/// The sermon as heard since Listen was pressed: the scriptures, then every word, with
/// times. Saved as a text file to edit into notes or posts.
#[tauri::command]
pub fn speech_save_notes(st: State<'_, AppState>, path: String) -> Res<usize> {
    let log = st.speech.log.lock().unwrap().clone();
    if log.is_empty() {
        return Err("Nothing has been heard yet. Press Listen during the sermon (Whisper works best).".into());
    }
    let refs = st.speech.log_refs.lock().unwrap().clone();
    let date = st.db.lock().unwrap().query_row("SELECT strftime('%d %m %Y', 'now', 'localtime')", [], |r| r.get::<_, String>(0)).unwrap_or_default();
    let out = notes_text(&date, &log, &refs);
    std::fs::write(&path, out.as_bytes()).map_err(|e| format!("The file could not be saved ({e})."))?;
    Ok(log.len())
}

/// The microphone level as the engines hear it: 0–100 %, 50 % leaves it as it is.
/// Takes effect at once, even while listening.
#[tauri::command(async)]
pub fn speech_set_gain(st: State<'_, AppState>, percent: u32) -> Res<()> {
    let p = percent.min(100);
    GAIN.store(gain_factor(p).to_bits(), Ordering::Relaxed);
    st.db.lock().unwrap().execute("INSERT OR REPLACE INTO setting (key, value) VALUES ('speech_gain', ?1)", [p.to_string()]).map_err(err)?;
    Ok(())
}

/// The system's default microphone (marked in the list).
#[tauri::command]
pub async fn speech_default_mic() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| {
        use cpal::traits::{DeviceTrait, HostTrait};
        cpal::default_host().default_input_device().and_then(|d| d.description().ok().map(|d| d.name().to_string()))
    })
    .await
    .ok()
    .flatten()
}

#[derive(Serialize)]
pub struct PastSession {
    name: String,
    bytes: u64,
}

/// Saved sermon transcripts, newest first.
#[tauri::command(async)]
pub fn speech_history(app: AppHandle) -> Vec<PastSession> {
    let mut v: Vec<PastSession> = std::fs::read_dir(history_dir(&app))
        .map(|rd| rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "txt")).map(|e| PastSession { name: e.file_name().to_string_lossy().trim_end_matches(".txt").to_string(), bytes: e.metadata().map(|m| m.len()).unwrap_or(0) }).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| b.name.cmp(&a.name));
    v
}

fn history_file(app: &AppHandle, name: &str) -> Res<PathBuf> {
    if name.contains(['/', '\\']) || name.contains("..") {
        return Err("Unknown transcript.".into());
    }
    Ok(history_dir(app).join(format!("{name}.txt")))
}

#[tauri::command(async)]
pub fn speech_history_read(app: AppHandle, name: String) -> Res<String> {
    std::fs::read_to_string(history_file(&app, &name)?).map_err(|_| "That transcript is no longer there.".into())
}

#[tauri::command(async)]
pub fn speech_history_delete(app: AppHandle, name: String) -> Res<()> {
    std::fs::remove_file(history_file(&app, &name)?).map_err(err)
}

#[cfg(test)]
mod gain_tests {
    #[test]
    fn the_gain_slider() {
        assert_eq!(super::gain_factor(50), 1.0, "50 % leaves the sound as it is");
        assert_eq!(super::gain_factor(0), 0.0);
        assert_eq!(super::gain_factor(25), 0.5);
        assert_eq!(super::gain_factor(100), 4.0, "100 % is four times louder");
        assert_eq!(super::gain_factor(250), 4.0, "never more");
    }
}
