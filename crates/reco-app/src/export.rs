//! Export: the stitched match written to a file by Reco's `StitchJob` on its
//! own thread, with progress, Cancel and a plain outcome. The options are
//! the Slint app's export dialog's, without AI tracking (Module 6b).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub use reco_core::calibration::MatchCalibration;
use reco_io::ffmpeg::encoder::{VideoCodec, available_encoders};
use reco_io::output::{Codec, Format, Quality};
use reco_io::stitch_job::{InputPath, StitchError, StitchJob};

/// The export sizes, named as the Slint app named them.
pub const RESOLUTIONS: [(&str, u32, u32); 4] = [
    ("1080p", 1920, 1080),
    ("720p", 1280, 720),
    ("2K", 2560, 1440),
    ("4K", 3840, 2160),
];

/// What to export and how.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    /// The file to write (`.mp4` is added when it has no extension).
    pub output: PathBuf,
    /// Frame size.
    pub size: (u32, u32),
    /// "h264", "hevc" or "av1".
    pub codec: String,
    /// "fast", "balanced" or "high".
    pub quality: String,
    /// Start and end on the stitched timeline, seconds (an `ExportRange`).
    pub range: (f64, f64),
    /// The source frame rate, for the progress total.
    pub fps: f64,
    /// Seam blend, from the live tuning.
    pub blend: f32,
    /// Colour matching, from the live tuning.
    pub color_match: bool,
    /// Also record the stacked raw input for re-stitching
    /// (`{output}.replay.mkv`).
    pub replay: bool,
    /// Also save the pipeline's events (`{output}.events.jsonl`).
    pub events: bool,
    /// AI tracking (Module 6b), when on.
    pub tracking: Option<crate::ai::Tracking>,
}

/// What an export reports.
#[derive(Clone, Debug, PartialEq)]
pub enum ExportEvent {
    /// Frames written so far, of how many, at what rate.
    Progress {
        /// Frames written.
        frames: u64,
        /// Frames in the range.
        total: u64,
        /// Frames per second so far.
        fps: f64,
    },
    /// The last frames are being written and the file closed.
    Finalizing,
    /// Written.
    Done {
        /// The file.
        path: PathBuf,
        /// Frames written.
        frames: u64,
        /// How long it took.
        seconds: f64,
    },
    /// It failed; why.
    Failed(String),
    /// It was cancelled.
    Cancelled,
    /// AI tracking started (`Ok`), or why it didn't (the export goes on
    /// without it).
    Tracking(Result<(), String>),
    /// How the tracking is doing (every half second or so).
    AiFigures(AiFigures),
    /// How the export is doing: its speed and where the time goes (every
    /// half second or so; the Slint app's Stats showed these).
    Figures(ExportFigures),
}

/// An export's speed and where its time goes, from the engine's telemetry.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportFigures {
    /// Frames a second, lately.
    pub fps: f64,
    /// Frames a second since the first frame.
    pub fps_average: f64,
    /// Decoding a frame pair, ms.
    pub decode_ms: f64,
    /// Stitching, ms.
    pub stitch_ms: f64,
    /// Reading the picture back for the encoder, ms.
    pub readback_ms: f64,
    /// Handing it to the encoder, ms (long when the encoder can't keep up).
    pub submit_ms: f64,
    /// A frame in all, ms.
    pub frame_ms: f64,
    /// The slowest frame in a hundred, ms.
    pub p99_ms: f64,
    /// The stage holding the export back, in the engine's words.
    pub bottleneck: Option<String>,
}

impl ExportFigures {
    /// The figures in `snapshot`, once frames have been timed.
    fn of(snapshot: &reco_core::telemetry::TelemetrySnapshot) -> Option<Self> {
        (snapshot.avg_total_ms > 0.0).then(|| Self {
            fps: f64::from(snapshot.fps_recent),
            fps_average: f64::from(snapshot.fps_average),
            decode_ms: f64::from(snapshot.avg_decode_ms),
            stitch_ms: f64::from(snapshot.avg_stitch_ms),
            readback_ms: f64::from(snapshot.avg_readback_ms),
            submit_ms: f64::from(snapshot.avg_submit_ms),
            frame_ms: f64::from(snapshot.avg_total_ms),
            p99_ms: f64::from(snapshot.p99_total_ms),
            bottleneck: snapshot.bottleneck.map(|stage| stage.to_string()),
        })
    }
}

