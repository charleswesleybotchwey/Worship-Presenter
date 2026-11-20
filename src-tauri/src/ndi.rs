//! NDI sources (Phase 4 prototype).
//!
//! Lower third: a dedicated output window ("ndi") draws the lower third on pure
//! green; Windows.Graphics.Capture grabs it, the core keys green to transparency
//! and the pump sends it as an NDI source with alpha. Main: the main output
//! window is captured as is (opaque). A test pattern needs no capture at all.
//!
//! Capture is Windows-only; elsewhere starting a capture says so.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use wp_core::ndi::pump::{self, Slot, Stats};
use wp_core::ndi::sdk::Runtime;

pub struct Job {
    pub name: String,
    pub stop: Arc<AtomicBool>,
    pub stats: Arc<Stats>,
    pub started: Instant,
    pub fps: u32,
    /// The NDI sender, shared by the video pump and the sound thread.
    pub sender: Arc<wp_core::ndi::sdk::Sender>,
    /// Set while the computer's sound is sent with this source.
    pub audio: Option<Arc<AtomicBool>>,
    #[cfg(windows)]
    pub capture: Option<capture::Running>,
}

#[derive(Default)]
pub struct Ndi {
    runtime: Mutex<Option<Arc<Runtime>>>,
    pub jobs: Mutex<HashMap<String, Job>>,
}

impl Ndi {
    pub fn runtime(&self) -> Result<Arc<Runtime>, String> {
        let mut r = self.runtime.lock().unwrap();
        if let Some(rt) = r.as_ref() {
            return Ok(rt.clone());
        }
        let rt = Arc::new(Runtime::load()?);
        *r = Some(rt.clone());
        Ok(rt)
    }

    /// Starts the pump for a source; returns the slot frames should be put into.
    pub fn start_pump(&self, key: &str, name: &str, groups: Option<&str>, fps: u32) -> Result<(Arc<Slot>, Arc<Stats>, Arc<AtomicBool>), String> {
        self.stop(key);
        let rt = self.runtime()?;
        let sender = Arc::new(rt.sender(name, groups)?);
        let slot = Arc::new(Slot::default());
        let stats = Arc::new(Stats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (s, st, sp, snd) = (slot.clone(), stats.clone(), stop.clone(), sender.clone());
        std::thread::Builder::new()
            .name(format!("ndi-{key}"))
            .spawn(move || pump::run(Box::new(snd), s, st, fps, sp))
            .map_err(|e| e.to_string())?;
        self.jobs.lock().unwrap().insert(
            key.to_string(),
            Job {
                name: name.to_string(),
                stop: stop.clone(),
                stats: stats.clone(),
                started: Instant::now(),
                fps,
                sender,
                audio: None,
                #[cfg(windows)]
                capture: None,
            },
        );
        Ok((slot, stats, stop))
    }

    pub fn stop(&self, key: &str) {
        if let Some(job) = self.jobs.lock().unwrap().remove(key) {
            job.stop.store(true, Ordering::Relaxed);
            if let Some(a) = &job.audio {
                a.store(true, Ordering::Relaxed);
            }
            #[cfg(windows)]
            if let Some(c) = job.capture {
                c.stop();
            }
        }
    }
}

/// A moving test pattern for checking OBS/vMix reception, for `seconds`.
pub fn run_test_pattern(slot: Arc<Slot>, stop: Arc<AtomicBool>, seconds: u64) {
    std::thread::spawn(move || {
        let t = Instant::now();
        let mut n = 0u64;
        while !stop.load(Ordering::Relaxed) && t.elapsed().as_secs() < seconds {
            slot.put(pump::test_pattern(1280, 720, n));
            n += 1;
            std::thread::sleep(std::time::Duration::from_millis(33));
        }
        stop.store(true, Ordering::Relaxed);
    });
}

#[cfg(windows)]
pub mod capture {
    use std::ffi::c_void;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::Instant;
    use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
    use windows_capture::frame::Frame as WFrame;
    use windows_capture::graphics_capture_api::InternalCaptureControl;
    use windows_capture::settings::{ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings};
    use windows_capture::window::Window;
    use wp_core::ndi::key;
    use wp_core::ndi::pump::{Frame, Slot, Stats};

    pub struct Flags {
        pub slot: Arc<Slot>,
        pub stats: Arc<Stats>,
        /// true: background is the green key colour; false: keep it opaque
        pub key: bool,
    }

    pub struct Handler {
        f: Flags,
        scratch: Vec<u8>,
    }

    impl GraphicsCaptureApiHandler for Handler {
        type Flags = Flags;
        type Error = String;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self { f: ctx.flags, scratch: Vec::new() })
        }

