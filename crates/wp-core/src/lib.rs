//! Worship Presenter core.
//!
//! Everything here is plain Rust with no UI or windowing, so it is unit-tested
//! on any machine and shared by the Tauri shell, the importers and the LAN remote.

pub mod announcements;
pub mod backup;
pub mod browse;
pub mod chords;
pub mod cues;
pub mod db;
pub mod error;
pub mod import;
pub mod jet;
pub mod item_collections;
pub mod find_replace;
pub mod library;
pub mod lyrics_online;
pub mod media;
pub mod ndi;
pub mod options;
pub mod outline;
pub mod presentations;
pub mod profiles;
pub mod live;
pub mod reference;
pub mod schedule;
pub mod songs;
pub mod macros;
pub mod props;
pub mod smart;
pub mod timelines;
pub mod speech_quote;
pub mod sync;
pub mod speech_ref;
pub mod splitter;
pub mod text;
pub mod theme_rules;
pub mod themes;

pub use error::{Error, Result};
