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
}

impl LiveState {
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
            }
            Command::Show { slide_idx, position } => {
                n.slide_idx = slide_idx;
                n.position = position;
                if n.mode == Mode::Blank || n.mode == Mode::Logo {
                    n.mode = Mode::Live;
                }
            }
            Command::SetMode { mode } => {
                // pressing the active mode again toggles back to live
                n.mode = if self.mode == mode && mode != Mode::Live { Mode::Live } else { mode };
            }
            Command::SetLinesPerSlide { lines } => n.lines_per_slide = lines.clamp(1, 8),
            Command::SetLangMode { lang_mode } => n.lang_mode = lang_mode,
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
        std::fs::write(&tmp, serde_json::to_vec_pretty(&durable)?)?;
        let f = std::fs::File::open(&tmp)?;
        f.sync_all()?;
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
        let c = b.apply(Command::SetMode { mode: Mode::Logo });
        assert_eq!(c.mode, Mode::Live);
        assert!(c.revision > b.revision);
    }

    #[test]
    fn undo_go_live_restores_previous_item() {
        let a = LiveState::default().apply(Command::GoLive { item_idx: 1, slide_idx: 3, position: None });
        let b = a.apply(Command::GoLive { item_idx: 4, slide_idx: 0, position: None });
        let c = b.apply(Command::UndoGoLive);
        assert_eq!((c.item_idx, c.slide_idx), (Some(1), 3));
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
    }
}