        fn on_frame_arrived(&mut self, frame: &mut WFrame, _control: InternalCaptureControl) -> Result<(), Self::Error> {
            let t = Instant::now();
            let fb = frame.buffer().map_err(|e| format!("{e:?}"))?;
            let (width, height) = (fb.width(), fb.height());
            let mut data = fb.as_nopadding_buffer(&mut self.scratch).to_vec();
            if self.f.key {
                key::key_in_place(&mut data);
            } else {
                for px in data.chunks_exact_mut(4) {
                    px[3] = 255;
                }
            }
            self.f.slot.put(Frame { width, height, data, fade_ms: 0 });
            self.f.stats.captured.fetch_add(1, Ordering::Relaxed);
            self.f.stats.process_us.store(t.elapsed().as_micros() as u64, Ordering::Relaxed);
            Ok(())
        }
    }

    pub struct Running(CaptureControl<Handler, String>);

    impl Running {
        pub fn stop(self) {
            let _ = self.0.stop();
        }
    }

    /// Captures one window (by its HWND) into the slot.
    pub fn start(hwnd: *mut c_void, slot: Arc<Slot>, stats: Arc<Stats>, key: bool) -> Result<Running, String> {
        let settings = Settings::new(
            Window::from_raw_hwnd(hwnd),
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            Flags { slot, stats, key },
        );
        Handler::start_free_threaded(settings).map(Running).map_err(|e| format!("Window capture could not start: {e:?}"))
    }
}

/// The built-in renderer feeding NDI: draws the lower third whenever what it shows
/// changes (and each second for a clock), with exact transparency and no window.
pub fn run_builtin(app: tauri::AppHandle, slot: Arc<Slot>, stats: Arc<Stats>, stop: Arc<AtomicBool>) {
    run_builtin_as(app, slot, stats, stop, None, 1920, 1080)
}

