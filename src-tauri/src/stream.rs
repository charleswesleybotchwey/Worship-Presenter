//! Recording the service and going live online: the main output (as the projector shows
//! it) and the computer's sound, encoded by FFmpeg into a video file and/or streamed to
//! YouTube, Facebook or any RTMP address (StreamRelay), all at once if wanted.
//! FFmpeg is downloaded once from the BtbN FFmpeg builds on GitHub and checked against
//! the checksums published with them.

use crate::{err, AppState, Res};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use wp_core::ndi::pump::{Slot, Stats};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Destination {
    /// "file", "youtube", "facebook" or "custom"
    pub kind: String,
    /// The stream key (YouTube, Facebook), the RTMP address (custom) or the folder (file).
    pub target: String,
}

pub fn rtmp_url(d: &Destination) -> Option<String> {
    let t = d.target.trim();
    match d.kind.as_str() {
        "youtube" if !t.is_empty() => Some(format!("rtmp://a.rtmp.youtube.com/live2/{t}")),
        "facebook" if !t.is_empty() => Some(format!("rtmps://live-api-s.facebook.com:443/rtmp/{t}")),
        "custom" if t.starts_with("rtmp://") || t.starts_with("rtmps://") || t.starts_with("srt://") => Some(t.to_string()),
        _ => None,
    }
}

/// FFmpeg's arguments: raw frames on stdin, sound from a local port, H.264 + AAC out
/// to every destination (a file written in fragments, so a power cut leaves it playable).
pub fn ffmpeg_args(w: u32, h: u32, fps: u32, audio_port: Option<u16>, file: Option<&Path>, streams: &[String]) -> Vec<String> {
    let mut a: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-stats".into()];
    a.extend(["-f", "rawvideo", "-pix_fmt", "bgra", "-s"].map(String::from));
    a.push(format!("{w}x{h}"));
    a.extend(["-r".to_string(), fps.to_string(), "-i".into(), "pipe:0".into()]);
    match audio_port {
        Some(p) => {
            a.extend(["-f", "s16le", "-ar", "48000", "-ac", "2", "-i"].map(String::from));
            a.push(format!("tcp://127.0.0.1:{p}?listen=1"));
        }
        None => {
            // silence, so every destination still gets a sound track
            a.extend(["-f", "lavfi", "-i", "anullsrc=channel_layout=stereo:sample_rate=48000"].map(String::from));
        }
    }
    a.extend(["-map", "0:v", "-map", "1:a"].map(String::from));
    // the picture: 1080p at most, YouTube's recommended settings for live H.264
    a.extend(["-vf".to_string(), "scale='min(1920,iw)':-2,format=yuv420p".into()]);
    a.extend(["-c:v", "libx264", "-preset", "veryfast", "-b:v", "4500k", "-maxrate", "4500k", "-bufsize", "9000k"].map(String::from));
    a.extend(["-g".to_string(), (fps * 2).to_string(), "-keyint_min".into(), (fps * 2).to_string(), "-sc_threshold".into(), "0".into()]);
    a.extend(["-c:a", "aac", "-b:a", "160k", "-ar", "48000"].map(String::from));
    // MP4 and FLV through the tee output need the codecs' setup in the header
    a.extend(["-flags:v", "+global_header", "-flags:a", "+global_header"].map(String::from));
    if audio_port.is_none() {
        a.push("-shortest".into());
    }
    let mut outs: Vec<String> = vec![];
    if let Some(f) = file {
        outs.push(format!("[f=mp4:movflags=+frag_keyframe+empty_moov+default_base_moof]{}", f.to_string_lossy().replace('\\', "/")));
    }
    for s in streams {
        // one failing destination does not stop the others
        outs.push(format!("[f=flv:onfail=ignore]{s}"));
    }
    a.extend(["-f".to_string(), "tee".into(), outs.join("|")]);
    a
}

pub struct Running {
    /// Stream keys and addresses in use: taken out of anything shown or logged.
    secrets: Vec<String>,
    child: Child,
    stop: Arc<AtomicBool>,
    started: Instant,
    pub file: Option<PathBuf>,
    pub destinations: Vec<String>,
    pub log: Arc<Mutex<String>>,
    #[cfg(windows)]
    _capture: Option<crate::ndi::capture::Running>,
}

#[derive(Default)]
pub struct Stream {
    run: Mutex<Option<Running>>,
}

fn tools_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("tools")
}

pub fn ffmpeg_path(app: &AppHandle) -> PathBuf {
    tools_dir(app).join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })
}

