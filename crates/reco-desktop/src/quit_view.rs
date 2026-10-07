//! Quitting (⌘Q, the menu, the window's close) with unsaved calibration edits
//! asks first. A termination signal still quits.

use makepad_widgets::*;

use crate::App;

/// Where a held quit is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Quitting {
    /// The sheet asks.
    Asking,
    /// Save was chosen: quit once the file is written.
    Saving,
}

impl App {
    /// Something in the calibration is unsaved (the Adjust panel says so).
    pub(crate) fn unsaved(&self) -> bool {
        self.project.calibration.is_some() && self.latest_values.as_ref().is_some_and(|v| v.dirty)
    }

    /// A quit or the window's close: `true` to hold it, while edits are
    /// unsaved (the sheet asks, once).
    pub(crate) fn hold_quit(&mut self, cx: &mut Cx) -> bool {
        if !self.unsaved() {
            return self.quitting.is_some();
        }
        if self.quitting.is_none() {
            self.quitting = Some(Quitting::Asking);
            let name = self
                .project
                .calibration
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let body = format!("The changes to {name} aren't saved.");
            self.set_label(cx, ids!(unsaved_body), &body);
            self.ui.modal(cx, ids!(unsaved_sheet)).open(cx);
        }
        true
    }

    /// The sheet's buttons.
    pub(crate) fn quit_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let sheet = self.ui.modal(cx, ids!(unsaved_sheet));
        if self.ui.button(cx, ids!(unsaved_save)).clicked(actions) {
            sheet.close(cx);
            self.quitting = Some(Quitting::Saving);
            self.save_calibration(cx);
        } else if self.ui.button(cx, ids!(unsaved_discard)).clicked(actions) {
            sheet.close(cx);
            self.quitting = None;
            cx.quit();
        } else if self.ui.button(cx, ids!(unsaved_cancel)).clicked(actions) {
            sheet.close(cx);
            self.quitting = None;
        } else if sheet.dismissed(actions) && self.quitting == Some(Quitting::Asking) {
            self.quitting = None;
        }
    }

    /// The save chosen on the way out landed (quit) or failed (stay; the
    /// failure has its toast).
    pub(crate) fn quit_saved(&mut self, cx: &mut Cx, saved: bool) {
        if self.quitting == Some(Quitting::Saving) {
            self.quitting = None;
            if saved {
                cx.quit();
            }
        }
    }
}
