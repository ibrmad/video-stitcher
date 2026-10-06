//! The preview's render thread. It owns the GPU, the session and the ring;
//! it renders when a new frame is due or the pose moves, waits for its own
//! GPU work, then tells the UI. It sleeps until the next frame while
//! playing, and blocks entirely while paused and still (about 0% CPU).
//!
//! Zero-copy (macOS, same `MTLDevice` as the UI): a ring of
//! [`RING_SLOTS`] textures. [`PreviewEvent::Ring`] hands the UI their raw
//! `MTLTexture` pointers; the worker keeps the textures alive until the UI
//! answers [`PreviewCommand::Adopted`], and holds back any new ring until
//! then. [`PreviewEvent::Frame`] says which slot to show;
//! [`PreviewCommand::Release`] hands a slot back after the UI retired it.
//!
//! Readback (other devices, or forced): each frame is copied to CPU pixels
//! and sent as [`PreviewEvent::Pixels`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use reco_core::calibration::{FieldRoi, MatchCalibration};
use reco_core::gpu::GpuContext;
use reco_core::wgpu;
use reco_io::stitch_job::InputPath;

use super::lanes::{self, Lanes};
use super::metal;
use super::playback::{PlayState, seek_goal};
use super::readback::read_bgra;
use super::recorder::Recording;
use super::session::{OUTPUT_FORMAT, PreviewSession};
use super::slots::{RING_SLOTS, SlotRing};
use super::stats::{Stats, StatsMeter};
use super::tuning::{CalibrationValues, Tuning};
use super::view::should_resize;
use crate::files::save_atomically;
use crate::project::Camera;
use crate::recording::RecordingFormat;

/// Commands queued before the UI's sends start failing.
const COMMAND_QUEUE: usize = 256;
/// How often the worker wakes while the pose eases or a frame waits for a
/// free slot: about one frame of a 120 Hz display, so easing renders no
/// faster than a display shows them.
const ANIMATION_TICK: Duration = Duration::from_millis(8);
/// The longest easing step: after a pause in input the first step counts as
/// one 30 Hz frame, not the whole idle time.
const MAX_EASE_STEP: Duration = Duration::from_millis(33);
/// How often the worker looks for the file probe's answer.
const PROBE_POLL: Duration = Duration::from_millis(50);

/// The shortest sleep while playing: a frame that is due but not decoded
/// yet is polled again after this long (reco-gui's 2 ms timer).
const POLL_FLOOR: Duration = Duration::from_millis(2);

/// How the worker should present frames.
#[derive(Clone, Copy, Debug, Default)]
pub struct PreviewConfig {
    /// The UI's `MTLDevice` pointer; zero-copy only if the worker's device
    /// is this one.
    pub display_device: Option<usize>,
    /// Always read frames back (the portable path, for checks).
    pub force_readback: bool,
}

