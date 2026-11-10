//! The real sound card, through rodio, on a thread of its own.

use crate::{Backend, Engine, Rules, Status, Track};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Rodio {
    out: MixerDeviceSink,
}

impl Backend for Rodio {
    type Handle = Player;

    fn load(&mut self, path: &std::path::Path) -> Result<(Player, Option<u64>), String> {
        let file = std::fs::File::open(path).map_err(|e| format!("the file cannot be opened ({e})"))?;
        let dec = Decoder::new(std::io::BufReader::new(file)).map_err(|e| format!("not a sound file this computer can play ({e})"))?;
        let length = dec.total_duration().map(|d| d.as_millis() as u64);
        let p = Player::connect_new(self.out.mixer());
        p.set_volume(0.0);
        p.pause();
        p.append(dec);
        Ok((p, length))
    }
    fn set_volume(&mut self, h: &Player, v: f32) {
        h.set_volume(v as rodio::Float);
    }
    fn set_paused(&mut self, h: &Player, paused: bool) {
        if paused {
            h.pause()
        } else {
            h.play()
        }
    }
    fn position_ms(&self, h: &Player) -> u64 {
        h.get_pos().as_millis() as u64
    }
    fn finished(&self, h: &Player) -> bool {
        h.empty()
    }
    fn stop(&mut self, h: Player) {
        h.stop();
    }
}

pub enum Cmd {
    Playlist(Vec<Track>),
    Play(Option<usize>),
    Pause(bool),
    Next,
    Previous,
    Stop,
    Volume(f32),
    Rules(Rules),
    GoLive(String),
    Unduck,
    Quit,
}

/// Handle used by the desktop app. Dropping it stops the music thread.
pub struct Music {
    tx: Sender<Cmd>,
    status: Arc<Mutex<Status>>,
}

impl Music {
    /// Opens the default sound output. Fails (with a reason) if there is none.
    pub fn start(rules: Rules) -> Result<Music, String> {
        let (tx, rx) = channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let (ready_tx, ready_rx) = channel();
        let st = status.clone();
        std::thread::Builder::new()
            .name("music".into())
            .spawn(move || {
                // the output stream must be created on the thread that uses it
                match DeviceSinkBuilder::open_default_sink() {
                    Ok(mut out) => {
                        out.log_on_drop(false);
                        let _ = ready_tx.send(Ok(()));
                        let mut e = Engine::new(Rodio { out });
                        e.rules = rules;
                        run(e, rx, st);
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(format!("No sound output was found ({err}). Check the computer's speakers or audio cable.")));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        ready_rx.recv().map_err(|_| "The music player stopped while starting.".to_string())??;
        Ok(Music { tx, status })
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}

impl Drop for Music {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Quit);
    }
}

fn run(mut e: Engine<Rodio>, rx: Receiver<Cmd>, status: Arc<Mutex<Status>>) {
    let t0 = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Cmd::Quit) | Err(RecvTimeoutError::Disconnected) => {
                e.stop(0);
                e.step(t0.elapsed().as_millis() as u64 + 1);
                return;
            }
            Ok(c) => {
                e.step(t0.elapsed().as_millis() as u64);
                match c {
                    Cmd::Playlist(t) => e.set_playlist(t),
                    Cmd::Play(i) => e.play(i),
                    Cmd::Pause(p) => e.pause(p),
                    Cmd::Next => e.next(),
                    Cmd::Previous => e.previous(),
                    Cmd::Stop => e.stop(2500),
                    Cmd::Volume(v) => e.set_volume(v),
                    Cmd::Rules(r) => e.rules = r,
                    Cmd::GoLive(k) => e.on_go_live(&k),
                    Cmd::Unduck => e.unduck(),
                    Cmd::Quit => unreachable!(),
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        e.step(t0.elapsed().as_millis() as u64);
        *status.lock().unwrap() = e.status();
    }
}
