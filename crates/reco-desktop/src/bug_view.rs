//! Report a bug in the App: the sheet's report, from what the person wrote
//! and (when they agree) what the app knows; Send with usage data on, Copy
//! report always.

use makepad_widgets::*;
use reco_app::ai;
use reco_app::bug_report::{compose, SystemFacts, LOG_LINES};
use reco_app::project::Camera;
use reco_app::telemetry::UsageEvent;
use reco_app::toasts::Severity;

use crate::help_view::version_line;
use crate::network::os_line;
use crate::App;

fn names(files: &[std::path::PathBuf]) -> Vec<String> {
    files
        .iter()
        .filter_map(|f| f.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .collect()
}

impl App {
    pub(crate) fn open_bug_report(&mut self, cx: &mut Cx) {
        self.show_bug_send(cx);
        self.ui.modal(cx, ids!(bug_sheet)).open(cx);
    }

    /// Send needs words and usage data on; without it, a hint says so.
    fn show_bug_send(&mut self, cx: &mut Cx) {
        let typed = !self
            .ui
            .text_input(cx, ids!(bug_message))
            .text()
            .trim()
            .is_empty();
        let on = self.settings.telemetry_enabled;
        self.set_button_enabled(cx, ids!(bug_send), on && typed);
        self.set_visible(cx, ids!(bug_hint), !on);
    }

    /// The sheet's controls.
    pub(crate) fn bug_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self
            .ui
            .text_input(cx, ids!(bug_message))
            .changed(actions)
            .is_some()
        {
            self.show_bug_send(cx);
        }
        if self.ui.button(cx, ids!(bug_copy)).clicked(actions) {
            let report = self.bug_report(cx);
            self.copy_text(cx, "bug_report", &report);
            self.toast(
                cx,
                Severity::Info,
                "Report copied",
                "Paste it into a post on the forum.",
            );
        }
        if self.ui.button(cx, ids!(bug_prefs)).clicked(actions) {
            self.ui.modal(cx, ids!(bug_sheet)).close(cx);
            self.open_preferences(cx);
        }
        if self.ui.button(cx, ids!(bug_cancel)).clicked(actions) {
            self.ui.modal(cx, ids!(bug_sheet)).close(cx);
        }
        if self.ui.button(cx, ids!(bug_send)).clicked(actions) && self.settings.telemetry_enabled {
            let report = self.bug_report(cx);
            self.ui.modal(cx, ids!(bug_sheet)).close(cx);
            self.ui.text_input(cx, ids!(bug_message)).set_text(cx, "");
            self.post_usage(cx, UsageEvent::BugReport { report });
        }
    }

    /// The report the sheet describes.
    fn bug_report(&mut self, cx: &mut Cx) -> String {
        let description = self.ui.text_input(cx, ids!(bug_message)).text();
        let contact = self.ui.text_input(cx, ids!(bug_contact)).text();
        let facts = self
            .ui
            .check_box(cx, ids!(bug_details))
            .active(cx)
            .then(|| self.system_facts());
        let home = std::env::home_dir();
        compose(
            description.trim(),
            &contact,
            facts.as_ref(),
            home.as_deref(),
        )
    }

    /// What the app knows that helps find a bug.
    fn system_facts(&mut self) -> SystemFacts {
        let log = self.log_tail(LOG_LINES);
        SystemFacts {
            version: version_line(),
            os: os_line(),
            gpu: self.gpu_name.clone(),
            ai: ai::capability_line(self.ai_availability.as_ref()),
            left: names(self.project.files(Camera::Left)),
            right: names(self.project.files(Camera::Right)),
            calibration: self
                .project
                .calibration
                .as_ref()
                .and_then(|c| c.file_name())
                .map(|n| n.to_string_lossy().into_owned()),
            stats: self.last_stats.clone(),
            calibration_run: self.last_calibration_run,
            log,
        }
    }
}