/// What the UI asks of the worker.
#[derive(Debug)]
pub enum PreviewCommand {
    /// Open a camera pair with its calibration.
    Open {
        /// The left camera's file(s).
        left: InputPath,
        /// The right camera's file(s).
        right: InputPath,
        /// The calibration JSON.
        calibration: PathBuf,
    },
    /// The render target's size in pixels.
    Resize {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// The UI adopted (retained) the ring of this generation.
    Adopted {
        /// The ring generation.
        generation: u64,
    },
    /// The UI no longer shows this slot.
    Release {
        /// The ring generation.
        generation: u64,
        /// The slot.
        slot: usize,
    },
    /// Drag by points.
    Pan {
        /// Horizontal points.
        dx: f32,
        /// Vertical points.
        dy: f32,
    },
    /// Change the FOV (negative zooms in).
    Zoom {
        /// Degrees.
        degrees: f32,
    },
    /// Back to the rest pose.
    ResetView,
    /// Aim the field of view at this many degrees.
    SetFov {
        /// Degrees.
        degrees: f32,
    },
    /// Keep the view inside the stitched picture (on by default) or let it
    /// go past the edges.
    StayInside(bool),
    /// Show one camera on its own, flat (the lens preview); `None`: the
    /// stitched picture.
    ShowCamera(Option<Camera>),
    /// Play or pause.
    TogglePlay,
    /// One frame forward or back.
    Step {
        /// Forward (`true`) or back.
        forward: bool,
    },
    /// Seek by seconds (negative goes back).
    SeekBy {
        /// Seconds.
        seconds: f64,
    },
    /// Show this frame (0-based). Seeks go by frame index, never by a
    /// fraction of the length.
    SeekTo {
        /// The frame.
        frame: u64,
    },
    /// Start recording the preview to `path`.
    StartRecording {
        /// The file to write.
        path: PathBuf,
        /// Its size, codec and quality.
        format: RecordingFormat,
    },
    /// Stop recording and close the file.
    StopRecording,
    /// Close the open videos (a recording is finished first). The thread
    /// and its texture ring stay for the next `Open`.
    Close,
    /// A change from the Adjust panel.
    Tune(Tuning),
    /// Play the cameras this many frames apart.
    SetSyncOffset {
        /// Frames (positive: the right camera started first).
        frames: i64,
    },
    /// Set or clear the field outline.
    SetFieldRoi(Option<FieldRoi>),
    /// Write the tuned calibration to `path`.
    SaveCalibration {
        /// The file.
        path: PathBuf,
    },
    /// Send the tuned calibration (`PreviewEvent::Snapshot`), for an export.
    Snapshot,
    /// Panic on the render thread (tests of the crash report).
    #[cfg(test)]
    Crash,
    /// Stop the worker.
    Quit,
}

/// What the worker tells the UI.
#[derive(Debug)]
pub enum PreviewEvent {
    /// Opening the videos (may take a few seconds).
    Opening,
    /// Open and showing the first frame.
    Ready(PreviewInfo),
    /// Opening failed; the message is for the user.
    Failed(String),
    /// Something failed once open (a render, a seek, a decode) and playback
    /// paused. The message is for the user; it is sent once, not per frame.
    Stopped(String),
    /// A new ring (zero-copy): adopt every texture, answer `Adopted`, and
    /// show `shown`, which already holds the current frame.
    Ring {
        /// The ring generation.
        generation: u64,
        /// Texture width in pixels.
        width: u32,
        /// Texture height in pixels.
        height: u32,
        /// Raw `MTLTexture` pointers, one per slot.
        textures: Vec<usize>,
        /// The slot to show now.
        shown: usize,
    },
    /// Show this slot (zero-copy).
    Frame {
        /// The ring generation.
        generation: u64,
        /// The slot.
        slot: usize,
    },
    /// A frame as CPU pixels (readback), BGRA in `u32`.
    Pixels {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// `width * height` pixels.
        data: Vec<u32>,
    },
    /// The live calibration's values (after opening and after every
    /// change).
    Calibration(CalibrationValues),
    /// The field of view the view is heading to, degrees (after a zoom, a
    /// reset, the slider or "stay inside").
    Fov(f32),
    /// The last second's frame figures (once a second while frames show).
    Stats(Stats),
    /// The tuned calibration was written here.
    CalibrationSaved(PathBuf),
    /// It could not be written; why.
    CalibrationSaveFailed(String),
    /// The calibration as tuned, for an export (answers `Snapshot`).
    Snapshot {
        /// The file's calibration with every live change folded in.
        calibration: Box<MatchCalibration>,
        /// Colour matching (a view setting, not in the calibration).
        color_match: bool,
    },
    /// A recording started.
    RecordingStarted {
        /// The file being written.
        path: PathBuf,
    },
    /// Frames recorded so far (one event per recorded frame).
    Recorded {
        /// The count so far.
        frames: u64,
    },
    /// A recording was finished and its file closed.
    RecordingSaved(Recording),
    /// A recording could not start or stopped on an error.
    RecordingFailed(String),
    /// Each camera's files on the timeline and the playable length (once
    /// per open, after the files were measured).
    Lanes(Lanes),
    /// The playhead moved, or the play state changed.
    Time {
        /// Frames taken so far (the frame on screen is `frame - 1`).
        frame: u64,
        /// Playing, paused or finished.
        state: PlayState,
    },
}

/// What an open session plays.
#[derive(Clone, Debug)]
pub struct PreviewInfo {
    /// Source frame rate.
    pub fps: f64,
    /// Frames in the shorter video, if known.
    pub total_frames: Option<u64>,
    /// Input width in pixels.
    pub width: u32,
    /// Input height in pixels.
    pub height: u32,
    /// Frames reach the UI with no copy.
    pub zero_copy: bool,
    /// The GPU's name.
    pub gpu: String,
    /// Its graphics backend ("Metal", "Vulkan", "Dx12").
    pub backend: String,
    /// The GPU's free and total memory at open, bytes, where it says
    /// (Metal, CUDA, DXGI): the export's lookahead zones come from it.
    pub vram: Option<(u64, u64)>,
}

/// The UI's handle to the render thread. Dropping it stops the thread.
pub struct PreviewWorker {
    commands: SyncSender<PreviewCommand>,
    events: Receiver<PreviewEvent>,
}

impl PreviewWorker {
    /// Start the render thread. `waker` is called after every event so the
    /// UI can drain them (Makepad: `SignalToUI::set_ui_signal`).
    pub fn spawn(config: PreviewConfig, waker: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (commands, command_rx) = mpsc::sync_channel(COMMAND_QUEUE);
        let (event_tx, events) = mpsc::channel();
        let outbox = Outbox {
            tx: event_tx,
            waker,
        };
        let report = outbox.clone();
        std::thread::Builder::new()
            .name("reco-preview".into())
            .spawn(move || {
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    Worker::new(config, outbox).run(command_rx)
                }));
                if let Err(panic) = run {
                    let reason = panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    report.send(PreviewEvent::Stopped(format!(
                        "The preview stopped unexpectedly: {reason}"
                    )));
                }
            })
            .expect("spawn the preview thread");
        Self { commands, events }
    }

    /// Queue a command without blocking; `false` if the queue is full or
    /// the worker stopped.
    pub fn send(&self, command: PreviewCommand) -> bool {
        self.commands.try_send(command).is_ok()
    }

    /// The next event, if any (never blocks).
    pub fn try_event(&self) -> Option<PreviewEvent> {
        self.events.try_recv().ok()
    }

    /// A sender for UI parts that queue commands themselves (non-blocking
    /// `try_send` only).
    pub fn sender(&self) -> SyncSender<PreviewCommand> {
        self.commands.clone()
    }
}

impl Drop for PreviewWorker {
    fn drop(&mut self) {
        let _ = self.commands.try_send(PreviewCommand::Quit);
    }
}

#[derive(Clone)]
struct Outbox {
    tx: mpsc::Sender<PreviewEvent>,
    waker: Arc<dyn Fn() + Send + Sync>,
}

impl Outbox {
    fn send(&self, event: PreviewEvent) {
        if self.tx.send(event).is_ok() {
            (self.waker)();
        }
    }
}

/// The current zero-copy ring.
struct Ring {
    generation: u64,
    textures: Vec<(wgpu::Texture, wgpu::TextureView)>,
    slots: SlotRing,
    adopted: bool,
}

struct Worker {
    config: PreviewConfig,
    out: Outbox,
    gpu: Option<GpuContext>,
    session: Option<PreviewSession>,
    zero_copy: bool,
    wanted: Option<(u32, u32)>,
    ring: Option<Ring>,
    /// The previous ring, kept alive until the UI adopted its successor.
    retired_ring: Option<Ring>,
    generation: u64,
    /// A frame is owed to the UI (input, resize, or a render that had to
    /// wait).
    dirty: bool,
    /// The pose is still easing toward its target.
    easing: bool,
    last_time: (u64, PlayState),
    /// A seek waiting for the end of this batch of commands.
    pending_seek: Option<u64>,
    /// Successful opens so far: each one's values carry its number.
    opened: u64,
    /// The file probe's answer, while it is being measured.
    lanes_rx: Option<Receiver<Lanes>>,
    /// The frame last recorded (`frame_index`), so each is recorded once.
    recorded_at: Option<u64>,
    /// Frames recorded so far.
    recorded: u64,
    /// The last failure reported since a frame was shown; not repeated.
    problem: Option<String>,
    /// When the pose last took a smoothing step.
    last_step: Instant,
    /// Frame times for the Stats section.
    stats: StatsMeter,
    quit: bool,
}

