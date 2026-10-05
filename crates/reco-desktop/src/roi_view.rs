//! The field outline in the App: the browser editor is written on a job
//! thread and opened, and the outline it copies is pasted back into the
//! Setup panel and sent to the live calibration (saved with it).

use std::sync::Arc;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::preview::worker::PreviewCommand;
use reco_app::project::Camera;
use reco_app::roi::{editor_folder, open_in_browser, parse_outline, EditorJob, EditorRequest};
use reco_app::toasts::Severity;

use crate::App;

/// Checks write the editor without opening a browser.
const NO_BROWSER: &str = "RECO_DESKTOP_NO_BROWSER";

impl App {
    /// The outline's status: how many points it has.
    pub(crate) fn show_outline(&mut self, cx: &mut Cx, points: usize) {
        let status = if points == 0 {
            "None".to_string()
        } else {
            format!("{points} points")
        };
        self.set_label(cx, ids!(roi_status), &status);
        self.set_visible(cx, ids!(roi_clear), points > 0);
    }

    /// Edit in browser, Use, and Remove outline.
    pub(crate) fn outline_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(roi_edit)).clicked(actions) {
            self.open_outline_editor(cx);
        }
        let field = self.ui.text_input(cx, ids!(roi_json));
        if field.returned(actions).is_some() || self.ui.button(cx, ids!(roi_use)).clicked(actions) {
            match parse_outline(&field.text()) {
                Ok(outline) => {
                    self.send_preview(PreviewCommand::SetFieldRoi(Some(outline)));
                    field.set_text(cx, "");
                }
                Err(why) => self.toast(cx, Severity::Warn, "That isn't a field outline", &why),
            }
        }
        if self.ui.button(cx, ids!(roi_clear)).clicked(actions) {
            self.send_preview(PreviewCommand::SetFieldRoi(None));
        }
    }

    /// Write the editor for the frame on screen (on a job thread).
    fn open_outline_editor(&mut self, cx: &mut Cx) {
        let (Some(left), Some(right), Some(calibration)) = (
            self.project.files(Camera::Left).first().cloned(),
            self.project.files(Camera::Right).first().cloned(),
            self.project.calibration.clone(),
        ) else {
            self.toast(
                cx,
                Severity::Warn,
                "The outline needs a stitch",
                "Add both cameras and a calibration first.",
            );
            return;
        };
        let frame = self.live.as_ref().map_or(0, |l| l.frame.saturating_sub(1));
        self.outline_editor = Some(EditorJob::start(
            EditorRequest {
                left,
                right,
                frame,
                calibration,
            },
            editor_folder(),
            Arc::new(SignalToUI::set_ui_signal),
        ));
    }

    /// The editor was written: open it in the browser.
    pub(crate) fn drain_outline_editor(&mut self, cx: &mut Cx) {
        let Some(result) = self.outline_editor.as_ref().and_then(EditorJob::try_result) else {
            return;
        };
        self.outline_editor = None;
        match result {
            Ok(page) => {
                log!("outline editor: {}", page.display());
                if std::env::var_os(NO_BROWSER).is_none() {
                    if let Err(e) = open_in_browser(&page) {
                        self.toast(
                            cx,
                            Severity::Error,
                            "Couldn't open the browser",
                            &e.to_string(),
                        );
                        return;
                    }
                }
                self.toast(
                    cx,
                    Severity::Info,
                    "Outline editor ready",
                    "Draw the pitch's outline in your browser, copy it, then paste it here.",
                );
            }
            Err(why) => self.toast(
                cx,
                Severity::Error,
                "Couldn't open the outline editor",
                &why,
            ),
        }
    }
}