/// A tracked export's detector and tracker figures, from the engine's
/// telemetry.
#[derive(Clone, Debug, PartialEq)]
pub struct AiFigures {
    /// Average time the detector takes, ms.
    pub detection_ms: f64,
    /// Detections a frame, on average.
    pub per_frame: f64,
    /// Tracks being followed (players mostly; one player can leave more
    /// than one).
    pub tracks: u32,
    /// Frames with the ball found, percent.
    pub ball_pct: f64,
}

/// The figures, once the detector has run (the engine reports zeros
/// before, and throughout a lookahead: FRICTION.md).
#[cfg(any(feature = "ai", test))]
fn ai_figures(
    detection_ms: f64,
    total_detections: u64,
    per_frame: f64,
    tracks: u32,
    ball_pct: f64,
) -> Option<AiFigures> {
    (detection_ms > 0.0 || total_detections > 0).then_some(AiFigures {
        detection_ms,
        per_frame,
        tracks,
        ball_pct,
    })
}

/// Frames between the figures an export sends.
const FIGURES_EVERY: u64 = 15;

/// Hands the engine's telemetry to the UI: the export's figures, and with
/// `ai` (tracking started) the detector's.
struct FiguresSink {
    out: Outbox,
    /// Only read with the AI engine in the build.
    #[cfg_attr(not(feature = "ai"), allow(dead_code))]
    ai: bool,
}

impl reco_core::telemetry::TelemetrySink for FiguresSink {
    fn on_snapshot(&mut self, snapshot: &reco_core::telemetry::TelemetrySnapshot) {
        if let Some(figures) = ExportFigures::of(snapshot) {
            self.out.send(ExportEvent::Figures(figures));
        }
        #[cfg(feature = "ai")]
        if self.ai {
            let figures = ai_figures(
                f64::from(snapshot.avg_detection_ms),
                snapshot.total_detections,
                f64::from(snapshot.detections_per_frame),
                snapshot.active_tracks,
                f64::from(snapshot.ball_presence_pct),
            );
            if let Some(figures) = figures {
                self.out.send(ExportEvent::AiFigures(figures));
            }
        }
    }
}

/// `path` with `.mp4` added when it has no extension.
pub fn with_mp4(path: &Path) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension("mp4")
    }
}

/// Where an export goes by default: `{first left stem}_stitched.mp4` beside
/// the left camera's first file.
pub fn default_output(first_left: &Path) -> PathBuf {
    let stem = first_left
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "match".into());
    first_left.with_file_name(format!("{stem}_stitched.mp4"))
}

/// The file to export to, from what was typed: `.mp4` added when there is no
/// extension, a bare name put beside the left camera's first file. Refused:
/// nothing typed, a folder that doesn't exist, a folder itself, a file that
/// isn't MP4, MOV or MKV (it would be overwritten with video), and one of
/// the videos being exported (the export would overwrite it while reading).
pub fn checked_output(
    typed: &str,
    first_left: &Path,
    inputs: &[PathBuf],
) -> Result<PathBuf, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Err("Choose where to save the export.".into());
    }
    let mut path = PathBuf::from(typed);
    if path.is_relative() {
        path = first_left.parent().unwrap_or(Path::new("")).join(path);
    }
    if path.is_dir() {
        return Err(format!(
            "{} is a folder; name a file in it.",
            path.display()
        ));
    }
    let path = with_mp4(&path);
    let kind = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(kind.as_deref(), Some("mp4" | "mov" | "mkv")) {
        return Err(
            "Exports are MP4, MOV or MKV files: end the name in .mp4, .mov or .mkv.".into(),
        );
    }
    let folder = path.parent().unwrap_or(Path::new(""));
    if !folder.is_dir() {
        return Err(format!("The folder {} doesn't exist.", folder.display()));
    }
    // Compare real paths: the same file can be named two ways.
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let target = real(&path);
    if inputs.iter().any(|input| real(input) == target) {
        return Err("That is one of the videos being exported; choose another file.".into());
    }
    Ok(path)
}

/// Why an empty range can't be exported.
const EMPTY_RANGE: &str = "The range is empty: the end must come after the start.";

/// Whether nothing lies between `start` and `end` (NaN included).
fn is_empty_range(start: f64, end: f64) -> bool {
    start.partial_cmp(&end) != Some(std::cmp::Ordering::Less)
}

/// The part of the match an export covers, in seconds on the stitched
/// timeline, kept inside the videos: the start never passes the end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportRange {
    start: f64,
    end: f64,
    length: f64,
}

