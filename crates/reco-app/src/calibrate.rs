//! Auto-calibration as a job: it runs Reco's calibration on its own thread,
//! reports each step, can be cancelled, and saves the result beside the
//! first left file (`Project::sibling_calibration`, where the Slint app
//! saved it) before it reports.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use reco_calibrate::video::{CalibrateVideosError, CalibrateVideosOptions, calibrate_videos};
use reco_calibrate::{CalibrationConfig, CalibrationStep, ProfileSource};
use reco_core::calibration::CameraParams;

/// Below this confidence a warning says the stitch may be poor (the Slint
/// app's threshold).
pub const LOW_CONFIDENCE: f64 = 0.5;

/// The steps a calibration reports.
pub const STEPS: usize = 7;

/// The frame counts the Frames dropdown offers.
pub const FRAME_CHOICES: [usize; 4] = [2, 4, 6, 8];

/// The Advanced options (the Slint app's defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationOptions {
    /// Frame pairs to match (2, 4, 6 or 8).
    pub frames: usize,
    /// Seed the rotation from the cameras' motion sensors.
    pub imu_seeds: bool,
    /// AKAZE response threshold: lower finds more features.
    pub akaze_threshold: f64,
    /// The band of the picture searched for features, top and bottom
    /// (fractions of its height).
    pub detect_y: (f64, f64),
    /// Seconds skipped at the start (the preview's time when
    /// recalibrating).
    pub skip_start: f64,
    /// Seconds skipped at the end.
    pub skip_end: f64,
}

impl Default for CalibrationOptions {
    fn default() -> Self {
        Self {
            frames: 4,
            imu_seeds: false,
            akaze_threshold: 0.0001,
            detect_y: (0.05, 0.95),
            skip_start: 0.0,
            skip_end: 0.0,
        }
    }
}

impl CalibrationOptions {
    /// The calibration's config for these options.
    pub fn config(&self) -> CalibrationConfig {
        let mut config = CalibrationConfig {
            num_frames: self.frames.max(2),
            skip_start_secs: self.skip_start,
            skip_end_secs: self.skip_end,
            use_imu_rotation_seeds: self.imu_seeds,
            ..CalibrationConfig::default()
        };
        config.akaze.threshold = self.akaze_threshold;
        config.akaze.detect_y_min = self.detect_y.0;
        config.akaze.detect_y_max = self.detect_y.1;
        config
    }
}

/// The step's number, from 1 to [`STEPS`].
pub fn step_index(step: CalibrationStep) -> usize {
    match step {
        CalibrationStep::Probing => 1,
        CalibrationStep::DetectingProfiles => 2,
        CalibrationStep::AudioSync => 3,
        CalibrationStep::ExtractingFrames => 4,
        CalibrationStep::Undistorting => 5,
        CalibrationStep::FeatureMatching => 6,
        CalibrationStep::Optimizing => 7,
    }
}

/// What the step does, in plain words.
pub fn step_words(step: CalibrationStep) -> &'static str {
    match step {
        CalibrationStep::Probing => "Reading the videos",
        CalibrationStep::DetectingProfiles => "Finding the lens profiles",
        CalibrationStep::AudioSync => "Syncing the cameras by sound",
        CalibrationStep::ExtractingFrames => "Taking frames from both cameras",
        CalibrationStep::Undistorting => "Straightening the lenses",
        CalibrationStep::FeatureMatching => "Matching the pitch markings",
        CalibrationStep::Optimizing => "Fitting the cameras together",
    }
}

/// A finished calibration.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationDone {
    /// Where it was saved.
    pub path: PathBuf,
    /// Confidence, 0 to 1.
    pub confidence: f64,
    /// Matched point pairs.
    pub matches: usize,
    /// Frame pairs that gave matches.
    pub frames_used: usize,
    /// A camera had no lens profile and a generic one was used.
    pub fallback_lens: bool,
}

/// What a calibration job reports.
#[derive(Clone, Debug, PartialEq)]
pub enum CalibrationEvent {
    /// Working on step `step` of `of`.
    Progress {
        /// The step's number.
        step: usize,
        /// Steps in all.
        of: usize,
        /// What it does.
        text: String,
    },
    /// Calibrated and saved.
    Done(CalibrationDone),
    /// It failed; why.
    Failed(String),
    /// It was cancelled.
    Cancelled,
}

