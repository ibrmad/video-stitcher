//! The work in progress: each camera's video files in play order, and the
//! calibration. Pure: the caller does the I/O (dialogs, probing, checking
//! that a calibration file exists).
//!
//! Files added in a batch are put in recording order. A GoPro file is
//! `GXccnnnn` (chapter `cc` of recording `nnnn`; older cameras wrote
//! `GOPRnnnn` then `GPccnnnn`), so a plain name sort would interleave
//! recordings.

use std::path::{Path, PathBuf};

use reco_io::stitch_job::InputPath;

/// One of the two cameras.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Camera {
    /// The left camera.
    Left,
    /// The right camera.
    Right,
}

impl Camera {
    /// The other camera.
    pub fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// How far the work has come: what the next step is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// No camera has videos.
    NoVideos,
    /// Only this camera has videos.
    OneCamera(Camera),
    /// Both cameras have videos; no calibration yet.
    Cameras,
    /// Both cameras and a calibration: the preview can open.
    Ready,
}

/// Each camera's files and the calibration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    left: Vec<PathBuf>,
    right: Vec<PathBuf>,
    /// The calibration file, when one is set.
    pub calibration: Option<PathBuf>,
}

/// Where dropped files go.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DropPlan {
    /// Videos and the camera they go to.
    pub videos: Option<(Camera, Vec<PathBuf>)>,
    /// A calibration file among the dropped files.
    pub calibration: Option<PathBuf>,
    /// Videos with nowhere to go (both cameras already have videos and the
    /// drop was not on a camera's row).
    pub refused: usize,
}

/// The extensions the file dialogs offer and drops accept.
pub const VIDEO_EXTENSIONS: [&str; 4] = ["mp4", "mov", "avi", "mkv"];

/// Whether `path` names a video by its extension (any case).
pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(e)))
}

/// `paths` in recording order: GoPro files by recording then chapter,
/// before any other files, which sort by name.
pub fn video_order(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort_by_key(|p| order_key(p));
    paths
}

/// GoPro files first, by (recording, chapter); then the rest by name.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum OrderKey {
    GoPro(u32, u32),
    Other(String),
}

