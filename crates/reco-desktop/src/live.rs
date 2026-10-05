//! The App's side of a live preview: the worker handle, what it plays, and
//! a meter for the frame rate shown in the status line.

use std::time::{Duration, Instant};

use reco_app::preview::worker::{PreviewInfo, PreviewWorker};

use crate::cli::FileArgs;

/// One open live preview.
pub struct Live {
    /// The render thread.
    pub worker: PreviewWorker,
    /// The files it opened.
    pub files: FileArgs,
    /// What it plays, once open.
    pub info: Option<PreviewInfo>,
    /// Frames taken so far.
    pub frame: u64,
    /// Whether playing.
    pub playing: bool,
    /// Frames per second actually shown.
    pub fps: FpsMeter,
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
}
