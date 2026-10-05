//! Video playback for the preview, ported from reco-gui's `playback.rs`
//! (no Slint dependency there either): play/pause/step/seek over an
//! `FfmpegFileSource`, paced by a [`FrameClock`].

use std::time::{Duration, Instant};

use reco_core::source::{FrameSource, SourceError, SourceInfo, StereoFrame, YuvData};
use reco_io::adapters::FfmpegFileSource;
use reco_io::stitch_job::InputPath;

use super::clock::FrameClock;

/// The frame a relative seek lands on: `current + delta`, kept inside
/// `0..total`.
pub fn seek_target(current: u64, delta_frames: i64, total: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    (current as i64 + delta_frames).clamp(0, total as i64 - 1) as u64
}

/// The frame a seek of `seconds` reaches from `from` (0-based) at `fps`,
/// inside `total` frames when known. Seeks in a burst chain through this
/// from the pending target, so they cost one seek.
pub fn seek_goal(from: u64, seconds: f64, fps: f64, total: Option<u64>) -> u64 {
    let delta = (seconds * fps).round() as i64;
    match total {
        Some(total) => seek_target(from, delta, total),
        None => (from as i64 + delta).max(0) as u64,
    }
}

/// Where playback stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    /// No source loaded.
    Empty,
    /// Paused on a frame.
    Paused,
    /// Playing at the source frame rate.
    Playing,
    /// Reached the end; toggling plays from where it stopped.
    Finished,
}

/// One decoded frame from each camera.
pub struct StereoYuv {
    /// The left camera's frame.
    pub left: YuvData,
    /// The right camera's frame.
    pub right: YuvData,
}

/// Plays a camera pair: decodes ahead on the source's own threads, and
/// hands out the current frame pair when its time has come.
pub struct Playback {
    source: Option<FfmpegFileSource>,
    info: Option<SourceInfo>,
    state: PlayState,
    current: Option<StereoYuv>,
    frame_index: u64,
    total_frames: Option<u64>,
    clock: FrameClock,
}

impl Default for Playback {
    fn default() -> Self {
        Self::new()
    }
}

impl Playback {
    /// No source loaded.
    pub fn new() -> Self {
        Self {
            source: None,
            info: None,
            state: PlayState::Empty,
            current: None,
            frame_index: 0,
            total_frames: None,
            clock: FrameClock::new(Duration::ZERO),
        }
    }

    /// Open a camera pair (blocking: probes the files and decodes the first
    /// frame). `sync_offset` comes from the calibration.
    pub fn open(
        &mut self,
        left: &InputPath,
        right: &InputPath,
        sync_offset: i64,
    ) -> Result<(), SourceError> {
        let source = FfmpegFileSource::open_from_inputs(left, right, sync_offset)?;
        let info = source.info();
        let duration = if info.fps > 0.0 {
            Duration::from_secs_f64(1.0 / info.fps)
        } else {
            Duration::ZERO
        };
        self.total_frames = source.total_frames();
        self.clock = FrameClock::new(duration);
        self.info = Some(info);
        self.source = Some(source);
        self.state = PlayState::Paused;
        self.frame_index = 0;
        self.current = None;
        self.step_forward()?;
        Ok(())
    }

    /// Decode the next frame now (blocking). `false` at the end.
    pub fn step_forward(&mut self) -> Result<bool, SourceError> {
        let Some(source) = self.source.as_mut() else {
            return Ok(false);
        };
        match source.next_frame()? {
            Some(frame) => {
                self.current = Some(Self::split(frame)?);
                self.frame_index += 1;
                Ok(true)
            }
            None => {
                self.state = PlayState::Finished;
                Ok(false)
            }
        }
    }

    /// Go back one frame (a seek to the frame before the current one).
    pub fn step_back(&mut self) -> Result<(), SourceError> {
        let target = self.frame_index.saturating_sub(2);
        self.seek_to(target)
    }