impl ExportRange {
    /// All of a `length`-second match.
    pub fn whole(length: f64) -> Self {
        let length = if length.is_finite() {
            length.max(0.0)
        } else {
            0.0
        };
        Self {
            start: 0.0,
            end: length,
            length,
        }
    }

    /// `start` to `end` inside `length`; an end of 0 means the end.
    pub fn new(start: f64, end: f64, length: f64) -> Self {
        let mut range = Self::whole(length);
        if end > 0.0 {
            range.set_end(end);
        }
        range.set_start(start);
        range
    }

    /// Where it starts.
    pub fn start(&self) -> f64 {
        self.start
    }

    /// Where it ends.
    pub fn end(&self) -> f64 {
        self.end
    }

    /// The match's length.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Move the start, no later than the end (a non-number is ignored).
    pub fn set_start(&mut self, seconds: f64) {
        if seconds.is_finite() {
            self.start = seconds.clamp(0.0, self.end);
        }
    }

    /// Move the end, no earlier than the start and no later than the match
    /// (a non-number is ignored).
    pub fn set_end(&mut self, seconds: f64) {
        if seconds.is_finite() {
            self.end = seconds.clamp(self.start, self.length);
        }
    }

    /// How long it is.
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    /// Whether nothing lies inside it.
    pub fn is_empty(&self) -> bool {
        is_empty_range(self.start, self.end)
    }

    /// Whether it covers the whole match.
    pub fn is_whole(&self) -> bool {
        self.start == 0.0 && self.end == self.length
    }

    /// The same part of a match whose length is now `length` (a new sync
    /// offset changes it): all of it stays all of it.
    pub fn refit(&self, length: f64) -> Self {
        let mut range = Self::whole(length);
        if !self.is_whole() {
            range.set_end(self.end);
            range.set_start(self.start);
        }
        range
    }

    /// Start and end as fractions of the length (for sliders).
    pub fn fractions(&self) -> (f64, f64) {
        if self.length > 0.0 {
            (self.start / self.length, self.end / self.length)
        } else {
            (0.0, 0.0)
        }
    }
}

/// The qualities, as settings and `ExportOptions` name them.
pub const QUALITIES: [&str; 3] = ["fast", "balanced", "high"];

/// The index in `RESOLUTIONS` of the size named `name` (1080p when unknown).
pub fn size_index(name: &str) -> usize {
    RESOLUTIONS
        .iter()
        .position(|(n, _, _)| *n == name)
        .unwrap_or(0)
}

/// A codec's name on screen: "H.264", "HEVC" or "AV1".
pub fn codec_label(code: &str) -> String {
    match code {
        "h264" => "H.264".into(),
        other => other.to_uppercase(),
    }
}

/// The codecs this machine can encode ("h264", "hevc", "av1"); h264 when
/// none answer. Blocking (it asks each encoder): run it off the UI thread.
pub fn available_codecs() -> Vec<String> {
    let mut codecs: Vec<String> = [
        ("h264", VideoCodec::H264),
        ("hevc", VideoCodec::Hevc),
        ("av1", VideoCodec::Av1),
    ]
    .into_iter()
    .filter(|(_, codec)| !available_encoders(*codec).is_empty())
    .map(|(name, _)| name.to_string())
    .collect();
    if codecs.is_empty() {
        codecs.push("h264".into());
    }
    codecs
}

/// How often progress is reported (the last frame always is).
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// Sends events to the UI and wakes it.
#[derive(Clone)]
struct Outbox {
    tx: Sender<ExportEvent>,
    waker: Arc<dyn Fn() + Send + Sync>,
}

impl Outbox {
    fn send(&self, event: ExportEvent) {
        if self.tx.send(event).is_ok() {
            (self.waker)();
        }
    }
}

/// The words for a failed export.
fn failure_words(error: &StitchError, codec: &str) -> String {
    match error {
        StitchError::EmptyOutput { .. } => {
            format!("The file has no picture: this machine may not encode {codec}. Try H.264.")
        }
        other => other.to_string(),
    }
}

