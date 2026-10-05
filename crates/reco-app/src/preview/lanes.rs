//! Where each camera's files sit on the stitched timeline, for the time
//! panel's lanes. Every file's duration is probed off the render thread and
//! laid end to end, shifted by the frames the sync offset trims from the
//! camera that started first. The pair plays only while both cameras have
//! video, which also fixes the engine's length estimate (it ignores the
//! sync offset).

use std::path::PathBuf;

use reco_io::stitch_job::InputPath;

use crate::durations::file_duration;

/// Each camera's files on the stitched timeline, and how long the pair plays.
#[derive(Clone, Debug, PartialEq)]
pub struct Lanes {
    /// The left camera's files as (start, end) seconds.
    pub left: Vec<(f64, f64)>,
    /// The right camera's files.
    pub right: Vec<(f64, f64)>,
    /// Seconds both cameras have video.
    pub length: f64,
}

/// Files of `durations` seconds laid end to end, less `skip` seconds the sync
/// offset trims from this camera's start. A file wholly before zero is
/// dropped and the first one kept is clipped at zero.
pub fn camera_spans(durations: &[f64], skip: f64) -> Vec<(f64, f64)> {
    let mut start = -skip;
    let mut spans = Vec::new();
    for duration in durations {
        let end = start + duration.max(0.0);
        if end > 0.0 {
            spans.push((start.max(0.0), end));
        }
        start = end;
    }
    spans
}

/// The seconds each camera skips for `sync_offset` frames at `fps`: a
/// positive offset skips the right camera's start (it started first), a
/// negative one the left's.
pub fn skips(sync_offset: i64, fps: f64) -> (f64, f64) {
    if fps <= 0.0 {
        return (0.0, 0.0);
    }
    (
        (-sync_offset).max(0) as f64 / fps,
        sync_offset.max(0) as f64 / fps,
    )
}

/// Both cameras' lanes from their files' durations.
pub fn lanes(left: &[f64], right: &[f64], sync_offset: i64, fps: f64) -> Lanes {
    let (left_skip, right_skip) = skips(sync_offset, fps);
    let left = camera_spans(left, left_skip);
    let right = camera_spans(right, right_skip);
    let end = |spans: &[(f64, f64)]| spans.last().map_or(0.0, |s| s.1);
    let length = end(&left).min(end(&right));
    Lanes {
        left,
        right,
        length,
    }
}

/// Each file's duration in seconds; a file that cannot be read counts as
/// zero. Blocking (it opens every file): run it off the render thread.
pub fn probe(input: &InputPath) -> Vec<f64> {
    let paths: Vec<PathBuf> = match input {
        InputPath::Single(path) => vec![path.clone()],
        InputPath::Chained(paths) => paths.clone(),
    };
    paths.iter().map(|path| file_duration(path)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_lie_end_to_end() {
        assert_eq!(
            camera_spans(&[60.0, 40.0], 0.0),
            [(0.0, 60.0), (60.0, 100.0)]
        );
    }

    #[test]
    fn the_sync_offset_trims_the_camera_that_started_first() {
        assert_eq!(skips(30, 30.0), (0.0, 1.0));
        assert_eq!(skips(-15, 30.0), (0.5, 0.0));
        assert_eq!(camera_spans(&[60.0], 1.0), [(0.0, 59.0)]);
    }

    #[test]
    fn a_file_wholly_before_the_start_is_dropped() {
        assert_eq!(camera_spans(&[1.0, 10.0], 2.0), [(0.0, 9.0)]);
    }

    #[test]
    fn length_is_where_both_cameras_have_video() {
        let l = lanes(&[60.0, 60.0], &[100.0], 30, 30.0);
        assert_eq!(l.right, [(0.0, 99.0)]);
        assert_eq!(l.length, 99.0);
        assert_eq!(lanes(&[], &[10.0], 0, 30.0).length, 0.0);
    }

    #[test]
    fn probing_the_fixture_gives_its_duration() {
        let Some((left, _, _)) = crate::preview::fixtures::fast_set() else {
            return;
        };
        let durations = probe(&InputPath::Single(left));
        assert_eq!(durations.len(), 1);
        assert!((durations[0] - 60.0).abs() < 0.5, "{durations:?}");
        assert_eq!(probe(&InputPath::Single("/nonexistent.mp4".into())), [0.0]);
    }
}