impl Worker {
    fn new(config: PreviewConfig, out: Outbox) -> Self {
        Self {
            config,
            out,
            gpu: None,
            session: None,
            zero_copy: false,
            wanted: None,
            ring: None,
            retired_ring: None,
            generation: 0,
            dirty: false,
            easing: false,
            last_time: (0, PlayState::Empty),
            pending_seek: None,
            opened: 0,
            stats: StatsMeter::new(Instant::now()),
            lanes_rx: None,
            recorded_at: None,
            recorded: 0,
            problem: None,
            last_step: Instant::now(),
            quit: false,
        }
    }

    /// Serve commands until `Quit` or until the UI drops its handle.
    fn run(mut self, commands: Receiver<PreviewCommand>) {
        while !self.quit {
            let command = match self.wake_after() {
                None => match commands.recv() {
                    Ok(c) => Some(c),
                    Err(_) => break,
                },
                Some(wait) => match commands.recv_timeout(wait) {
                    Ok(c) => Some(c),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
            };
            if let Some(c) = command {
                self.apply(c);
            }
            while let Ok(c) = commands.try_recv() {
                self.apply(c);
            }
            if let Some(frame) = self.pending_seek.take() {
                self.transport(|s| s.playback_mut().seek_to_frame(frame));
            }
            self.collect_lanes();
            let now = Instant::now();
            let dt = now
                .saturating_duration_since(self.last_step)
                .min(MAX_EASE_STEP);
            self.last_step = now;
            self.step(dt);
        }
        self.finish_recording();
    }

    /// `None`: nothing moves, so block until a command (about 0% CPU while
    /// paused). Otherwise wake after this long.
    fn wake_after(&self) -> Option<Duration> {
        let session = self.session.as_ref()?;
        let next_frame = session
            .playback()
            .until_next_frame()
            .map(|d| d.max(POLL_FLOOR));
        let sooner =
            |wait: Option<Duration>, other: Duration| Some(wait.map_or(other, |w| w.min(other)));
        let mut wait = next_frame;
        if self.dirty || self.easing {
            wait = sooner(wait, ANIMATION_TICK);
        }
        if self.lanes_rx.is_some() {
            wait = sooner(wait, PROBE_POLL);
        }
        wait
    }

    /// Take the probe's answer when it has come: the exact length goes to
    /// playback, the lanes to the UI.
    fn collect_lanes(&mut self) {
        let Some(rx) = self.lanes_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(lanes) => {
                self.lanes_rx = None;
                if let Some(s) = self.session.as_mut() {
                    let frames = (lanes.length * s.playback().fps()).floor() as u64;
                    if frames > 0 {
                        s.playback_mut().set_total_frames(frames);
                    }
                }
                self.out.send(PreviewEvent::Lanes(lanes));
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.lanes_rx = None,
        }
    }

    fn apply(&mut self, command: PreviewCommand) {
        match command {
            PreviewCommand::Open {
                left,
                right,
                calibration,
            } => self.open(&left, &right, &calibration),
            PreviewCommand::Resize { width, height } => {
                self.wanted = Some((width, height));
                self.dirty = true;
            }
            PreviewCommand::Adopted { generation } => {
                if let Some(ring) = self.ring.as_mut().filter(|r| r.generation == generation) {
                    ring.adopted = true;
                    self.retired_ring = None;
                    self.dirty = true;
                }
            }
            PreviewCommand::Release { generation, slot } => {
                if let Some(ring) = self.ring.as_mut().filter(|r| r.generation == generation) {
                    ring.slots.release(slot);
                }
            }
            PreviewCommand::Pan { dx, dy } => self.with_session(|s| s.pan(dx, dy)),
            PreviewCommand::Zoom { degrees } => {
                self.with_session(|s| s.zoom(degrees));
                self.send_fov();
            }
            PreviewCommand::ResetView => {
                self.with_session(PreviewSession::reset_view);
                self.send_fov();
            }
            PreviewCommand::SetFov { degrees } => {
                self.with_session(|s| s.set_fov(degrees));
                self.send_fov();
            }
            PreviewCommand::ShowCamera(camera) => self.with_session(|s| s.show_camera(camera)),
            PreviewCommand::StayInside(on) => {
                self.with_session(|s| s.set_constrained(on));
                self.send_fov();
            }
            PreviewCommand::TogglePlay => {
                // Figures start with the playing, not the frames drawn
                // while paused.
                self.stats = StatsMeter::new(Instant::now());
                self.transport(|s| s.playback_mut().toggle().map(|_| ()));
            }
            PreviewCommand::Step { forward } => self.transport(|s| {
                let playback = s.playback_mut();
                if playback.state() == PlayState::Playing {
                    playback.toggle()?;
                }
                if forward {
                    playback.step_forward().map(|_| ())
                } else {
                    playback.step_back()
                }
            }),
            PreviewCommand::SeekTo { frame } => self.pending_seek = Some(frame),
            PreviewCommand::SeekBy { seconds } => {
                if let Some(s) = self.session.as_ref() {
                    let playback = s.playback();
                    let from = self
                        .pending_seek
                        .unwrap_or(playback.frame_index().saturating_sub(1));
                    self.pending_seek = Some(seek_goal(
                        from,
                        seconds,
                        playback.fps(),
                        playback.total_frames(),
                    ));
                }
            }
            #[cfg(test)]
            PreviewCommand::Crash => panic!("a test crash"),
            PreviewCommand::StartRecording { path, format } => self.start_recording(&path, &format),
            PreviewCommand::StopRecording => self.finish_recording(),
            PreviewCommand::Close => self.close(),
            PreviewCommand::Tune(tuning) => {
                self.with_session(|s| s.tune(tuning));
                self.send_calibration();
            }
            PreviewCommand::SetSyncOffset { frames } => self.set_sync_offset(frames),
            PreviewCommand::SetFieldRoi(roi) => {
                self.with_session(|s| s.set_field_roi(roi));
                self.send_calibration();
            }
            PreviewCommand::SaveCalibration { path } => self.save_calibration(&path),
            PreviewCommand::Snapshot => self.send_snapshot(),
            PreviewCommand::Quit => self.quit = true,
        }
    }

    fn start_recording(&mut self, path: &std::path::Path, format: &RecordingFormat) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if session.is_recording() {
            return;
        }
        match session.start_recording(path, format) {
            Ok(()) => {
                self.recorded_at = None;
                self.recorded = 0;
                self.dirty = true;
                self.out.send(PreviewEvent::RecordingStarted {
                    path: path.to_path_buf(),
                });
            }
            Err(e) => self.out.send(PreviewEvent::RecordingFailed(e.to_string())),
        }
    }

