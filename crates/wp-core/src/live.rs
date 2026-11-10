//! The single source of truth for what is on air.
//!
//! Every change bumps `revision`; outputs drop any message older than the last one
//! they rendered and acknowledge the revision they actually show. The durable part
//! is written to state.json atomically (temp file, then rename) on every change, so
//! a power cut reopens at the same schedule item and line.

use crate::splitter::{LangMode, Position};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Slide is showing.
    Live,
    /// Black (or transparent on keyed outputs, per output rule).
    Blank,
    /// Logo image.
    Logo,
    /// Text hidden, background kept.
    Clear,
}

/// What each output does when the operator presses Logo or Blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputRule {
    Logo,
    Blank,
    Transparent,
    /// Keep showing the slide.
    Ignore,
}

/// The visual an output should render for a given global mode.
pub fn resolve(mode: Mode, on_logo: OutputRule, on_blank: OutputRule) -> Mode {
    let pick = |rule: OutputRule| match rule {
        OutputRule::Logo => Mode::Logo,
        OutputRule::Blank | OutputRule::Transparent => Mode::Blank,
        OutputRule::Ignore => Mode::Live,
    };
    match mode {
        Mode::Logo => pick(on_logo),
        Mode::Blank => pick(on_blank),
        m => m,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveState {
    pub revision: u64,
    pub schedule_id: Option<i64>,
    pub item_idx: Option<usize>,
    pub position: Option<Position>,
    pub slide_idx: usize,
    pub mode: Mode,
    pub lang_mode: LangMode,
    pub lines_per_slide: u8,
    /// Mode to return to when "Undo go live" is pressed.
    #[serde(default)]
    pub previous: Option<Box<Snapshot>>,
    /// Live video: playing or paused, and a counter that restarts it from the top.
    #[serde(default = "yes")]
    pub media_playing: bool,
    #[serde(default)]
    pub media_restart: u64,
    /// A note from the operator shown on the stage display only ("Two minutes").
    #[serde(default)]
    pub stage_message: Option<String>,
    #[serde(default)]
    pub timer: Option<crate::cues::Timer>,
    #[serde(default)]
    pub alert: Option<crate::cues::Alert>,
    /// Announcements that advance by themselves.
    #[serde(default)]
    pub auto: Option<AutoAdvance>,
    /// When the current slide went up (ms since 1970), for auto-advance and "next in".
    #[serde(default)]
    pub slide_since_ms: i64,
    /// Chorus repeat counter shown on the stage display.
    #[serde(default)]
    pub repeat: Option<Repeat>,
    /// Lining out: one line at a time, next line large on the stage display.
    #[serde(default)]
    pub lining_out: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoAdvance {
    /// Milliseconds each slide shows.
    pub durations_ms: Vec<i64>,
    pub looping: bool,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Repeat {
    /// The section being counted, e.g. "C".
    pub label: String,
    pub total: u32,
    /// Times it has started so far (the first time counts as 1).
    pub done: u32,
}

impl LiveState {
    /// The slide to move to if auto-advance is due at `now`, or None.
    pub fn auto_due(&self, now: i64, slides: usize) -> Option<usize> {
        let a = self.auto.as_ref()?;
        if a.paused || self.mode != Mode::Live || slides == 0 || self.item_idx.is_none() {
            return None;
        }
        let d = *a.durations_ms.get(self.slide_idx)?;
        if now - self.slide_since_ms < d {
            return None;
        }
        if self.slide_idx + 1 < slides {
            Some(self.slide_idx + 1)
        } else if a.looping {
            Some(0)
        } else {
            None
        }
    }
}

fn yes() -> bool {
    true
}

/// The part of the state that "Undo go live" restores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub item_idx: Option<usize>,
    pub position: Option<Position>,
    pub slide_idx: usize,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            revision: 0,
            schedule_id: None,
            item_idx: None,
            position: None,
            slide_idx: 0,
            mode: Mode::Blank,
            lang_mode: LangMode::Primary,
            lines_per_slide: 2,
            previous: None,
            media_playing: true,
            media_restart: 0,
            stage_message: None,
            timer: None,
            alert: None,
            auto: None,
            slide_since_ms: 0,
            repeat: None,
            lining_out: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    GoLive { item_idx: usize, slide_idx: usize, position: Option<Position> },
    Show { slide_idx: usize, position: Option<Position> },
    SetMode { mode: Mode },
    SetLinesPerSlide { lines: u8 },
    SetLangMode { lang_mode: LangMode },
    UndoGoLive,
    SetMediaPlaying { playing: bool },
    RestartMedia,
    SetStageMessage { message: Option<String> },
    SetTimer { timer: Option<crate::cues::Timer> },
    SetAlert { alert: Option<crate::cues::Alert> },
    SetAuto { auto: Option<AutoAdvance> },
    SetRepeat { repeat: Option<Repeat> },
    SetLiningOut { on: bool },
}

impl LiveState {
    /// Blank, Logo or Clear is holding the screens. While latched, going live and
    /// stepping through slides are refused until the operator resumes output.
    pub fn latched(&self) -> bool {
        self.item_idx.is_some() && self.mode != Mode::Live
    }

    /// Applies a command and returns the new state with the revision bumped.
    /// Commands that change nothing still bump, so a reconnecting output resyncs.
    pub fn apply(&self, c: Command) -> LiveState {
        let mut n = self.clone();
        n.revision = self.revision + 1;
        match c {
            Command::GoLive { item_idx, slide_idx, position } => {
                n.previous = Some(Box::new(Snapshot { item_idx: self.item_idx, position: self.position, slide_idx: self.slide_idx }));
                n.item_idx = Some(item_idx);
                n.slide_idx = slide_idx;
                n.position = position;
                n.mode = Mode::Live;
                n.media_playing = true;
                n.media_restart = self.media_restart + 1;
                n.auto = None;
                n.repeat = None;
            }
            Command::Show { slide_idx, position } => {
                n.slide_idx = slide_idx;
                n.position = position;
            }
            // Blank, Logo and Clear latch; pressing one again keeps it. Only Resume
            // (SetMode Live) brings the confirmed cue back.
            Command::SetMode { mode } => n.mode = mode,
            Command::SetLinesPerSlide { lines } => n.lines_per_slide = lines.clamp(1, 8),
            Command::SetLangMode { lang_mode } => n.lang_mode = lang_mode,
            Command::SetMediaPlaying { playing } => n.media_playing = playing,
            Command::SetTimer { timer } => n.timer = timer,
            Command::SetAlert { alert } => n.alert = alert,
            Command::SetAuto { auto } => n.auto = auto,
            Command::SetRepeat { repeat } => n.repeat = repeat,
            Command::SetLiningOut { on } => n.lining_out = on,
            Command::SetStageMessage { message } => n.stage_message = message.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()),
            Command::RestartMedia => {
                n.media_restart = self.media_restart + 1;
                n.media_playing = true;
            }
            Command::UndoGoLive => {
                if let Some(p) = self.previous.as_deref() {
                    n.item_idx = p.item_idx;
                    n.position = p.position;
                    n.slide_idx = p.slide_idx;
                    n.previous = None;
                }
            }
        }
        n
    }

    /// Writes the durable state atomically: a crash mid-write leaves the old file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("json.tmp");
        let mut durable = self.clone();
        durable.previous = None;
        {
            // flush through the handle that wrote it (Windows refuses to flush a
            // read-only handle), and close it before the swap (Windows cannot rename
            // an open file)
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(&durable)?)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Restores after a restart. Projection stays off unless `project_on_restart`.
    pub fn load(path: &Path, project_on_restart: bool) -> LiveState {
        let mut s: LiveState = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if !project_on_restart {
            s.mode = Mode::Blank;
        }
        s.revision += 1;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_increase_and_toggles_work() {
        let s = LiveState::default();
        let a = s.apply(Command::GoLive { item_idx: 2, slide_idx: 0, position: None });
        assert_eq!((a.revision, a.mode), (1, Mode::Live));
        let b = a.apply(Command::SetMode { mode: Mode::Logo });
        assert_eq!(b.mode, Mode::Logo);
        assert!(b.latched());
        let c = b.apply(Command::SetMode { mode: Mode::Logo });
        assert_eq!(c.mode, Mode::Logo, "pressing Logo again keeps the logo; it does not toggle");
        let d = c.apply(Command::SetMode { mode: Mode::Live });
        assert_eq!((d.mode, d.item_idx, d.slide_idx), (Mode::Live, a.item_idx, a.slide_idx), "resume restores the confirmed cue");
        assert!(d.revision > c.revision);
    }

    #[test]
    fn undo_go_live_restores_previous_item() {
        let a = LiveState::default().apply(Command::GoLive { item_idx: 1, slide_idx: 3, position: None });
        let b = a.apply(Command::GoLive { item_idx: 4, slide_idx: 0, position: None });
        let c = b.apply(Command::UndoGoLive);
        assert_eq!((c.item_idx, c.slide_idx), (Some(1), 3));
    }

    #[test]
    fn auto_advance_and_loop() {
        let mut s = LiveState::default().apply(Command::GoLive { item_idx: 0, slide_idx: 0, position: None });
        s.auto = Some(AutoAdvance { durations_ms: vec![1000, 1000], looping: true, paused: false });
        s.slide_since_ms = 10_000;
        assert_eq!(s.auto_due(10_500, 2), None);
        assert_eq!(s.auto_due(11_000, 2), Some(1));
        s.slide_idx = 1;
        assert_eq!(s.auto_due(11_000, 2), Some(0), "loops back");
        s.auto.as_mut().unwrap().looping = false;
        assert_eq!(s.auto_due(99_000, 2), None, "stops on the last slide");
        s.auto.as_mut().unwrap().looping = true;
        let held = s.apply(Command::SetMode { mode: Mode::Blank });
        assert_eq!(held.auto_due(99_000, 2), None, "nothing advances while blank");
        let next = held.apply(Command::GoLive { item_idx: 0, slide_idx: 0, position: None });
        assert!(next.auto.is_none() && next.repeat.is_none(), "a new item starts clean");
    }

    #[test]
    fn per_output_rules() {
        assert_eq!(resolve(Mode::Logo, OutputRule::Logo, OutputRule::Blank), Mode::Logo);
        assert_eq!(resolve(Mode::Logo, OutputRule::Transparent, OutputRule::Transparent), Mode::Blank);
        assert_eq!(resolve(Mode::Blank, OutputRule::Logo, OutputRule::Ignore), Mode::Live);
    }

    #[test]
    fn atomic_save_and_safe_restore() {
        let dir = std::env::temp_dir().join(format!("wp-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("state.json");
        let s = LiveState::default().apply(Command::GoLive { item_idx: 5, slide_idx: 2, position: None });
        s.save(&p).unwrap();
        let r = LiveState::load(&p, false);
        assert_eq!((r.item_idx, r.slide_idx, r.mode), (Some(5), 2, Mode::Blank));
        let r = LiveState::load(&p, true);
        assert_eq!(r.mode, Mode::Live);
        assert!(!p.with_extension("json.tmp").exists());
        // saving again replaces the file (the case Windows is strict about)
        s.apply(Command::Show { slide_idx: 3, position: None }).save(&p).unwrap();
        assert_eq!(LiveState::load(&p, false).slide_idx, 3);
    }
}
