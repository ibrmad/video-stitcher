//! Calibration in the App: Auto-calibrate and Recalibrate run
//! `reco_app::calibrate`'s job, its steps show in the Setup panel, the
//! next-step card and the status line, Cancel stops it, and the saved file
//! becomes the project's calibration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::calibrate::{
    CalibrationDone, CalibrationEvent, CalibrationJob, CalibrationOptions, KeptLens, FRAME_CHOICES,
    LOW_CONFIDENCE,
};
use reco_app::project::Camera;
use reco_app::telemetry::UsageEvent;
use reco_app::toasts::Severity;

use crate::value_text::Reading;
use crate::App;

/// Where a calibration stands while it runs.
#[derive(Clone, Debug, Default)]
pub(crate) struct Calibrating {
    /// The first files it calibrates (a change to them cancels it).
    pub files: (PathBuf, PathBuf),
    /// The last step reported: number, of, words.
    pub step: Option<(usize, usize, String)>,
    /// Cancel was asked for.
    pub cancelling: bool,
}

/// An Advanced slider, its value field, and how the value reads.
type ValueRow<'a> = (&'a [LiveId], &'a [LiveId], Reading);

/// A calibration failure in plain words: the common one says what to do.
pub(crate) fn failure_words(reason: &str) -> String {
    if reason.contains("no usable frame pairs") {
        "Too few pitch markings were in view of both cameras. Try again at a moment when more of the pitch is visible, or load a calibration file.".to_string()
    } else {
        let mut chars = reason.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl App {
    /// The Advanced options start at the calibration's defaults.
    pub(crate) fn show_calibration_defaults(&mut self, cx: &mut Cx) {
        let frames = CalibrationOptions::default().frames;
        let index = FRAME_CHOICES.iter().position(|f| *f == frames).unwrap_or(0);
        self.ui
            .drop_down(cx, ids!(cal_frames))
            .set_selected_item(cx, index);
    }

    /// The Advanced options as set in the Setup panel.
    fn calibration_options(&self, cx: &mut Cx) -> CalibrationOptions {
        let slider =
            |id: &[LiveId], default: f64| self.ui.slider(cx, id).value().unwrap_or(default);
        let defaults = CalibrationOptions::default();
        CalibrationOptions {
            frames: FRAME_CHOICES
                .get(self.ui.drop_down(cx, ids!(cal_frames)).selected_item())
                .copied()
                .unwrap_or(defaults.frames),
            imu_seeds: self.ui.check_box(cx, ids!(cal_imu)).active(cx),
            akaze_threshold: slider(ids!(cal_akaze), defaults.akaze_threshold),
            detect_y: (
                slider(ids!(cal_y_min), defaults.detect_y.0),
                slider(ids!(cal_y_max), defaults.detect_y.1),
            ),
            skip_start: self.calibration_start(),
            skip_end: slider(ids!(cal_skip_end), defaults.skip_end),
            blend: defaults.blend,
        }
    }

    /// Where a recalibration starts: the preview's time, when both cameras'
    /// first files reach it (as the Slint app did); else the start.
    fn calibration_start(&self) -> f64 {
        let Some((frame, fps)) = self
            .live
            .as_ref()
            .filter(|l| l.open)
            .and_then(|l| l.info.as_ref().map(|i| (l.frame, i.fps)))
        else {
            return 0.0;
        };
        if fps <= 0.0 {
            return 0.0;
        }
        let now = frame.saturating_sub(1) as f64 / fps;
        let first_files = [Camera::Left, Camera::Right].map(|c| {
            self.project
                .files(c)
                .first()
                .and_then(|f| self.durations.as_ref()?.duration(f))
                .unwrap_or(0.0)
        });
        if first_files.iter().all(|secs| now < *secs) {
            now
        } else {
            0.0
        }
    }

    /// Auto-calibrate (or recalibrate) the cameras' first files.
    pub(crate) fn start_calibration(&mut self, cx: &mut Cx) {
        if self.calibration_job.is_some() {
            return;
        }
        let (Some(left), Some(right), Some(save_to)) = (
            self.project.files(Camera::Left).first().cloned(),
            self.project.files(Camera::Right).first().cloned(),
            self.project.sibling_calibration(),
        ) else {
            return;
        };
        let options = self.calibration_options(cx);
        log!(
            "calibrate: {} frames, skip {:.1} s to -{:.0} s, imu={}, akaze={}, detect y {:.2}-{:.2}, blend {:.2}",
            options.frames,
            options.skip_start,
            options.skip_end,
            options.imu_seeds,
            options.akaze_threshold,
            options.detect_y.0,
            options.detect_y.1,
            options.blend
        );
        // The lenses in use stay, saved or not (a picked profile,
        // fine-tuning).
        let in_use = self
            .latest_values
            .as_ref()
            .filter(|v| v.lens_changed)
            .map(|v| (v.left_lens, v.right_lens));
        let kept = self
            .project
            .calibration
            .clone()
            .map(|file| KeptLens { file, in_use });
        self.calibration_job = Some(CalibrationJob::start(
            left.clone(),
            right.clone(),
            save_to,
            kept,
            options,
            Arc::new(SignalToUI::set_ui_signal),
        ));
        self.calibrating = Some(Calibrating {
            files: (left, right),
            ..Calibrating::default()
        });
        self.calibration_failure = None;
        self.show_project(cx);
    }

    /// Stop a running calibration (it reports when it has stopped).
    pub(crate) fn cancel_calibration(&mut self, cx: &mut Cx) {
        if let (Some(job), Some(state)) = (&self.calibration_job, self.calibrating.as_mut()) {
            job.cancel();
            state.cancelling = true;
            self.show_project(cx);
        }
    }

    /// A camera's first file changed under a running calibration: stop it.
    pub(crate) fn cancel_stale_calibration(&mut self, cx: &mut Cx) {
        let current = (
            self.project.files(Camera::Left).first().cloned(),
            self.project.files(Camera::Right).first().cloned(),
        );
        let stale = self
            .calibrating
            .as_ref()
            .is_some_and(|c| (Some(c.files.0.clone()), Some(c.files.1.clone())) != current);
        if stale && self.calibration_job.take().is_some() {
            self.calibrating = None;
            self.toast(
                cx,
                Severity::Info,
                "Calibration stopped",
                "The cameras' first videos changed.",
            );
        }
    }

    /// The calibration's news.
    pub(crate) fn drain_calibration(&mut self, cx: &mut Cx) {
        while let Some(event) = self.calibration_job.as_ref().and_then(|j| j.try_event()) {
            match event {
                CalibrationEvent::Progress { step, of, text } => {
                    if let Some(state) = self.calibrating.as_mut() {
                        state.step = Some((step, of, text));
                    }
                    self.show_project(cx);
                }
                CalibrationEvent::Done(done) => self.calibrated(cx, done),
                CalibrationEvent::Failed(reason) => {
                    self.end_calibration();
                    log!("calibration failed: {reason}");
                    self.send_usage(
                        cx,
                        UsageEvent::CalibrationError {
                            error: reason.clone(),
                        },
                    );
                    self.toast(
                        cx,
                        Severity::Error,
                        "Calibration failed",
                        &failure_words(&reason),
                    );
                    self.calibration_failure = Some(reason);
                    self.show_project(cx);
                }
                CalibrationEvent::Cancelled => {
                    self.end_calibration();
                    self.show_project(cx);
                }
            }
        }
    }

    fn end_calibration(&mut self) {
        self.calibration_job = None;
        self.calibrating = None;
    }

    /// Calibrated: the saved file becomes the calibration, the preview
    /// opens (or reopens with it), and the toasts say how it went.
    fn calibrated(&mut self, cx: &mut Cx, done: CalibrationDone) {
        self.end_calibration();
        self.send_usage(
            cx,
            UsageEvent::CalibrationComplete {
                confidence: done.confidence,
                matches: done.matches,
            },
        );
        log!(
            "calibrated: {} matches, confidence {:.2}, saved to {}",
            done.matches,
            done.confidence,
            done.path.display()
        );
        self.toast(
            cx,
            Severity::Info,
            "Calibrated",
            &format!(
                "{} matched points · {}",
                done.matches,
                file_name(&done.path)
            ),
        );
        if done.confidence < LOW_CONFIDENCE {
            self.toast(
                cx,
                Severity::Warn,
                "Low calibration confidence",
                &format!(
                    "{:.0}% confidence ({} matches). Try a moment with more of the pitch in view of both cameras.",
                    done.confidence * 100.0,
                    done.matches
                ),
            );
        }
        if done.fallback_lens {
            self.toast(
                cx,
                Severity::Warn,
                "Using a generic lens profile",
                "No lens profile matched these cameras, so a generic one was used. If the stitch looks wrong, pick one under Lens in the Adjust panel.",
            );
        }
        // A run that looked the lenses up names them (a generic one
        // truthfully); one that kept the old lenses leaves the lookup to the
        // reopen.
        self.show_calibration_stats(cx, done.confidence, done.matches);
        self.calibrated_lenses =
            (done.left_lens.is_some() || done.right_lens.is_some()).then(|| {
                (
                    done.path.clone(),
                    done.left_lens.clone(),
                    done.right_lens.clone(),
                )
            });
        self.offered_calibration = Some(done.path.clone());
        self.project.calibration = Some(done.path);
        self.calibration_failure = None;
        // The same file may now hold a new calibration: open it again.
        self.refresh_project(cx, true);
    }

    /// Recalibrate, Cancel, the calibration's remove button, and the
    /// Advanced options' values.
    pub(crate) fn calibration_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(auto_calibrate)).clicked(actions)
            || self.ui.button(cx, ids!(recalibrate)).clicked(actions)
        {
            self.start_calibration(cx);
        }
        // Cancel in the Setup panel, or on the next-step card.
        if self
            .ui
            .button(cx, ids!(cancel_calibration))
            .clicked(actions)
            || self.ui.button(cx, ids!(calibrate_cancel)).clicked(actions)
        {
            self.cancel_calibration(cx);
        }
        if self.ui.button(cx, ids!(clear_calibration)).clicked(actions) {
            self.project.calibration = None;
            self.calibration_failure = None;
            self.project_changed(cx);
        }
        let rows: [ValueRow; 4] = [
            (
                ids!(cal_akaze),
                ids!(cal_akaze_value),
                Reading::number(4, ""),
            ),
            (
                ids!(cal_y_min),
                ids!(cal_y_min_value),
                Reading::number(2, ""),
            ),
            (
                ids!(cal_y_max),
                ids!(cal_y_max_value),
                Reading::number(2, ""),
            ),
            (
                ids!(cal_skip_end),
                ids!(cal_skip_end_value),
                Reading::number(0, " s"),
            ),
        ];
        // Read when a calibration starts; nothing to send now.
        for (slider, field, reading) in rows {
            self.slider_input(cx, actions, slider, field, reading);
        }
    }

    /// "Calibrating · step 3 of 7" while one runs.
    pub(crate) fn calibration_status(&self) -> Option<String> {
        let state = self.calibrating.as_ref()?;
        Some(match &state.step {
            Some((step, of, _)) => format!("Calibrating · step {step} of {of}"),
            None => "Calibrating…".to_string(),
        })
    }
}
