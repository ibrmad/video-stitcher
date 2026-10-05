//! AI tracking in the export sheet (Module 6b): whether this machine can
//! run it, the rows' rules, Choose…, the style presets, the lookahead's
//! zones, and the tracking an export runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::ai::{
    self, Availability, AvailabilityProbe, LookaheadZones, PannerKnobs, Tracking, CLUSTER_MODES,
    FRAMINGS, INTERVALS, MAX_LOOKAHEAD, MODES, PRESETS,
};

use crate::project_view::Pick;
use crate::ui::zones::RecoZones;
use crate::App;

/// A reason the detector can't run, standing in for the machine's answer
/// (checks).
const FAKE_AI: &str = "RECO_DESKTOP_FAKE_AI";

/// The status line until the machine answers.
const CHECKING: &str = "Checking whether this machine can run it…";

/// A lookahead to a tenth of a second, inside the slider's range.
fn rounded_lookahead(secs: f64) -> f64 {
    ((secs * 10.0).round() / 10.0).clamp(0.0, MAX_LOOKAHEAD)
}

/// The lookahead as its value reads.
fn lookahead_text(secs: f64) -> String {
    let secs = rounded_lookahead(secs);
    if secs <= 0.0 {
        "Off".into()
    } else {
        format!("{secs:.1} s")
    }
}

/// The line under the lookahead, and whether it warns: nothing without a
/// reading of the GPU's memory.
fn lookahead_note(secs: f64, zones: Option<LookaheadZones>) -> Option<(String, bool)> {
    let zones = zones?;
    Some(match zone_index(secs, zones) {
        0 => ("Fits this GPU's memory.".into(), false),
        1 => (
            format!(
                "Tight: past {:.1} s this GPU's memory may run short.",
                zones.safe
            ),
            true,
        ),
        _ => (
            format!(
                "Too long: this GPU's memory holds about {:.1} s.",
                zones.max
            ),
            true,
        ),
    })
}

/// The zone a lookahead sits in: 0 comfortable, 1 tight, 2 too long.
fn zone_index(secs: f64, zones: LookaheadZones) -> usize {
    if secs <= zones.safe {
        0
    } else if secs <= zones.max {
        1
    } else {
        2
    }
}

/// Where the zones end on the slider's track, from 0 to 1.
fn zone_ends(zones: Option<LookaheadZones>) -> Option<(f64, f64)> {
    zones.map(|z| (z.safe / MAX_LOOKAHEAD, z.max / MAX_LOOKAHEAD))
}

/// The row of `name` in `names` (the first when none matches).
fn row_of(names: &[&str], name: &str) -> usize {
    names.iter().position(|n| *n == name).unwrap_or(0)
}

/// The Detection row for `interval` (every 15 frames when it isn't listed).
fn interval_row(interval: u32) -> usize {
    INTERVALS
        .iter()
        .position(|i| *i == interval)
        .or_else(|| INTERVALS.iter().position(|i| *i == 15))
        .unwrap_or(0)
}

/// A knob's value as its row shows it.
fn knob_text(value: f64) -> String {
    format!("{value:.2}")
}

/// A field of view as its row shows it.
fn degrees_text(value: f64) -> String {
    format!("{value:.0}°")
}

/// An Advanced tier slider: the slider, its value, and how it reads.
type KnobSlider = (&'static [LiveId], &'static [LiveId], fn(f64) -> String);

/// The Advanced tier's sliders, in `show_ai_knobs`' order.
fn knob_sliders() -> [KnobSlider; 6] {
    [
        (ids!(ai_ball_weight), ids!(ai_ball_weight_value), knob_text),
        (ids!(ai_bandwidth), ids!(ai_bandwidth_value), knob_text),
        (ids!(ai_dead_zone), ids!(ai_dead_zone_value), knob_text),
        (ids!(ai_fov_tight), ids!(ai_fov_tight_value), degrees_text),
        (
            ids!(ai_fov_default),
            ids!(ai_fov_default_value),
            degrees_text,
        ),
        (ids!(ai_fov_wide), ids!(ai_fov_wide_value), degrees_text),
    ]
}

