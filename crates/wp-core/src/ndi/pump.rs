//! Sends the newest frame at a steady rate. Window capture only delivers a frame
//! when something on screen changes, but receivers expect a continuous stream, so
//! the last frame is repeated until a new one arrives.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// BGRA, rows packed (stride = width × 4), straight alpha.
    pub data: Vec<u8>,
    /// Fade in from the previous frame over this many ms (0: cut). The built-in renderer
    /// sets it on a slide change; captured frames carry their own transitions.
    pub fade_ms: u32,
}

/// Straight-alpha BGRA, `t` of the way from `a` to `b`.
pub fn blend(a: &[u8], b: &[u8], t: f32) -> Vec<u8> {
    let k = (t.clamp(0.0, 1.0) * 256.0) as u32;
    a.iter().zip(b).map(|(x, y)| ((*x as u32 * (256 - k) + *y as u32 * k) >> 8) as u8).collect()
}

/// Something that can put frames on the network (the NDI sender, or a test double).
pub trait Sink: Send {
    /// A frame already converted to NDI's UYVA layout.
    fn send(&mut self, width: u32, height: u32, uyva: &[u8], fps: u32);
    fn connections(&self) -> i32;
}

impl Sink for Arc<super::sdk::Sender> {
    fn send(&mut self, width: u32, height: u32, uyva: &[u8], fps: u32) {
        self.send_uyva(width, height, uyva, fps);
    }
    fn connections(&self) -> i32 {
        super::sdk::Sender::connections(self)
    }
}

impl Sink for super::sdk::Sender {
    fn send(&mut self, width: u32, height: u32, uyva: &[u8], fps: u32) {
        self.send_uyva(width, height, uyva, fps);
    }
    fn connections(&self) -> i32 {
        super::sdk::Sender::connections(self)
    }
}

#[derive(Default)]
pub struct Stats {
    pub captured: AtomicU64,
    pub sent: AtomicU64,
    /// Microseconds the last capture spent being keyed and handed over.
    pub process_us: AtomicU64,
    pub connections: AtomicU64,
    pub width: AtomicU64,
    pub height: AtomicU64,
}

/// Shared between the capture callback (writer) and the pump (reader).
#[derive(Default)]
pub struct Slot {
    frame: Mutex<Option<Arc<Frame>>>,
}

impl Slot {
    pub fn put(&self, f: Frame) {
        *self.frame.lock().unwrap() = Some(Arc::new(f));
    }
    pub fn latest(&self) -> Option<Arc<Frame>> {
        self.frame.lock().unwrap().clone()
    }
}

/// Runs until `stop` is set. If the sink paces itself (NDI with clock_video),
/// sleeping here is only a fallback for sinks that do not.
/// While nobody is receiving, one frame a second keeps the source alive (FreeShow
/// does the same): no work is wasted on frames no one sees.
pub const IDLE_FPS: u32 = 1;

pub fn run(mut sink: Box<dyn Sink>, slot: Arc<Slot>, stats: Arc<Stats>, fps: u32, stop: Arc<AtomicBool>) {
    let mut last_conn = Instant::now() - Duration::from_secs(5);
    let mut receivers = 0i32;
    // converted once per new frame; repeats reuse it (always the newest frame)
    let mut current: Option<(Arc<Frame>, Vec<u8>)> = None;
    // a fade in progress: from this frame, since then
    let mut fading: Option<(Arc<Frame>, Instant)> = None;
    while !stop.load(Ordering::Relaxed) {
        let t = Instant::now();
        if last_conn.elapsed() > Duration::from_secs(1) {
            receivers = sink.connections().max(0);
            stats.connections.store(receivers as u64, Ordering::Relaxed);
            last_conn = Instant::now();
        }
        let rate = if receivers > 0 { fps.max(1) } else { IDLE_FPS };
        if let Some(f) = slot.latest() {
            if !current.as_ref().map_or(false, |(c, _)| Arc::ptr_eq(c, &f)) {
                // a fade from the frame on air (only when someone is watching)
                fading = match &current {
                    Some((old, _)) if f.fade_ms > 0 && receivers > 0 && old.width == f.width && old.height == f.height => Some((old.clone(), Instant::now())),
                    _ => None,
                };
                let uyva = super::convert::bgra_to_uyva(&f.data, f.width as usize, f.height as usize);
                current = Some((f, uyva));
            }
            let (f, uyva) = current.as_ref().unwrap();
            let mid = fading.as_ref().and_then(|(old, since)| {
                let t = since.elapsed().as_secs_f32() * 1000.0 / f.fade_ms.max(1) as f32;
                (t < 1.0).then(|| super::convert::bgra_to_uyva(&blend(&old.data, &f.data, t), f.width as usize, f.height as usize))
            });
            if mid.is_none() {
                fading = None;
            }
            sink.send(f.width, f.height, mid.as_deref().unwrap_or(uyva), rate);
            stats.sent.fetch_add(1, Ordering::Relaxed);
            stats.width.store(f.width as u64, Ordering::Relaxed);
            stats.height.store(f.height as u64, Ordering::Relaxed);
        }
        // wake every 100 ms at most, so a receiver connecting is noticed quickly
        let period = Duration::from_secs_f64(1.0 / rate as f64).min(if receivers > 0 { Duration::from_secs(1) } else { Duration::from_millis(1000) });
        let mut left = period.saturating_sub(t.elapsed());
        while left > Duration::ZERO && !stop.load(Ordering::Relaxed) {
            let step = left.min(Duration::from_millis(100));
            std::thread::sleep(step);
            left = left.saturating_sub(step);
            if receivers == 0 && last_conn.elapsed() > Duration::from_millis(300) && sink.connections() > 0 {
                break;
            }
        }
    }
}

