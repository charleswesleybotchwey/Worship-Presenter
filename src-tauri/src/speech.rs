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
use std::sync::atomic::{AtomicBool, Ordering};
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
    install: Mutex<Option<String>>,
}

impl Speech {
    fn heard(&self, text: &str, aliases: &[wp_core::reference::Alias]) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut recent = self.recent.lock().unwrap();
        for h in speech_ref::find(text, aliases) {
            // the same reference again within half a minute is the preacher repeating it
            if recent.iter().any(|d| d.heard.reference == h.reference && now.saturating_sub(d.at) < 30) {
                continue;
            }
            recent.push_front(Detection { heard: h, at: now });
            recent.truncate(20);
        }
    }

    /// Sends what is being said to the outputs as subtitles, when they are switched on.
    fn subtitle(app: &AppHandle, text: &str, done: bool) {
        use tauri::Emitter;
        let on = app.state::<AppState>().live.lock().unwrap().subtitles.is_some();
        if on && !text.trim().is_empty() {
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
}

#[tauri::command]
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
    }
}

#[tauri::command]
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
        host.input_devices().map(|d| d.filter_map(|x| x.description().ok().map(|d| d.name().to_string())).collect()).unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub fn speech_stop(st: State<'_, AppState>) {
    if let Some(s) = st.speech.stop.lock().unwrap().take() {
        s.store(true, Ordering::Relaxed);
    }
    *st.speech.engine.lock().unwrap() = None;
    st.speech.caption.lock().unwrap().clear();
}

#[tauri::command]
pub async fn speech_start(app: AppHandle, engine: String, mic: Option<String>, references_only: bool) -> Res<()> {
    let st = app.state::<AppState>();
    speech_stop(st.clone());
    *st.speech.problem.lock().unwrap() = None;
    let aliases = wp_core::library::aliases(&st.db.lock().unwrap()).map_err(err)?;
    let stop = Arc::new(AtomicBool::new(false));
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
        "whisper" | "cloud" => return Err("That engine is not available yet. Use Vosk or Windows Speech.".into()),
        other => return Err(format!("Unknown speech engine {other}.")),
    };
    *st.speech.engine.lock().unwrap() = Some(label.into());
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
                    speech.heard(t, aliases);
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
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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
    // microphone: chosen by name, or Windows' default
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
    let mut convert = move |mono: &mut dyn Iterator<Item = f32>| -> Vec<i16> {
        let frames: Vec<f32> = mono.collect();
        let mut out = Vec::with_capacity((frames.len() as f64 / step) as usize + 1);
        while (pos as usize) < frames.len() {
            out.push((frames[pos as usize].clamp(-1.0, 1.0) * 32767.0) as i16);
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
                speech.heard(&t, aliases);
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

fn download(url: &str, progress: &dyn Fn(String), what: &str) -> Result<Vec<u8>, String> {
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
