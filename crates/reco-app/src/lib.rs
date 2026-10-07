//! Reco's app logic, free of any UI framework (see the desktop crate's
//! DESIGN.md, Architecture).
//!
//! The live preview is [`preview::worker::PreviewWorker`]: it opens two
//! camera videos and a calibration on its own thread and renders the
//! stitched view into textures a UI can show without a copy.

pub mod ai;
pub mod bug_report;
pub mod calibrate;
pub mod durations;
pub mod export;
pub mod files;
pub mod help;
pub mod lens;
pub mod log_file;
pub mod preview;
pub mod project;
pub mod recording;
pub mod reveal;
pub mod roi;
pub mod settings;
pub mod telemetry;
pub mod toasts;
