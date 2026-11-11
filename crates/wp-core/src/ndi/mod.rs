//! NDI output (Phase 4). The NDI Runtime is loaded at run time from where the
//! official installer puts it, so nothing proprietary is linked into the app.
//! Frames come from a capture of an output window (Windows), are keyed from a
//! pure-green background to real transparency, and are paced by the pump.
//!
//! NDI® is a registered trademark of Vizrt NDI AB.

pub mod convert;
pub mod key;
pub mod pump;
pub mod sdk;
