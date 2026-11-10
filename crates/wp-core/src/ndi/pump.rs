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
}

/// Something that can put frames on the network (the NDI sender, or a test double).
pub trait Sink: Send {
    fn send(&mut self, f: &Frame, fps: u32);
    fn connections(&self) -> i32;
}

impl Sink for super::sdk::Sender {
    fn send(&mut self, f: &Frame, fps: u32) {
        self.send_bgra(f.width, f.height, f.width * 4, &f.data, fps);
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
    fn latest(&self) -> Option<Arc<Frame>> {
        self.frame.lock().unwrap().clone()
    }
}

/// Runs until `stop` is set. If the sink paces itself (NDI with clock_video),
/// sleeping here is only a fallback for sinks that do not.
pub fn run(mut sink: Box<dyn Sink>, slot: Arc<Slot>, stats: Arc<Stats>, fps: u32, stop: Arc<AtomicBool>) {
    let period = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
    let mut last_conn = Instant::now() - Duration::from_secs(5);
    while !stop.load(Ordering::Relaxed) {
        let t = Instant::now();
        if let Some(f) = slot.latest() {
            sink.send(&f, fps);
            stats.sent.fetch_add(1, Ordering::Relaxed);
            stats.width.store(f.width as u64, Ordering::Relaxed);
            stats.height.store(f.height as u64, Ordering::Relaxed);
        }
        if last_conn.elapsed() > Duration::from_secs(1) {
            stats.connections.store(sink.connections().max(0) as u64, Ordering::Relaxed);
            last_conn = Instant::now();
        }
        if let Some(rest) = period.checked_sub(t.elapsed()) {
            std::thread::sleep(rest);
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
    Frame { width, height, data }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Count(Arc<AtomicU64>);
    impl Sink for Count {
        fn send(&mut self, _f: &Frame, _fps: u32) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        fn connections(&self) -> i32 {
            2
        }
    }

    #[test]
    fn repeats_the_latest_frame_at_the_rate() {
        let sent = Arc::new(AtomicU64::new(0));
        let slot = Arc::new(Slot::default());
        let stats = Arc::new(Stats::default());
        let stop = Arc::new(AtomicBool::new(false));
        slot.put(Frame { width: 4, height: 2, data: vec![0; 32] });
        let (s2, st2, stop2, sent2) = (slot.clone(), stats.clone(), stop.clone(), sent.clone());
        // 20 fps over 1 s: loose enough for Windows' coarse sleep timer (~16 ms steps)
        let h = std::thread::spawn(move || run(Box::new(Count(sent2)), s2, st2, 20, stop2));
        std::thread::sleep(Duration::from_millis(1000));
        stop.store(true, Ordering::Relaxed);
        h.join().unwrap();
        let n = sent.load(Ordering::Relaxed);
        assert!((12..=24).contains(&n), "about 20 frames in 1 s at 20 fps, got {n}");
        assert_eq!(stats.connections.load(Ordering::Relaxed), 2);
        assert_eq!(stats.width.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn test_pattern_has_a_clear_half() {
        let f = test_pattern(64, 4, 0);
        assert_eq!(f.data[3], 255);
        assert!(f.data[(63 * 4 + 3) as usize] < 20);
    }
}