/// A moving test pattern with a transparent half, for checking OBS/vMix without
/// any capture: colour bars slide across; the right half fades to see-through.
pub fn test_pattern(width: u32, height: u32, frame: u64) -> Frame {
    let bars: [[u8; 3]; 7] = [[192, 192, 192], [0, 192, 192], [192, 192, 0], [0, 192, 0], [192, 0, 192], [0, 0, 192], [192, 0, 0]];
    let mut data = vec![0u8; (width * height * 4) as usize];
    let shift = (frame * 8) as u32;
    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let bar = bars[(((x + shift) % width) * 7 / width) as usize];
            let alpha = if x < width / 2 { 255 } else { 255 - ((x - width / 2) * 255 / (width / 2)) as u8 };
            data[i..i + 4].copy_from_slice(&[bar[0], bar[1], bar[2], alpha]);
        }
    }
    Frame { width, height, data, fade_ms: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Count(Arc<AtomicU64>, i32);
    impl Sink for Count {
        fn send(&mut self, _w: u32, _h: u32, _d: &[u8], _fps: u32) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        fn connections(&self) -> i32 {
            self.1
        }
    }

    #[test]
    fn repeats_the_latest_frame_at_the_rate() {
        let sent = Arc::new(AtomicU64::new(0));
        let slot = Arc::new(Slot::default());
        let stats = Arc::new(Stats::default());
        let stop = Arc::new(AtomicBool::new(false));
        slot.put(Frame { width: 4, height: 2, data: vec![0; 32], fade_ms: 0 });
        let (s2, st2, stop2, sent2) = (slot.clone(), stats.clone(), stop.clone(), sent.clone());
        // 20 fps over 1 s: loose enough for Windows' coarse sleep timer (~16 ms steps)
        let h = std::thread::spawn(move || run(Box::new(Count(sent2, 2)), s2, st2, 20, stop2));
        std::thread::sleep(Duration::from_millis(1000));
        stop.store(true, Ordering::Relaxed);
        h.join().unwrap();
        let n = sent.load(Ordering::Relaxed);
        assert!((12..=24).contains(&n), "about 20 frames in 1 s at 20 fps, got {n}");
        assert_eq!(stats.connections.load(Ordering::Relaxed), 2);
        assert_eq!(stats.width.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn idles_at_one_frame_a_second_with_no_receivers() {
        let sent = Arc::new(AtomicU64::new(0));
        let slot = Arc::new(Slot::default());
        let stop = Arc::new(AtomicBool::new(false));
        slot.put(Frame { width: 4, height: 2, data: vec![0; 32], fade_ms: 0 });
        let (s2, stop2, sent2) = (slot.clone(), stop.clone(), sent.clone());
        let h = std::thread::spawn(move || run(Box::new(Count(sent2, 0)), s2, Arc::new(Stats::default()), 30, stop2));
        std::thread::sleep(Duration::from_millis(1500));
        stop.store(true, Ordering::Relaxed);
        h.join().unwrap();
        let n = sent.load(Ordering::Relaxed);
        assert!((1..=3).contains(&n), "about 2 frames in 1.5 s while idle, got {n}");
    }

    #[test]
    fn test_pattern_has_a_clear_half() {
        let f = test_pattern(64, 4, 0);
        assert_eq!(f.data[3], 255);
        assert!(f.data[(63 * 4 + 3) as usize] < 20);
    }
}

#[cfg(test)]
mod fade_tests {
    use super::*;

    /// Records the first byte of every frame sent (one receiver watching).
    struct Watch(Arc<Mutex<Vec<u8>>>);
    impl Sink for Watch {
        fn send(&mut self, _w: u32, _h: u32, d: &[u8], _fps: u32) {
            self.0.lock().unwrap().push(d[1]); // the Y (brightness) of the first pixel
        }
        fn connections(&self) -> i32 {
            1
        }
    }

    #[test]
    fn a_slide_change_fades_rather_than_cuts() {
        assert_eq!(blend(&[0, 100, 200, 255], &[200, 100, 0, 255], 0.5), vec![100, 100, 100, 255]);
        let seen = Arc::new(Mutex::new(vec![]));
        let slot = Arc::new(Slot::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (s2, st2, w) = (slot.clone(), stop.clone(), Watch(seen.clone()));
        slot.put(Frame { width: 2, height: 1, data: vec![0, 0, 0, 255, 0, 0, 0, 255], fade_ms: 0 });
        let h = std::thread::spawn(move || run(Box::new(w), s2, Arc::new(Stats::default()), 50, st2));
        std::thread::sleep(Duration::from_millis(150));
        slot.put(Frame { width: 2, height: 1, data: vec![255, 255, 255, 255, 255, 255, 255, 255], fade_ms: 400 });
        std::thread::sleep(Duration::from_millis(700));
        stop.store(true, Ordering::Relaxed);
        h.join().unwrap();
        let y = seen.lock().unwrap().clone();
        let (lo, hi) = (*y.first().unwrap(), *y.last().unwrap());
        let between = y.iter().filter(|v| **v > lo + 10 && **v < hi - 10).count();
        assert!(hi > lo + 100, "black to white");
        assert!(between >= 3, "frames in between (a fade, not a cut): {y:?}");
    }
}
