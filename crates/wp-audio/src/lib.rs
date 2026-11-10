//! Background music for the service: a playlist from the media library, crossfades
//! between tracks, and rules for what happens when something goes live:
//! keep playing, lower the music under the words, or fade it out; and optionally
//! start the music when announcements go live.
//!
//! `Engine` holds all the decisions and is driven by `step(now_ms)`; it talks to the
//! sound card through `Backend`, so it is tested with a fake backend and clock.
//! `rodio_backend` is the real one, and `Music` runs it on its own thread so a
//! window reload never stops what the congregation hears.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod rodio_backend;
pub use rodio_backend::Music;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnLive {
    /// Leave the music as it is.
    Keep,
    /// Lower it under the words (to a quarter).
    Duck,
    /// Fade it out and stop.
    FadeOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rules {
    pub on_live: OnLive,
    /// Start the music when an announcement deck goes live.
    pub with_announcements: bool,
    pub crossfade_ms: u64,
    /// Start the playlist again after the last track.
    pub looping: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { on_live: OnLive::FadeOut, with_announcements: true, crossfade_ms: 4000, looping: true }
    }
}

pub trait Backend {
    type Handle;
    /// Starts a file paused at volume 0; returns its length when known.
    fn load(&mut self, path: &std::path::Path) -> Result<(Self::Handle, Option<u64>), String>;
    fn set_volume(&mut self, h: &Self::Handle, v: f32);
    fn set_paused(&mut self, h: &Self::Handle, paused: bool);
    fn position_ms(&self, h: &Self::Handle) -> u64;
    fn finished(&self, h: &Self::Handle) -> bool;
    fn stop(&mut self, h: Self::Handle);
}

/// A volume that moves from `from` to `to` over `dur` ms.
#[derive(Debug, Clone, Copy)]
struct Ramp {
    from: f32,
    to: f32,
    start: u64,
    dur: u64,
}

impl Ramp {
    fn hold(v: f32) -> Ramp {
        Ramp { from: v, to: v, start: 0, dur: 0 }
    }
    fn to(&self, now: u64, to: f32, dur: u64) -> Ramp {
        Ramp { from: self.at(now), to, start: now, dur }
    }
    fn at(&self, now: u64) -> f32 {
        if self.dur == 0 || now >= self.start + self.dur {
            return self.to;
        }
        let t = (now.saturating_sub(self.start)) as f32 / self.dur as f32;
        self.from + (self.to - self.from) * t
    }
    fn done(&self, now: u64) -> bool {
        now >= self.start + self.dur
    }
}

struct Playing<H> {
    handle: H,
    index: usize,
    length: Option<u64>,
    /// fade-in at the start (crossfade), fade-out when leaving
    ramp: Ramp,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub playing: bool,
    pub paused: bool,
    pub title: Option<String>,
    pub index: Option<usize>,
    pub position_ms: u64,
    pub length_ms: Option<u64>,
    pub volume: f32,
    pub ducked: bool,
    pub playlist: Vec<Track>,
    pub problem: Option<String>,
}

pub struct Engine<B: Backend> {
    backend: B,
    pub playlist: Vec<Track>,
    pub rules: Rules,
    volume: f32,
    duck: Ramp,
    /// master fade (fade out to stop, fade in from silence)
    master: Ramp,
    stop_when_silent: bool,
    current: Option<Playing<B::Handle>>,
    leaving: Vec<Playing<B::Handle>>,
    paused: bool,
    problem: Option<String>,
    now: u64,
}

const DUCK_TO: f32 = 0.25;
const DUCK_MS: u64 = 1500;
const FADE_OUT_MS: u64 = 2500;
const FADE_IN_MS: u64 = 2000;

impl<B: Backend> Engine<B> {
    pub fn new(backend: B) -> Self {
        Engine {
            backend,
            playlist: vec![],
            rules: Rules::default(),
            volume: 0.8,
            duck: Ramp::hold(1.0),
            master: Ramp::hold(1.0),
            stop_when_silent: false,
            current: None,
            leaving: vec![],
            paused: false,
            problem: None,
            now: 0,
        }
    }

