//! AI tracking for an export (Module 6b): the sheet's choices, the engine's
//! tracking config built from them (the style preset as the base and the
//! visible knobs over it, as the Slint app built it), the lookahead's VRAM
//! zones, and whether this machine can run the detector.

use std::path::PathBuf;

/// Tracking modes, as the sheet lists them.
pub const MODES: [&str; 3] = ["field", "ball", "sweep"];
/// "Detect every N frames", as the sheet lists them.
pub const INTERVALS: [u32; 6] = [1, 3, 5, 10, 15, 30];
/// Style presets (the engine's names).
pub const PRESETS: [&str; 3] = ["broadcast", "action", "frame_all"];
/// Framings.
pub const FRAMINGS: [&str; 2] = ["action", "frame_all"];
/// How the action cluster is found.
pub const CLUSTER_MODES: [&str; 2] = ["density", "trimmed_mean"];
/// The lookahead slider's end, seconds.
pub const MAX_LOOKAHEAD: f64 = 2.5;

/// The panner knobs the sheet shows; a style preset sets them all.
#[derive(Clone, Debug, PartialEq)]
pub struct PannerKnobs {
    /// "action" or "frame_all".
    pub framing: String,
    /// Pan only, holding the pitch level.
    pub lock_pitch: bool,
    /// "density" or "trimmed_mean".
    pub cluster_mode: String,
    /// The density cluster's radius, radians.
    pub cluster_bandwidth: f32,
    /// The soft dead zone, radians.
    pub dead_zone: f32,
    /// How much the ball pulls against the players, 0 to 1.
    pub ball_weight: f32,
    /// Field of view, degrees: tightest, starting, widest.
    pub fov_tight: f32,
    pub fov_default: f32,
    pub fov_wide: f32,
}

impl PannerKnobs {
    /// The knobs the style preset `name` sets (Broadcast for an unknown
    /// name), from the engine's presets.
    #[cfg(feature = "ai")]
    pub fn of_preset(name: &str) -> Self {
        use reco_autocam::panners::{ClusterMode, FieldPannerConfig, FramingMode};
        let c = FieldPannerConfig::from_preset_name(name).unwrap_or_default();
        Self {
            framing: match c.framing {
                FramingMode::FrameAll => "frame_all",
                _ => "action",
            }
            .into(),
            lock_pitch: c.lock_pitch,
            cluster_mode: match c.cluster_mode {
                ClusterMode::TrimmedMean => "trimmed_mean",
                _ => "density",
            }
            .into(),
            cluster_bandwidth: c.cluster_bandwidth_rad,
            dead_zone: c.dead_zone_rad,
            ball_weight: c.ball_weight,
            fov_tight: c.fov_tight,
            fov_default: c.fov_default,
            fov_wide: c.fov_wide,
        }
    }

    /// Without the engine (a build without AI): the Slint app's starting
    /// values, which are Broadcast's.
    #[cfg(not(feature = "ai"))]
    pub fn of_preset(_name: &str) -> Self {
        Self {
            framing: "action".into(),
            lock_pitch: false,
            cluster_mode: "density".into(),
            cluster_bandwidth: 0.20,
            dead_zone: 0.03,
            ball_weight: 0.60,
            fov_tight: 22.0,
            fov_default: 40.0,
            fov_wide: 58.0,
        }
    }
}

/// An export's tracking, as chosen in the sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct Tracking {
    /// The detector's model; Sweep needs none.
    pub model: Option<PathBuf>,
    /// "field", "ball" or "sweep".
    pub mode: String,
    /// Detect every N frames.
    pub interval: u32,
    /// The style preset the knobs start from.
    pub preset: String,
    /// The visible knobs (over the preset).
    pub knobs: PannerKnobs,
    /// Seconds of frames buffered ahead so the pan can smooth (0: off).
    pub lookahead_secs: f64,
}

impl Tracking {
    /// Why the tracking can't run as chosen, if it can't. The model is an
    /// .onnx file that exists, as Preferences takes it.
    pub fn problem(&self) -> Option<String> {
        if self.mode == "sweep" || self.usable_model().is_some() {
            return None;
        }
        match &self.model {
            None => Some("Choose the AI model (an .onnx file) to track.".into()),
            Some(model) if !is_onnx(model) => Some("The AI model must be an .onnx file.".into()),
            Some(model) => Some(format!(
                "The AI model isn't there any more: {}",
                model.display()
            )),
        }
    }

    /// The model, when it is one Preferences would take: an .onnx file
    /// that exists.
    pub fn usable_model(&self) -> Option<&PathBuf> {
        self.model
            .as_ref()
            .filter(|model| is_onnx(model) && model.is_file())
    }