impl App {
    /// Ask once, at start, whether this machine can run the detector.
    pub(crate) fn start_ai_probe(&mut self) {
        if let Some(why) = std::env::var_os(FAKE_AI) {
            let answer = Availability::Unavailable(why.to_string_lossy().into_owned());
            log!("AI tracking: {}", ai::availability_line(&answer));
            self.ai_availability = Some(answer);
            return;
        }
        self.ai_probe = Some(AvailabilityProbe::start(Arc::new(
            SignalToUI::set_ui_signal,
        )));
    }

    /// The machine's answer, once in.
    pub(crate) fn collect_ai_availability(&mut self, cx: &mut Cx) {
        let Some(answer) = self
            .ai_probe
            .as_ref()
            .and_then(AvailabilityProbe::try_result)
        else {
            return;
        };
        self.ai_probe = None;
        log!("AI tracking: {}", ai::availability_line(&answer));
        self.ai_availability = Some(answer);
        let on = self.settings.ai_enabled && self.ai_available();
        self.ui
            .check_box(cx, ids!(ai_enable))
            .set_active(cx, on, Animate::No);
        self.show_ai_availability(cx);
    }

    fn ai_available(&self) -> bool {
        matches!(self.ai_availability, Some(Availability::Ready(_)))
    }

    /// The status line and Enable: off, and dimmed, until the machine can
    /// run the detector.
    fn show_ai_availability(&mut self, cx: &mut Cx) {
        let (line, failed) = match &self.ai_availability {
            None => (CHECKING.to_string(), false),
            Some(answer) => (
                ai::availability_line(answer),
                matches!(answer, Availability::Unavailable(_)),
            ),
        };
        self.set_label(cx, ids!(ai_status), &line);
        self.set_label(cx, ids!(ai_status_error), &line);
        self.set_visible(cx, ids!(ai_status), !failed);
        self.set_visible(cx, ids!(ai_status_error), failed);
        let available = self.ai_available();
        self.ui
            .widget(cx, ids!(ai_enable))
            .set_disabled(cx, !available);
        if !available {
            self.ui
                .check_box(cx, ids!(ai_enable))
                .set_active(cx, false, Animate::No);
        }
        self.show_ai_rows(cx);
    }

    /// The rows show while tracking is on; the model's problem and Export
    /// follow.
    fn show_ai_rows(&mut self, cx: &mut Cx) {
        let on = self.ui.check_box(cx, ids!(ai_enable)).active(cx);
        self.set_visible(cx, ids!(ai_rows), on);
        self.show_ai_problem(cx);
    }

    /// Why the tracking can't run as chosen, under the model; Export
    /// follows.
    fn show_ai_problem(&mut self, cx: &mut Cx) {
        let problem = self.sheet_tracking(cx).and_then(|t| t.problem());
        self.set_label(
            cx,
            ids!(ai_model_problem_text),
            problem.as_deref().unwrap_or(""),
        );
        self.set_visible(cx, ids!(ai_model_problem), problem.is_some());
        self.show_export_enabled(cx);
    }

    /// Whether the tracking chosen keeps Export from starting.
    pub(crate) fn ai_blocks_export(&self, cx: &mut Cx) -> bool {
        self.sheet_tracking(cx).and_then(|t| t.problem()).is_some()
    }

