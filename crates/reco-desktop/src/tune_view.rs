//! The Adjust panel's stitch settings in the App: the sliders send
//! `Tune`, the worker answers with the live calibration's values, and Save
//! writes them to the calibration file.

use makepad_widgets::*;
use reco_app::preview::tuning::{CalibrationValues, Tuning};
use reco_app::preview::worker::PreviewCommand;
use reco_app::toasts::Severity;

use crate::value_text::Reading;
use crate::App;

/// A slider, its value field, the change it sends, and how its value reads.
type TuneRow<'a> = (&'a [LiveId], &'a [LiveId], fn(f64) -> Tuning, Reading);

const DEGREES: Reading = Reading::number(1, "°");
const TWO: Reading = Reading::number(2, "");
const THREE: Reading = Reading::number(3, "");

/// Every tuning slider.
fn rows() -> [TuneRow<'static>; 6] {
    [
        (
            ids!(seam_blend),
            ids!(seam_value),
            |v| Tuning::Blend(v as f32),
            TWO,
        ),
        (
            ids!(rig_tilt),
            ids!(tilt_value),
            |v| Tuning::Tilt(v as f32),
            DEGREES,
        ),
        (
            ids!(rig_roll),
            ids!(roll_value),
            |v| Tuning::Roll(v as f32),
            DEGREES,
        ),
        (
            ids!(intersect),
            ids!(intersect_value),
            Tuning::Intersect,
            THREE,
        ),
        (
            ids!(axis_offset),
            ids!(axis_value),
            Tuning::AxisOffset,
            THREE,
        ),
        (ids!(x_ty), ids!(x_ty_value), Tuning::XTy, THREE),
    ]
}

impl App {
    pub(crate) fn send_preview(&self, command: PreviewCommand) {
        if let Some(live) = self.live.as_ref().filter(|l| l.open) {
            live.worker.send(command);
        }
    }

    /// The live calibration's values, into the Adjust panel; Save shows
    /// while something is unsaved.
    pub(crate) fn show_calibration_values(&mut self, cx: &mut Cx, values: CalibrationValues) {
        let numbers = [
            f64::from(values.blend),
            f64::from(values.tilt),
            f64::from(values.roll),
            values.intersect,
            values.axis_offset,
            values.x_ty,
        ];
        // The sliders lead for changes made with them: an echo still on its
        // way would pull a knob back. They take the worker's values once per
        // open, from the first values of a newer open.
        let adopt = values.opened > self.adopted_open;
        if adopt {
            self.adopted_open = values.opened;
            self.send_source_info(cx, values.sync_offset);
            for ((slider, field, _, reading), value) in rows().into_iter().zip(numbers) {
                self.ui.slider(cx, slider).set_value(cx, value);
                self.set_label(cx, field, &reading.text(value));
            }
            self.ui.check_box(cx, ids!(match_colours)).set_active(
                cx,
                values.color_match,
                Animate::No,
            );
            self.loaded_values = Some(values.clone());
        }
        let sync = self.ui.text_input(cx, ids!(sync_input));
        if !sync.key_focus(cx) {
            sync.set_text(cx, &values.sync_offset.to_string());
        }
        self.show_lens_values(cx, &values, adopt);
        self.show_outline(cx, values.roi_points);
        let unsaved = values.dirty && self.project.calibration.is_some();
        self.set_visible(cx, ids!(calibration_unsaved), unsaved);
        self.latest_values = Some(values);
    }

    /// The Adjust panel's sliders, switch, Reset and Apply; Save.
    pub(crate) fn tune_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for (slider, field, tuning, reading) in rows() {
            if let Some(value) = self.slider_input(cx, actions, slider, field, reading) {
                self.send_preview(PreviewCommand::Tune(tuning(value)));
            }
        }
        if let Some(on) = self.ui.check_box(cx, ids!(match_colours)).changed(actions) {
            self.send_preview(PreviewCommand::Tune(Tuning::ColorMatch(on)));
        }
        if self.ui.button(cx, ids!(reset_layout)).clicked(actions) {
            // The layout sliders go back to the file's values here; the
            // worker does the same to the picture.
            if let Some(loaded) = self.loaded_values.clone() {
                let layout = [loaded.intersect, loaded.axis_offset, loaded.x_ty];
                for ((slider, field, _, reading), value) in rows()[3..].iter().copied().zip(layout)
                {
                    self.ui.slider(cx, slider).set_value(cx, value);
                    self.set_label(cx, field, &reading.text(value));
                }
            }
            self.send_preview(PreviewCommand::Tune(Tuning::ResetLayout));
        }
        let sync = self.ui.text_input(cx, ids!(sync_input));
        if sync.returned(actions).is_some() || self.ui.button(cx, ids!(sync_apply)).clicked(actions)
        {
            match sync.text().trim().parse::<i64>() {
                Ok(frames) => self.send_preview(PreviewCommand::SetSyncOffset { frames }),
                Err(_) => self.toast(
                    cx,
                    Severity::Warn,
                    "Sync offset",
                    "Enter a whole number of frames, such as 12 or -30.",
                ),
            }
        }
        if self.ui.button(cx, ids!(save_calibration)).clicked(actions) {
            self.save_calibration(cx);
        }
    }

    /// Save the adjusted calibration to its file, when something is
    /// unsaved (Save in the Adjust panel's title row, ⌘S). The row goes at
    /// once, so a second press before the worker answers saves nothing
    /// twice; a failed save brings it back with the next values.
    pub(crate) fn save_calibration(&mut self, cx: &mut Cx) {
        let unsaved = self.latest_values.as_ref().is_some_and(|v| v.dirty);
        let Some(path) = self.project.calibration.clone().filter(|_| unsaved) else {
            return;
        };
        self.send_preview(PreviewCommand::SaveCalibration { path });
        if let Some(values) = self.latest_values.as_mut() {
            values.dirty = false;
        }
        self.set_visible(cx, ids!(calibration_unsaved), false);
    }
}
