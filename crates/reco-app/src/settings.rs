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
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            preview_aspect: PreviewAspect::Auto.name().into(),
            recording_quality: RecordingQuality::Balanced.name().into(),
            recording_folder: None,
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