/// A calibration running on its own thread. Dropping it cancels it.
pub struct CalibrationJob {
    cancel: Arc<AtomicBool>,
    events: Receiver<CalibrationEvent>,
}

impl CalibrationJob {
    /// Calibrate `left` against `right` (each camera's first file) and save
    /// the result to `save_to`. `lens` keeps the current cameras' lens when
    /// recalibrating. `waker` runs after every event.
    pub fn start(
        left: PathBuf,
        right: PathBuf,
        save_to: PathBuf,
        lens: Option<(CameraParams, CameraParams)>,
        options: CalibrationOptions,
        waker: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let report = tx.clone();
        let flag = Arc::clone(&cancel);
        let wake = Arc::clone(&waker);
        let spawned = std::thread::Builder::new()
            .name("reco-calibrate".into())
            .spawn(move || {
                let send = |event: CalibrationEvent| {
                    if tx.send(event).is_ok() {
                        wake();
                    }
                };
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    calibrate(&left, &right, &save_to, lens, &options, &flag, &send)
                }));
                send(run.unwrap_or_else(|panic| {
                    let reason = panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    CalibrationEvent::Failed(format!("the calibration crashed: {reason}"))
                }));
            });
        if let Err(e) = spawned {
            let _ = report.send(CalibrationEvent::Failed(format!(
                "the calibration couldn't start: {e}"
            )));
            waker();
        }
        Self { cancel, events }
    }

    /// Stop as soon as the calibration checks in (it then reports
    /// `Cancelled`).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The next event, if any (never blocks).
    pub fn try_event(&self) -> Option<CalibrationEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for CalibrationJob {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Run the calibration, save it and say how it ended.
fn calibrate(
    left: &Path,
    right: &Path,
    save_to: &Path,
    lens: Option<(CameraParams, CameraParams)>,
    options: &CalibrationOptions,
    cancel: &AtomicBool,
    send: &dyn Fn(CalibrationEvent),
) -> CalibrationEvent {
    let (left_params, right_params) = lens.map_or((None, None), |(l, r)| (Some(l), Some(r)));
    let result = calibrate_videos(
        left,
        right,
        CalibrateVideosOptions {
            config: Some(options.config()),
            left_params,
            right_params,
            ..CalibrateVideosOptions::default()
        },
        &mut |progress| {
            send(CalibrationEvent::Progress {
                step: step_index(progress.step),
                of: STEPS,
                text: step_words(progress.step).to_string(),
            })
        },
        cancel,
    );
    let result = match result {
        Ok(_) if cancel.load(Ordering::Relaxed) => return CalibrationEvent::Cancelled,
        Ok(result) => result,
        Err(CalibrateVideosError::Cancelled) => return CalibrationEvent::Cancelled,
        Err(e) => return CalibrationEvent::Failed(e.to_string()),
    };
    if let Err(e) = save_atomically(save_to, &result.calibration.to_json_pretty()) {
        return CalibrationEvent::Failed(format!("couldn't save {}: {e}", save_to.display()));
    }
    let fallback_lens = [&result.left_lens_profile, &result.right_lens_profile]
        .into_iter()
        .flatten()
        .any(|p| matches!(p.source, ProfileSource::Fallback));
    CalibrationEvent::Done(CalibrationDone {
        path: save_to.to_path_buf(),
        confidence: result.confidence,
        matches: result.total_matches,
        frames_used: result.frames_used,
        fallback_lens,
    })
}

/// Write `json` to `path` through a temporary file, so a half-written
/// calibration never replaces a good one.
fn save_atomically(path: &Path, json: &str) -> std::io::Result<()> {
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use reco_core::calibration::MatchCalibration;

    use super::*;
    use crate::preview::fixtures;

    fn wait_for(job: &CalibrationJob, secs: u64) -> Vec<CalibrationEvent> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            while let Some(e) = job.try_event() {
                let last = !matches!(e, CalibrationEvent::Progress { .. });
                seen.push(e);
                if last {
                    return seen;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        seen
    }

    /// A fixture pair linked into a fresh folder, so the saved calibration
    /// lands there and not beside the fixtures.
    #[cfg(unix)]
    fn linked_pair(
        name: &str,
        set: Option<(PathBuf, PathBuf, PathBuf)>,
    ) -> Option<(PathBuf, PathBuf, PathBuf)> {
        let (left, right, _) = set?;
        let dir = std::env::temp_dir().join(format!("reco-app-cal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        let l = dir.join(left.file_name()?);
        let r = dir.join(format!("r-{}", right.file_name()?.to_string_lossy()));
        std::os::unix::fs::symlink(&left, &l).ok()?;
        std::os::unix::fs::symlink(&right, &r).ok()?;
        let stem = left.file_stem()?.to_string_lossy().into_owned();
        Some((l, r, dir.join(format!("{stem}_calibration.json"))))
    }

    #[test]
    fn options_map_onto_the_config() {
        let options = CalibrationOptions {
            frames: 6,
            imu_seeds: true,
            akaze_threshold: 0.002,
            detect_y: (0.1, 0.8),
            skip_start: 12.5,
            skip_end: 30.0,
        };
        let c = options.config();
        assert_eq!((c.num_frames, c.use_imu_rotation_seeds), (6, true));
        assert_eq!((c.skip_start_secs, c.skip_end_secs), (12.5, 30.0));
        assert_eq!(
            (
                c.akaze.threshold,
                c.akaze.detect_y_min,
                c.akaze.detect_y_max
            ),
            (0.002, 0.1, 0.8)
        );
        let too_few = CalibrationOptions {
            frames: 0,
            ..CalibrationOptions::default()
        };
        assert_eq!(too_few.config().num_frames, 2);
        assert_eq!(CalibrationOptions::default().config().num_frames, 4);
    }

    #[test]
    fn steps_count_from_one_to_seven() {
        assert_eq!(step_index(CalibrationStep::Probing), 1);
        assert_eq!(step_index(CalibrationStep::FeatureMatching), 6);
        assert_eq!(step_index(CalibrationStep::Optimizing), STEPS);
        assert_eq!(
            step_words(CalibrationStep::FeatureMatching),
            "Matching the pitch markings"
        );
    }

    #[test]
    fn saving_replaces_the_file_whole() {
        let dir = std::env::temp_dir().join(format!("reco-app-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c_calibration.json");
        save_atomically(&path, "{\"a\":1}").unwrap();
        save_atomically(&path, "{\"a\":2}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
        let leftovers = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary file is left behind");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn cancelling_stops_the_job() {
        let Some((left, right, save_to)) = linked_pair("cancel", fixtures::fast_set()) else {
            return;
        };
        let job = CalibrationJob::start(
            left,
            right,
            save_to.clone(),
            None,
            CalibrationOptions::default(),
            Arc::new(|| {}),
        );
        job.cancel();
        let events = wait_for(&job, 30);
        assert_eq!(
            events.last(),
            Some(&CalibrationEvent::Cancelled),
            "{events:?}"
        );
        assert!(!save_to.exists());
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "slow: a full calibration attempt (about 35 s); run with --ignored"]
    fn footage_with_no_matches_fails_plainly() {
        // The alfheim pair is not GoPro footage: Reco's calibration finds no
        // usable frame pairs in it.
        let Some((left, right, save_to)) = linked_pair("fail", fixtures::fast_set()) else {
            return;
        };
        let job = CalibrationJob::start(
            left,
            right,
            save_to.clone(),
            None,
            CalibrationOptions::default(),
            Arc::new(|| {}),
        );
        let events = wait_for(&job, 300);
        assert!(
            matches!(events.last(), Some(CalibrationEvent::Failed(why)) if why.contains("no usable frame pairs")),
            "{events:?}"
        );
        assert!(!save_to.exists());
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "slow: calibrates the 5.3K pair (about 4 min); run with --ignored"]
    fn calibrating_saves_beside_the_left_file() {
        let Some((left, right, save_to)) = linked_pair("save", fixtures::real_set()) else {
            return;
        };
        let job = CalibrationJob::start(
            left,
            right,
            save_to.clone(),
            None,
            CalibrationOptions::default(),
            Arc::new(|| {}),
        );
        let events = wait_for(&job, 300);
        assert!(
            events.iter().any(|e| matches!(
                e,
                CalibrationEvent::Progress {
                    step: 1,
                    of: STEPS,
                    ..
                }
            )),
            "{events:?}"
        );
        let Some(CalibrationEvent::Done(done)) = events.last() else {
            panic!("{events:?}")
        };
        assert_eq!(done.path, save_to);
        assert!((0.0..=1.0).contains(&done.confidence), "{done:?}");
        MatchCalibration::from_file(&save_to).expect("a calibration the preview can read");
    }
}
