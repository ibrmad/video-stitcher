//! The cameras' lenses as the Adjust panel shows and tunes them: the
//! intrinsics a slider moves (`Lens`), the fine-tune sliders' ranges, and a
//! lens profile scaled to the videos' size. The rules are the Slint app's
//! (`set_lens_sliders`, the lens picker).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use reco_calibrate::lens_database::{LensDatabase, detect_profile, load_from_file};
pub use reco_calibrate::types::LensProfileSummary;
use reco_calibrate::types::{LensProfileInfo, ProfileSource};
use reco_core::calibration::CameraParams;

/// A camera's lens: focal lengths and principal point in pixels, and the
/// four fisheye (KB4) distortion terms. The size it was modelled at stays
/// with the calibration.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lens {
    /// Focal length, x (pixels).
    pub fx: f64,
    /// Focal length, y (pixels).
    pub fy: f64,
    /// Principal point, x (pixels).
    pub cx: f64,
    /// Principal point, y (pixels).
    pub cy: f64,
    /// Distortion k1–k4.
    pub k: [f64; 4],
}

impl Lens {
    /// The lens in `params`.
    pub fn of(params: &CameraParams) -> Self {
        Self {
            fx: params.fx,
            fy: params.fy,
            cx: params.cx,
            cy: params.cy,
            k: params.d,
        }
    }

    /// `params` with this lens (its size kept).
    pub fn applied_to(&self, params: &CameraParams) -> CameraParams {
        CameraParams {
            fx: self.fx,
            fy: self.fy,
            cx: self.cx,
            cy: self.cy,
            d: self.k,
            ..params.clone()
        }
    }
}

/// Which cameras a lens change is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cameras {
    /// The left camera.
    Left,
    /// The right camera.
    Right,
    /// Both, in step.
    Both,
}

impl Cameras {
    /// Whether the left camera is included.
    pub fn left(self) -> bool {
        matches!(self, Self::Left | Self::Both)
    }

    /// Whether the right camera is included.
    pub fn right(self) -> bool {
        matches!(self, Self::Right | Self::Both)
    }
}

/// The fine-tune sliders' ranges, centred on a lens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FineTuneRanges {
    /// fx (pixels).
    pub fx: (f64, f64),
    /// fy (pixels).
    pub fy: (f64, f64),
    /// cx (pixels).
    pub cx: (f64, f64),
    /// cy (pixels).
    pub cy: (f64, f64),
    /// k1–k4, each around its own value.
    pub k: [(f64, f64); 4],
}

impl FineTuneRanges {
    /// The Slint app's ranges around `lens` for a `width`×`height` picture:
    /// fx and fy ±15% of the larger focal length, cx and cy ±10% of the
    /// picture, at least 5 px each; each of k1–k4 ±0.3 around its own value
    /// (the Slint app's ±0.3 was absolute, which can't show a calibrated k1
    /// of 0.333).
    pub fn around(lens: &Lens, width: u32, height: u32) -> Self {
        let f = (lens.fx.max(lens.fy) * 0.15).max(5.0);
        let x = (f64::from(width.max(1)) * 0.10).max(5.0);
        let y = (f64::from(height.max(1)) * 0.10).max(5.0);
        Self {
            fx: (lens.fx - f, lens.fx + f),
            fy: (lens.fy - f, lens.fy + f),
            cx: (lens.cx - x, lens.cx + x),
            cy: (lens.cy - y, lens.cy + y),
            k: lens.k.map(|k| (k - 0.3, k + 0.3)),
        }
    }
}

/// Where a camera's lens came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LensSource {
    /// Read from the video's own telemetry.
    Detected,
    /// Matched in Reco's lens database.
    Database,
    /// Picked in the lens picker.
    Picker,
    /// Loaded from a profile file.
    File,
    /// A generic profile: nothing matched.
    Fallback,
}

impl LensSource {
    /// The word the Adjust panel shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Detected => "Detected",
            Self::Database => "Database",
            Self::Picker => "Picked",
            Self::File => "File",
            Self::Fallback => "Generic",
        }
    }
}

