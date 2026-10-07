//! The Adjust panel's View and Lens sections in the App: the field of view,
//! "stay inside" and Reset view; each camera's lens name, lens correction, and
//! the fine-tune sliders with Reset lens. The sliders run from 0 to 1 over
//! each term's range (Makepad's slider has no runtime range).

use std::sync::Arc;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::lens::{Cameras, FineTuneRanges, Lens, LensDetection, LensInfo, LensSource};
use reco_app::preview::tuning::{CalibrationValues, Tuning};
use reco_app::preview::worker::{PreviewCommand, PreviewInfo};
use reco_app::project::Camera;

use crate::ui::fold::RecoFold;
use crate::value_text::Reading;
use crate::App;

/// The field of view as its field shows it.
const FOV: Reading = Reading::number(0, "°");

/// One fine-tune slider: its id, its value field, how to read and set its
/// field of a lens, its range, and how its value reads.
struct LensField {
    slider: &'static [LiveId],
    value: &'static [LiveId],
    get: fn(&Lens) -> f64,
    set: fn(&mut Lens, f64),
    range: fn(&FineTuneRanges) -> (f64, f64),
    digits: usize,
}

impl LensField {
    /// How the value reads in its field.
    fn reading(&self) -> Reading {
        Reading::number(self.digits, "")
    }
}

fn fields() -> [LensField; 8] {
    [
        LensField {
            slider: ids!(lens_fx),
            value: ids!(lens_fx_value),
            get: |l| l.fx,
            set: |l, v| l.fx = v,
            range: |r| r.fx,
            digits: 0,
        },
        LensField {
            slider: ids!(lens_fy),
            value: ids!(lens_fy_value),
            get: |l| l.fy,
            set: |l, v| l.fy = v,
            range: |r| r.fy,
            digits: 0,
        },
        LensField {
            slider: ids!(lens_cx),
            value: ids!(lens_cx_value),
            get: |l| l.cx,
            set: |l, v| l.cx = v,
            range: |r| r.cx,
            digits: 0,
        },
        LensField {
            slider: ids!(lens_cy),
            value: ids!(lens_cy_value),
            get: |l| l.cy,
            set: |l, v| l.cy = v,
            range: |r| r.cy,
            digits: 0,
        },
        LensField {
            slider: ids!(lens_k1),
            value: ids!(lens_k1_value),
            get: |l| l.k[0],
            set: |l, v| l.k[0] = v,
            range: |r| r.k[0],
            digits: 3,
        },
        LensField {
            slider: ids!(lens_k2),
            value: ids!(lens_k2_value),
            get: |l| l.k[1],
            set: |l, v| l.k[1] = v,
            range: |r| r.k[1],
            digits: 3,
        },
        LensField {
            slider: ids!(lens_k3),
            value: ids!(lens_k3_value),
            get: |l| l.k[2],
            set: |l, v| l.k[2] = v,
            range: |r| r.k[2],
            digits: 3,
        },
        LensField {
            slider: ids!(lens_k4),
            value: ids!(lens_k4_value),
            get: |l| l.k[3],
            set: |l, v| l.k[3] = v,
            range: |r| r.k[3],
            digits: 3,
        },
    ]
}