/// The built-in renderer for an output sent as an NDI stream (EasyWorship's "NDI Stream"
/// output monitor): `role` None is the lower third; Some("main") the full screen.
pub fn run_builtin_as(app: tauri::AppHandle, slot: Arc<Slot>, stats: Arc<Stats>, stop: Arc<AtomicBool>, role: Option<String>, width: u32, height: u32) {
    use tauri::Manager;
    std::thread::Builder::new()
        .name("ndi-render".into())
        .spawn(move || {
            let mut r = wp_render::Renderer::new();
            let mut last = String::new();
            let mut last_at = Instant::now();
            while !stop.load(Ordering::Relaxed) {
                let Some(st) = app.try_state::<crate::AppState>() else { break };
                let (content, theme, sig, clock) = match role.as_deref() {
                    Some(r) => crate::full_content(&st, r),
                    None => crate::ndi_content(&st),
                };
                if sig != last || (clock && last_at.elapsed().as_secs() >= 1) {
                    let t = Instant::now();
                    let data = r.render(&theme, &content, width, height, &st.profile_dir().join("media"));
                    // a new slide fades in as the theme says ("cut": at once); the clock's
                    // ticks do not fade
                    let fade_ms = if sig != last && theme["transition"]["kind"].as_str() != Some("cut") { theme["transition"]["ms"].as_u64().unwrap_or(300).min(1500) as u32 } else { 0 };
                    slot.put(pump::Frame { width, height, data, fade_ms });
                    stats.captured.fetch_add(1, Ordering::Relaxed);
                    stats.process_us.store(t.elapsed().as_micros() as u64, Ordering::Relaxed);
                    last = sig;
                    last_at = Instant::now();
                }
                drop(st);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        })
        .ok();
}

/// Sends what this computer plays (music, video sound, everything heard from its
/// speakers) with an NDI source. Windows records it as "loopback" from the default
/// sound output; the sound keeps playing in the room as before.
pub fn start_audio(sender: Arc<wp_core::ndi::sdk::Sender>) -> Result<Arc<AtomicBool>, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let s2 = stop.clone();
    std::thread::Builder::new()
        .name("ndi-audio".into())
        .spawn(move || {
            use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
            let open = || -> Result<(cpal::Stream, std::sync::mpsc::Receiver<Vec<f32>>, u32, u32), String> {
                if !cfg!(windows) {
                    return Err("Sending the computer's sound over NDI works on Windows.".into());
                }
                let host = cpal::default_host();
                let dev = host.default_output_device().ok_or("This computer has no sound output to send.")?;
                let cfg = dev.default_output_config().map_err(|e| format!("The sound output cannot be recorded ({e})."))?;
                let (rate, channels) = (cfg.sample_rate(), cfg.channels() as u32);
                let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);
                let err = |e: cpal::Error| eprintln!("ndi audio: {e}");
                // an input stream on an output device records it (WASAPI loopback)
                let stream = match cfg.sample_format() {
                    cpal::SampleFormat::F32 => dev.build_input_stream::<f32, _, _>(cfg.config(), move |d: &[f32], _| drop(tx.try_send(d.to_vec())), err, None),
                    cpal::SampleFormat::I16 => dev.build_input_stream::<i16, _, _>(cfg.config(), move |d: &[i16], _| drop(tx.try_send(d.iter().map(|v| *v as f32 / 32768.0).collect())), err, None),
                    other => return Err(format!("The sound output's format ({other:?}) is not supported.")),
                }
                .map_err(|e| format!("The computer's sound could not be recorded ({e})."))?;
                stream.play().map_err(|e| format!("The computer's sound could not be recorded ({e})."))?;
                Ok((stream, rx, rate, channels))
            };
            match open() {
                Ok((stream, rx, rate, channels)) => {
                    let _ = ready_tx.send(Ok(()));
                    while !s2.load(Ordering::Relaxed) {
                        let Ok(chunk) = rx.recv_timeout(std::time::Duration::from_millis(200)) else { continue };
                        // interleaved (L R L R…) → planar (L L … R R …), as NDI wants
                        let n = chunk.len() / channels as usize;
                        let mut planar = vec![0f32; n * channels as usize];
                        for (i, v) in chunk.iter().enumerate() {
                            let (frame, ch) = (i / channels as usize, i % channels as usize);
                            if frame < n {
                                planar[ch * n + frame] = *v;
                            }
                        }
                        sender.send_audio(rate, channels, &planar);
                    }
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })
        .map_err(|e| e.to_string())?;
    ready_rx.recv().map_err(|_| "The sound thread stopped.".to_string())??;
    Ok(stop)
}

// ---------- NDI sources as inputs ----------

/// One NDI source being received: the newest picture, encoded on demand for screens.
pub struct Input {
    latest: Mutex<Option<(u64, wp_core::ndi::sdk::Picture)>>,
    jpeg: Mutex<Option<(u64, Vec<u8>)>>,
    asked: Mutex<Instant>,
    stop: AtomicBool,
}

#[derive(Default)]
pub struct Inputs {
    pub map: Mutex<HashMap<String, Arc<Input>>>,
}

impl Inputs {
    /// The newest picture of `name` as JPEG (at most 1280 wide); starts receiving the
    /// source on first use and stops it 10 s after screens stop asking.
    pub fn frame(&self, ndi: &Ndi, name: &str, url: &str) -> Option<Vec<u8>> {
        let input = {
            let mut map = self.map.lock().unwrap();
            if let Some(i) = map.get(name).filter(|i| !i.stop.load(Ordering::Relaxed)) {
                i.clone()
            } else {
                let rt = ndi.runtime().ok()?;
                let rx = rt.receiver(&wp_core::ndi::sdk::Source { name: name.to_string(), url: url.to_string() }).ok()?;
                let input = Arc::new(Input { latest: Mutex::new(None), jpeg: Mutex::new(None), asked: Mutex::new(Instant::now()), stop: AtomicBool::new(false) });
                let me = input.clone();
                std::thread::Builder::new()
                    .name("ndi-input".into())
                    .spawn(move || {
                        let mut seq = 0u64;
                        while !me.stop.load(Ordering::Relaxed) {
                            if me.asked.lock().unwrap().elapsed().as_secs() > 10 {
                                me.stop.store(true, Ordering::Relaxed);
                                break;
                            }
                            if let Some(p) = rx.next_picture(200) {
                                seq += 1;
                                *me.latest.lock().unwrap() = Some((seq, p));
                            }
                        }
                    })
                    .ok()?;
                map.insert(name.to_string(), input.clone());
                input
            }
        };
        *input.asked.lock().unwrap() = Instant::now();
        // wait briefly for the first picture of a new connection
        for _ in 0..20 {
            if input.latest.lock().unwrap().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let latest = input.latest.lock().unwrap();
        let (seq, pic) = latest.as_ref()?;
        if let Some((s, j)) = input.jpeg.lock().unwrap().as_ref() {
            if s == seq {
                return Some(j.clone());
            }
        }
        let jpeg = to_jpeg(pic);
        *input.jpeg.lock().unwrap() = Some((*seq, jpeg.clone()));
        Some(jpeg)
    }
}

/// BGRA picture → JPEG, scaled down to at most 1280 pixels wide.
pub fn to_jpeg(p: &wp_core::ndi::sdk::Picture) -> Vec<u8> {
    let step = (p.width as usize).div_ceil(1280).max(1);
    let (w, h) = (p.width as usize / step, p.height as usize / step);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * step) * p.width as usize + x * step) * 4;
            rgb.extend_from_slice(&[p.data[i + 2], p.data[i + 1], p.data[i]]);
        }
    }
    let mut out = Vec::new();
    let _ = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode(&rgb, w as u32, h as u32, image::ExtendedColorType::Rgb8);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn jpeg_of_a_received_picture_is_scaled_and_valid() {
        let p = wp_core::ndi::sdk::Picture { width: 1920, height: 1080, data: vec![128u8; 1920 * 1080 * 4] };
        let j = super::to_jpeg(&p);
        assert!(j.starts_with(&[0xFF, 0xD8]), "a JPEG");
        let img = image::load_from_memory(&j).unwrap();
        assert_eq!((img.width(), img.height()), (960, 540), "at most 1280 wide");
    }

    #[test]
    fn addresses_decode() {
        assert_eq!(crate::pct_decode("PTZ-CAM-1%20(Stage)"), "PTZ-CAM-1 (Stage)");
        assert_eq!(crate::pct_decode("192.168.1.31%3A5961"), "192.168.1.31:5961");
        assert_eq!(crate::pct_decode("bad%2"), "bad%2");
    }
}
