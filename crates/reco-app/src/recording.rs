//! The choices behind Record: quality, frame size, folder and file name.
//! The name and folder rules are the Slint app's, so recordings from both
//! apps sort together.

use std::path::{Path, PathBuf};

use crate::preview::view::PreviewAspect;

/// The codec recordings use (the Slint app's default; a choice arrives with
/// Preferences in Module 7).
pub const RECORDING_CODEC: &str = "h264";

/// Recording quality, named as the encoder presets name it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecordingQuality {
    /// Smaller files, quicker to encode.
    Fast,
    /// The default.
    #[default]
    Balanced,
    /// Larger files, the best picture.
    High,
}

impl RecordingQuality {
    /// The encoder's and the settings' name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::High => "high",
        }
    }

    /// The quality called `name`; Balanced for anything else.
    pub fn from_name(name: &str) -> Self {
        match name {
            "fast" => Self::Fast,
            "high" => Self::High,
            _ => Self::Balanced,
        }
    }

    /// The dropdown index (Fast, Balanced, High).
    pub fn index(self) -> usize {
        match self {
            Self::Fast => 0,
            Self::Balanced => 1,
            Self::High => 2,
        }
    }

    /// The quality at a dropdown index; Balanced out of range.
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Fast,
            2 => Self::High,
            _ => Self::Balanced,
        }
    }
}

/// The frame size a recording is made at: 1080 rows, as wide as the preview
/// aspect (16:9 for Auto). Widths are multiples of four for the NV12 path.
pub fn recording_size(aspect: PreviewAspect) -> (u32, u32) {
    match aspect {
        PreviewAspect::Auto | PreviewAspect::Wide16x9 => (1920, 1080),
        PreviewAspect::Classic4x3 => (1440, 1080),
        PreviewAspect::Cinema21x9 => (2520, 1080),
    }
}

/// Where recordings go: the saved folder if it is a directory, else beside
/// the left camera's first file, else the current folder (canonical).
pub fn recording_folder(saved: Option<&Path>, first_left: &Path) -> PathBuf {
    let folder = saved
        .filter(|p| p.is_dir())
        .map(Path::to_path_buf)
        .or_else(|| {
            first_left
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::canonicalize(&folder).unwrap_or(folder)
}

/// `reco_recording_<unix seconds>.mp4`, the Slint app's name.
pub fn recording_file_name(unix_secs: u64) -> String {
    format!("reco_recording_{unix_secs}.mp4")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualities_round_trip() {
        for q in [
            RecordingQuality::Fast,
            RecordingQuality::Balanced,
            RecordingQuality::High,
        ] {
            assert_eq!(RecordingQuality::from_name(q.name()), q);
            assert_eq!(RecordingQuality::from_index(q.index()), q);
        }
        assert_eq!(RecordingQuality::High.name(), "high");
        assert_eq!(
            RecordingQuality::from_name("ultra"),
            RecordingQuality::Balanced
        );
        assert_eq!(RecordingQuality::from_index(7), RecordingQuality::Balanced);
    }

    #[test]
    fn recordings_are_1080_rows_as_wide_as_the_aspect() {
        assert_eq!(recording_size(PreviewAspect::Auto), (1920, 1080));
        assert_eq!(recording_size(PreviewAspect::Wide16x9), (1920, 1080));
        assert_eq!(recording_size(PreviewAspect::Classic4x3), (1440, 1080));
        assert_eq!(recording_size(PreviewAspect::Cinema21x9), (2520, 1080));
        for aspect in [PreviewAspect::Classic4x3, PreviewAspect::Cinema21x9] {
            let (w, h) = recording_size(aspect);
            assert_eq!((w % 4, h % 2), (0, 0), "NV12 needs w % 4 and even h");
        }
    }

    #[test]
    fn recordings_go_to_the_saved_folder_or_beside_the_video() {
        let dir = std::env::temp_dir().join(format!("reco-app-rec-folder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let canonical = std::fs::canonicalize(&dir).unwrap();
        let video = dir.join("left.mp4");
        assert_eq!(recording_folder(None, &video), canonical);
        assert_eq!(
            recording_folder(Some(Path::new("/nonexistent/folder")), &video),
            canonical
        );
        assert_eq!(
            recording_folder(Some(&dir), Path::new("elsewhere.mp4")),
            canonical
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_follow_the_slint_app() {
        assert_eq!(
            recording_file_name(1_700_000_000),
            "reco_recording_1700000000.mp4"
        );
    }
}