/// Downloads FFmpeg once (about 100 MB for Windows) and checks it.
#[tauri::command]
pub async fn stream_install_ffmpeg(app: AppHandle) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let progress = |m: String| *st.speech.install.lock().unwrap() = Some(m);
        let r = install(&app, &progress);
        *st.speech.install.lock().unwrap() = None;
        r
    })
    .await
    .map_err(err)?
}

fn install(app: &AppHandle, progress: &dyn Fn(String)) -> Res<()> {
    use sha2::Digest;
    let base = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest";
    let name = if cfg!(windows) { "ffmpeg-master-latest-win64-gpl.zip" } else { "ffmpeg-master-latest-linux64-gpl.tar.xz" };
    let sums = crate::speech::download(&format!("{base}/checksums.sha256"), &|_| {}, "the FFmpeg checksums")?;
    let want = String::from_utf8_lossy(&sums).lines().find(|l| l.trim_end().ends_with(name)).and_then(|l| l.split_whitespace().next()).map(str::to_lowercase).ok_or("FFmpeg's checksums do not list the download.")?;
    let bytes = crate::speech::download(&format!("{base}/{name}"), progress, "FFmpeg")?;
    let got = format!("{:x}", sha2::Sha256::digest(&bytes));
    if got != want {
        return Err("The FFmpeg download did not match its checksum, so it was not installed. Try again.".into());
    }
    let dir = tools_dir(app);
    std::fs::create_dir_all(&dir).map_err(err)?;
    let exe = ffmpeg_path(app);
    if cfg!(windows) {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(err)?;
        let idx = (0..z.len()).find(|i| z.by_index(*i).map(|f| String::from_utf8_lossy(f.name_raw()).ends_with("bin/ffmpeg.exe")).unwrap_or(false)).ok_or("ffmpeg.exe was not in the download.")?;
        let mut f = z.by_index(idx).map_err(err)?;
        let mut out = Vec::new();
        f.read_to_end(&mut out).map_err(err)?;
        std::fs::write(&exe, out).map_err(err)?;
    } else {
        let tmp = dir.join(name);
        std::fs::write(&tmp, &bytes).map_err(err)?;
        let ok = Command::new("tar").args(["-xJf", &tmp.to_string_lossy(), "-C", &dir.to_string_lossy(), "--strip-components=2", "--wildcards", "*/bin/ffmpeg"]).status().map_err(err)?;
        let _ = std::fs::remove_file(&tmp);
        if !ok.success() {
            return Err("FFmpeg could not be unpacked.".into());
        }
    }
    Ok(())
}

/// The main output's frames: captured from its window on Windows (as NDI does), else
/// drawn by the built-in renderer.
fn start_frames(app: &AppHandle, slot: Arc<Slot>, stop: Arc<AtomicBool>) -> Res<Option<Box<dyn std::any::Any + Send>>> {
    let stats = Arc::new(Stats::default());
    #[cfg(windows)]
    {
        for label in ["output-ndi-main", "output-main"] {
            if let Some(w) = app.get_webview_window(label) {
                let hwnd = w.hwnd().map_err(err)?.0 as *mut std::ffi::c_void;
                let running = crate::ndi::capture::start(hwnd, slot, stats, false)?;
                return Ok(Some(Box::new(running)));
            }
        }
    }
    crate::ndi::run_builtin_as(app.clone(), slot, stats, stop, Some("main".into()), 1920, 1080);
    Ok(None)
}