    pub fn set_playlist(&mut self, tracks: Vec<Track>) {
        // keep playing the current track if it is still in the list
        if let Some(c) = &mut self.current {
            let id = self.playlist.get(c.index).map(|t| t.id);
            match tracks.iter().position(|t| Some(t.id) == id) {
                Some(i) => c.index = i,
                None => {
                    let c = self.current.take().unwrap();
                    self.fade_away(c, 800);
                }
            }
        }
        self.playlist = tracks;
    }

    fn start(&mut self, index: usize, fade_in_ms: u64) {
        let Some(t) = self.playlist.get(index).cloned() else { return };
        match self.backend.load(&t.path) {
            Ok((h, length)) => {
                self.backend.set_paused(&h, false);
                self.problem = None;
                let ramp = Ramp { from: 0.0, to: 1.0, start: self.now, dur: fade_in_ms };
                self.current = Some(Playing { handle: h, index, length, ramp });
            }
            Err(e) => {
                // skip a broken file rather than stopping the music, and say so
                let msg = format!("“{}” could not be played: {e}", t.title);
                if self.playlist.len() > 1 && index + 1 < self.playlist.len() {
                    self.start(index + 1, fade_in_ms);
                }
                self.problem = Some(msg);
            }
        }
    }

    fn fade_away(&mut self, mut p: Playing<B::Handle>, ms: u64) {
        p.ramp = p.ramp.to(self.now, 0.0, ms);
        self.leaving.push(p);
    }

    /// Play from `index`, or carry on / start from the top.
    pub fn play(&mut self, index: Option<usize>) {
        self.paused = false;
        self.stop_when_silent = false;
        self.master = self.master.to(self.now, 1.0, FADE_IN_MS);
        match (index, self.current.as_ref().map(|c| c.index)) {
            (Some(i), Some(cur)) if i == cur => {}
            (Some(i), _) => {
                if let Some(c) = self.current.take() {
                    self.fade_away(c, self.rules.crossfade_ms.min(1500));
                }
                self.start(i, self.rules.crossfade_ms.min(1500));
            }
            (None, Some(_)) => {}
            (None, None) => self.start(0, FADE_IN_MS),
        }
        self.apply_pause();
    }

    pub fn pause(&mut self, paused: bool) {
        self.paused = paused;
        self.apply_pause();
    }

    fn apply_pause(&mut self) {
        if let Some(c) = &self.current {
            self.backend.set_paused(&c.handle, self.paused);
        }
        for l in &self.leaving {
            self.backend.set_paused(&l.handle, self.paused);
        }
    }

    pub fn next(&mut self) {
        let i = self.current.as_ref().map(|c| c.index + 1).unwrap_or(0);
        self.advance_to(i);
    }

    pub fn previous(&mut self) {
        let i = self.current.as_ref().map(|c| c.index.saturating_sub(1)).unwrap_or(0);
        self.advance_to(i);
    }

    fn advance_to(&mut self, i: usize) {
        let i = if i >= self.playlist.len() {
            if self.rules.looping { 0 } else { return self.stop(FADE_OUT_MS) }
        } else {
            i
        };
        if let Some(c) = self.current.take() {
            self.fade_away(c, self.rules.crossfade_ms);
        }
        self.start(i, self.rules.crossfade_ms);
        self.apply_pause();
    }