/// A value's place on a slider over `range`, 0 to 1.
fn fraction_of(value: f64, (lo, hi): (f64, f64)) -> f64 {
    if hi > lo {
        ((value - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// The value at `fraction` of a slider over `range`.
fn value_at(fraction: f64, (lo, hi): (f64, f64)) -> f64 {
    lo + fraction.clamp(0.0, 1.0) * (hi - lo)
}

/// What Show puts in the preview: the stitch, or one camera flat.
fn shown_at(index: usize) -> Option<Camera> {
    match index {
        1 => Some(Camera::Left),
        2 => Some(Camera::Right),
        _ => None,
    }
}

/// The Fine-tune camera choice, as the dropdown lists it.
fn cameras_at(index: usize) -> Cameras {
    match index {
        1 => Cameras::Right,
        2 => Cameras::Both,
        _ => Cameras::Left,
    }
}

/// A lens row's words: the camera and lens, marked when picked or loaded
/// by hand; "Generic lens" for the fallback.
pub(crate) fn lens_line(info: Option<&LensInfo>) -> String {
    match info {
        None => "Unknown camera".into(),
        Some(info) => match info.source {
            LensSource::Fallback => "Generic lens".into(),
            LensSource::Picker => format!("{} (picked)", info.name()),
            LensSource::File => format!("{} (file)", info.name()),
            LensSource::Detected | LensSource::Database => info.name(),
        },
    }
}

impl App {
    /// The View section: the field of view, stay inside, Reset view.
    pub(crate) fn view_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let fov = self.ui.slider(cx, ids!(fov_slider));
        if fov.start_slide(actions) {
            self.fov_dragging = true;
        }
        if let Some(degrees) =
            self.slider_input(cx, actions, ids!(fov_slider), ids!(fov_value), FOV)
        {
            self.send_preview(PreviewCommand::SetFov {
                degrees: degrees as f32,
            });
        }
        if fov.end_slide(actions).is_some() {
            self.fov_dragging = false;
        }
        if let Some(on) = self
            .ui
            .check_box(cx, ids!(constrained_look))
            .changed(actions)
        {
            self.send_preview(PreviewCommand::StayInside(on));
        }
        if self.ui.button(cx, ids!(reset_view)).clicked(actions) {
            self.send_preview(PreviewCommand::ResetView);
        }
    }

    /// Where the field of view is heading (a zoom, Reset view, stay
    /// inside): the slider follows unless it is being dragged.
    pub(crate) fn show_fov(&mut self, cx: &mut Cx, degrees: f32) {
        if self.fov_dragging {
            return;
        }
        self.ui
            .slider(cx, ids!(fov_slider))
            .set_value(cx, f64::from(degrees));
        self.set_label(cx, ids!(fov_value), &FOV.text(f64::from(degrees)));
    }

    /// Look up both cameras' lenses for the open videos.
    pub(crate) fn detect_lenses(&mut self, cx: &mut Cx, info: &PreviewInfo) {
        let Some(files) = self.live.as_ref().map(|l| l.files.clone()) else {
            return;
        };
        // The calibration run that made this file named its lenses.
        if let Some((_, left, right)) = self
            .calibrated_lenses
            .clone()
            .filter(|(path, _, _)| *path == files.calibration)
        {
            self.lens_detection = None;
            return self.show_lens_names(cx, left, right);
        }
        let (Some(left), Some(right)) = (files.left.first(), files.right.first()) else {
            return;
        };
        self.lens_detection = Some(LensDetection::start(
            left.clone(),
            right.clone(),
            (info.width, info.height),
            Arc::new(SignalToUI::set_ui_signal),
        ));
        self.set_label(cx, ids!(left_lens_name), "Looking up…");
        self.set_label(cx, ids!(right_lens_name), "Looking up…");
    }

    /// The lenses, once looked up.
    pub(crate) fn collect_lenses(&mut self, cx: &mut Cx) {
        let Some((left, right)) = self
            .lens_detection
            .as_ref()
            .and_then(LensDetection::try_result)
        else {
            return;
        };
        self.lens_detection = None;
        log!(
            "lenses: {} | {}",
            lens_line(left.as_ref()),
            lens_line(right.as_ref())
        );
        self.show_lens_names(cx, left, right);
    }

    /// Each camera's lens name; `None` keeps a side as it is.
    pub(crate) fn show_lens_names(
        &mut self,
        cx: &mut Cx,
        left: Option<LensInfo>,
        right: Option<LensInfo>,
    ) {
        self.set_label(cx, ids!(left_lens_name), &lens_line(left.as_ref()));
        self.set_label(cx, ids!(right_lens_name), &lens_line(right.as_ref()));
        self.lens_names = (left, right);
    }

    /// The live lenses into the Lens section: correction, Reset lens, and
    /// (once per open) the fine-tune sliders.
    pub(crate) fn show_lens_values(
        &mut self,
        cx: &mut Cx,
        values: &CalibrationValues,
        adopt: bool,
    ) {
        if adopt {
            // A new open shows the stitch.
            self.ui
                .drop_down(cx, ids!(lens_preview))
                .set_selected_item(cx, 0);
            self.ui.check_box(cx, ids!(lens_correction)).set_active(
                cx,
                values.lens_correction,
                Animate::No,
            );
            self.lens_base = values.left_lens;
            self.show_fine_tune(cx, values);
        }
        self.set_button_enabled(cx, ids!(reset_lens), values.lens_changed);
    }

    /// The fine-tune sliders for the chosen camera (Both shows the left).
    pub(crate) fn show_fine_tune(&mut self, cx: &mut Cx, values: &CalibrationValues) {
        let cameras = cameras_at(self.ui.drop_down(cx, ids!(lens_camera)).selected_item());
        self.fine_lens = if cameras == Cameras::Right {
            values.right_lens
        } else {
            values.left_lens
        };
        let (w, h) = values.lens_size;
        let ranges = FineTuneRanges::around(&self.lens_base, w, h);
        for field in fields() {
            let value = (field.get)(&self.fine_lens);
            self.ui
                .slider(cx, field.slider)
                .set_value(cx, fraction_of(value, (field.range)(&ranges)));
            self.set_label(cx, field.value, &field.reading().text(value));
        }
    }

    /// Lens correction, the fine-tune camera and sliders, Reset lens.
    pub(crate) fn lens_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if let Some(on) = self
            .ui
            .check_box(cx, ids!(lens_correction))
            .changed(actions)
        {
            self.send_preview(PreviewCommand::Tune(Tuning::LensCorrection(on)));
        }
        if let Some(index) = self.ui.drop_down(cx, ids!(lens_preview)).changed(actions) {
            let shown = shown_at(index);
            self.send_preview(PreviewCommand::ShowCamera(shown));
            // One camera on its own is for tuning its lens.
            if shown.is_some() {
                if let Some(mut fold) = self
                    .ui
                    .widget(cx, ids!(lens_advanced))
                    .borrow_mut::<RecoFold>()
                {
                    fold.set_is_open(cx, true, Animate::Yes);
                }
            }
        }
        if self
            .ui
            .drop_down(cx, ids!(lens_camera))
            .changed(actions)
            .is_some()
        {
            if let Some(values) = self.latest_values.clone() {
                self.show_fine_tune(cx, &values);
            }
        }
        let Some(size) = self.latest_values.as_ref().map(|v| v.lens_size) else {
            return;
        };
        let ranges = FineTuneRanges::around(&self.lens_base, size.0, size.1);
        let cameras = cameras_at(self.ui.drop_down(cx, ids!(lens_camera)).selected_item());
        for field in fields() {
            let range = (field.range)(&ranges);
            let reading = field.reading();
            let slider = self.ui.slider(cx, field.slider);
            let value = if let Some(fraction) = slider.slided(actions).or(slider.end_slide(actions))
            {
                Some(value_at(fraction, range))
            } else if let Some(input) = self.field_input(cx, actions, field.value) {
                // Typed or stepped: kept in the slider's range here, the
                // slider running from 0 to 1.
                let current = (field.get)(&self.fine_lens);
                let value = input
                    .value(reading, current)
                    .map(|v| v.max(range.0).min(range.1));
                match value {
                    Some(value) => slider.set_value(cx, fraction_of(value, range)),
                    None => self.set_label(cx, field.value, &reading.text(current)),
                }
                value
            } else {
                None
            };
            if let Some(value) = value {
                (field.set)(&mut self.fine_lens, value);
                self.set_label(cx, field.value, &reading.text(value));
                self.send_preview(PreviewCommand::Tune(Tuning::Lens {
                    cameras,
                    lens: self.fine_lens,
                }));
            }
        }
        if self.ui.button(cx, ids!(reset_lens)).clicked(actions) {
            self.send_preview(PreviewCommand::Tune(Tuning::ResetLens));
            // The sliders go back to the file's lenses here; the worker does
            // the same to the picture.
            if let Some(loaded) = self.loaded_values.clone() {
                self.lens_base = loaded.left_lens;
                self.show_fine_tune(cx, &loaded);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_places_and_values_meet() {
        assert_eq!(fraction_of(900.0, (765.0, 1035.0)), 0.5);
        assert_eq!(value_at(0.5, (765.0, 1035.0)), 900.0);
        assert_eq!(
            fraction_of(2000.0, (765.0, 1035.0)),
            1.0,
            "kept on the slider"
        );
        assert_eq!(
            fraction_of(1.0, (1.0, 1.0)),
            0.5,
            "an empty range sits mid-way"
        );
        assert_eq!(value_at(1.5, (-0.3, 0.3)), 0.3);
    }

    #[test]
    fn show_reads_stitch_left_right() {
        assert_eq!(shown_at(0), None);
        assert_eq!(shown_at(1), Some(Camera::Left));
        assert_eq!(shown_at(2), Some(Camera::Right));
    }

    #[test]
    fn the_camera_choice_reads_left_right_both() {
        assert_eq!(cameras_at(0), Cameras::Left);
        assert_eq!(cameras_at(1), Cameras::Right);
        assert_eq!(cameras_at(2), Cameras::Both);
    }

    #[test]
    fn lens_rows_say_where_a_lens_came_from() {
        let gopro = LensInfo {
            camera: "GoPro HERO9 Black".into(),
            lens: "Wide".into(),
            source: LensSource::Detected,
        };
        assert_eq!(lens_line(Some(&gopro)), "GoPro HERO9 Black · Wide");
        let picked = LensInfo {
            source: LensSource::Picker,
            ..gopro.clone()
        };
        assert_eq!(
            lens_line(Some(&picked)),
            "GoPro HERO9 Black · Wide (picked)"
        );
        let generic = LensInfo {
            source: LensSource::Fallback,
            ..gopro
        };
        assert_eq!(lens_line(Some(&generic)), "Generic lens");
        assert_eq!(lens_line(None), "Unknown camera");
    }
}