/// What a camera's lens is: the camera, its lens mode, and where that
/// came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LensInfo {
    /// Camera brand and model ("GoPro HERO9 Black").
    pub camera: String,
    /// Lens mode ("Wide 5.3K").
    pub lens: String,
    /// Where it came from.
    pub source: LensSource,
}

impl LensInfo {
    /// From a calibration's or a detection's profile.
    pub fn from_profile(profile: &LensProfileInfo) -> Self {
        let source = match profile.source {
            ProfileSource::AutoDetected => LensSource::Detected,
            ProfileSource::Database => LensSource::Database,
            ProfileSource::File(_) => LensSource::File,
            ProfileSource::Fallback => LensSource::Fallback,
        };
        Self {
            camera: profile.camera.clone(),
            lens: profile.lens.clone(),
            source,
        }
    }

    /// "GoPro HERO9 Black · Wide 5.3K".
    pub fn name(&self) -> String {
        if self.lens.is_empty() {
            self.camera.clone()
        } else {
            format!("{} · {}", self.camera, self.lens)
        }
    }
}

/// The lens of `video` (`width`×`height`), read from its telemetry or
/// matched in the database; `None` when neither knows it. Blocking (it
/// reads the video's telemetry): run it off the UI thread.
pub fn detect(video: &Path, width: u32, height: u32) -> Option<LensInfo> {
    detect_profile(video, width, height, LensDatabase::embedded(), None)
        .map(|(_, profile)| LensInfo::from_profile(&profile))
}

/// Both cameras' lenses being detected on a short thread.
pub struct LensDetection {
    result: Receiver<(Option<LensInfo>, Option<LensInfo>)>,
}

impl LensDetection {
    /// Detect the lenses of `left` and `right` (`size` each); `waker` runs
    /// when they are known.
    pub fn start(
        left: PathBuf,
        right: PathBuf,
        size: (u32, u32),
        waker: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (tx, result) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("reco-lens".into()).spawn({
            let tx = tx.clone();
            let waker = Arc::clone(&waker);
            move || {
                let (w, h) = size;
                if tx.send((detect(&left, w, h), detect(&right, w, h))).is_ok() {
                    waker();
                }
            }
        });
        if spawned.is_err() {
            // Unknown, rather than never answering.
            let _ = tx.send((None, None));
            waker();
        }
        Self { result }
    }

    /// The two lenses, once known (never blocks).
    pub fn try_result(&self) -> Option<(Option<LensInfo>, Option<LensInfo>)> {
        self.result.try_recv().ok()
    }
}

/// A profile's line in the picker: "GoPro HERO9 Black · Wide · 5312 × 2988".
pub fn profile_line(profile: &LensProfileSummary) -> String {
    format!(
        "{} · {} · {} × {}",
        profile.camera, profile.lens, profile.width, profile.height
    )
}

/// A database profile's lens at `size` (the calibration's lens size).
pub fn profile_lens(profile: &LensProfileSummary, size: (u32, u32)) -> Option<Lens> {
    LensDatabase::embedded()
        .load_by_summary(profile)
        .map(|params| Lens::of(&scaled_to(&params, size.0, size.1)))
}

/// A profile file's lens at `size` (Gyroflow and Reco formats).
pub fn file_lens(path: &Path, size: (u32, u32)) -> Result<Lens, String> {
    load_from_file(path)
        .map(|params| Lens::of(&scaled_to(&params, size.0, size.1)))
        .map_err(|e| e.to_string())
}

/// The picker's search: each query on a short thread; only the newest
/// query's results are kept.
pub struct ProfileSearch {
    newest: u64,
    tx: Sender<(u64, Vec<LensProfileSummary>)>,
    rx: Receiver<(u64, Vec<LensProfileSummary>)>,
}

impl Default for ProfileSearch {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { newest: 0, tx, rx }
    }
}

impl ProfileSearch {
    /// Search for `query` among profiles, `size` first; `waker` runs when
    /// the results are in.
    pub fn search(&mut self, query: &str, size: (u32, u32), waker: Arc<dyn Fn() + Send + Sync>) {
        self.newest += 1;
        let (number, query, tx) = (self.newest, query.to_string(), self.tx.clone());
        let _ = std::thread::Builder::new()
            .name("reco-lens-search".into())
            .spawn(move || {
                let found = LensDatabase::embedded().search(&query, size.0, size.1);
                if tx.send((number, found)).is_ok() {
                    waker();
                }
            });
    }