/// Run the export on this thread; the last event says how it ended.
fn run(
    left: InputPath,
    right: InputPath,
    calibration: MatchCalibration,
    options: ExportOptions,
    cancel: &AtomicBool,
    out: &Outbox,
) -> ExportEvent {
    let output = with_mp4(&options.output);
    if let Some(problem) = options
        .tracking
        .as_ref()
        .and_then(crate::ai::Tracking::problem)
    {
        return ExportEvent::Failed(problem);
    }
    let field_roi = calibration.field_roi.clone();
    let (start, end) = options.range;
    let total = ((end - start) * options.fps).round().max(0.0) as u64;
    let codec: Codec = options.codec.parse().unwrap_or_default();
    let quality: Quality = options.quality.parse().unwrap_or_default();
    let mut job = StitchJob::with_calibration(left, right, calibration, &output)
        .codec(codec)
        .quality(quality)
        .format(Format::for_output(&output.to_string_lossy()))
        .resolution(options.size.0, options.size.1)
        .blend_width(options.blend)
        .color_match(options.color_match)
        .end_time(end);
    if start > 0.0 {
        job = job.start_time(start);
    }
    if options.replay {
        job = job.with_replay_recording(output.with_extension("replay.mkv"));
    }
    if options.events {
        job = job.events(output.with_extension("events.jsonl"));
    }
    let figures = out.clone();
    job = job.on_session(move |session, _| {
        session.telemetry_mut().set_sink(
            Box::new(FiguresSink {
                out: figures,
                ai: false,
            }),
            FIGURES_EVERY,
        );
    });
    if let Some(tracking) = options.tracking.clone() {
        job = with_tracking(job, tracking, field_roi, out.clone());
    }
    // The rate counts from the first frame: opening and seeking to the
    // start are not exporting.
    let progress = out.clone();
    let mut first: Option<Instant> = None;
    let mut last_sent: Option<Instant> = None;
    job = job.on_progress(move |p| {
        let now = Instant::now();
        let since = *first.get_or_insert(now);
        let frames = p.frames_completed;
        let due = last_sent.is_none_or(|at| now - at >= PROGRESS_EVERY) || frames >= total;
        if due {
            last_sent = Some(now);
            let seconds = (now - since).as_secs_f64();
            let fps = if seconds > 0.0 {
                frames.saturating_sub(1) as f64 / seconds
            } else {
                0.0
            };
            progress.send(ExportEvent::Progress { frames, total, fps });
        }
    });
    let finalizing = out.clone();
    job = job.on_finalizing(move || finalizing.send(ExportEvent::Finalizing));
    let result = job.run(cancel);
    if cancel.load(Ordering::Relaxed) {
        return ExportEvent::Cancelled;
    }
    match result {
        Ok(done) => ExportEvent::Done {
            path: output,
            frames: done.frames_processed,
            seconds: done.elapsed.as_secs_f64(),
        },
        Err(e) => ExportEvent::Failed(failure_words(&e, &options.codec)),
    }
}

/// `job` with AI tracking: the lookahead buffer, and the detector,
/// trackers and panner set up on the session as it opens (it says whether
/// they started).
#[cfg(feature = "ai")]
fn with_tracking(
    job: StitchJob,
    tracking: crate::ai::Tracking,
    field_roi: Option<reco_core::calibration::FieldRoi>,
    out: Outbox,
) -> StitchJob {
    let job = if tracking.lookahead_secs > 0.0 {
        job.lookahead(tracking.lookahead_secs)
    } else {
        job
    };
    job.on_session(move |session, source| {
        let is_10bit =
            source.gpu_pixel_format() == reco_core::render::renderer::GpuPixelFormat::P010;
        let config = tracking.config(is_10bit, field_roi);
        let fps = source.info().fps as f32;
        let status =
            match reco_autocam::setup_autocam(session, &config, fps, source.is_gpu_resident()) {
                Ok(true) => Ok(()),
                Ok(false) => Err("no detector this export can use on this machine".to_string()),
                Err(e) => Err(e.to_string()),
            };
        match &status {
            Ok(()) => log::info!("export: AI tracking active ({})", tracking.mode),
            Err(why) => log::warn!("export: AI tracking not active: {why}"),
        }
        if status.is_ok() {
            session.telemetry_mut().set_sink(
                Box::new(FiguresSink {
                    out: out.clone(),
                    ai: true,
                }),
                FIGURES_EVERY,
            );
        }
        out.send(ExportEvent::Tracking(status));
    })
}

/// Without the engine (a build without AI): the export says so and goes on.
#[cfg(not(feature = "ai"))]
fn with_tracking(
    job: StitchJob,
    _tracking: crate::ai::Tracking,
    _field_roi: Option<reco_core::calibration::FieldRoi>,
    out: Outbox,
) -> StitchJob {
    out.send(ExportEvent::Tracking(Err(
        "AI tracking isn't in this build".to_string(),
    )));
    job
}

/// A panic's message.
fn panic_words(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "an unknown error".into())
}