/// The computer's sound (Windows: what the speakers play), sent to FFmpeg's port.
fn start_sound(port: u16, stop: Arc<AtomicBool>) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<i16>>(64);
    let s2 = stop.clone();
    std::thread::spawn(move || {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let Some(dev) = cpal::default_host().default_output_device() else { return };
        let Ok(cfg) = dev.default_output_config() else { return };
        let ch = cfg.channels() as usize;
        let rate = cfg.sample_rate();
        let tx2 = tx.clone();
        // to 48 kHz stereo 16-bit, as FFmpeg is told
        let convert = move |d: &[f32]| -> Vec<i16> {
            let frames = d.len() / ch.max(1);
            let out_frames = (frames as u64 * 48_000 / rate.max(1) as u64) as usize;
            let mut out = Vec::with_capacity(out_frames * 2);
            for i in 0..out_frames {
                let src = (i as u64 * rate as u64 / 48_000) as usize * ch;
                let l = d.get(src).copied().unwrap_or(0.0);
                let r = if ch > 1 { d.get(src + 1).copied().unwrap_or(l) } else { l };
                out.push((l.clamp(-1.0, 1.0) * 32767.0) as i16);
                out.push((r.clamp(-1.0, 1.0) * 32767.0) as i16);
            }
            out
        };
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => dev.build_input_stream::<f32, _, _>(cfg.config(), move |d: &[f32], _| drop(tx2.try_send(convert(d))), |_| {}, None),
            cpal::SampleFormat::I16 => dev.build_input_stream::<i16, _, _>(cfg.config(), move |d: &[i16], _| drop(tx2.try_send(convert(&d.iter().map(|v| *v as f32 / 32768.0).collect::<Vec<_>>()))), |_| {}, None),
            _ => return,
        };
        let Ok(stream) = stream else { return };
        let _ = stream.play();
        while !s2.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(200));
        }
        drop(stream);
    });
    std::thread::spawn(move || {
        // FFmpeg listens; connect once it is ready
        let mut sock = None;
        for _ in 0..50 {
            if let Ok(s) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                sock = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let Some(mut sock) = sock else { return };
        while !stop.load(Ordering::Relaxed) {
            if let Ok(buf) = rx.recv_timeout(Duration::from_millis(200)) {
                let bytes: Vec<u8> = buf.iter().flat_map(|s| s.to_le_bytes()).collect();
                if sock.write_all(&bytes).is_err() {
                    break;
                }
            }
        }
    });
    true
}

#[derive(Deserialize)]
pub struct StartRequest {
    pub record: bool,
    pub folder: Option<String>,
    pub destinations: Vec<Destination>,
}

/// Starts recording and/or streaming.
#[tauri::command]
pub async fn stream_start(app: AppHandle, req: StartRequest) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || start(&app, req)).await.map_err(err)?
}

