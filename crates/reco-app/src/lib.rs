//! Reco's app logic, free of any UI framework (DESIGN.md, Architecture).
//!
//! Module 1 brings the live preview: [`preview::worker::PreviewWorker`]
//! opens two camera videos and a calibration on its own thread and renders
//! the stitched view into textures a UI can show without a copy.

pub mod calibrate;
pub mod durations;
pub mod export;
pub mod files;
pub mod preview;
pub mod project;
pub mod recording;
pub mod reveal;
pub mod roi;
pub mod settings;
pub mod toasts;