    /// The tracking chosen in the sheet: `None` when it is off.
    pub(crate) fn sheet_tracking(&self, cx: &mut Cx) -> Option<Tracking> {
        if !self.ai_available() || !self.ui.check_box(cx, ids!(ai_enable)).active(cx) {
            return None;
        }
        let typed = self.ui.text_input(cx, ids!(ai_model)).text();
        let typed = typed.trim();
        let row = |app: &Self, cx: &mut Cx, id: &[LiveId]| app.ui.drop_down(cx, id).selected_item();
        let mode = MODES[row(self, cx, ids!(ai_mode)).min(MODES.len() - 1)];
        let interval = INTERVALS[row(self, cx, ids!(ai_interval)).min(INTERVALS.len() - 1)];
        let preset = PRESETS[row(self, cx, ids!(ai_preset)).min(PRESETS.len() - 1)];
        let lookahead = self
            .ui
            .slider(cx, ids!(ai_lookahead))
            .value()
            .unwrap_or(MAX_LOOKAHEAD);
        Some(Tracking {
            model: (!typed.is_empty()).then(|| PathBuf::from(typed)),
            mode: mode.into(),
            interval,
            preset: preset.into(),
            knobs: self.sheet_knobs(cx),
            lookahead_secs: rounded_lookahead(lookahead),
        })
    }

    /// The panner's knobs as the sheet shows them.
    fn sheet_knobs(&self, cx: &mut Cx) -> PannerKnobs {
        let row = |app: &Self, cx: &mut Cx, id: &[LiveId]| app.ui.drop_down(cx, id).selected_item();
        let value = |app: &Self, cx: &mut Cx, id: &[LiveId]| {
            app.ui.slider(cx, id).value().unwrap_or_default() as f32
        };
        PannerKnobs {
            framing: FRAMINGS[row(self, cx, ids!(ai_framing)).min(FRAMINGS.len() - 1)].into(),
            lock_pitch: self.ui.check_box(cx, ids!(ai_lock_pitch)).active(cx),
            cluster_mode: CLUSTER_MODES
                [row(self, cx, ids!(ai_cluster_mode)).min(CLUSTER_MODES.len() - 1)]
            .into(),
            cluster_bandwidth: value(self, cx, ids!(ai_bandwidth)),
            dead_zone: value(self, cx, ids!(ai_dead_zone)),
            ball_weight: value(self, cx, ids!(ai_ball_weight)),
            fov_tight: value(self, cx, ids!(ai_fov_tight)),
            fov_default: value(self, cx, ids!(ai_fov_default)),
            fov_wide: value(self, cx, ids!(ai_fov_wide)),
        }
    }

    /// The knobs into their rows.
    fn show_ai_knobs(&mut self, cx: &mut Cx, knobs: &PannerKnobs) {
        self.ui
            .drop_down(cx, ids!(ai_framing))
            .set_selected_item(cx, row_of(&FRAMINGS, &knobs.framing));
        self.ui
            .check_box(cx, ids!(ai_lock_pitch))
            .set_active(cx, knobs.lock_pitch, Animate::No);
        self.ui
            .drop_down(cx, ids!(ai_cluster_mode))
            .set_selected_item(cx, row_of(&CLUSTER_MODES, &knobs.cluster_mode));
        let values = [
            knobs.ball_weight,
            knobs.cluster_bandwidth,
            knobs.dead_zone,
            knobs.fov_tight,
            knobs.fov_default,
            knobs.fov_wide,
        ];
        for ((slider, label, text), value) in knob_sliders().into_iter().zip(values) {
            let value = f64::from(value);
            self.ui.slider(cx, slider).set_value(cx, value);
            self.set_label(cx, label, &text(value));
        }
    }

    /// The lookahead's value, its track and the line under it.
    fn show_lookahead(&mut self, cx: &mut Cx, secs: f64) {
        let secs = rounded_lookahead(secs);
        self.set_label(cx, ids!(ai_lookahead_value), &lookahead_text(secs));
        let zone = self.ai_zones.map_or(0, |zones| zone_index(secs, zones));
        if let Some(mut track) = self
            .ui
            .widget(cx, ids!(ai_lookahead_zones))
            .borrow_mut::<RecoZones>()
        {
            track.set_track(cx, secs / MAX_LOOKAHEAD, zone_ends(self.ai_zones), zone);
        }
        let note = lookahead_note(secs, self.ai_zones);
        let (text, warns) = note.clone().unwrap_or_default();
        self.set_label(cx, ids!(ai_lookahead_note), &text);
        self.set_label(cx, ids!(ai_lookahead_warning), &text);
        self.set_visible(cx, ids!(ai_lookahead_notes), note.is_some());
        self.set_visible(cx, ids!(ai_lookahead_note), !warns);
        self.set_visible(cx, ids!(ai_lookahead_warning), warns);
    }

