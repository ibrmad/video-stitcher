//! The live stitched preview: view math, playback, the session that drives
//! Reco's renderer, and the worker thread that owns it.

pub mod clock;
#[cfg(test)]
pub(crate) mod fixtures;
pub mod lanes;
pub mod metal;
pub mod playback;
pub mod readback;
pub mod recorder;
pub mod session;
pub mod slots;
pub mod view;
pub mod worker;