fn start(app: &AppHandle, req: StartRequest) -> Res<String> {
    let st = app.state::<AppState>();
    if st.stream.run.lock().unwrap().is_some() {
        return Err("Already recording or live. Stop first.".into());
    }
    let ffmpeg = ffmpeg_path(app);
    if !ffmpeg.exists() {
        return Err("FFmpeg is needed once: press Download FFmpeg.".into());
    }
    // the keys come from this computer's protected store, never from the interface
    let mut secrets: Vec<String> = vec![];
    let destinations: Vec<Destination> = {
        let conn = st.db.lock().unwrap();
        req.destinations
            .iter()
            .map(|d| match d.kind.as_str() {
                "youtube" | "facebook" | "custom" => {
                    let target = crate::secrets::get(&conn, &format!("stream:{}", d.kind)).unwrap_or_else(|| d.target.clone());
                    if !target.trim().is_empty() {
                        secrets.push(target.trim().to_string());
                    }
                    Destination { kind: d.kind.clone(), target }
                }
                _ => d.clone(),
            })
            .collect()
    };
    let streams: Vec<String> = destinations.iter().filter_map(rtmp_url).collect();
    secrets.extend(streams.iter().cloned());
    let file = if req.record {
        let folder = req.folder.filter(|f| !f.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| dirs_videos().join("Worship Presenter"));
        std::fs::create_dir_all(&folder).map_err(err)?;
        let stamp = st.db.lock().unwrap().query_row("SELECT strftime('%Y-%m-%d %H%M', 'now', 'localtime')", [], |r| r.get::<_, String>(0)).unwrap_or_else(|_| "service".into());
        Some(folder.join(format!("Service {stamp}.mp4")))
    } else {
        None
    };
    if file.is_none() && streams.is_empty() {
        return Err("Choose Record, or give a stream key or address.".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let slot = Arc::new(Slot::default());
    let capture = start_frames(app, slot.clone(), stop.clone())?;
    // the first frame decides the size
    let t = Instant::now();
    let first = loop {
        if let Some(f) = slot.latest() {
            break f;
        }
        if t.elapsed() > Duration::from_secs(5) {
            stop.store(true, Ordering::Relaxed);
            return Err("The main output gave no picture. Open the main output (or NDI Stream) first.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let (w, h) = (first.width & !1, first.height & !1);
    let fps = 30;
    let port = std::net::TcpListener::bind(("127.0.0.1", 0)).ok().and_then(|l| l.local_addr().ok()).map(|a| a.port());
    let audio_port = port.filter(|_| cfg!(windows));
    let args = ffmpeg_args(w, h, fps, audio_port, file.as_deref(), &streams);
    let mut cmd = Command::new(&ffmpeg);
    cmd.args(&args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let mut child = cmd.spawn().map_err(|e| format!("FFmpeg could not start ({e})."))?;
    let stdin: ChildStdin = child.stdin.take().ok_or("FFmpeg's input is not available.")?;
    let log = Arc::new(Mutex::new(String::new()));
    if let Some(mut errp) = child.stderr.take() {
        let log2 = log.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while let Ok(n) = errp.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut l = log2.lock().unwrap();
                l.push_str(&String::from_utf8_lossy(&buf[..n]));
                if l.len() > 8000 {
                    let cut = l.len() - 4000;
                    *l = l[cut..].to_string();
                }
            }
        });
    }
    if let Some(p) = audio_port {
        start_sound(p, stop.clone());
    }
    // a steady 30 frames a second, the newest picture each time
    let s2 = stop.clone();
    std::thread::Builder::new()
        .name("record-frames".into())
        .spawn(move || {
            let mut stdin = stdin;
            let period = Duration::from_secs_f64(1.0 / fps as f64);
            let mut next = Instant::now();
            let mut last: Option<Arc<wp_core::ndi::pump::Frame>> = None;
            while !s2.load(Ordering::Relaxed) {
                if let Some(f) = slot.latest() {
                    if f.width & !1 == w && f.height & !1 == h {
                        last = Some(f);
                    }
                }
                if let Some(f) = &last {
                    let row = (f.width * 4) as usize;
                    let ok = (0..h as usize).all(|y| stdin.write_all(&f.data[y * row..y * row + w as usize * 4]).is_ok());
                    if !ok {
                        break;
                    }
                }
                next += period;
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                } else {
                    next = now;
                }
            }
            drop(stdin); // FFmpeg finishes the file
        })
        .map_err(err)?;
    let what = match (&file, streams.len()) {
        (Some(f), 0) => format!("Recording to {}", f.display()),
        (Some(f), n) => format!("Recording to {} and live to {n} destination{}", f.display(), if n == 1 { "" } else { "s" }),
        (None, n) => format!("Live to {n} destination{}", if n == 1 { "" } else { "s" }),
    };
    crate::applog::write("STREAM", &format!("started: {} ({w}x{h})", what));
    let _ = capture;
    *st.stream.run.lock().unwrap() = Some(Running {
        child,
        stop,
        started: Instant::now(),
        file,
        destinations: destinations.iter().filter(|d| rtmp_url(d).is_some()).map(|d| d.kind.clone()).collect(),
        secrets,
        log,
        #[cfg(windows)]
        _capture: capture.and_then(|c| c.downcast::<crate::ndi::capture::Running>().ok().map(|b| *b)),
    });
    Ok(what)
}

fn dirs_videos() -> PathBuf {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map(|h| PathBuf::from(h).join("Videos")).unwrap_or_else(|_| std::env::temp_dir())
}

/// Stops recording and streaming; the file is finished properly.
#[tauri::command]
pub async fn stream_stop(app: AppHandle) -> Res<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let Some(mut r) = st.stream.run.lock().unwrap().take() else { return Ok(None) };
        r.stop.store(true, Ordering::Relaxed);
        // FFmpeg ends when its input ends; give it time to finish the file
        for _ in 0..100 {
            if r.child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = r.child.kill();
        crate::applog::write("STREAM", "stopped");
        Ok(r.file.map(|f| f.to_string_lossy().into_owned()))
    })
    .await
    .map_err(err)?
}

/// Takes stream keys and addresses out of a message before it is shown or logged.
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    let mut s: Vec<&String> = secrets.iter().filter(|x| x.len() >= 4).collect();
    s.sort_by_key(|x| std::cmp::Reverse(x.len())); // whole addresses before the keys inside them
    for x in s {
        out = out.replace(x.as_str(), "[hidden]");
    }
    out
}

#[derive(Serialize)]
pub struct StreamStatus {
    ffmpeg: bool,
    running: bool,
    seconds: u64,
    file: Option<String>,
    destinations: Vec<String>,
    /// FFmpeg's last progress line (frame, fps, bitrate) or error.
    report: String,
    installing: Option<String>,
}