    /// The engine's config: the preset with the knobs over it, the mode,
    /// the interval, the field outline, and a higher confidence floor for a
    /// ball-only model (as the Slint app and the CLI set it).
    #[cfg(feature = "ai")]
    pub fn config(
        &self,
        is_10bit: bool,
        field_roi: Option<reco_core::calibration::FieldRoi>,
    ) -> reco_autocam::AutocamConfig {
        use reco_autocam::panners::{ClusterMode, FieldPannerConfig, FramingMode};
        use reco_autocam::{AutocamConfig, TrackingMode};
        let mode = match self.mode.as_str() {
            "ball" => TrackingMode::Ball,
            "sweep" => TrackingMode::Sweep,
            _ => TrackingMode::Field,
        };
        let k = &self.knobs;
        let panner = FieldPannerConfig {
            framing: if k.framing == "frame_all" {
                FramingMode::FrameAll
            } else {
                FramingMode::Action
            },
            cluster_mode: if k.cluster_mode == "trimmed_mean" {
                ClusterMode::TrimmedMean
            } else {
                ClusterMode::Density
            },
            cluster_bandwidth_rad: k.cluster_bandwidth,
            dead_zone_rad: k.dead_zone,
            ball_weight: k.ball_weight,
            lock_pitch: k.lock_pitch,
            fov_tight: k.fov_tight,
            fov_default: k.fov_default,
            fov_wide: k.fov_wide,
            ..FieldPannerConfig::from_preset_name(&self.preset).unwrap_or_default()
        };
        let mut config = AutocamConfig::new(self.model.clone().unwrap_or_default())
            .with_tracking_mode(mode)
            .with_detection_interval(u64::from(self.interval.max(1)))
            .with_10bit(is_10bit);
        config.field_panner_config = Some(panner);
        if mode == TrackingMode::Ball {
            config.confidence_threshold = Some(0.25);
        }
        match field_roi {
            Some(roi) => config.with_field_roi(roi),
            None => config,
        }
    }
}

/// Whether `path` names an .onnx file.
fn is_onnx(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("onnx"))
}

/// The lookahead slider's zones: comfortable up to `safe`, still fitting
/// up to `max`, past it the export can't hold the frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookaheadZones {
    /// The comfortable ceiling, seconds.
    pub safe: f64,
    /// The hard ceiling, seconds.
    pub max: f64,
}

/// The zones for a `size` source at `fps`, from the GPU's free and total
/// memory (`None`, as without a reading: no zones). The lookahead holds
/// source-size frames, so the source's size counts, not the export's.
pub fn lookahead_zones(
    vram: Option<(u64, u64)>,
    size: (u32, u32),
    fps: f64,
) -> Option<LookaheadZones> {
    let (free, total) = vram?;
    if total == 0 || size.0 == 0 || size.1 == 0 {
        return None;
    }
    let budget = reco_core::session::lookahead_budget_bytes(free, total);
    let fit = reco_core::session::lookahead_fit(size.0, size.1, 1, budget, fps);
    Some(LookaheadZones {
        safe: fit.safe_secs,
        max: fit.max_secs,
    })
}

/// The lookahead the sheet starts at: the saved one, or the comfortable
/// ceiling when the saved one doesn't fit (as the Slint app lowered it).
pub fn fitted_lookahead(saved: f64, zones: Option<LookaheadZones>) -> f64 {
    match zones {
        Some(zones) if saved > zones.max => zones.safe,
        _ => saved,
    }
}

/// Whether this machine can run the detector.
#[derive(Clone, Debug, PartialEq)]
pub enum Availability {
    /// It can, on these engines ("CPU", "CoreML, CPU").
    Ready(String),
    /// It can't; why.
    Unavailable(String),
}

/// The status line under the sheet's Enable.
pub fn availability_line(availability: &Availability) -> String {
    match availability {
        Availability::Ready(engines) => format!("Ready: runs on {engines}"),
        Availability::Unavailable(why) => format!("Not available: {why}"),
    }
}

/// The export card's line for whether the tracking started.
pub fn tracking_line(status: &Result<(), String>) -> String {
    match status {
        Ok(()) => "AI tracking: active".into(),
        Err(why) => format!("AI tracking didn't start: {why}"),
    }
}

/// Ask ONNX Runtime which engines load here.
#[cfg(feature = "ai")]
fn probe() -> Availability {
    let result = reco_detect::probe_execution_providers();
    if result.is_available() {
        Availability::Ready(result.providers.join(", "))
    } else {
        Availability::Unavailable(
            result
                .errors
                .first()
                .cloned()
                .unwrap_or_else(|| "no inference engine loads on this machine".into()),
        )
    }
}

