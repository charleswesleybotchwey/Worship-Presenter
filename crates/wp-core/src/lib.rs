//! Worship Presenter core.
//!
//! Everything here is plain Rust with no UI or windowing, so it is unit-tested
//! on any machine and shared by the Tauri shell, the importers and the LAN remote.

pub mod db;
pub mod error;
pub mod import;
pub mod jet;
pub mod library;
pub mod live;
pub mod reference;
pub mod splitter;
pub mod text;

pub use error::{Error, Result};