    /// Record the frame on screen when it is new since the last one recorded
    /// (playing, stepping or seeking); a pan alone records nothing.
    fn record_new_frame(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let frame = session.playback().frame_index();
        if !session.is_recording() || self.recorded_at == Some(frame) {
            return;
        }
        match session.record_frame() {
            Ok(()) => {
                self.recorded_at = Some(frame);
                self.recorded += 1;
                self.out.send(PreviewEvent::Recorded {
                    frames: self.recorded,
                });
            }
            Err(e) => {
                let _ = session.stop_recording();
                self.out.send(PreviewEvent::RecordingFailed(e.to_string()));
            }
        }
    }

    /// Finish a running recording and say how it went.
    fn finish_recording(&mut self) {
        let Some(result) = self
            .session
            .as_mut()
            .and_then(PreviewSession::stop_recording)
        else {
            return;
        };
        match result {
            Ok(recording) => self.out.send(PreviewEvent::RecordingSaved(recording)),
            Err(e) => self.out.send(PreviewEvent::RecordingFailed(e.to_string())),
        }
    }

    fn with_session(&mut self, f: impl FnOnce(&mut PreviewSession)) {
        if let Some(s) = self.session.as_mut() {
            f(s);
            self.dirty = true;
        }
    }

    fn transport(
        &mut self,
        f: impl FnOnce(&mut PreviewSession) -> Result<(), reco_core::source::SourceError>,
    ) {
        if let Some(s) = self.session.as_mut() {
            let result = f(s);
            if let Err(e) = result {
                self.stop(format!("Couldn't seek: {e}"));
            }
            self.dirty = true;
        }
    }

    /// A failure once open (a render, a seek, a decode): pause, and tell the
    /// UI once, so a frame that keeps failing is not reported every frame.
    fn stop(&mut self, message: String) {
        if let Some(s) = self.session.as_mut()
            && s.playback().state() == PlayState::Playing
        {
            // Pausing never seeks, so it cannot fail.
            let _ = s.playback_mut().toggle();
        }
        if self.problem.as_ref() != Some(&message) {
            self.out.send(PreviewEvent::Stopped(message.clone()));
            self.problem = Some(message);
        }
    }

    /// Measure the files on a short-lived thread: the lanes and the exact
    /// length arrive later (`collect_lanes`).
    fn start_lanes_probe(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let (left, right) = session.inputs();
        let (sync_offset, fps) = (session.sync_offset(), session.playback().fps());
        let (tx, rx) = mpsc::channel();
        let probe = std::thread::Builder::new()
            .name("reco-probe".into())
            .spawn(move || {
                let _ = tx.send(lanes::lanes(
                    &lanes::probe(&left),
                    &lanes::probe(&right),
                    sync_offset,
                    fps,
                ));
            });
        self.lanes_rx = probe.is_ok().then_some(rx);
    }

    /// Tell the UI where the field of view is heading.
    fn send_fov(&self) {
        if let Some(session) = self.session.as_ref() {
            self.out.send(PreviewEvent::Fov(session.target_fov()));
        }
    }

    /// Tell the UI the live calibration's values.
    fn send_calibration(&self) {
        if let Some(session) = self.session.as_ref() {
            let mut values = session.values();
            values.opened = self.opened;
            self.out.send(PreviewEvent::Calibration(values));
        }
    }

