//! Preferences in the App: the sheet shows the saved settings; Save checks
//! the typed folder and model, keeps everything and applies it; Cancel,
//! Escape or a press outside leaves the settings as they were (the next
//! open shows them again).

use std::path::{Path, PathBuf};

use makepad_widgets::*;
use reco_app::export::{self, QUALITIES};
use reco_app::recording::RecordingQuality;
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

/// The AI model typed in the sheet: none, or an .onnx file that exists.
pub(crate) fn checked_model(typed: &str) -> Result<Option<PathBuf>, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Ok(None);
    }
    let model = PathBuf::from(typed);
    let onnx = model
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("onnx"));
    if !onnx {
        return Err("The AI model must be an .onnx file.".into());
    }
    if !model.is_file() {
        return Err("That model file doesn't exist.".into());
    }
    Ok(Some(model))
}

/// A path for a text field ("" for none).
fn path_text(path: Option<&Path>) -> String {
    path.map(|p| p.display().to_string()).unwrap_or_default()
}

impl App {
    /// The codecs Preferences offers: this machine's, and the saved
    /// `codec` even before the probe has answered (Save keeps what the
    /// person chose).
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
        let codecs = self.prefs_codecs(&[&settings.export_codec, &settings.recording_codec]);
        let labels: Vec<String> = codecs.iter().map(|c| export::codec_label(c)).collect();
        for (id, saved) in [
            (ids!(prefs_export_codec), &settings.export_codec),
            (ids!(prefs_record_codec), &settings.recording_codec),
        ] {
            let dropdown = self.ui.drop_down(cx, id);
            dropdown.set_labels(cx, labels.clone());
            let at = codecs.iter().position(|c| c == saved).unwrap_or(0);
            dropdown.set_selected_item(cx, at);
        }
        self.prefs_codec_list = codecs;
        let quality = QUALITIES
            .iter()
            .position(|q| *q == settings.export_quality)
            .unwrap_or(1);
        self.ui
            .drop_down(cx, ids!(prefs_export_quality))
            .set_selected_item(cx, quality);
        self.ui
            .drop_down(cx, ids!(prefs_record_quality))
            .set_selected_item(cx, settings.quality().index());
        self.ui
            .text_input(cx, ids!(prefs_folder))
            .set_text(cx, &path_text(settings.recording_folder.as_deref()));
        self.ui
            .slider(cx, ids!(prefs_blend))
            .set_value(cx, f64::from(settings.blend()));
        self.show_prefs_blend(cx, f64::from(settings.blend()));
        self.ui
            .text_input(cx, ids!(prefs_model))
            .set_text(cx, &path_text(settings.ai_model_path.as_deref()));
        self.ui.check_box(cx, ids!(prefs_telemetry)).set_active(
            cx,
            settings.telemetry_enabled,
            Animate::No,
        );
        self.show_prefs_error(cx, None);
        self.ui.modal(cx, ids!(prefs_sheet)).open(cx);
    }

    fn show_prefs_blend(&mut self, cx: &mut Cx, blend: f64) {
        self.set_label(cx, ids!(prefs_blend_value), &format!("{blend:.2}"));
    }

    fn show_prefs_error(&mut self, cx: &mut Cx, error: Option<&str>) {
        self.set_visible(cx, ids!(prefs_error), error.is_some());
        self.set_label(cx, ids!(prefs_error_text), error.unwrap_or(""));
    }

    /// The sheet's controls.
    pub(crate) fn prefs_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if let Some(blend) = self.ui.slider(cx, ids!(prefs_blend)).slided(actions) {
            self.show_prefs_blend(cx, blend);
        }
        if self
            .ui
            .button(cx, ids!(prefs_folder_browse))
            .clicked(actions)
        {
            self.pick(cx, Pick::RecordingFolder);
        }
        if self
            .ui
            .button(cx, ids!(prefs_model_browse))
            .clicked(actions)
        {
            self.pick(cx, Pick::Model);
        }
        if self.ui.button(cx, ids!(prefs_cancel)).clicked(actions) {
            self.ui.modal(cx, ids!(prefs_sheet)).close(cx);
        }
        if self.ui.button(cx, ids!(prefs_save)).clicked(actions) {
            self.save_preferences(cx);
        }
    }

    /// A folder or model chosen in a dialog, into its field.
    pub(crate) fn prefs_path_picked(&mut self, cx: &mut Cx, pick: Pick, path: &Path) {
        let field = match pick {
            Pick::RecordingFolder => ids!(prefs_folder),
            _ => ids!(prefs_model),
        };
        self.ui
            .text_input(cx, field)
            .set_text(cx, &path.display().to_string());
        self.show_prefs_error(cx, None);
    }

    /// Check the typed paths, then keep and apply everything.
    fn save_preferences(&mut self, cx: &mut Cx) {
        let folder = checked_folder(&self.ui.text_input(cx, ids!(prefs_folder)).text());
        let model = checked_model(&self.ui.text_input(cx, ids!(prefs_model)).text());
        let (folder, model) = match (folder, model) {
            (Ok(folder), Ok(model)) => (folder, model),
            (Err(why), _) | (_, Err(why)) => {
                self.show_prefs_error(cx, Some(&why));
                return;
            }
        };
        let codec = |app: &Self, cx: &mut Cx, id: &[LiveId]| {
            let at = app.ui.drop_down(cx, id).selected_item();
            app.prefs_codec_list
                .get(at)
                .cloned()
                .unwrap_or_else(|| "h264".into())
        };
        let export_codec = codec(self, cx, ids!(prefs_export_codec));
        let recording_codec = codec(self, cx, ids!(prefs_record_codec));
        let export_quality = QUALITIES
            .get(
                self.ui
                    .drop_down(cx, ids!(prefs_export_quality))
                    .selected_item(),
            )
            .copied()
            .unwrap_or("balanced");
        let recording_quality = RecordingQuality::from_index(
            self.ui
                .drop_down(cx, ids!(prefs_record_quality))
                .selected_item(),
        );
        let blend = self
            .ui
            .slider(cx, ids!(prefs_blend))
            .value()
            .unwrap_or(0.05);
        let telemetry = self.ui.check_box(cx, ids!(prefs_telemetry)).active(cx);

        let turned_on = telemetry && !self.settings.telemetry_enabled;
        let s = &mut self.settings;
        s.export_codec = export_codec;
        s.export_quality = export_quality.into();
        s.recording_codec = recording_codec;
        s.set_quality(recording_quality);
        s.recording_folder = folder;
        s.default_blend = blend as f32;
        s.ai_model_path = model;
        s.telemetry_enabled = telemetry;
        log!(
            "preferences: export {} {}, recording {} {}, blend {:.2}, usage data {}",
            s.export_codec,
            s.export_quality,
            s.recording_codec,
            s.recording_quality,
            s.default_blend,
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

    #[test]
    fn the_model_is_an_onnx_file_that_exists() {
        assert_eq!(checked_model(""), Ok(None), "no model");
        let model = temp_file("yolo.ONNX");
        assert_eq!(
            checked_model(&model.display().to_string()),
            Ok(Some(model.clone()))
        );
        assert_eq!(
            checked_model("/no/such/model.onnx"),
            Err("That model file doesn't exist.".to_string())
        );
        let text = temp_file("notes.txt");
        assert_eq!(
            checked_model(&text.display().to_string()),
            Err("The AI model must be an .onnx file.".to_string())
        );
        let _ = (std::fs::remove_file(model), std::fs::remove_file(text));
    }
}
