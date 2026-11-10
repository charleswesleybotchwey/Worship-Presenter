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
    pub fn start_pump(&self, key: &str, name: &str, fps: u32) -> Result<(Arc<Slot>, Arc<Stats>, Arc<AtomicBool>), String> {
        self.stop(key);
        let rt = self.runtime()?;
        let sender = rt.sender(name)?;
        let slot = Arc::new(Slot::default());
        let stats = Arc::new(Stats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (s, st, sp) = (slot.clone(), stats.clone(), stop.clone());
        std::thread::Builder::new()
            .name(format!("ndi-{key}"))
            .spawn(move || pump::run(Box::new(sender), s, st, fps, sp))
            .map_err(|e| e.to_string())?;
        self.jobs.lock().unwrap().insert(
            key.to_string(),
            Job {
                name: name.to_string(),
                stop: stop.clone(),
                stats: stats.clone(),
                started: Instant::now(),
                fps,
                #[cfg(windows)]
                capture: None,
            },
        );
        Ok((slot, stats, stop))
    }

    pub fn stop(&self, key: &str) {
        if let Some(job) = self.jobs.lock().unwrap().remove(key) {
            job.stop.store(true, Ordering::Relaxed);
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
            self.f.slot.put(Frame { width, height, data });
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
