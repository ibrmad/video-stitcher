//! The desktop app's settings, kept in `desktop.json` in reco-io's settings
//! folder (beside the Slint app's `gui.json`; `RECO_CONFIG_DIR` overrides
//! the folder). Values are stored by name, as the Slint app stored them,
//! and anything missing or unknown reads as its default.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::preview::view::PreviewAspect;
use crate::recording::RecordingQuality;

/// The settings file's name in reco-io's settings folder.
pub const NAMESPACE: &str = "desktop";

/// What the desktop app remembers between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopSettings {
    /// The preview aspect: "auto", "16:9", "4:3" or "21:9".
    pub preview_aspect: String,
    /// Recording quality: "fast", "balanced" or "high".
    pub recording_quality: String,
    /// Where recordings go; beside the left video when unset or missing.
    pub recording_folder: Option<PathBuf>,
    /// Recently opened sessions, newest first.
    pub recent: Vec<RecentSession>,
}

/// Sessions the Recent menu keeps.
pub const MAX_RECENT: usize = 8;

/// A camera pair and its calibration, as the Recent menu restores it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentSession {
    /// The left camera's files, in play order.
    pub left: Vec<PathBuf>,
    /// The right camera's files.
    pub right: Vec<PathBuf>,
    /// The calibration.
    pub calibration: Option<PathBuf>,
}

impl RecentSession {
    /// "GX010120 + GX010092": each camera's first file, without extension.
    pub fn label(&self) -> String {
        let first = |files: &[PathBuf]| {
            files
                .first()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
        };
        match (first(&self.left), first(&self.right)) {
            (Some(l), Some(r)) => format!("{l} + {r}"),
            (Some(one), None) | (None, Some(one)) => one,
            (None, None) => String::new(),
        }
    }
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            preview_aspect: PreviewAspect::Auto.name().into(),
            recording_quality: RecordingQuality::Balanced.name().into(),
            recording_folder: None,
            recent: Vec::new(),
        }
    }
}

impl DesktopSettings {
    /// The saved preview aspect (Auto when unknown).
    pub fn aspect(&self) -> PreviewAspect {
        PreviewAspect::from_name(&self.preview_aspect)
    }

    /// Remember `aspect`.
    pub fn set_aspect(&mut self, aspect: PreviewAspect) {
        self.preview_aspect = aspect.name().into();
    }

    /// The saved recording quality (Balanced when unknown).
    pub fn quality(&self) -> RecordingQuality {
        RecordingQuality::from_name(&self.recording_quality)
    }

    /// Put `session` first in the Recent menu: an earlier session of the
    /// same pair (the same first file for each camera, so adding or
    /// reordering files keeps one entry) is replaced, and only the newest
    /// [`MAX_RECENT`] are kept.
    pub fn push_recent(&mut self, session: RecentSession) {
        let key = |r: &RecentSession| (r.left.first().cloned(), r.right.first().cloned());
        let pair = key(&session);
        self.recent.retain(|r| key(r) != pair);
        self.recent.insert(0, session);
        self.recent.truncate(MAX_RECENT);
    }

    /// Forget every recent session.
    pub fn clear_recent(&mut self) {
        self.recent.clear();
    }

    /// Remember `quality`.
    pub fn set_quality(&mut self, quality: RecordingQuality) {
        self.recording_quality = quality.name().into();
    }
}

/// The saved settings, or the defaults when the file is missing or
/// unreadable (reco-io logs a warning for a malformed file).
pub fn load() -> DesktopSettings {
    reco_io::settings::load_or_default(NAMESPACE)
}

/// Save the settings: a small atomic write, the one file write the UI
/// thread may do.
pub fn save(settings: &DesktopSettings) -> Result<(), String> {
    reco_io::settings::save(NAMESPACE, settings).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_slint_app() {
        let d = DesktopSettings::default();
        assert_eq!(d.preview_aspect, "auto");
        assert_eq!(d.recording_quality, "balanced");
        assert_eq!(d.recording_folder, None);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let s: DesktopSettings = serde_json::from_str(r#"{"preview_aspect":"4:3"}"#).unwrap();
        assert_eq!(s.aspect(), PreviewAspect::Classic4x3);
        assert_eq!(s.quality(), RecordingQuality::Balanced);
    }

    #[test]
    fn unknown_values_fall_back() {
        let s: DesktopSettings = serde_json::from_str(
            r#"{"preview_aspect":"5:4","recording_quality":"ultra","extra":1}"#,
        )
        .unwrap();
        assert_eq!(s.aspect(), PreviewAspect::Auto);
        assert_eq!(s.quality(), RecordingQuality::Balanced);
    }

    fn session(left: &str, right: &str) -> RecentSession {
        RecentSession {
            left: vec![PathBuf::from(format!("/m/{left}.MP4"))],
            right: vec![PathBuf::from(format!("/m/{right}.MP4"))],
            calibration: None,
        }
    }

    #[test]
    fn recent_sessions_are_newest_first_without_repeats() {
        let mut s = DesktopSettings::default();
        for i in 0..10 {
            s.push_recent(session(&format!("L{i}"), "R"));
        }
        assert_eq!(s.recent.len(), MAX_RECENT);
        assert_eq!(s.recent[0].label(), "L9 + R");
        let mut again = session("L5", "R");
        again.calibration = Some("/m/c.json".into());
        s.push_recent(again.clone());
        assert_eq!(s.recent.len(), MAX_RECENT);
        assert_eq!(s.recent[0], again);
        assert_eq!(s.recent.iter().filter(|r| r.label() == "L5 + R").count(), 1);
        // Adding a file to a camera, or reordering, keeps one session.
        let mut grown = session("L5", "R");
        grown.left.push(PathBuf::from("/m/L5b.MP4"));
        s.push_recent(grown.clone());
        assert_eq!(s.recent.iter().filter(|r| r.label() == "L5 + R").count(), 1);
        assert_eq!(s.recent[0], grown);
        s.clear_recent();
        assert!(s.recent.is_empty());
    }

    #[test]
    fn a_settings_file_without_sessions_still_loads() {
        let s: DesktopSettings = serde_json::from_str(r#"{"preview_aspect":"16:9"}"#).unwrap();
        assert!(s.recent.is_empty());
        assert_eq!(RecentSession::default().label(), "");
    }

    #[test]
    fn setters_store_names() {
        let mut s = DesktopSettings::default();
        s.set_aspect(PreviewAspect::Cinema21x9);
        s.set_quality(RecordingQuality::High);
        let text = serde_json::to_string(&s).unwrap();
        assert!(
            text.contains("\"21:9\"") && text.contains("\"high\""),
            "{text}"
        );
        let back: DesktopSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back, s);
    }
}
