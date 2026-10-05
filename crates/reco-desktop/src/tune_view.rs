//! The Adjust panel's stitch settings in the App: the sliders send
//! `Tune`, the worker answers with the live calibration's values, and Save
//! writes them to the calibration file.

use makepad_widgets::*;
use reco_app::preview::tuning::{CalibrationValues, Tuning};
use reco_app::preview::worker::PreviewCommand;
use reco_app::toasts::Severity;

use crate::App;

/// A slider, its value label, the change it sends, and how its value reads.
type TuneRow<'a> = (
    &'a [LiveId],
    &'a [LiveId],
    fn(f64) -> Tuning,
    fn(f64) -> String,
);

fn degrees(v: f64) -> String {
    format!("{v:.1}°")
}

fn two(v: f64) -> String {
    format!("{v:.2}")
}

fn three(v: f64) -> String {
    format!("{v:.3}")
}

/// Every tuning slider.
fn rows() -> [TuneRow<'static>; 6] {
    [
        (
            ids!(seam_blend),
            ids!(seam_value),
            |v| Tuning::Blend(v as f32),
            two,
        ),
        (
            ids!(rig_tilt),
            ids!(tilt_value),
            |v| Tuning::Tilt(v as f32),
            degrees,
        ),
        (
            ids!(rig_roll),
            ids!(roll_value),
            |v| Tuning::Roll(v as f32),
            degrees,
        ),
        (
            ids!(intersect),
            ids!(intersect_value),
            Tuning::Intersect,
            three,
        ),
        (
            ids!(axis_offset),
            ids!(axis_value),
            Tuning::AxisOffset,
            three,
        ),
        (ids!(x_ty), ids!(x_ty_value), Tuning::XTy, three),
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
        for ((slider, label, _, show), value) in rows().into_iter().zip(numbers) {
            self.ui.slider(cx, slider).set_value(cx, value);
            self.set_label(cx, label, &show(value));
        }
        self.ui
            .check_box(cx, ids!(match_colours))
            .set_active(cx, values.color_match, Animate::No);
        let sync = self.ui.text_input(cx, ids!(sync_input));
        if !sync.key_focus(cx) {
            sync.set_text(cx, &values.sync_offset.to_string());
        }
        self.show_outline(cx, values.roi_points);
        let unsaved = values.dirty && self.project.calibration.is_some();
        self.set_visible(cx, ids!(save_calibration), unsaved);
    }

    /// The Adjust panel's sliders, switch, Reset and Apply; Save.
    pub(crate) fn tune_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for (slider, label, tuning, show) in rows() {
            if let Some(value) = self.ui.slider(cx, slider).slided(actions) {
                self.set_label(cx, label, &show(value));
                self.send_preview(PreviewCommand::Tune(tuning(value)));
            }
        }
        if let Some(on) = self.ui.check_box(cx, ids!(match_colours)).changed(actions) {
            self.send_preview(PreviewCommand::Tune(Tuning::ColorMatch(on)));
        }
        if self.ui.button(cx, ids!(reset_layout)).clicked(actions) {
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
            if let Some(path) = self.project.calibration.clone() {
                self.send_preview(PreviewCommand::SaveCalibration { path });
            }
        }
    }
}