/// A build without AI.
#[cfg(not(feature = "ai"))]
fn probe() -> Availability {
    Availability::Unavailable("AI tracking isn't in this build".into())
}

/// Asks once, off the UI thread, whether the detector can run (ONNX
/// Runtime's engines can take a moment to load).
pub struct AvailabilityProbe(std::sync::mpsc::Receiver<Availability>);

impl AvailabilityProbe {
    /// Start asking; `waker` runs once the answer is in.
    pub fn start(waker: std::sync::Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("reco-ai-probe".into())
            .spawn(move || {
                let answer = std::panic::catch_unwind(probe).unwrap_or_else(|_| {
                    Availability::Unavailable("the inference engine crashed while loading".into())
                });
                if tx.send(answer).is_ok() {
                    waker();
                }
            });
        if let Err(e) = spawned {
            log::warn!("AI probe: couldn't start ({e})");
        }
        Self(rx)
    }

    /// The answer, once in (never blocks).
    pub fn try_result(&self) -> Option<Availability> {
        self.0.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_says_where_tracking_runs_or_why_not() {
        assert_eq!(
            availability_line(&Availability::Ready("CPU".into())),
            "Ready: runs on CPU"
        );
        assert_eq!(
            availability_line(&Availability::Unavailable(
                "AI tracking isn't in this build".into()
            )),
            "Not available: AI tracking isn't in this build"
        );
    }

    #[test]
    fn the_probe_answers() {
        let probe = AvailabilityProbe::start(std::sync::Arc::new(|| {}));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut answer = None;
        while answer.is_none() && std::time::Instant::now() < deadline {
            answer = probe.try_result();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        match answer {
            Some(Availability::Ready(engines)) => assert!(!engines.is_empty()),
            Some(Availability::Unavailable(why)) => assert!(!why.is_empty()),
            None => panic!("no answer in 30 s"),
        }
    }

    fn temp_file(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("reco-ai-{}-{name}", std::process::id()));
        std::fs::write(&path, b"onnx").expect("a temp file");
        path
    }

    fn tracking(model: Option<PathBuf>, mode: &str) -> Tracking {
        Tracking {
            model,
            mode: mode.into(),
            interval: 15,
            preset: "broadcast".into(),
            knobs: PannerKnobs::of_preset("broadcast"),
            lookahead_secs: 2.5,
        }
    }

    #[test]
    fn broadcast_is_the_slint_apps_starting_point() {
        assert_eq!(
            PannerKnobs::of_preset("broadcast"),
            PannerKnobs {
                framing: "action".into(),
                lock_pitch: false,
                cluster_mode: "density".into(),
                cluster_bandwidth: 0.20,
                dead_zone: 0.03,
                ball_weight: 0.60,
                fov_tight: 22.0,
                fov_default: 40.0,
                fov_wide: 58.0,
            }
        );
        assert_eq!(
            PannerKnobs::of_preset("nonsense"),
            PannerKnobs::of_preset("broadcast")
        );
    }

    #[cfg(feature = "ai")]
    #[test]
    fn presets_set_the_knobs_from_the_engine() {
        use reco_autocam::panners::FieldPannerConfig;
        let frame_all = PannerKnobs::of_preset("frame_all");
        assert_eq!(frame_all.framing, "frame_all");
        let action = PannerKnobs::of_preset("action");
        assert!((action.dead_zone - FieldPannerConfig::action().dead_zone_rad).abs() < 1e-6);
    }

    #[test]
    fn a_model_is_needed_except_for_sweep() {
        assert_eq!(
            tracking(None, "field").problem().as_deref(),
            Some("Choose the AI model (an .onnx file) to track.")
        );
        assert_eq!(
            tracking(None, "sweep").problem(),
            None,
            "a sweep needs no model"
        );
        let gone = PathBuf::from("/no/such/yolo.onnx");
        assert_eq!(
            tracking(Some(gone), "ball").problem().as_deref(),
            Some("The AI model isn't there any more: /no/such/yolo.onnx")
        );
        let model = temp_file("yolo.onnx");
        assert_eq!(tracking(Some(model.clone()), "field").problem(), None);
        let _ = std::fs::remove_file(model);
        // As Preferences has it: an .onnx file, whatever else exists there.
        let notes = temp_file("notes.txt");
        assert_eq!(
            tracking(Some(notes.clone()), "field").problem().as_deref(),
            Some("The AI model must be an .onnx file.")
        );
        let _ = std::fs::remove_file(notes);
    }

    #[test]
    fn only_an_onnx_file_that_exists_is_a_usable_model() {
        let model = temp_file("usable.onnx");
        let notes = temp_file("usable.txt");
        let usable = |path: Option<PathBuf>| tracking(path, "sweep").usable_model().cloned();
        assert_eq!(usable(Some(model.clone())), Some(model.clone()));
        assert_eq!(usable(Some(notes.clone())), None, "not an .onnx file");
        assert_eq!(usable(Some(PathBuf::from("/no/such/m.onnx"))), None);
        assert_eq!(usable(None), None);
        let _ = std::fs::remove_file(model);
        let _ = std::fs::remove_file(notes);
    }

    #[test]
    fn the_card_says_whether_tracking_started() {
        assert_eq!(tracking_line(&Ok(())), "AI tracking: active");
        assert_eq!(
            tracking_line(&Err(
                "no detector this export can use on this machine".into()
            )),
            "AI tracking didn't start: no detector this export can use on this machine"
        );
    }

    #[cfg(feature = "ai")]
    #[test]
    fn the_config_is_the_preset_with_the_knobs_over_it() {
        use reco_autocam::TrackingMode;
        use reco_autocam::panners::{ClusterMode, FieldPannerConfig, FramingMode};
        let mut t = tracking(Some(PathBuf::from("/m/yolo.onnx")), "field");
        t.preset = "action".into();
        t.knobs = PannerKnobs::of_preset("action");
        t.knobs.framing = "frame_all".into();
        t.knobs.cluster_mode = "trimmed_mean".into();
        t.knobs.fov_tight = 30.0;
        t.interval = 5;
        let roi = reco_core::calibration::FieldRoi {
            left: vec![[0.1, 0.2], [0.9, 0.2], [0.5, 0.9]],
            ..Default::default()
        };
        let config = t.config(true, Some(roi));
        assert_eq!(config.model_path, PathBuf::from("/m/yolo.onnx"));
        assert_eq!(config.tracking_mode, TrackingMode::Field);
        assert_eq!(config.detection_interval, 5);
        assert!(config.is_10bit);
        assert_eq!(config.field_roi.map(|r| r.left.len()), Some(3));
        assert_eq!(config.confidence_threshold, None);
        let panner = config.field_panner_config.expect("a panner config");
        assert_eq!(panner.framing, FramingMode::FrameAll);
        assert_eq!(panner.cluster_mode, ClusterMode::TrimmedMean);
        assert!((panner.fov_tight - 30.0).abs() < 1e-6);
        let action = FieldPannerConfig::action();
        assert_eq!(
            panner.edge_push, action.edge_push,
            "the rest is the preset's"
        );
        let ball = Tracking {
            mode: "ball".into(),
            ..t.clone()
        };
        let config = ball.config(false, None);
        assert_eq!(config.tracking_mode, TrackingMode::Ball);
        assert_eq!(
            config.confidence_threshold,
            Some(0.25),
            "a ball-only model's floor"
        );
        let sweep = Tracking {
            mode: "sweep".into(),
            model: None,
            ..t
        };
        assert_eq!(sweep.config(false, None).tracking_mode, TrackingMode::Sweep);
    }

    #[test]
    fn zones_come_from_the_gpu_and_the_source() {
        assert_eq!(lookahead_zones(None, (5312, 2988), 29.97), None);
        let gib = 1u64 << 30;
        let zones =
            lookahead_zones(Some((12 * gib, 16 * gib)), (5312, 2988), 29.97).expect("zones");
        let fit = reco_core::session::lookahead_fit(
            5312,
            2988,
            1,
            reco_core::session::lookahead_budget_bytes(12 * gib, 16 * gib),
            29.97,
        );
        assert_eq!((zones.safe, zones.max), (fit.safe_secs, fit.max_secs));
        assert!(zones.safe > 0.0 && zones.safe <= zones.max);
        assert_eq!(
            lookahead_zones(Some((0, 0)), (5312, 2988), 29.97),
            None,
            "no reading"
        );
    }

    #[test]
    fn a_lookahead_that_does_not_fit_starts_at_the_safe_value() {
        let zones = LookaheadZones {
            safe: 1.0,
            max: 1.8,
        };
        assert_eq!(fitted_lookahead(2.5, Some(zones)), 1.0);
        assert_eq!(fitted_lookahead(1.5, Some(zones)), 1.5, "it fits: kept");
        assert_eq!(fitted_lookahead(2.5, None), 2.5);
    }
}
