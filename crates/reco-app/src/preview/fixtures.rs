//! Local test footage (never committed). Tests that need it skip with a
//! message when it is missing.

use std::path::PathBuf;

/// Left video, right video and calibration: the `RECO_FIXTURE_*` variables,
/// else the alfheim set under `~/dev/pitchcam-data`.
pub fn fast_set() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let base = home.join("dev/pitchcam-data/alfheim");
    let pick =
        |var: &str, default: PathBuf| std::env::var_os(var).map(PathBuf::from).unwrap_or(default);
    let set = (
        pick("RECO_FIXTURE_LEFT", base.join("cam0.mp4")),
        pick("RECO_FIXTURE_RIGHT", base.join("cam1.mp4")),
        pick("RECO_FIXTURE_CAL", base.join("reco/match.json")),
    );
    if set.0.exists() && set.1.exists() && set.2.exists() {
        Some(set)
    } else {
        eprintln!("skipping: preview fixtures not found (set RECO_FIXTURE_LEFT/RIGHT/CAL)");
        None
    }
}

/// A YOLO model for AI tracking (`RECO_FIXTURE_MODEL`, else the one beside
/// the alfheim set).
pub fn model() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let path = std::env::var_os("RECO_FIXTURE_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("dev/pitchcam-data/alfheim/reco/yolo26n.onnx"));
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipping: no AI model (set RECO_FIXTURE_MODEL)");
        None
    }
}

/// A video of another size than the fast set (the 5.3K match pair's left
/// file, or `RECO_FIXTURE_OTHER_SIZE`).
pub fn other_size() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let path = std::env::var_os("RECO_FIXTURE_OTHER_SIZE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("Downloads/match_recording/left/GX010120.MP4"));
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipping: no video of another size (set RECO_FIXTURE_OTHER_SIZE)");
        None
    }
}

/// The 5.3K GoPro match pair and its calibration (`RECO_FIXTURE_REAL_DIR`,
/// else `~/Downloads/match_recording`): real footage that calibrates.
pub fn real_set() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let base = std::env::var_os("RECO_FIXTURE_REAL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("Downloads/match_recording"));
    let set = (
        base.join("left/GX010120.MP4"),
        base.join("right/GX010092.MP4"),
        base.join("left/GX010120_calibration.json"),
    );
    if set.0.exists() && set.1.exists() && set.2.exists() {
        Some(set)
    } else {
        eprintln!("skipping: the real match pair was not found (set RECO_FIXTURE_REAL_DIR)");
        None
    }
}

/// A small, quick recording: 640x360, H.264, Fast.
pub fn small_recording() -> crate::recording::RecordingFormat {
    crate::recording::RecordingFormat {
        size: (640, 360),
        codec: "h264".into(),
        quality: crate::recording::RecordingQuality::Fast,
    }
}