    /// Seek by `seconds` (negative goes back), kept inside the video.
    pub fn seek_by(&mut self, seconds: f64) -> Result<(), SourceError> {
        let (Some(total), fps) = (self.total_frames, self.fps()) else {
            return Ok(());
        };
        if fps <= 0.0 {
            return Ok(());
        }
        let current = self.frame_index.saturating_sub(1);
        self.seek_to(seek_target(current, (seconds * fps).round() as i64, total))
    }

    /// The exact source rate as a fraction (30000/1001 for 29.97), for the
    /// encoder; the rounded rate when the source does not say.
    pub fn fps_rational(&self) -> (i32, i32) {
        self.info
            .as_ref()
            .and_then(|i| i.fps_rational)
            .unwrap_or_else(|| ((self.fps().round() as i32).max(1), 1))
    }

    /// Replace the engine's length estimate (it ignores the sync offset)
    /// with the probed one.
    pub fn set_total_frames(&mut self, frames: u64) {
        self.total_frames = Some(frames);
    }

    /// Show `frame` (0-based), kept inside the videos (blocking: decodes).
    pub fn seek_to_frame(&mut self, frame: u64) -> Result<(), SourceError> {
        let frame = self
            .total_frames
            .map_or(frame, |total| frame.min(total.saturating_sub(1)));
        self.seek_to(frame)
    }

    fn seek_to(&mut self, frame: u64) -> Result<(), SourceError> {
        let Some(source) = self.source.as_mut() else {
            return Ok(());
        };
        let before = self.frame_index;
        source.seek(frame)?;
        self.frame_index = frame;
        self.clock.reset();
        if self.state == PlayState::Finished {
            self.state = PlayState::Paused;
        }
        if !self.step_forward()? {
            // The engine reports a seek it could not make as the end of the
            // stream: keep the frame on screen and say so.
            self.frame_index = before;
            self.state = PlayState::Paused;
            return Err(SourceError::Read {
                reason: format!("the videos end before frame {frame}"),
            });
        }
        Ok(())
    }

    /// Advance when the next frame is due (never blocks: takes a frame only
    /// if the decoder has one). `true` when a new frame was taken.
    pub fn tick(&mut self) -> Result<bool, SourceError> {
        if self.state != PlayState::Playing || !self.clock.due(Instant::now(), self.frame_index) {
            return Ok(false);
        }
        let Some(source) = self.source.as_mut() else {
            return Ok(false);
        };
        match source.try_next_frame()? {
            Some(frame) => {
                self.current = Some(Self::split(frame)?);
                self.frame_index += 1;
                Ok(true)
            }
            None => {
                if source.is_exhausted() {
                    self.state = PlayState::Finished;
                }
                Ok(false)
            }
        }
    }

    /// Play or pause; after the end, play again from the start (a seek,
    /// so it can fail).
    pub fn toggle(&mut self) -> Result<PlayState, SourceError> {
        self.state = match self.state {
            PlayState::Finished => {
                self.seek_to(0)?;
                self.clock.reset();
                PlayState::Playing
            }
            PlayState::Paused => {
                self.clock.reset();
                PlayState::Playing
            }
            PlayState::Playing => PlayState::Paused,
            PlayState::Empty => PlayState::Empty,
        };
        Ok(self.state)
    }

    /// How long until the next frame is due while playing; `None` otherwise.
    pub fn until_next_frame(&self) -> Option<Duration> {
        (self.state == PlayState::Playing)
            .then(|| self.clock.until_next(Instant::now(), self.frame_index))
    }

    /// Where playback stands.
    pub fn state(&self) -> PlayState {
        self.state
    }

    /// The frame pair on screen.
    pub fn current_frame(&self) -> Option<&StereoYuv> {
        self.current.as_ref()
    }