    /// The newest query's results, once in (never blocks).
    pub fn try_results(&mut self) -> Option<Vec<LensProfileSummary>> {
        let mut newest = None;
        while let Ok((number, found)) = self.rx.try_recv() {
            if number == self.newest {
                newest = Some(found);
            }
        }
        newest
    }
}

/// A lens profile (from the database or a file) scaled to a
/// `width`×`height` video, as the Slint app's picker did.
pub fn scaled_to(profile: &CameraParams, width: u32, height: u32) -> CameraParams {
    if profile.width == 0 || profile.height == 0 {
        return profile.clone();
    }
    let sx = f64::from(width) / f64::from(profile.width);
    let sy = f64::from(height) / f64::from(profile.height);
    CameraParams {
        width,
        height,
        fx: profile.fx * sx,
        fy: profile.fy * sy,
        cx: profile.cx * sx,
        cy: profile.cy * sy,
        d: profile.d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> CameraParams {
        CameraParams {
            width: 1920,
            height: 1080,
            fx: 900.0,
            fy: 880.0,
            cx: 960.0,
            cy: 540.0,
            d: [0.03, 0.06, -0.07, 0.02],
        }
    }

    #[test]
    fn a_lens_round_trips_through_its_camera() {
        let lens = Lens::of(&params());
        assert_eq!(
            lens,
            Lens {
                fx: 900.0,
                fy: 880.0,
                cx: 960.0,
                cy: 540.0,
                k: [0.03, 0.06, -0.07, 0.02]
            }
        );
        let moved = Lens { fx: 950.0, ..lens }.applied_to(&params());
        assert_eq!((moved.fx, moved.width, moved.height), (950.0, 1920, 1080));
        assert_eq!(Lens::of(&moved).k, lens.k);
    }

    #[test]
    fn which_cameras() {
        assert!(Cameras::Both.left() && Cameras::Both.right());
        assert!(Cameras::Left.left() && !Cameras::Left.right());
        assert!(!Cameras::Right.left() && Cameras::Right.right());
    }

    #[test]
    fn fine_tune_ranges_are_the_slint_apps() {
        let r = FineTuneRanges::around(&Lens::of(&params()), 1920, 1080);
        // ±15% of max(fx, fy) = 135.
        assert_eq!(r.fx, (765.0, 1035.0));
        assert_eq!(r.fy, (745.0, 1015.0));
        // ±10% of the width and height.
        assert_eq!(r.cx, (768.0, 1152.0));
        assert_eq!(r.cy, (432.0, 648.0));
        // ±0.3 around each term (the Slint app's ±0.3 was absolute, which
        // can't show a calibrated k1 of 0.333).
        let k = [0.03, 0.06, -0.07, 0.02];
        for (range, term) in r.k.iter().zip(k) {
            assert!(
                (range.0 - (term - 0.3)).abs() < 1e-12 && (range.1 - (term + 0.3)).abs() < 1e-12
            );
        }
        let tiny = FineTuneRanges::around(
            &Lens {
                fx: 10.0,
                fy: 10.0,
                ..Lens::default()
            },
            20,
            20,
        );
        assert_eq!(tiny.fx, (5.0, 15.0), "at least 5 px");
        assert_eq!(tiny.cx, (-5.0, 5.0), "at least 5 px");
    }

    #[test]
    fn lens_info_reads_like_the_panel() {
        let info = LensInfo::from_profile(&LensProfileInfo {
            camera: "GoPro HERO9 Black".into(),
            lens: "Wide".into(),
            source: ProfileSource::AutoDetected,
            path: None,
        });
        assert_eq!(info.source, LensSource::Detected);
        assert_eq!(info.name(), "GoPro HERO9 Black · Wide");
        let bare = LensInfo {
            lens: String::new(),
            ..info.clone()
        };
        assert_eq!(bare.name(), "GoPro HERO9 Black", "no lens mode, no dot");
        let file = LensInfo::from_profile(&LensProfileInfo {
            camera: "Custom".into(),
            lens: String::new(),
            source: ProfileSource::File("/m/lens.json".into()),
            path: Some("/m/lens.json".into()),
        });
        assert_eq!(file.source, LensSource::File);
    }

    #[test]
    fn a_gopro_video_names_its_camera() {
        let Some((left, _, _)) = crate::preview::fixtures::real_set() else {
            return;
        };
        let video = reco_io::ffmpeg::decoder::VideoDecoder::open(&left).unwrap();
        let info = detect(&left, video.width(), video.height());
        let info = info.expect("a GoPro tells its camera");
        assert!(info.camera.contains("HERO"), "{info:?}");
    }

    #[test]
    fn the_detection_job_reports_both_cameras() {
        let Some((left, right, _)) = crate::preview::fixtures::fast_set() else {
            return;
        };
        let job = LensDetection::start(left, right, (1280, 960), Arc::new(|| {}));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut result = None;
        while result.is_none() && std::time::Instant::now() < deadline {
            result = job.try_result();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(result.is_some(), "it answers, known or not");
    }

    #[test]
    fn profiles_read_as_camera_lens_and_size() {
        let profile = LensProfileSummary {
            camera: "GoPro HERO9 Black".into(),
            lens: "Wide".into(),
            width: 5312,
            height: 2988,
        };
        assert_eq!(
            profile_line(&profile),
            "GoPro HERO9 Black · Wide · 5312 × 2988"
        );
    }

    fn search_until(search: &mut ProfileSearch) -> Option<Vec<LensProfileSummary>> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            if let Some(found) = search.try_results() {
                return Some(found);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }

    #[test]
    fn the_newest_search_wins() {
        let mut search = ProfileSearch::default();
        search.search("gopro", (5312, 2988), Arc::new(|| {}));
        search.search("hero9", (5312, 2988), Arc::new(|| {}));
        let found = search_until(&mut search).expect("results");
        assert!(!found.is_empty());
        assert!(
            found
                .iter()
                .all(|p| profile_line(p).to_lowercase().contains("hero9")),
            "only the newest query's results"
        );
        assert!(search.try_results().is_none(), "an older answer is dropped");
    }

    #[test]
    fn a_profile_gives_its_lens_at_the_calibration_size() {
        let mut search = ProfileSearch::default();
        search.search("hero9", (0, 0), Arc::new(|| {}));
        let profile = search_until(&mut search).unwrap().remove(0);
        let native = profile_lens(&profile, (profile.width, profile.height)).expect("its lens");
        let double = profile_lens(&profile, (profile.width * 2, profile.height * 2)).unwrap();
        assert!((double.fx - native.fx * 2.0).abs() < 1e-6);
        assert_eq!(double.k, native.k);
    }

    #[test]
    fn a_profile_file_gives_its_lens() {
        let path = std::env::temp_dir().join(format!("reco-app-lens-{}.json", std::process::id()));
        std::fs::write(
            &path,
            r#"{"width": 1920, "height": 1080, "fx": 900.0, "fy": 880.0, "cx": 960.0, "cy": 540.0, "d": [0.03, 0.06, -0.07, 0.02]}"#,
        )
        .unwrap();
        let lens = file_lens(&path, (3840, 2160)).expect("a Reco lens file");
        assert_eq!((lens.fx, lens.cy), (1800.0, 1080.0));
        std::fs::write(&path, "{").unwrap();
        assert!(file_lens(&path, (3840, 2160)).is_err(), "not a lens file");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_profile_is_scaled_to_the_videos() {
        let scaled = scaled_to(&params(), 3840, 2160);
        assert_eq!((scaled.width, scaled.height), (3840, 2160));
        assert_eq!((scaled.fx, scaled.fy), (1800.0, 1760.0));
        assert_eq!((scaled.cx, scaled.cy), (1920.0, 1080.0));
        assert_eq!(scaled.d, params().d, "distortion has no size");
        let no_size = CameraParams {
            width: 0,
            height: 0,
            ..params()
        };
        assert_eq!(
            scaled_to(&no_size, 3840, 2160).fx,
            900.0,
            "no size: as it is"
        );
    }
}
