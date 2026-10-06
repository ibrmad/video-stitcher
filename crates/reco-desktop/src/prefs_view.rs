//! Preferences in the App: the app-wide settings (the recording codec and
//! folder, and usage data). Settings with a home elsewhere stay there: the
//! export sheet remembers its own codec, quality and AI model, the view
//! bar has the recording quality, and Adjust the seam blend. Save checks
//! the typed folder, keeps everything and applies it; Cancel, Escape or a
//! press outside leaves the settings as they were (the next open shows
//! them again).

use std::path::{Path, PathBuf};

use makepad_widgets::*;
use reco_app::export;
use reco_app::telemetry::UsageEvent;

use crate::project_view::Pick;
use crate::App;

/// The recording folder typed in the sheet: none (recordings go beside the
/// left video) or a folder that exists.
pub(crate) fn checked_folder(typed: &str) -> Result<Option<PathBuf>, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Ok(None);
    }
    let folder = PathBuf::from(typed);
    if !folder.is_dir() {
        return Err("That recording folder doesn't exist.".into());
    }
    Ok(Some(folder))
}

/// A path for a text field ("" for none).
fn path_text(path: Option<&Path>) -> String {
    path.map(|p| p.display().to_string()).unwrap_or_default()
}

impl App {
    /// The codecs Preferences offers: this machine's, and the `saved` ones
    /// even before the probe has answered (Save keeps what the person
    /// chose).
    fn prefs_codecs(&self, saved: &[&str]) -> Vec<String> {
        let mut codecs = self.codecs();
        for codec in saved {
            if !codecs.iter().any(|c| c == codec) {
                codecs.push(codec.to_string());
            }
        }
        codecs
    }

    /// Open the sheet on the saved settings.
    pub(crate) fn open_preferences(&mut self, cx: &mut Cx) {
        let settings = self.settings.clone();
        let codecs = self.prefs_codecs(&[&settings.recording_codec]);
        let labels: Vec<String> = codecs.iter().map(|c| export::codec_label(c)).collect();
        let dropdown = self.ui.drop_down(cx, ids!(prefs_record_codec));
        dropdown.set_labels(cx, labels);
        let at = codecs
            .iter()
            .position(|c| *c == settings.recording_codec)
            .unwrap_or(0);
        dropdown.set_selected_item(cx, at);
        self.prefs_codec_list = codecs;
        self.ui
            .text_input(cx, ids!(prefs_folder))
            .set_text(cx, &path_text(settings.recording_folder.as_deref()));
        self.ui.check_box(cx, ids!(prefs_telemetry)).set_active(
            cx,
            settings.telemetry_enabled,
            Animate::No,
        );
        self.show_prefs_error(cx, None);
        self.ui.modal(cx, ids!(prefs_sheet)).open(cx);
    }

    fn show_prefs_error(&mut self, cx: &mut Cx, error: Option<&str>) {
        self.set_visible(cx, ids!(prefs_error), error.is_some());
        self.set_label(cx, ids!(prefs_error_text), error.unwrap_or(""));
    }

    /// The sheet's controls.
    pub(crate) fn prefs_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self
            .ui
            .button(cx, ids!(prefs_folder_browse))
            .clicked(actions)
        {
            self.pick(cx, Pick::RecordingFolder);
        }
        if self.ui.button(cx, ids!(prefs_cancel)).clicked(actions) {
            self.ui.modal(cx, ids!(prefs_sheet)).close(cx);
        }
        if self.ui.button(cx, ids!(prefs_save)).clicked(actions) {
            self.save_preferences(cx);
        }
    }

    /// The recording folder chosen in a dialog, into its field.
    pub(crate) fn prefs_folder_picked(&mut self, cx: &mut Cx, path: &Path) {
        self.ui
            .text_input(cx, ids!(prefs_folder))
            .set_text(cx, &path.display().to_string());
        self.show_prefs_error(cx, None);
    }

    /// Check the typed folder, then keep and apply everything.
    fn save_preferences(&mut self, cx: &mut Cx) {
        let folder = match checked_folder(&self.ui.text_input(cx, ids!(prefs_folder)).text()) {
            Ok(folder) => folder,
            Err(why) => {
                self.show_prefs_error(cx, Some(&why));
                return;
            }
        };
        let at = self
            .ui
            .drop_down(cx, ids!(prefs_record_codec))
            .selected_item();
        let recording_codec = self
            .prefs_codec_list
            .get(at)
            .cloned()
            .unwrap_or_else(|| "h264".into());
        let telemetry = self.ui.check_box(cx, ids!(prefs_telemetry)).active(cx);

        let turned_on = telemetry && !self.settings.telemetry_enabled;
        let s = &mut self.settings;
        s.recording_codec = recording_codec;
        s.recording_folder = folder;
        s.telemetry_enabled = telemetry;
        log!(
            "preferences: recording {} to {}, usage data {}",
            s.recording_codec,
            s.recording_folder
                .as_deref()
                .map_or("beside the left video".into(), |f| f.display().to_string()),
            if telemetry { "on" } else { "off" }
        );
        self.save_settings();
        self.apply_settings(cx);
        self.ui.modal(cx, ids!(prefs_sheet)).close(cx);
        if turned_on {
            self.send_usage(cx, UsageEvent::AppOpen);
            self.send_context(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("reco-prefs-{}-{name}", std::process::id()));
        std::fs::write(&path, b"x").expect("a temp file");
        path
    }

    #[test]
    fn the_recording_folder_must_exist() {
        let dir = std::env::temp_dir();
        assert_eq!(checked_folder("  "), Ok(None), "empty: beside the video");
        assert_eq!(
            checked_folder(&format!(" {} ", dir.display())),
            Ok(Some(dir.clone()))
        );
        let missing = Err("That recording folder doesn't exist.".to_string());
        assert_eq!(checked_folder("/no/such/folder"), missing);
        let file = temp_file("not-a-folder.txt");
        assert_eq!(checked_folder(&file.display().to_string()), missing);
        let _ = std::fs::remove_file(file);
    }
}
