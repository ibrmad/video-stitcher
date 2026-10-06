//! The App's side of a live preview: the worker handle, what it plays, and
//! a meter for the frame rate shown in the status line.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use reco_app::preview::lanes::Lanes;
use reco_app::preview::playback::PlayState;
use reco_app::preview::worker::{PreviewInfo, PreviewWorker};

use crate::cli::FileArgs;
use crate::time_ruler::clock;

/// The live preview: the render thread, and what it has open.
pub struct Live {
    /// The render thread. It stays for the app's life once started, so its
    /// texture ring outlives any picture on screen.
    pub worker: PreviewWorker,
    /// Whether videos are open (`files`); false after `close`.
    pub open: bool,
    /// The files it opened last.
    pub files: FileArgs,
    /// What it plays, once open.
    pub info: Option<PreviewInfo>,
    /// Frames taken so far.
    pub frame: u64,
    /// Playing, paused or finished.
    pub state: PlayState,
    /// Whether playback has run since opening.
    pub played: bool,
    /// Frames per second actually shown.
    pub fps: FpsMeter,
    /// The last frame-rate reading while playing.
    pub fps_reading: Option<f64>,
    /// A failure to show in the status line until playback moves on.
    pub problem: Option<String>,
    /// Each camera's files, once probed.
    pub lanes: Option<Lanes>,
    /// Frames recorded so far, while recording.
    pub recording: Option<u64>,
    /// The last file written, for Show in folder.
    pub last_output: Option<PathBuf>,
}

impl Live {
    /// A session that has just been asked to open `files`.
    pub fn new(worker: PreviewWorker, files: FileArgs) -> Self {
        Self {
            worker,
            open: true,
            files,
            info: None,
            frame: 0,
            state: PlayState::Paused,
            played: false,
            fps: FpsMeter::default(),
            fps_reading: None,
            problem: None,
            lanes: None,
            recording: None,
            last_output: None,
        }
    }

    /// New files are opening: forget the last session.
    pub fn reopen(&mut self, files: FileArgs) {
        self.open = true;
        self.files = files;
        self.forget();
    }

    /// The videos were closed.
    pub fn close(&mut self) {
        self.open = false;
        self.forget();
    }

    fn forget(&mut self) {
        self.info = None;
        self.frame = 0;
        self.state = PlayState::Paused;
        self.played = false;
        self.fps.reset();
        self.fps_reading = None;
        self.problem = None;
        self.lanes = None;
        self.recording = None;
    }

    /// The status line for this session.
    pub fn status(&self) -> String {
        status_line(&StatusInputs {
            state: self.info.as_ref().map(|_| self.state),
            played: self.played,
            fps: self.fps_reading,
            recording: self
                .recording
                .map(|frames| frames as f64 / self.info.as_ref().map_or(30.0, |i| i.fps.max(1.0))),
            problem: self.problem.as_deref(),
        })
    }
}

/// Counts frames and reports their rate once a second.
#[derive(Debug, Default)]
pub struct FpsMeter {
    since: Option<Instant>,
    frames: u32,
}

impl FpsMeter {
    /// A frame was shown at `now`; a reading once a second has passed.
    pub fn tick(&mut self, now: Instant) -> Option<f64> {
        let since = *self.since.get_or_insert(now);
        self.frames += 1;
        let elapsed = now.duration_since(since);
        if elapsed < Duration::from_secs(1) {
            return None;
        }
        let rate = f64::from(self.frames - 1) / elapsed.as_secs_f64();
        self.since = Some(now);
        self.frames = 1;
        Some(rate)
    }

    /// Start over: the next reading waits a full second again.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// What the status line is made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusInputs<'a> {
    /// The play state; `None` before the videos opened.
    pub state: Option<PlayState>,
    /// Whether playback has run since opening ("Paused", not "Ready").
    pub played: bool,
    /// The last frame-rate reading while playing.
    pub fps: Option<f64>,
    /// Seconds recorded, while recording.
    pub recording: Option<f64>,
    /// A failure to show until playback moves on.
    pub problem: Option<&'a str>,
}

/// The status line: recording, then a problem, then what playback does.
pub fn status_line(s: &StatusInputs) -> String {
    if let Some(secs) = s.recording {
        return format!("Recording · {}", clock(secs));
    }
    if let Some(problem) = s.problem {
        return problem.to_string();
    }
    match s.state {
        Some(PlayState::Playing) => s
            .fps
            .map_or_else(|| "Playing".to_string(), |f| format!("{f:.1} fps")),
        Some(PlayState::Paused) if s.played => "Paused".to_string(),
        Some(PlayState::Paused) => "Ready".to_string(),
        Some(PlayState::Finished) => "Finished".to_string(),
        Some(PlayState::Empty) | None => String::new(),
    }
}