#[tauri::command]
pub fn stream_status(app: AppHandle, st: State<'_, AppState>) -> StreamStatus {
    let mut g = st.stream.run.lock().unwrap();
    // FFmpeg stopped by itself (a destination refused, the disk is full…)
    let ended = g.as_mut().is_some_and(|r| r.child.try_wait().ok().flatten().is_some());
    let report = g.as_ref().map(|r| redact(r.log.lock().unwrap().rsplit(['\r', '\n']).find(|l| !l.trim().is_empty()).unwrap_or(""), &r.secrets)).unwrap_or_default();
    if ended {
        if let Some(r) = g.take() {
            r.stop.store(true, Ordering::Relaxed);
            crate::applog::write("STREAM", &format!("FFmpeg stopped: {report}"));
        }
    }
    StreamStatus {
        ffmpeg: ffmpeg_path(&app).exists(),
        running: g.is_some(),
        seconds: g.as_ref().map(|r| r.started.elapsed().as_secs()).unwrap_or(0),
        file: g.as_ref().and_then(|r| r.file.as_ref().map(|f| f.to_string_lossy().into_owned())),
        destinations: g.as_ref().map(|r| r.destinations.clone()).unwrap_or_default(),
        report: if ended { format!("Stopped: {report}") } else { report },
        installing: st.speech.install.lock().unwrap().clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With WP_FFMPEG set, runs the real FFmpeg on the app's own arguments (3 s of frames).
    #[test]
    fn real_ffmpeg_makes_a_playable_file() {
        let Some(ff) = std::env::var_os("WP_FFMPEG") else { return };
        let out = std::env::temp_dir().join("wp-record-test.mp4");
        let _ = std::fs::remove_file(&out);
        let args = ffmpeg_args(1920, 1080, 30, None, Some(&out), &[]);
        let mut child = Command::new(ff).args(&args).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        for i in 0..90u32 {
            // a frame that changes colour, as slides would
            let px = [(i * 3) as u8, 80, 200, 255];
            let frame: Vec<u8> = px.iter().copied().cycle().take(1920 * 1080 * 4).collect();
            stdin.write_all(&frame).unwrap();
        }
        drop(stdin);
        let o = child.wait_with_output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let len = std::fs::metadata(&out).unwrap().len();
        println!("recorded {len} bytes to {}", out.display());
        assert!(len > 10_000);
    }

    #[test]
    fn keys_are_hidden_in_messages() {
        let secrets = vec!["abcd-1234".to_string(), "rtmp://a.rtmp.youtube.com/live2/abcd-1234".to_string()];
        let msg = "[flv @ 0x1] rtmp://a.rtmp.youtube.com/live2/abcd-1234: Connection refused (key abcd-1234)";
        assert_eq!(redact(msg, &secrets), "[flv @ 0x1] [hidden]: Connection refused (key [hidden])");
    }

    #[test]
    fn destinations_and_arguments() {
        let yt = Destination { kind: "youtube".into(), target: "abcd-1234".into() };
        assert_eq!(rtmp_url(&yt).unwrap(), "rtmp://a.rtmp.youtube.com/live2/abcd-1234");
        assert!(rtmp_url(&Destination { kind: "custom".into(), target: "http://nope".into() }).is_none(), "only RTMP or SRT addresses");
        assert!(rtmp_url(&Destination { kind: "youtube".into(), target: " ".into() }).is_none());
        let a = ffmpeg_args(1920, 1080, 30, Some(5555), Some(Path::new("C:\\Videos\\Service.mp4")), &["rtmp://x/live/key".into()]);
        let s = a.join(" ");
        assert!(s.contains("-s 1920x1080 -r 30 -i pipe:0"));
        assert!(s.contains("tcp://127.0.0.1:5555?listen=1"));
        assert!(s.contains("[f=mp4:movflags=+frag_keyframe+empty_moov+default_base_moof]C:/Videos/Service.mp4|[f=flv:onfail=ignore]rtmp://x/live/key"));
        assert!(s.contains("-g 60"), "a key frame every 2 seconds");
        assert!(s.contains("-flags:v +global_header"), "playable through tee");
        let silent = ffmpeg_args(1280, 720, 30, None, Some(Path::new("/tmp/a.mp4")), &[]).join(" ");
        assert!(silent.contains("anullsrc") && silent.contains("-shortest"));
    }
}

/// The recording or stream as a tile on the output dashboard (None: not running).
pub fn running_summary(app: &AppHandle) -> Option<crate::check::OutputTile> {
    let st = app.state::<AppState>();
    let g = st.stream.run.lock().unwrap();
    let r = g.as_ref()?;
    let secs = r.started.elapsed().as_secs();
    let time = format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    let online = !r.destinations.is_empty();
    let what = match (r.file.is_some(), online) {
        (true, true) => format!("Recording and live on {}", r.destinations.join(", ")),
        (false, true) => format!("Live on {}", r.destinations.join(", ")),
        _ => "Recording".to_string(),
    };
    Some(crate::check::OutputTile { role: "recording".into(), name: if online { "Online stream".into() } else { "Recording".into() }, kind: "recording".into(), state: if online { "streaming".into() } else { "recording".into() }, detail: format!("{what} · {time}") })
}