/// An export running on its own thread. Dropping it cancels it.
pub struct ExportJob {
    cancel: Arc<AtomicBool>,
    events: Receiver<ExportEvent>,
}

impl ExportJob {
    /// Export `left` and `right` with `calibration` (the live tuning);
    /// `waker` runs after every event.
    pub fn start(
        left: InputPath,
        right: InputPath,
        calibration: MatchCalibration,
        options: ExportOptions,
        waker: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let out = Outbox { tx, waker };
        let job = Self {
            cancel: Arc::clone(&cancel),
            events,
        };
        if is_empty_range(options.range.0, options.range.1) {
            out.send(ExportEvent::Failed(EMPTY_RANGE.into()));
            return job;
        }
        let thread_out = out.clone();
        let spawned = std::thread::Builder::new()
            .name("reco-export".into())
            .spawn(move || {
                let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(left, right, calibration, options, &cancel, &thread_out)
                }));
                let last = ran.unwrap_or_else(|panic| {
                    ExportEvent::Failed(format!(
                        "The export stopped unexpectedly: {}",
                        panic_words(panic.as_ref())
                    ))
                });
                thread_out.send(last);
            });
        if let Err(e) = spawned {
            out.send(ExportEvent::Failed(format!(
                "Couldn't start the export: {e}"
            )));
        }
        job
    }

    /// Stop at the next frame (it then reports `Cancelled`).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The next event, if any (never blocks).
    pub fn try_event(&self) -> Option<ExportEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use reco_io::ffmpeg::decoder::VideoDecoder;

    use super::*;
    use crate::preview::fixtures;

    fn options(output: PathBuf, range: (f64, f64)) -> ExportOptions {
        ExportOptions {
            output,
            size: (1280, 720),
            codec: "h264".into(),
            quality: "fast".into(),
            range,
            fps: 30.0,
            blend: 0.05,
            color_match: true,
            replay: false,
            events: false,
            tracking: None,
        }
    }

    /// A calibration for jobs that never open the videos.
    fn some_calibration() -> MatchCalibration {
        use reco_core::calibration::{CameraParams, PlaneLayout};
        let camera = CameraParams {
            width: 1920,
            height: 1080,
            fx: 900.0,
            fy: 900.0,
            cx: 960.0,
            cy: 540.0,
            d: [0.0; 4],
        };
        MatchCalibration {
            left: camera.clone(),
            right: camera,
            layout: PlaneLayout {
                camera_axis_offset: 0.24,
                intersect: 0.54,
                x_ty: 0.0,
                x_rz: 0.0,
                z_rx: 0.0,
                x_rx: 0.0,
                z_rz: 0.0,
            },
            rig_tilt: 0.0,
            rig_roll: 0.0,
            sync_offset: 0,
            field_roi: None,
            lens_correction_amount: 1.0,
            blend_width: 0.05,
        }
    }

    fn until_done(job: &ExportJob, secs: u64, cancel_at_first_progress: bool) -> Vec<ExportEvent> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            while let Some(event) = job.try_event() {
                let last = matches!(
                    event,
                    ExportEvent::Done { .. } | ExportEvent::Failed(_) | ExportEvent::Cancelled
                );
                if cancel_at_first_progress && matches!(event, ExportEvent::Progress { .. }) {
                    job.cancel();
                }
                seen.push(event);
                if last {
                    return seen;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        seen
    }

    #[test]
    fn outputs_get_an_mp4_extension() {
        assert_eq!(
            with_mp4(Path::new("/m/match")),
            PathBuf::from("/m/match.mp4")
        );
        assert_eq!(
            with_mp4(Path::new("/m/match.mkv")),
            PathBuf::from("/m/match.mkv")
        );
        assert_eq!(
            default_output(Path::new("/m/GX010120.MP4")),
            PathBuf::from("/m/GX010120_stitched.mp4")
        );
    }

    #[test]
    fn the_range_stays_inside_the_videos() {
        let whole = ExportRange::whole(60.0);
        assert_eq!((whole.start(), whole.end()), (0.0, 60.0));
        assert!(whole.is_whole() && !whole.is_empty());
        let r = ExportRange::new(50.0, 70.0, 60.0);
        assert_eq!(
            (r.start(), r.end()),
            (50.0, 60.0),
            "the end stops at the match's"
        );
        let r = ExportRange::new(10.0, 0.0, 60.0);
        assert_eq!((r.start(), r.end()), (10.0, 60.0), "an end of 0 is the end");
        assert!(ExportRange::new(70.0, 0.0, 60.0).is_empty());
    }

    #[test]
    fn the_start_never_passes_the_end() {
        let mut r = ExportRange::whole(60.0);
        r.set_end(20.0);
        r.set_start(30.0);
        assert_eq!((r.start(), r.end()), (20.0, 20.0));
        assert!(r.is_empty() && r.duration() == 0.0);
        r.set_start(-5.0);
        r.set_end(90.0);
        assert_eq!((r.start(), r.end()), (0.0, 60.0));
        r.set_start(15.0);
        r.set_end(5.0);
        assert_eq!(
            (r.start(), r.end()),
            (15.0, 15.0),
            "the end stops at the start"
        );
        r.set_end(f64::NAN);
        r.set_start(f64::INFINITY);
        assert_eq!(
            (r.start(), r.end()),
            (15.0, 15.0),
            "non-numbers change nothing"
        );
        r.set_end(45.0);
        assert_eq!(r.fractions(), (0.25, 0.75));
        assert!(!r.is_whole());
        assert_eq!(ExportRange::whole(0.0).fractions(), (0.0, 0.0));
    }

    #[test]
    fn a_range_keeps_its_place_when_the_length_changes() {
        assert_eq!(
            ExportRange::whole(60.0).refit(59.0),
            ExportRange::whole(59.0),
            "all of it stays all of it"
        );
        let r = ExportRange::new(10.0, 40.0, 60.0);
        let shorter = r.refit(30.0);
        assert_eq!((shorter.start(), shorter.end()), (10.0, 30.0));
        let longer = r.refit(90.0);
        assert_eq!((longer.start(), longer.end()), (10.0, 40.0));
        let mut empty = ExportRange::whole(60.0);
        empty.set_end(0.0);
        assert!(empty.refit(50.0).is_empty(), "an empty range stays empty");
    }

    #[test]
    fn sizes_and_codecs_have_names() {
        assert_eq!(size_index("4K"), 3);
        assert_eq!(size_index("720p"), 1);
        assert_eq!(size_index("8K"), 0, "unknown sizes are 1080p");
        assert_eq!(codec_label("h264"), "H.264");
        assert_eq!(codec_label("hevc"), "HEVC");
        assert_eq!(codec_label("av1"), "AV1");
    }

    #[test]
    fn this_machine_encodes_h264() {
        assert!(available_codecs().contains(&"h264".to_string()));
    }

    #[test]
    fn exporting_two_seconds_writes_a_playable_file() {
        let _heavy = fixtures::heavy();
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let output = std::env::temp_dir().join(format!("reco-app-export-{}", std::process::id()));
        let job = ExportJob::start(
            InputPath::Single(left),
            InputPath::Single(right),
            MatchCalibration::from_file(&cal).unwrap(),
            options(output.clone(), (0.0, 2.0)),
            Arc::new(|| {}),
        );
        let events = until_done(&job, 120, false);
        let Some(ExportEvent::Done { path, frames, .. }) = events.last() else {
            panic!("{events:?}")
        };
        assert_eq!(path, &output.with_extension("mp4"));
        assert!(
            (55..=65).contains(frames),
            "about two seconds at 30 fps: {frames}"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ExportEvent::Progress { total: 60, .. }))
        );
        let video = VideoDecoder::open(path).expect("a playable file");
        assert_eq!((video.width(), video.height()), (1280, 720));
        let _ = std::fs::remove_file(path);
    }

    fn tracking(model: Option<PathBuf>) -> crate::ai::Tracking {
        crate::ai::Tracking {
            model,
            mode: "field".into(),
            interval: 15,
            preset: "broadcast".into(),
            knobs: crate::ai::PannerKnobs::of_preset("broadcast"),
            lookahead_secs: 0.5,
        }
    }

    /// Debug builds turn on wgpu's validation, which turns on Metal's, and
    /// that asserts in reco-detect's Metal preprocessing (an early return
    /// leaves an encoder open; FRICTION.md). Optimized builds run it as the
    /// app does: `cargo test --profile desktop -p reco-app export::tests`.
    #[cfg(feature = "ai")]
    #[cfg_attr(
        debug_assertions,
        ignore = "needs an optimized build: Metal validation (FRICTION.md)"
    )]
    #[test]
    fn an_export_tracks_with_the_model() {
        let _heavy = fixtures::heavy();
        let (Some((left, right, cal)), Some(model)) = (fixtures::fast_set(), fixtures::model())
        else {
            return;
        };
        let output =
            std::env::temp_dir().join(format!("reco-app-export-ai-{}", std::process::id()));
        let mut with_ai = options(output.clone(), (0.0, 1.0));
        with_ai.events = true;
        with_ai.tracking = Some(tracking(Some(model)));
        let job = ExportJob::start(
            InputPath::Single(left),
            InputPath::Single(right),
            MatchCalibration::from_file(&cal).unwrap(),
            with_ai,
            Arc::new(|| {}),
        );
        let events = until_done(&job, 180, false);
        assert!(
            events.contains(&ExportEvent::Tracking(Ok(()))),
            "tracking is active: {events:?}"
        );
        let Some(ExportEvent::Done { path, .. }) = events.last() else {
            panic!("{events:?}")
        };
        let log =
            std::fs::read_to_string(output.with_extension("events.jsonl")).unwrap_or_default();
        assert!(log.contains("detect"), "the events file has the detections");
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(output.with_extension("events.jsonl"));
    }

    /// The engine's telemetry has the detector's figures only without a
    /// lookahead (its buffered produce phase records none; FRICTION.md).
    #[cfg(feature = "ai")]
    #[cfg_attr(
        debug_assertions,
        ignore = "needs an optimized build: Metal validation (FRICTION.md)"
    )]
    #[test]
    fn a_tracked_export_reports_the_detectors_figures() {
        let _heavy = fixtures::heavy();
        let (Some((left, right, cal)), Some(model)) = (fixtures::fast_set(), fixtures::model())
        else {
            return;
        };
        let output =
            std::env::temp_dir().join(format!("reco-app-export-figures-{}", std::process::id()));
        let mut with_ai = options(output, (0.0, 2.0));
        let mut tracking = tracking(Some(model));
        tracking.lookahead_secs = 0.0;
        with_ai.tracking = Some(tracking);
        let job = ExportJob::start(
            InputPath::Single(left),
            InputPath::Single(right),
            MatchCalibration::from_file(&cal).unwrap(),
            with_ai,
            Arc::new(|| {}),
        );
        let events = until_done(&job, 180, false);
        let figures: Vec<&AiFigures> = events
            .iter()
            .filter_map(|e| match e {
                ExportEvent::AiFigures(f) => Some(f),
                _ => None,
            })
            .collect();
        assert!(
            !figures.is_empty() && figures.iter().all(|f| f.detection_ms > 0.0),
            "the detector's figures arrive while exporting: {figures:?}"
        );
        assert!(figures.iter().any(|f| f.per_frame > 0.0), "{figures:?}");
        if let Some(ExportEvent::Done { path, .. }) = events.last() {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn an_export_reports_its_figures() {
        let _heavy = fixtures::heavy();
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let output =
            std::env::temp_dir().join(format!("reco-app-export-stages-{}", std::process::id()));
        let job = ExportJob::start(
            InputPath::Single(left),
            InputPath::Single(right),
            MatchCalibration::from_file(&cal).unwrap(),
            options(output, (0.0, 2.0)),
            Arc::new(|| {}),
        );
        let events = until_done(&job, 180, false);
        let figures: Vec<&ExportFigures> = events
            .iter()
            .filter_map(|e| match e {
                ExportEvent::Figures(f) => Some(f),
                _ => None,
            })
            .collect();
        assert!(
            !figures.is_empty(),
            "figures arrive while exporting: {events:?}"
        );
        let last = figures.last().unwrap();
        assert!(last.fps > 0.0 && last.frame_ms > 0.0, "{last:?}");
        assert!(last.p99_ms >= last.frame_ms * 0.5, "{last:?}");
        if let Some(ExportEvent::Done { path, .. }) = events.last() {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn figures_come_only_once_the_detector_has_run() {
        assert_eq!(ai_figures(0.0, 0, 0.0, 0, 0.0), None, "nothing measured");
        assert_eq!(
            ai_figures(12.5, 40, 1.3, 9, 62.0),
            Some(AiFigures {
                detection_ms: 12.5,
                per_frame: 1.3,
                tracks: 9,
                ball_pct: 62.0
            })
        );
        assert!(
            ai_figures(8.0, 0, 0.0, 0, 0.0).is_some(),
            "it ran and found nothing"
        );
    }

    #[test]
    fn tracking_without_its_model_fails_before_writing() {
        let _heavy = fixtures::heavy();
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let output = std::env::temp_dir().join(format!(
            "reco-app-export-nomodel-{}.mp4",
            std::process::id()
        ));
        let mut no_model = options(output.clone(), (0.0, 1.0));
        no_model.tracking = Some(tracking(Some(PathBuf::from("/no/such/yolo.onnx"))));
        let job = ExportJob::start(
            InputPath::Single(left),
            InputPath::Single(right),
            MatchCalibration::from_file(&cal).unwrap(),
            no_model,
            Arc::new(|| {}),
        );
        let events = until_done(&job, 30, false);
        assert_eq!(
            events.last(),
            Some(&ExportEvent::Failed(
                "The AI model isn't there any more: /no/such/yolo.onnx".into()
            ))
        );
        assert!(!output.exists(), "nothing written");
    }

    #[test]
    fn cancelling_an_export_stops_it() {
        let _heavy = fixtures::heavy();
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let output =
            std::env::temp_dir().join(format!("reco-app-export-cancel-{}.mp4", std::process::id()));
        let job = ExportJob::start(
            InputPath::Single(left),
            InputPath::Single(right),
            MatchCalibration::from_file(&cal).unwrap(),
            options(output.clone(), (0.0, 50.0)),
            Arc::new(|| {}),
        );
        let events = until_done(&job, 120, true);
        assert_eq!(events.last(), Some(&ExportEvent::Cancelled), "{events:?}");
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn outputs_are_checked_before_exporting() {
        let folder =
            std::env::temp_dir().join(format!("reco-app-export-out-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let left = folder.join("GX010120.MP4");
        std::fs::write(&left, b"").unwrap();
        let inputs = [left.clone()];
        assert_eq!(
            checked_output(&folder.join("match").display().to_string(), &left, &inputs),
            Ok(folder.join("match.mp4"))
        );
        assert_eq!(
            checked_output("match.mkv", &left, &inputs),
            Ok(folder.join("match.mkv")),
            "a bare name goes beside the videos"
        );
        assert!(
            checked_output("  ", &left, &inputs).is_err(),
            "nothing typed"
        );
        assert!(
            checked_output("match.json", &left, &inputs).is_err(),
            "not a video file (a calibration would be overwritten)"
        );
        assert_eq!(
            checked_output("match.MOV", &left, &inputs),
            Ok(folder.join("match.MOV"))
        );
        assert!(
            checked_output(
                &folder.join("missing/match.mp4").display().to_string(),
                &left,
                &inputs
            )
            .is_err(),
            "no such folder"
        );
        assert!(
            checked_output(&folder.display().to_string(), &left, &inputs).is_err(),
            "a folder"
        );
        assert!(
            checked_output(&left.display().to_string(), &left, &inputs).is_err(),
            "the left video itself"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn an_empty_range_fails_at_once() {
        let _heavy = fixtures::heavy();
        let output =
            std::env::temp_dir().join(format!("reco-app-export-empty-{}.mp4", std::process::id()));
        let job = ExportJob::start(
            InputPath::Single("left.mp4".into()),
            InputPath::Single("right.mp4".into()),
            some_calibration(),
            options(output.clone(), (10.0, 10.0)),
            Arc::new(|| {}),
        );
        let events = until_done(&job, 5, false);
        assert!(
            matches!(events.as_slice(), [ExportEvent::Failed(_)]),
            "{events:?}"
        );
        assert!(!output.exists(), "nothing was written");
    }

    #[test]
    fn the_replay_and_events_files_go_beside_the_export() {
        let _heavy = fixtures::heavy();
        let Some((left, right, cal)) = fixtures::fast_set() else {
            return;
        };
        let output =
            std::env::temp_dir().join(format!("reco-app-export-extras-{}.mp4", std::process::id()));
        let mut options = options(output.clone(), (0.0, 0.5));
        options.replay = true;
        options.events = true;
        let job = ExportJob::start(
            InputPath::Single(left),
            InputPath::Single(right),
            MatchCalibration::from_file(&cal).unwrap(),
            options,
            Arc::new(|| {}),
        );
        let events = until_done(&job, 120, false);
        assert!(
            matches!(events.last(), Some(ExportEvent::Done { .. })),
            "{events:?}"
        );
        let replay = output.with_extension("replay.mkv");
        let jsonl = output.with_extension("events.jsonl");
        assert!(
            replay.metadata().is_ok_and(|m| m.len() > 0),
            "the replay recording"
        );
        assert!(jsonl.exists(), "the pipeline events");
        for file in [&output, &replay, &jsonl] {
            let _ = std::fs::remove_file(file);
        }
    }
}