/// The first file of each camera, by name (for the title and messages).
pub fn names(files: &FileArgs) -> (String, String) {
    let name = |paths: &[std::path::PathBuf]| {
        paths
            .first()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    (name(&files.left), name(&files.right))
}

/// "1 file", "21 files".
pub fn file_count(n: usize) -> String {
    if n == 1 {
        "1 file".to_string()
    } else {
        format!("{n} files")
    }
}

/// The playable length in seconds (0 when unknown).
pub fn length_secs(info: &PreviewInfo) -> f64 {
    match info.total_frames {
        Some(frames) if info.fps > 0.0 => frames as f64 / info.fps,
        _ => 0.0,
    }
}

/// A worker failure ("Couldn't open the videos: file not found") as a
/// title and a detail with its first letter raised.
pub fn failure_text(message: &str) -> (String, String) {
    let Some((title, detail)) = message.split_once(": ") else {
        return (message.to_string(), String::new());
    };
    let mut chars = detail.chars();
    let detail = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    (title.to_string(), detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn names_are_the_first_files() {
        let files = FileArgs {
            left: vec![
                PathBuf::from("/a/GX010120.MP4"),
                PathBuf::from("/a/GX020120.MP4"),
            ],
            right: vec![PathBuf::from("/b/GX010092.MP4")],
            calibration: PathBuf::from("/a/cal.json"),
        };
        assert_eq!(
            names(&files),
            ("GX010120.MP4".to_string(), "GX010092.MP4".to_string())
        );
    }

    #[test]
    fn counts_files() {
        assert_eq!(file_count(1), "1 file");
        assert_eq!(file_count(21), "21 files");
    }

    #[test]
    fn length_from_frames_and_rate() {
        let info = PreviewInfo {
            fps: 30.0,
            total_frames: Some(1800),
            width: 1,
            height: 1,
            zero_copy: true,
            gpu: String::new(),
            backend: "Metal".into(),
            vram: None,
        };
        assert_eq!(length_secs(&info), 60.0);
        assert_eq!(
            length_secs(&PreviewInfo {
                total_frames: None,
                ..info.clone()
            }),
            0.0
        );
        assert_eq!(length_secs(&PreviewInfo { fps: 0.0, ..info }), 0.0);
    }

    #[test]
    fn failures_split_into_title_and_detail() {
        assert_eq!(
            failure_text("Couldn't open the videos: invalid input path (/x/l.mp4): file not found"),
            (
                "Couldn't open the videos".to_string(),
                "Invalid input path (/x/l.mp4): file not found".to_string()
            )
        );
        assert_eq!(
            failure_text("No frame decoded yet"),
            ("No frame decoded yet".to_string(), String::new())
        );
    }

    #[test]
    fn reads_once_a_second() {
        let t0 = Instant::now();
        let mut m = FpsMeter::default();
        assert_eq!(m.tick(t0), None);
        for i in 1..30 {
            assert_eq!(m.tick(t0 + Duration::from_millis(i * 33)), None);
        }
        let reading = m.tick(t0 + Duration::from_millis(1000)).expect("a reading");
        assert!((reading - 30.0).abs() < 0.5, "{reading}");
    }

    #[test]
    fn status_says_what_playback_does() {
        let s = |state, played, fps| {
            status_line(&StatusInputs {
                state: Some(state),
                played,
                fps,
                ..StatusInputs::default()
            })
        };
        assert_eq!(s(PlayState::Paused, false, None), "Ready");
        assert_eq!(s(PlayState::Playing, true, None), "Playing");
        assert_eq!(s(PlayState::Playing, true, Some(29.96)), "30.0 fps");
        assert_eq!(s(PlayState::Paused, true, None), "Paused");
        assert_eq!(s(PlayState::Finished, true, None), "Finished");
        assert_eq!(status_line(&StatusInputs::default()), "");
    }

    #[test]
    fn recording_and_problems_come_first() {
        let recording = StatusInputs {
            state: Some(PlayState::Playing),
            recording: Some(75.0),
            problem: Some("x"),
            ..StatusInputs::default()
        };
        assert_eq!(status_line(&recording), "Recording · 1:15");
        let problem = StatusInputs {
            state: Some(PlayState::Paused),
            problem: Some("Couldn't seek"),
            ..StatusInputs::default()
        };
        assert_eq!(status_line(&problem), "Couldn't seek");
    }

    #[test]
    fn a_reset_meter_starts_over() {
        let t0 = Instant::now();
        let mut m = FpsMeter::default();
        m.tick(t0);
        m.reset();
        assert_eq!(
            m.tick(t0 + Duration::from_secs(5)),
            None,
            "a reset meter waits a second again"
        );
    }
}