    /// Fill the AI rows from the settings and the open match (the sheet is
    /// opening).
    pub(crate) fn show_ai_sheet(&mut self, cx: &mut Cx) {
        let saved = self.settings.clone();
        let on = saved.ai_enabled && self.ai_available();
        self.ui
            .check_box(cx, ids!(ai_enable))
            .set_active(cx, on, Animate::No);
        let model = saved
            .ai_model_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.ui.text_input(cx, ids!(ai_model)).set_text(cx, &model);
        self.ui
            .drop_down(cx, ids!(ai_mode))
            .set_selected_item(cx, row_of(&MODES, &saved.ai_mode));
        self.ui
            .drop_down(cx, ids!(ai_interval))
            .set_selected_item(cx, interval_row(saved.ai_interval));
        self.ui
            .drop_down(cx, ids!(ai_preset))
            .set_selected_item(cx, row_of(&PRESETS, &saved.ai_preset));
        // The Advanced tier follows the preset; the framing and the tilt are
        // remembered.
        let knobs = PannerKnobs {
            framing: saved.ai_framing.clone(),
            lock_pitch: saved.ai_lock_pitch,
            ..PannerKnobs::of_preset(&saved.ai_preset)
        };
        self.show_ai_knobs(cx, &knobs);
        // The lookahead's zones: the GPU's memory at open, and the source's
        // size and rate.
        self.ai_zones = self
            .live
            .as_ref()
            .and_then(|l| l.info.as_ref())
            .and_then(|info| ai::lookahead_zones(info.vram, (info.width, info.height), info.fps));
        let lookahead = ai::fitted_lookahead(saved.ai_lookahead, self.ai_zones);
        self.ui
            .slider(cx, ids!(ai_lookahead))
            .set_value(cx, lookahead);
        self.show_lookahead(cx, lookahead);
        self.show_ai_availability(cx);
    }