    /// Fades everything out over `ms`, then stops.
    pub fn stop(&mut self, ms: u64) {
        self.master = self.master.to(self.now, 0.0, ms);
        self.stop_when_silent = true;
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    /// What the service did: "song", "scripture", "media" or "announcement".
    pub fn on_go_live(&mut self, kind: &str) {
        if kind == "announcement" {
            self.duck = self.duck.to(self.now, 1.0, DUCK_MS);
            if self.rules.with_announcements && (self.current.is_none() || self.stop_when_silent) && !self.playlist.is_empty() {
                self.play(None);
            }
            return;
        }
        if self.current.is_none() {
            return;
        }
        match self.rules.on_live {
            OnLive::Keep => {}
            OnLive::Duck => self.duck = self.duck.to(self.now, DUCK_TO, DUCK_MS),
            OnLive::FadeOut => self.stop(FADE_OUT_MS),
        }
    }

    /// Back to full volume after a duck (the operator's "Raise music").
    pub fn unduck(&mut self) {
        self.duck = self.duck.to(self.now, 1.0, DUCK_MS);
    }

    /// Advances time: volumes, crossfades, finished tracks.
    pub fn step(&mut self, now: u64) {
        self.now = now;
        let common = self.volume * self.duck.at(now) * self.master.at(now);
        // tracks on their way out
        let mut keep = vec![];
        for l in std::mem::take(&mut self.leaving) {
            if l.ramp.done(now) && l.ramp.to == 0.0 || self.backend.finished(&l.handle) {
                self.backend.stop(l.handle);
            } else {
                self.backend.set_volume(&l.handle, common * l.ramp.at(now));
                keep.push(l);
            }
        }
        self.leaving = keep;
        if self.stop_when_silent && self.master.done(now) && self.master.to == 0.0 {
            if let Some(c) = self.current.take() {
                self.backend.stop(c.handle);
            }
            for l in std::mem::take(&mut self.leaving) {
                self.backend.stop(l.handle);
            }
            self.stop_when_silent = false;
            self.master = Ramp::hold(1.0);
            self.duck = Ramp::hold(1.0);
            return;
        }
        let Some(c) = &self.current else { return };
        if self.paused {
            return;
        }
        self.backend.set_volume(&c.handle, common * c.ramp.at(now));
        // crossfade into the next track near the end; or move on when it has ended
        let near_end = c.length.map_or(false, |len| self.backend.position_ms(&c.handle) + self.rules.crossfade_ms >= len);
        if near_end || self.backend.finished(&c.handle) {
            let next = c.index + 1;
            if next < self.playlist.len() || self.rules.looping {
                self.advance_to(next);
            } else if self.backend.finished(&c.handle) {
                let c = self.current.take().unwrap();
                self.backend.stop(c.handle);
            }
        }
    }

    pub fn status(&self) -> Status {
        let c = self.current.as_ref();
        Status {
            playing: c.is_some() && !self.stop_when_silent,
            paused: self.paused,
            title: c.and_then(|c| self.playlist.get(c.index)).map(|t| t.title.clone()),
            index: c.map(|c| c.index),
            position_ms: c.map(|c| self.backend.position_ms(&c.handle)).unwrap_or(0),
            length_ms: c.and_then(|c| c.length),
            volume: self.volume,
            ducked: self.duck.to < 1.0,
            playlist: self.playlist.clone(),
            problem: self.problem.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Pretend sound card: every track is `len` ms long; time comes from `step`.
    #[derive(Default)]
    struct Fake {
        now: Rc<RefCell<u64>>,
        started: Vec<(u64, String, u64)>, // id, path, start time
        volumes: std::collections::HashMap<u64, f32>,
        stopped: Vec<u64>,
        next_id: u64,
    }
    impl Backend for Fake {
        type Handle = u64;
        fn load(&mut self, path: &std::path::Path) -> Result<(u64, Option<u64>), String> {
            if path.to_string_lossy().contains("broken") {
                return Err("not a sound file".into());
            }
            self.next_id += 1;
            self.started.push((self.next_id, path.to_string_lossy().into(), *self.now.borrow()));
            Ok((self.next_id, Some(60_000)))
        }
        fn set_volume(&mut self, h: &u64, v: f32) {
            self.volumes.insert(*h, v);
        }
        fn set_paused(&mut self, _h: &u64, _p: bool) {}
        fn position_ms(&self, h: &u64) -> u64 {
            let start = self.started.iter().find(|s| s.0 == *h).map(|s| s.2).unwrap_or(0);
            *self.now.borrow() - start
        }
        fn finished(&self, h: &u64) -> bool {
            self.position_ms(h) >= 60_000
        }
        fn stop(&mut self, h: u64) {
            self.stopped.push(h);
        }
    }

    fn tracks(n: usize) -> Vec<Track> {
        (0..n).map(|i| Track { id: i as i64 + 1, title: format!("Track {}", i + 1), path: format!("audio/t{}.mp3", i + 1).into() }).collect()
    }

    fn run(e: &mut Engine<Fake>, clock: &Rc<RefCell<u64>>, to: u64) {
        while *clock.borrow() < to {
            *clock.borrow_mut() += 50;
            let t = *clock.borrow();
            e.step(t);
        }
    }

    fn engine() -> (Engine<Fake>, Rc<RefCell<u64>>) {
        let clock = Rc::new(RefCell::new(0));
        let e = Engine::new(Fake { now: clock.clone(), ..Default::default() });
        (e, clock)
    }

    #[test]
    fn fades_in_then_crossfades_into_the_next_track() {
        let (mut e, c) = engine();
        e.set_playlist(tracks(2));
        e.play(None);
        run(&mut e, &c, 1000);
        let v = e.backend.volumes[&1];
        assert!(v > 0.2 && v < 0.6, "half way through the 2 s fade-in: {v}");
        run(&mut e, &c, 30_000);
        assert!((e.backend.volumes[&1] - 0.8).abs() < 0.01, "full user volume");
        run(&mut e, &c, 57_000); // crossfade starts 4 s before the end
        assert_eq!(e.status().title.as_deref(), Some("Track 2"));
        run(&mut e, &c, 58_000);
        let (a, b) = (e.backend.volumes[&1], e.backend.volumes[&2]);
        assert!(a > 0.0 && b > 0.0, "both audible during the crossfade: {a} {b}");
        run(&mut e, &c, 62_000);
        assert!(e.backend.stopped.contains(&1), "old track released after the crossfade");
    }

    #[test]
    fn going_live_follows_the_rules() {
        let (mut e, c) = engine();
        e.set_playlist(tracks(3));
        // music starts by itself with the welcome loop
        e.on_go_live("announcement");
        assert!(e.status().playing);
        run(&mut e, &c, 5_000);
        // a hymn goes live: fade out and stop (default rule)
        e.on_go_live("song");
        run(&mut e, &c, 6_000);
        assert!(e.backend.volumes[&1] < 0.8);
        run(&mut e, &c, 8_000);
        assert!(!e.status().playing && e.backend.stopped.contains(&1));

        // duck rule: lower under the words, back up for announcements
        e.rules.on_live = OnLive::Duck;
        e.play(None);
        run(&mut e, &c, 12_000);
        e.on_go_live("scripture");
        run(&mut e, &c, 14_000);
        let h = e.status().index.map(|_| *e.backend.volumes.keys().max().unwrap()).unwrap();
        assert!((e.backend.volumes[&h] - 0.8 * DUCK_TO).abs() < 0.02, "ducked to a quarter");
        assert!(e.status().ducked);
        e.on_go_live("announcement");
        run(&mut e, &c, 16_000);
        assert!((e.backend.volumes[&h] - 0.8).abs() < 0.02, "back up for announcements");

        // keep rule: nothing changes
        e.rules.on_live = OnLive::Keep;
        e.on_go_live("song");
        run(&mut e, &c, 18_000);
        assert!((e.backend.volumes[&h] - 0.8).abs() < 0.02);
    }

    #[test]
    fn broken_files_are_skipped_and_reported() {
        let (mut e, c) = engine();
        let mut t = tracks(2);
        t[0].path = "audio/broken.mp3".into();
        e.set_playlist(t);
        e.play(None);
        run(&mut e, &c, 500);
        let s = e.status();
        assert_eq!(s.title.as_deref(), Some("Track 2"));
        assert!(s.problem.unwrap().contains("Track 1"));
    }

    #[test]
    fn last_track_stops_unless_looping() {
        let (mut e, c) = engine();
        e.rules.looping = false;
        e.set_playlist(tracks(1));
        e.play(None);
        run(&mut e, &c, 61_000);
        assert!(!e.status().playing);
        e.rules.looping = true;
        e.play(None);
        run(&mut e, &c, 61_000 + 58_000);
        assert!(e.status().playing, "starts the list again");
    }

    #[test]
    fn changing_the_playlist_keeps_the_current_track() {
        let (mut e, c) = engine();
        e.set_playlist(tracks(3));
        e.play(Some(1));
        run(&mut e, &c, 1_000);
        let mut t = tracks(3);
        t.reverse();
        e.set_playlist(t);
        assert_eq!(e.status().title.as_deref(), Some("Track 2"));
        assert_eq!(e.status().index, Some(1));
    }
}