    /// Frames taken so far (1 after open).
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Frames in the shorter video, if known.
    pub fn total_frames(&self) -> Option<u64> {
        self.total_frames
    }

    /// The source frame rate (0 before open).
    pub fn fps(&self) -> f64 {
        self.info.as_ref().map_or(0.0, |i| i.fps)
    }

    /// The input video size.
    pub fn input_dimensions(&self) -> Option<(u32, u32)> {
        self.info.as_ref().map(|i| (i.width, i.height))
    }

    fn split(frame: StereoFrame) -> Result<StereoYuv, SourceError> {
        match frame {
            StereoFrame::Yuv420p(pair) => Ok(StereoYuv {
                left: pair.left,
                right: pair.right,
            }),
            _ => Err(SourceError::Read {
                reason: "the preview expects Yuv420p frames".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seek_target_moves_and_clamps() {
        assert_eq!(seek_target(300, 150, 1000), 450);
        assert_eq!(seek_target(300, -150, 1000), 150);
        assert_eq!(seek_target(100, -150, 1000), 0);
        assert_eq!(seek_target(900, 150, 1000), 999);
        assert_eq!(seek_target(0, 5, 0), 0);
    }

    #[test]
    fn finished_playback_stops_ticking() {
        let mut playback = Playback::new();
        playback.state = PlayState::Finished;
        assert!(!playback.tick().unwrap());
        assert_eq!(playback.until_next_frame(), None);
        assert_eq!(playback.toggle().unwrap(), PlayState::Playing);
    }

    #[test]
    fn space_after_the_end_restarts() {
        let Some((left, right, _)) = crate::preview::fixtures::fast_set() else {
            return;
        };
        let mut playback = Playback::new();
        playback
            .open(&InputPath::Single(left), &InputPath::Single(right), 0)
            .unwrap();
        // Past the end: lands on the last frame; playing then finishes.
        playback.seek_by(600.0).unwrap();
        playback.toggle().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while playback.state() != PlayState::Finished && Instant::now() < deadline {
            playback.tick().unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(playback.state(), PlayState::Finished);
        assert_eq!(playback.toggle().unwrap(), PlayState::Playing);
        assert!(
            playback.frame_index() <= 1,
            "plays from the start again, not frame {}",
            playback.frame_index()
        );
    }

    #[test]
    fn relative_seeks_accumulate() {
        // Five presses of ] at 30 fps from the first frame: 25 s on.
        let mut at = 0;
        for _ in 0..5 {
            at = seek_goal(at, 5.0, 30.0, Some(1800));
        }
        assert_eq!(at, 750);
        assert_eq!(seek_goal(1790, 5.0, 30.0, Some(1800)), 1799);
        assert_eq!(seek_goal(10, -5.0, 30.0, None), 0);
    }

    #[test]
    fn seek_to_frame_shows_that_frame() {
        let Some((left, right, _)) = crate::preview::fixtures::fast_set() else {
            return;
        };
        let mut playback = Playback::new();
        playback
            .open(&InputPath::Single(left), &InputPath::Single(right), 0)
            .unwrap();
        playback.seek_to_frame(300).unwrap();
        assert_eq!(playback.frame_index(), 301);
        // Past the length: the last frame.
        playback.seek_to_frame(99_999).unwrap();
        assert_eq!(Some(playback.frame_index()), playback.total_frames());
    }

    #[test]
    fn seek_past_the_end_is_an_error() {
        let Some((left, right, _)) = crate::preview::fixtures::fast_set() else {
            return;
        };
        let mut playback = Playback::new();
        playback
            .open(&InputPath::Single(left), &InputPath::Single(right), 0)
            .unwrap();
        let before = playback.frame_index();
        // Past the real end, as a sync offset can make the engine's total.
        assert!(playback.seek_to(99_999).is_err());
        assert_eq!(playback.frame_index(), before, "the frame on screen stays");
        assert_eq!(playback.state(), PlayState::Paused);
    }
}