    /// The AI rows' controls.
    pub(crate) fn ai_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self
            .ui
            .check_box(cx, ids!(ai_enable))
            .changed(actions)
            .is_some()
        {
            // Makepad's checkbox takes clicks while it looks disabled.
            if !self.ai_available() {
                self.ui
                    .check_box(cx, ids!(ai_enable))
                    .set_active(cx, false, Animate::No);
            }
            self.show_ai_rows(cx);
        }
        if self.ui.button(cx, ids!(ai_model_browse)).clicked(actions) {
            self.pick(cx, Pick::ExportModel);
        }
        let model_typed = self
            .ui
            .text_input(cx, ids!(ai_model))
            .changed(actions)
            .is_some();
        let mode_chosen = self
            .ui
            .drop_down(cx, ids!(ai_mode))
            .changed(actions)
            .is_some();
        if model_typed || mode_chosen {
            self.show_ai_problem(cx);
        }
        if let Some(row) = self.ui.drop_down(cx, ids!(ai_preset)).changed(actions) {
            let knobs = PannerKnobs::of_preset(PRESETS[row.min(PRESETS.len() - 1)]);
            self.show_ai_knobs(cx, &knobs);
        }
        for (slider, label, text) in knob_sliders() {
            let slider = self.ui.slider(cx, slider);
            if let Some(value) = slider.slided(actions).or(slider.end_slide(actions)) {
                self.set_label(cx, label, &text(value));
            }
        }
        let lookahead = self.ui.slider(cx, ids!(ai_lookahead));
        if let Some(secs) = lookahead.slided(actions).or(lookahead.end_slide(actions)) {
            self.show_lookahead(cx, secs);
        }
    }

    /// Choose…'s answer: the sheet's model, and the default from now on
    /// (Preferences shows it).
    pub(crate) fn export_model_picked(&mut self, cx: &mut Cx, path: &Path) {
        self.ui
            .text_input(cx, ids!(ai_model))
            .set_text(cx, &path.display().to_string());
        self.settings.ai_model_path = Some(path.to_path_buf());
        self.save_settings();
        self.show_ai_problem(cx);
    }

    /// The sheet's AI choices are the next export's: kept with the others,
    /// and a usable model typed in becomes the default (Sweep takes any
    /// text there; Preferences wouldn't).
    pub(crate) fn remember_ai(&mut self, tracking: Option<&Tracking>) {
        self.settings.ai_enabled = tracking.is_some();
        let Some(tracking) = tracking else {
            return;
        };
        self.settings.ai_mode = tracking.mode.clone();
        self.settings.ai_interval = tracking.interval;
        self.settings.ai_preset = tracking.preset.clone();
        self.settings.ai_framing = tracking.knobs.framing.clone();
        self.settings.ai_lock_pitch = tracking.knobs.lock_pitch;
        self.settings.ai_lookahead = tracking.lookahead_secs;
        if let Some(model) = tracking.usable_model() {
            self.settings.ai_model_path = Some(model.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reco_app::ai::LookaheadZones;

    #[test]
    fn the_lookahead_reads_in_seconds() {
        assert_eq!(lookahead_text(2.5), "2.5 s");
        assert_eq!(lookahead_text(0.04), "Off");
        assert_eq!(rounded_lookahead(1.26), 1.3);
        assert_eq!(rounded_lookahead(3.0), 2.5);
        assert_eq!(rounded_lookahead(-1.0), 0.0);
    }

    #[test]
    fn the_note_says_whether_the_lookahead_fits() {
        let zones = Some(LookaheadZones {
            safe: 1.2,
            max: 2.0,
        });
        assert_eq!(
            lookahead_note(1.0, zones),
            Some(("Fits this GPU's memory.".to_string(), false))
        );
        assert_eq!(
            lookahead_note(1.5, zones),
            Some((
                "Tight: past 1.2 s this GPU's memory may run short.".to_string(),
                true
            ))
        );
        assert_eq!(
            lookahead_note(2.5, zones),
            Some((
                "Too long: this GPU's memory holds about 2.0 s.".to_string(),
                true
            ))
        );
        assert_eq!(lookahead_note(2.5, None), None, "no reading, no note");
    }

    #[test]
    fn a_lookahead_sits_in_one_zone() {
        let zones = LookaheadZones {
            safe: 1.2,
            max: 2.0,
        };
        assert_eq!(zone_index(1.2, zones), 0, "the safe end is still safe");
        assert_eq!(zone_index(1.3, zones), 1);
        assert_eq!(zone_index(2.0, zones), 1);
        assert_eq!(zone_index(2.1, zones), 2);
    }

    #[test]
    fn the_zones_end_where_the_track_says() {
        let zones = LookaheadZones {
            safe: 1.0,
            max: 2.0,
        };
        assert_eq!(zone_ends(Some(zones)), Some((0.4, 0.8)));
        assert_eq!(zone_ends(None), None);
    }

    #[test]
    fn names_find_their_rows() {
        assert_eq!(row_of(&reco_app::ai::MODES, "sweep"), 2);
        assert_eq!(row_of(&reco_app::ai::MODES, "nonsense"), 0);
        assert_eq!(interval_row(15), 4);
        assert_eq!(interval_row(7), 4, "an interval not listed shows 15");
    }

    #[test]
    fn knobs_read_as_numbers() {
        assert_eq!(knob_text(0.6), "0.60");
        assert_eq!(degrees_text(22.0), "22°");
    }
}
