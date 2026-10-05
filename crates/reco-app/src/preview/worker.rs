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

use reco_core::gpu::GpuContext;
use reco_core::wgpu;
use reco_io::stitch_job::InputPath;

use super::metal;
use super::playback::PlayState;
use super::readback::read_bgra;
use super::session::{OUTPUT_FORMAT, PreviewSession};
use super::slots::{RING_SLOTS, SlotRing};
use super::view::should_resize;

/// Commands queued before the UI's sends start failing.
const COMMAND_QUEUE: usize = 256;
/// How often the worker wakes while the pose eases or a frame waits for a
/// free slot: about one frame of a 120 Hz display, so easing renders no
/// faster than a display shows them.
const ANIMATION_TICK: Duration = Duration::from_millis(8);
/// The longest easing step: after a pause in input the first step counts as
/// one 30 Hz frame, not the whole idle time.
const MAX_EASE_STEP: Duration = Duration::from_millis(33);
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
    /// The playhead moved or play started or stopped.
    Time {
        /// Frames taken so far.
        frame: u64,
        /// Whether playing.
        playing: bool,
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
        std::thread::Builder::new()
            .name("reco-preview".into())
            .spawn(move || Worker::new(config, outbox).run(command_rx))
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
    last_time: (u64, bool),
    /// The last failure reported since a frame was shown; not repeated.
    problem: Option<String>,
    /// When the pose last took a smoothing step.
    last_step: Instant,
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
            last_time: (0, false),
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
            let now = Instant::now();
            let dt = now
                .saturating_duration_since(self.last_step)
                .min(MAX_EASE_STEP);
            self.last_step = now;
            self.step(dt);
        }
    }

    /// `None`: nothing moves, so block until a command (about 0% CPU while
    /// paused). Otherwise wake after this long.
    fn wake_after(&self) -> Option<Duration> {
        let session = self.session.as_ref()?;
        let next_frame = session
            .playback()
            .until_next_frame()
            .map(|d| d.max(POLL_FLOOR));
        if self.dirty || self.easing {
            return Some(next_frame.map_or(ANIMATION_TICK, |d| d.min(ANIMATION_TICK)));
        }
        next_frame
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
            PreviewCommand::Zoom { degrees } => self.with_session(|s| s.zoom(degrees)),
            PreviewCommand::ResetView => self.with_session(PreviewSession::reset_view),
            PreviewCommand::TogglePlay => self.transport(|s| s.playback_mut().toggle().map(|_| ())),
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
            PreviewCommand::SeekBy { seconds } => {
                self.transport(|s| s.playback_mut().seek_by(seconds))
            }
            PreviewCommand::Quit => self.quit = true,
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

    fn open(&mut self, left: &InputPath, right: &InputPath, calibration: &std::path::Path) {
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
                self.ring = None;
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
        let advanced = match session.playback_mut().tick() {
            Ok(advanced) => advanced,
            Err(e) => {
                self.stop(format!("Playback stopped: {e}"));
                false
            }
        };
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let moved = session.smooth(dt);
        self.easing = moved;
        let time = (
            session.playback().frame_index(),
            session.playback().state() == PlayState::Playing,
        );
        if time != self.last_time {
            self.last_time = time;
            self.out.send(PreviewEvent::Time {
                frame: time.0,
                playing: time.1,
            });
        }
        if advanced || moved || self.dirty {
            self.dirty = !self.present();
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
        self.retired_ring = self.ring.take();
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
            |e| matches!(e, PreviewEvent::Time { frame, playing: true } if *frame >= 10),
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
}