fn order_key(path: &Path) -> OrderKey {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    let digits = |s: &str| {
        (s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    if let Some(recording) = stem.strip_prefix("GOPR").and_then(digits) {
        return OrderKey::GoPro(recording, 0);
    }
    if stem.len() == 8
        && ["GX", "GH", "GP"].iter().any(|p| stem.starts_with(p))
        && stem[2..].bytes().all(|b| b.is_ascii_digit())
    {
        let chapter = stem[2..4].parse().unwrap_or(0);
        let recording = stem[4..8].parse().unwrap_or(0);
        return OrderKey::GoPro(recording, chapter);
    }
    OrderKey::Other(
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

impl Project {
    /// The files of `camera`, in play order.
    pub fn files(&self, camera: Camera) -> &[PathBuf] {
        match camera {
            Camera::Left => &self.left,
            Camera::Right => &self.right,
        }
    }

    fn files_mut(&mut self, camera: Camera) -> &mut Vec<PathBuf> {
        match camera {
            Camera::Left => &mut self.left,
            Camera::Right => &mut self.right,
        }
    }

    /// Replace `camera`'s files as given (the command line and recent
    /// sessions keep their order and repeats).
    pub fn set_files(&mut self, camera: Camera, files: Vec<PathBuf>) {
        *self.files_mut(camera) = files;
    }

    /// Append the videos among `paths`, in recording order, skipping files
    /// `camera` already has. Returns how many were added.
    pub fn add(&mut self, camera: Camera, paths: Vec<PathBuf>) -> usize {
        let files = self.files_mut(camera);
        let mut added = 0;
        for path in video_order(paths.into_iter().filter(|p| is_video(p)).collect()) {
            if !files.contains(&path) {
                files.push(path);
                added += 1;
            }
        }
        added
    }

    /// Remove `camera`'s file at `index`; whether there was one.
    pub fn remove(&mut self, camera: Camera, index: usize) -> bool {
        let files = self.files_mut(camera);
        if index >= files.len() {
            return false;
        }
        files.remove(index);
        true
    }

    /// Move `camera`'s file at `from` so it ends at `to` (clamped to the
    /// list); whether anything moved.
    pub fn move_file(&mut self, camera: Camera, from: usize, to: usize) -> bool {
        let files = self.files_mut(camera);
        if from >= files.len() {
            return false;
        }
        let to = to.min(files.len() - 1);
        if to == from {
            return false;
        }
        let file = files.remove(from);
        files.insert(to, file);
        true
    }

    /// Remove all of `camera`'s files.
    pub fn clear(&mut self, camera: Camera) {
        self.files_mut(camera).clear();
    }

    /// `camera`'s files as one input: one file, or a chain.
    pub fn input(&self, camera: Camera) -> Option<InputPath> {
        match self.files(camera) {
            [] => None,
            [one] => Some(InputPath::Single(one.clone())),
            many => Some(InputPath::Chained(many.to_vec())),
        }
    }

    /// The next step.
    pub fn stage(&self) -> Stage {
        match (self.left.is_empty(), self.right.is_empty()) {
            (true, true) => Stage::NoVideos,
            (false, true) => Stage::OneCamera(Camera::Left),
            (true, false) => Stage::OneCamera(Camera::Right),
            (false, false) if self.calibration.is_some() => Stage::Ready,
            (false, false) => Stage::Cameras,
        }
    }

    /// Where auto-calibrate saves and looks for this pair's calibration:
    /// `{first left stem}_calibration.json` beside the first left file.
    pub fn sibling_calibration(&self) -> Option<PathBuf> {
        let first = self.left.first()?;
        let stem = first.file_stem()?.to_string_lossy();
        Some(first.with_file_name(format!("{stem}_calibration.json")))
    }

    /// Where dropped `paths` go when dropped over `over` (a camera's row)
    /// or anywhere else (`None`).
    pub fn route_drop(&self, paths: &[PathBuf], over: Option<Camera>) -> DropPlan {
        let videos: Vec<PathBuf> = paths.iter().filter(|p| is_video(p)).cloned().collect();
        let calibration = paths
            .iter()
            .find(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
            })
            .cloned();
        let target = over.or_else(|| {
            [Camera::Left, Camera::Right]
                .into_iter()
                .find(|c| self.files(*c).is_empty())
        });
        match (target, videos.is_empty()) {
            (_, true) => DropPlan {
                calibration,
                ..DropPlan::default()
            },
            (Some(camera), false) => DropPlan {
                videos: Some((camera, videos)),
                calibration,
                refused: 0,
            },
            (None, false) => DropPlan {
                videos: None,
                calibration,
                refused: videos.len(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|n| PathBuf::from(format!("/m/{n}")))
            .collect()
    }

    fn names(files: &[PathBuf]) -> Vec<String> {
        files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn gopro_files_go_in_recording_then_chapter_order() {
        let sorted = video_order(paths(&[
            "GX020120.MP4",
            "notes.mov",
            "GX010121.MP4",
            "GX010120.MP4",
            "GP010119.MP4",
            "GOPR0119.MP4",
            "a.mp4",
        ]));
        assert_eq!(
            names(&sorted),
            [
                "GOPR0119.MP4",
                "GP010119.MP4",
                "GX010120.MP4",
                "GX020120.MP4",
                "GX010121.MP4",
                "a.mp4",
                "notes.mov",
            ]
        );
    }

    #[test]
    fn only_videos_are_added_once() {
        let mut p = Project::default();
        assert!(is_video(Path::new("/m/a.MOV")) && !is_video(Path::new("/m/a.txt")));
        assert_eq!(
            p.add(Camera::Left, paths(&["b.mp4", "notes.txt", "a.MKV"])),
            2
        );
        assert_eq!(p.add(Camera::Left, paths(&["a.MKV", "c.avi"])), 1);
        assert_eq!(names(p.files(Camera::Left)), ["a.MKV", "b.mp4", "c.avi"]);
        assert!(p.files(Camera::Right).is_empty());
    }

    #[test]
    fn moves_and_removes_stay_in_bounds() {
        let mut p = Project::default();
        p.set_files(Camera::Right, paths(&["a.mp4", "b.mp4", "c.mp4"]));
        assert!(p.move_file(Camera::Right, 0, 9));
        assert_eq!(names(p.files(Camera::Right)), ["b.mp4", "c.mp4", "a.mp4"]);
        assert!(p.move_file(Camera::Right, 2, 0));
        assert_eq!(names(p.files(Camera::Right)), ["a.mp4", "b.mp4", "c.mp4"]);
        assert!(!p.move_file(Camera::Right, 1, 1));
        assert!(!p.move_file(Camera::Right, 5, 0));
        assert!(p.remove(Camera::Right, 1));
        assert!(!p.remove(Camera::Right, 2));
        assert_eq!(names(p.files(Camera::Right)), ["a.mp4", "c.mp4"]);
    }

    #[test]
    fn the_stage_follows_the_files_and_the_calibration() {
        let mut p = Project::default();
        assert_eq!(p.stage(), Stage::NoVideos);
        p.set_files(Camera::Right, paths(&["r.mp4"]));
        assert_eq!(p.stage(), Stage::OneCamera(Camera::Right));
        p.set_files(Camera::Left, paths(&["l.mp4", "l2.mp4"]));
        assert_eq!(p.stage(), Stage::Cameras);
        p.calibration = Some("/m/c.json".into());
        assert_eq!(p.stage(), Stage::Ready);
        assert!(matches!(p.input(Camera::Left), Some(InputPath::Chained(v)) if v.len() == 2));
        assert!(matches!(p.input(Camera::Right), Some(InputPath::Single(_))));
        p.clear(Camera::Left);
        assert_eq!(p.stage(), Stage::OneCamera(Camera::Right));
        assert!(p.input(Camera::Left).is_none());
    }

    #[test]
    fn the_saved_calibration_sits_beside_the_first_left_file() {
        let mut p = Project::default();
        assert_eq!(p.sibling_calibration(), None);
        p.set_files(Camera::Left, paths(&["GX010120.MP4", "GX020120.MP4"]));
        assert_eq!(
            p.sibling_calibration(),
            Some(PathBuf::from("/m/GX010120_calibration.json"))
        );
    }

    #[test]
    fn drops_go_to_the_row_or_the_first_empty_camera() {
        let mut p = Project::default();
        let dropped = paths(&["b.mp4", "a.mp4", "cal.json", "x.txt"]);
        let plan = p.route_drop(&dropped, None);
        assert_eq!(
            plan.videos,
            Some((Camera::Left, paths(&["b.mp4", "a.mp4"])))
        );
        assert_eq!(plan.calibration, Some(PathBuf::from("/m/cal.json")));
        assert_eq!(
            p.route_drop(&dropped, Some(Camera::Right))
                .videos
                .unwrap()
                .0,
            Camera::Right
        );
        p.set_files(Camera::Left, paths(&["l.mp4"]));
        assert_eq!(
            p.route_drop(&dropped, None).videos.unwrap().0,
            Camera::Right
        );
        p.set_files(Camera::Right, paths(&["r.mp4"]));
        let full = p.route_drop(&dropped, None);
        assert_eq!((full.videos, full.refused), (None, 2));
        assert_eq!(
            p.route_drop(&dropped, Some(Camera::Left)).videos.unwrap().0,
            Camera::Left
        );
    }
}
