//! Reco's app logic, free of any UI framework (DESIGN.md, Architecture).
//!
//! Module 1 brings the live preview: [`preview::worker::PreviewWorker`]
//! opens two camera videos and a calibration on its own thread and renders
//! the stitched view into textures a UI can show without a copy.

pub mod preview;
pub mod recording;
pub mod reveal;
pub mod settings;
pub mod toasts;