    /// Play the cameras `frames` apart; the lanes are measured again.
    fn set_sync_offset(&mut self, frames: i64) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        match session.set_sync_offset(frames) {
            Ok(()) => {
                self.start_lanes_probe();
                self.dirty = true;
                self.send_calibration();
            }
            Err(e) => self.stop(e.to_string()),
        }
    }

    /// Send the tuned calibration, for an export.
    fn send_snapshot(&self) {
        if let Some(session) = self.session.as_ref() {
            self.out.send(PreviewEvent::Snapshot {
                calibration: Box::new(session.calibration_to_save()),
                color_match: session.values().color_match,
            });
        }
    }

    /// Write the tuned calibration to `path` (atomically).
    fn save_calibration(&mut self, path: &std::path::Path) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let json = session.calibration_to_save().to_json_pretty();
        match save_atomically(path, &json) {
            Ok(()) => {
                session.mark_saved();
                self.out
                    .send(PreviewEvent::CalibrationSaved(path.to_path_buf()));
                self.send_calibration();
            }
            Err(e) => self.out.send(PreviewEvent::CalibrationSaveFailed(format!(
                "couldn't write {}: {e}",
                path.display()
            ))),
        }
    }

    /// Drop the open videos, finishing a recording first. The ring stays:
    /// the UI may still show one of its textures until the next open.
    fn close(&mut self) {
        self.finish_recording();
        self.session = None;
        self.lanes_rx = None;
        self.pending_seek = None;
        self.problem = None;
        self.easing = false;
        self.dirty = false;
        self.last_time = (0, PlayState::Empty);
    }

    /// The shown ring waits, retired, until the UI adopts the next one. A
    /// ring never adopted was never shown, so it goes now.
    fn retire_ring(&mut self) {
        if let Some(shown) = self.ring.take().filter(|r| r.adopted) {
            self.retired_ring = Some(shown);
        }
    }

    fn open(&mut self, left: &InputPath, right: &InputPath, calibration: &std::path::Path) {
        // New videos end a recording of the old ones.
        self.finish_recording();
        self.out.send(PreviewEvent::Opening);
        if self.gpu.is_none() {
            match GpuContext::new_blocking() {
                Ok(gpu) => self.gpu = Some(gpu),
                Err(e) => {
                    return self
                        .out
                        .send(PreviewEvent::Failed(format!("No usable GPU: {e}")));
                }
            }
        }
        let gpu = self.gpu.clone().expect("gpu set above");
        let size = self.wanted.unwrap_or((1280, 720));
        match PreviewSession::open(gpu.clone(), left, right, calibration, size) {
            Ok(session) => {
                self.zero_copy = !self.config.force_readback
                    && self.config.display_device.is_some()
                    && metal::device_ptr(&gpu) == self.config.display_device;
                let playback = session.playback();
                let (width, height) = playback.input_dimensions().unwrap_or((0, 0));
                self.out.send(PreviewEvent::Ready(PreviewInfo {
                    fps: playback.fps(),
                    total_frames: playback.total_frames(),
                    width,
                    height,
                    zero_copy: self.zero_copy,
                    gpu: gpu.gpu_name().to_string(),
                    backend: gpu.backend_name().to_string(),
                    vram: gpu.available_vram(),
                }));
                log::info!(
                    "preview: {}x{} input, {} on {}",
                    width,
                    height,
                    if self.zero_copy {
                        "zero-copy"
                    } else {
                        "readback"
                    },
                    gpu.gpu_name()
                );
                self.session = Some(session);
                self.opened += 1;
                self.start_lanes_probe();
                self.send_calibration();
                // The picture can narrow the opening field of view.
                self.send_fov();
                self.retire_ring();
                self.problem = None;
                self.dirty = true;
            }
            Err(e) => self.out.send(PreviewEvent::Failed(e.to_string())),
        }
    }

    /// Advance playback, and the pose by `dt`, and present a frame if
    /// anything moved.
    fn step(&mut self, dt: Duration) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let ticked = Instant::now();
        let advanced = match session.playback_mut().tick() {
            Ok(advanced) => advanced,
            Err(e) => {
                self.stop(format!("Playback stopped: {e}"));
                false
            }
        };
        let decode = ticked.elapsed();
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let moved = session.smooth(dt);
        self.easing = moved;
        let time = (session.playback().frame_index(), session.playback().state());
        if time != self.last_time {
            self.last_time = time;
            self.out.send(PreviewEvent::Time {
                frame: time.0,
                state: time.1,
            });
        }
        self.record_new_frame();
        if advanced || moved || self.dirty {
            let rendering = Instant::now();
            let shown = self.present();
            self.dirty = !shown;
            if shown {
                self.stats
                    .frame(Instant::now(), decode, rendering.elapsed());
            }
        }
        if let Some(stats) = self.stats.report(Instant::now()) {
            self.out.send(PreviewEvent::Stats(stats));
        }
    }

    /// Render and hand over a frame; `false` if it has to wait (no free
    /// slot, or the UI has not adopted the ring yet).
    fn present(&mut self) -> bool {
        if let Some(wanted) = self.wanted {
            let current = self.session.as_ref().map(PreviewSession::size);
            if should_resize(current, wanted) || (self.zero_copy && self.ring.is_none()) {
                if self.zero_copy && self.ring.as_ref().is_some_and(|r| !r.adopted) {
                    return false;
                }
                if let Some(s) = self.session.as_mut() {
                    s.resize(wanted.0, wanted.1);
                }
                if self.zero_copy {
                    return self.new_ring();
                }
            }
        }
        if self.zero_copy {
            self.present_slot()
        } else {
            self.present_pixels()
        }
    }

    fn new_ring(&mut self) -> bool {
        let (Some(session), Some(gpu)) = (self.session.as_ref(), self.gpu.as_ref()) else {
            return false;
        };
        let size = session.size();
        let textures: Vec<_> = (0..RING_SLOTS)
            .map(|_| {
                let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
                    label: Some("preview ring"),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: OUTPUT_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                (texture, view)
            })
            .collect();
        let Some(pointers) = textures
            .iter()
            .map(|(t, _)| metal::texture_ptr(t))
            .collect::<Option<Vec<_>>>()
        else {
            self.zero_copy = false;
            return self.present_pixels();
        };
        self.generation += 1;
        let mut slots = SlotRing::new(RING_SLOTS);
        let shown = slots.acquire().expect("a new ring has free slots");
        if let Err(e) = session.render(&textures[shown].1) {
            self.stop(e.to_string());
            return true;
        }
        let _ = gpu.device().poll(wgpu::PollType::wait_indefinitely());
        if let Some(shown) = self.ring.take() {
            self.retired_ring = Some(shown);
        }
        self.ring = Some(Ring {
            generation: self.generation,
            textures,
            slots,
            adopted: false,
        });
        self.out.send(PreviewEvent::Ring {
            generation: self.generation,
            width: size.0,
            height: size.1,
            textures: pointers,
            shown,
        });
        self.problem = None;
        true
    }

    fn present_slot(&mut self) -> bool {
        let (Some(session), Some(gpu), Some(ring)) =
            (self.session.as_ref(), self.gpu.as_ref(), self.ring.as_mut())
        else {
            return false;
        };
        if !ring.adopted {
            return false;
        }
        let Some(slot) = ring.slots.acquire() else {
            return false;
        };
        if let Err(e) = session.render(&ring.textures[slot].1) {
            ring.slots.release(slot);
            self.stop(e.to_string());
            return true;
        }
        let _ = gpu.device().poll(wgpu::PollType::wait_indefinitely());
        self.out.send(PreviewEvent::Frame {
            generation: ring.generation,
            slot,
        });
        self.problem = None;
        true
    }

    fn present_pixels(&mut self) -> bool {
        let (Some(session), Some(gpu)) = (self.session.as_ref(), self.gpu.as_ref()) else {
            return false;
        };
        let (width, height) = session.size();
        let target = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("preview readback target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        if let Err(e) = session.render(&target.create_view(&Default::default())) {
            self.stop(e.to_string());
            return true;
        }
        match read_bgra(gpu, &target, width, height) {
            Ok(data) => {
                self.out.send(PreviewEvent::Pixels {
                    width,
                    height,
                    data,
                });
                self.problem = None;
            }
            Err(e) => self.stop(format!("Couldn't read the frame back: {e}")),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::fixtures;
    use std::time::{Duration, Instant};

    fn wait_for(
        worker: &PreviewWorker,
        secs: u64,
        mut want: impl FnMut(&PreviewEvent) -> bool,
    ) -> Option<PreviewEvent> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            while let Some(e) = worker.try_event() {
                if want(&e) {
                    return Some(e);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    fn readback_worker() -> PreviewWorker {
        PreviewWorker::spawn(
            PreviewConfig {
                display_device: None,
                force_readback: true,
            },
            Arc::new(|| {}),
        )
    }

    #[test]
    fn bad_paths_fail_cleanly() {
        let worker = readback_worker();
        worker.send(PreviewCommand::Resize {
            width: 320,
            height: 180,
        });
        worker.send(PreviewCommand::Open {
            left: InputPath::Single("/nonexistent/l.mp4".into()),
            right: InputPath::Single("/nonexistent/r.mp4".into()),
            calibration: "/nonexistent/c.json".into(),
        });
        let event = wait_for(&worker, 20, |e| {
            matches!(e, PreviewEvent::Failed(_) | PreviewEvent::Ready(_))
        });
        assert!(matches!(event, Some(PreviewEvent::Failed(_))), "{event:?}");
    }

    #[test]
    fn readback_renders_and_plays() {
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let worker = readback_worker();
        worker.send(PreviewCommand::Resize {
            width: 320,
            height: 180,
        });
        worker.send(PreviewCommand::Open {
            left: InputPath::Single(left),
            right: InputPath::Single(right),
            calibration: cal,
        });
        let ready = wait_for(&worker, 30, |e| {
            matches!(e, PreviewEvent::Ready(_) | PreviewEvent::Failed(_))
        });
        assert!(
            matches!(
                ready,
                Some(PreviewEvent::Ready(PreviewInfo {
                    zero_copy: false,
                    ..
                }))
            ),
            "{ready:?}"
        );
        let first = wait_for(&worker, 10, |e| matches!(e, PreviewEvent::Pixels { .. }));
        assert!(
            matches!(
                first,
                Some(PreviewEvent::Pixels {
                    width: 320,
                    height: 180,
                    ..
                })
            ),
            "{first:?}"
        );
        worker.send(PreviewCommand::TogglePlay);
        let moved = wait_for(
            &worker,
            10,
            |e| matches!(e, PreviewEvent::Time { frame, state: PlayState::Playing } if *frame >= 10),
        );
        assert!(moved.is_some(), "playback did not reach frame 10");
    }

    #[test]
    fn runtime_failures_report_once() {
        let (tx, rx) = mpsc::channel();
        let mut worker = Worker::new(
            PreviewConfig::default(),
            Outbox {
                tx,
                waker: Arc::new(|| {}),
            },
        );
        worker.stop("The preview failed to render: x".into());
        worker.stop("The preview failed to render: x".into());
        worker.stop("Couldn't seek: y".into());
        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(events.len(), 2, "{events:?}");
        assert!(
            matches!(&events[0], PreviewEvent::Stopped(m) if m.ends_with(": x")),
            "{events:?}"
        );
        assert!(
            matches!(&events[1], PreviewEvent::Stopped(m) if m.ends_with(": y")),
            "{events:?}"
        );
    }

    #[test]
    fn another_gpu_falls_back_to_readback() {
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        // The UI's device is not the worker's: no ring, frames as pixels.
        let worker = PreviewWorker::spawn(
            PreviewConfig {
                display_device: Some(1),
                force_readback: false,
            },
            Arc::new(|| {}),
        );
        worker.send(PreviewCommand::Resize {
            width: 320,
            height: 180,
        });
        worker.send(PreviewCommand::Open {
            left: InputPath::Single(left),
            right: InputPath::Single(right),
            calibration: cal,
        });
        let ready = wait_for(&worker, 30, |e| {
            matches!(e, PreviewEvent::Ready(_) | PreviewEvent::Failed(_))
        });
        assert!(
            matches!(
                ready,
                Some(PreviewEvent::Ready(PreviewInfo {
                    zero_copy: false,
                    ..
                }))
            ),
            "{ready:?}"
        );
        let first = wait_for(&worker, 10, |e| {
            matches!(e, PreviewEvent::Pixels { .. } | PreviewEvent::Ring { .. })
        });
        assert!(
            matches!(first, Some(PreviewEvent::Pixels { .. })),
            "{first:?}"
        );
    }

    #[test]
    fn an_open_reports_the_gpus_memory() {
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let worker = readback_worker();
        worker.send(PreviewCommand::Open {
            left: InputPath::Single(left),
            right: InputPath::Single(right),
            calibration: cal,
        });
        let ready = wait_for(&worker, 30, |e| {
            matches!(e, PreviewEvent::Ready(_) | PreviewEvent::Failed(_))
        });
        let Some(PreviewEvent::Ready(info)) = ready else {
            panic!("{ready:?}")
        };
        if cfg!(target_os = "macos") {
            let (free, total) = info.vram.expect("Metal reports its memory");
            assert!(total > 0 && free <= total, "{free} of {total}");
        }
    }

    #[test]
    fn ring_waits_for_adoption() {
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let gpu = reco_core::gpu::GpuContext::new_blocking().ok();
        let Some(device) = gpu.as_ref().and_then(crate::preview::metal::device_ptr) else {
            eprintln!("skipping: no Metal device for the zero-copy ring");
            return;
        };
        let worker = PreviewWorker::spawn(
            PreviewConfig {
                display_device: Some(device),
                force_readback: false,
            },
            Arc::new(|| {}),
        );
        worker.send(PreviewCommand::Resize {
            width: 320,
            height: 180,
        });
        worker.send(PreviewCommand::Open {
            left: InputPath::Single(left),
            right: InputPath::Single(right),
            calibration: cal,
        });
        let ring = wait_for(&worker, 30, |e| {
            matches!(e, PreviewEvent::Ring { .. } | PreviewEvent::Failed(_))
        });
        let Some(PreviewEvent::Ring {
            generation,
            textures,
            ..
        }) = ring
        else {
            panic!("{ring:?}")
        };
        assert_eq!(textures.len(), crate::preview::slots::RING_SLOTS);
        // A resize before adoption is held back: no second ring yet.
        worker.send(PreviewCommand::Resize {
            width: 640,
            height: 360,
        });
        assert!(wait_for(&worker, 1, |e| matches!(e, PreviewEvent::Ring { .. })).is_none());
        worker.send(PreviewCommand::Adopted { generation });
        let second = wait_for(&worker, 10, |e| matches!(e, PreviewEvent::Ring { .. }));
        assert!(
            matches!(
                second,
                Some(PreviewEvent::Ring {
                    width: 640,
                    height: 360,
                    ..
                })
            ),
            "{second:?}"
        );
    }

    #[test]
    fn seek_burst_lands_on_the_sum() {
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let worker = readback_worker();
        worker.send(PreviewCommand::Resize {
            width: 320,
            height: 180,
        });
        worker.send(PreviewCommand::Open {
            left: InputPath::Single(left),
            right: InputPath::Single(right),
            calibration: cal,
        });
        assert!(wait_for(&worker, 30, |e| matches!(e, PreviewEvent::Ready(_))).is_some());
        for _ in 0..5 {
            worker.send(PreviewCommand::SeekBy { seconds: 5.0 });
        }
        let landed = wait_for(
            &worker,
            20,
            |e| matches!(e, PreviewEvent::Time { frame, .. } if *frame >= 751),
        );
        assert!(
            matches!(landed, Some(PreviewEvent::Time { frame: 751, .. })),
            "{landed:?}"
        );
    }

    #[test]
    fn a_crash_is_reported() {
        let worker = readback_worker();
        worker.send(PreviewCommand::Crash);
        let event = wait_for(&worker, 5, |e| matches!(e, PreviewEvent::Stopped(_)));
        assert!(
            matches!(&event, Some(PreviewEvent::Stopped(m)) if m.contains("stopped unexpectedly")),
            "{event:?}"
        );
    }

    #[test]
    fn lanes_arrive_after_open() {
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let worker = readback_worker();
        worker.send(PreviewCommand::Resize {
            width: 320,
            height: 180,
        });
        worker.send(PreviewCommand::Open {
            left: InputPath::Single(left),
            right: InputPath::Single(right),
            calibration: cal,
        });
        let event = wait_for(&worker, 30, |e| {
            matches!(e, PreviewEvent::Lanes(_) | PreviewEvent::Failed(_))
        });
        let Some(PreviewEvent::Lanes(lanes)) = event else {
            panic!("{event:?}")
        };
        assert_eq!((lanes.left.len(), lanes.right.len()), (1, 1));
        assert!((lanes.length - 60.0).abs() < 0.5, "{lanes:?}");
    }

    fn temp_video(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("reco-app-worker-{name}-{}.mp4", std::process::id()))
    }

    fn open_fast(worker: &PreviewWorker) -> bool {
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return false;
        };
        worker.send(PreviewCommand::Resize {
            width: 320,
            height: 180,
        });
        worker.send(PreviewCommand::Open {
            left: InputPath::Single(left),
            right: InputPath::Single(right),
            calibration: cal,
        });
        wait_for(worker, 30, |e| matches!(e, PreviewEvent::Ready(_))).is_some()
    }

    fn start_recording(worker: &PreviewWorker, path: &std::path::Path) {
        start_recording_with(worker, path, "h264");
    }

    fn start_recording_with(worker: &PreviewWorker, path: &std::path::Path, codec: &str) {
        worker.send(PreviewCommand::StartRecording {
            path: path.to_path_buf(),
            format: RecordingFormat {
                codec: codec.into(),
                ..fixtures::small_recording()
            },
        });
        let started = wait_for(worker, 10, |e| {
            matches!(
                e,
                PreviewEvent::RecordingStarted { .. } | PreviewEvent::RecordingFailed(_)
            )
        });
        assert!(
            matches!(started, Some(PreviewEvent::RecordingStarted { .. })),
            "{started:?}"
        );
    }

    /// The highest `Recorded` count seen within `secs`.
    fn recorded_within(worker: &PreviewWorker, secs: f64) -> u64 {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        let mut frames = 0;
        while Instant::now() < deadline {
            while let Some(e) = worker.try_event() {
                if let PreviewEvent::Recorded { frames: n } = e {
                    frames = frames.max(n);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        frames
    }

    fn saved(worker: &PreviewWorker) -> Recording {
        worker.send(PreviewCommand::StopRecording);
        let event = wait_for(worker, 20, |e| {
            matches!(
                e,
                PreviewEvent::RecordingSaved(_) | PreviewEvent::RecordingFailed(_)
            )
        });
        match event {
            Some(PreviewEvent::RecordingSaved(recording)) => recording,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn records_while_playing() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("plays");
        start_recording(&worker, &path);
        worker.send(PreviewCommand::TogglePlay);
        std::thread::sleep(Duration::from_millis(1000));
        let recording = saved(&worker);
        assert!(
            (20..=40).contains(&recording.frames),
            "about a second at 30 fps: {}",
            recording.frames
        );
        let secs = reco_io::ffmpeg::decoder::VideoDecoder::open(&path)
            .expect("playable")
            .duration_secs()
            .unwrap_or(0.0);
        assert!(
            (secs - recording.frames as f64 / 30.0).abs() < 0.1,
            "{secs}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The video codec of `path`'s first stream, by ffprobe (`None` when
    /// ffprobe isn't installed).
    fn codec_of(path: &std::path::Path) -> Option<String> {
        let out = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", "v:0"])
            .args(["-show_entries", "stream=codec_name", "-of", "csv=p=0"])
            .arg(path)
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    #[test]
    fn recording_uses_the_chosen_codec() {
        if !crate::export::available_codecs()
            .iter()
            .any(|c| c == "hevc")
        {
            return;
        }
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("hevc");
        start_recording_with(&worker, &path, "hevc");
        worker.send(PreviewCommand::TogglePlay);
        std::thread::sleep(Duration::from_millis(400));
        assert!(saved(&worker).frames > 0);
        if let Some(codec) = codec_of(&path) {
            assert_eq!(codec, "hevc");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pausing_adds_no_frames() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("paused");
        start_recording(&worker, &path);
        worker.send(PreviewCommand::TogglePlay);
        std::thread::sleep(Duration::from_millis(600));
        worker.send(PreviewCommand::TogglePlay);
        let at_pause = recorded_within(&worker, 0.4);
        // Panning while paused moves the view, not the video: no new frames.
        worker.send(PreviewCommand::Pan { dx: 80.0, dy: 0.0 });
        assert_eq!(recorded_within(&worker, 0.5), 0, "no frames while paused");
        assert_eq!(saved(&worker).frames, at_pause);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn quitting_while_recording_finishes_the_file() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("quit");
        start_recording(&worker, &path);
        worker.send(PreviewCommand::TogglePlay);
        std::thread::sleep(Duration::from_millis(500));
        drop(worker);
        // The worker closes the file on its own thread after Quit.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut playable = false;
        while Instant::now() < deadline && !playable {
            playable = reco_io::ffmpeg::decoder::VideoDecoder::open(&path)
                .ok()
                .and_then(|v| v.duration_secs())
                .is_some_and(|s| s > 0.0);
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(
            playable,
            "the recording was finished after the worker was dropped"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn close_finishes_and_a_new_open_plays_again() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("closed");
        start_recording(&worker, &path);
        worker.send(PreviewCommand::Close);
        let saved = wait_for(&worker, 20, |e| {
            matches!(e, PreviewEvent::RecordingSaved(_))
        });
        assert!(saved.is_some(), "closing finishes the recording");
        worker.send(PreviewCommand::TogglePlay);
        let moved = wait_for(&worker, 1, |e| matches!(e, PreviewEvent::Time { .. }));
        assert!(moved.is_none(), "nothing plays once closed: {moved:?}");
        assert!(open_fast(&worker), "the same worker opens again");
        let shown = wait_for(&worker, 10, |e| matches!(e, PreviewEvent::Pixels { .. }));
        assert!(shown.is_some());
        let _ = std::fs::remove_file(&path);
    }

    /// The next `Calibration` event that `accept`s.
    fn calibration_where(
        worker: &PreviewWorker,
        accept: impl Fn(&CalibrationValues) -> bool,
    ) -> Option<CalibrationValues> {
        match wait_for(
            worker,
            10,
            |e| matches!(e, PreviewEvent::Calibration(v) if accept(v)),
        ) {
            Some(PreviewEvent::Calibration(values)) => Some(values),
            _ => None,
        }
    }

    #[test]
    fn tuning_is_reported_with_its_values() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        assert!(
            calibration_where(&worker, |v| !v.dirty).is_some(),
            "the loaded values come first"
        );
        worker.send(PreviewCommand::Tune(Tuning::Blend(0.2)));
        let tuned = calibration_where(&worker, |v| v.dirty);
        assert!(tuned.is_some_and(|v| (v.blend - 0.2).abs() < 1e-6));
    }

    #[test]
    fn saving_writes_the_file_and_clears_the_change() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        worker.send(PreviewCommand::Tune(Tuning::Tilt(2.0)));
        assert!(calibration_where(&worker, |v| v.dirty).is_some());
        let path =
            std::env::temp_dir().join(format!("reco-app-worker-cal-{}.json", std::process::id()));
        worker.send(PreviewCommand::SaveCalibration { path: path.clone() });
        let saved = wait_for(&worker, 10, |e| {
            matches!(
                e,
                PreviewEvent::CalibrationSaved(_) | PreviewEvent::CalibrationSaveFailed(_)
            )
        });
        assert!(
            matches!(&saved, Some(PreviewEvent::CalibrationSaved(p)) if *p == path),
            "{saved:?}"
        );
        assert!(
            calibration_where(&worker, |v| !v.dirty).is_some(),
            "saved: nothing unsaved"
        );
        let back = reco_core::calibration::MatchCalibration::from_file(&path).unwrap();
        assert!((back.rig_tilt - 2f64.to_radians()).abs() < 1e-6);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_snapshot_carries_the_tuning() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        worker.send(PreviewCommand::Tune(Tuning::Blend(0.2)));
        worker.send(PreviewCommand::Tune(Tuning::ColorMatch(false)));
        worker.send(PreviewCommand::SetSyncOffset { frames: 12 });
        worker.send(PreviewCommand::Snapshot);
        let snapshot = wait_for(&worker, 10, |e| matches!(e, PreviewEvent::Snapshot { .. }));
        let Some(PreviewEvent::Snapshot {
            calibration,
            color_match,
        }) = snapshot
        else {
            panic!("no snapshot: {snapshot:?}");
        };
        assert!((calibration.blend_width - 0.2).abs() < 1e-6);
        assert_eq!(calibration.sync_offset, 12);
        assert!(!color_match);
    }

    #[test]
    fn the_view_reports_its_field_of_view() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let fov = |worker: &PreviewWorker| match wait_for(worker, 5, |e| {
            matches!(e, PreviewEvent::Fov(_))
        }) {
            Some(PreviewEvent::Fov(degrees)) => Some(degrees),
            _ => None,
        };
        assert!(
            fov(&worker).is_some(),
            "the opening field of view is reported"
        );
        worker.send(PreviewCommand::StayInside(false));
        assert!(
            fov(&worker).is_some(),
            "stay inside reports the field of view"
        );
        worker.send(PreviewCommand::SetFov { degrees: 90.0 });
        assert_eq!(fov(&worker), Some(90.0));
        worker.send(PreviewCommand::Zoom { degrees: -10.0 });
        assert_eq!(fov(&worker), Some(80.0), "a zoom moves it");
        worker.send(PreviewCommand::ResetView);
        assert_eq!(
            fov(&worker),
            Some(super::super::session::FOV_DEFAULT),
            "reset goes back"
        );
    }

    #[test]
    fn stats_arrive_while_playing() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        worker.send(PreviewCommand::TogglePlay);
        let stats = wait_for(&worker, 5, |e| matches!(e, PreviewEvent::Stats(_)));
        let Some(PreviewEvent::Stats(stats)) = stats else {
            panic!("no stats while playing: {stats:?}");
        };
        assert!(stats.fps > 1.0 && stats.frame_ms > 0.0, "{stats:?}");
    }

    #[test]
    fn a_new_sync_offset_brings_new_lanes() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        assert!(wait_for(&worker, 30, |e| matches!(e, PreviewEvent::Lanes(_))).is_some());
        worker.send(PreviewCommand::SetSyncOffset { frames: 30 });
        assert!(calibration_where(&worker, |v| v.sync_offset == 30).is_some());
        let lanes = wait_for(&worker, 30, |e| matches!(e, PreviewEvent::Lanes(_)));
        let Some(PreviewEvent::Lanes(lanes)) = lanes else {
            panic!("{lanes:?}")
        };
        assert!(
            (lanes.length - 59.0).abs() < 0.5,
            "30 frames at 30 fps less: {lanes:?}"
        );
    }

    #[test]
    fn each_open_has_its_own_number() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let first = calibration_where(&worker, |_| true).map(|v| v.opened);
        assert!(open_fast(&worker));
        let second = calibration_where(&worker, |v| Some(v.opened) != first).map(|v| v.opened);
        assert_eq!((first, second), (Some(1), Some(2)));
    }
}
