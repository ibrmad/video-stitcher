//! Export in the App: the sheet (its fields, rules and the Save dialog), the
//! job on its own thread, its card over the picture, and the notice when it
//! ends. The preview pauses for an export and stays where it was; closing
//! it would drop the Adjust panel's unsaved changes.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::ai::{self, Tracking};
use reco_app::export::{
    self, ExportEvent, ExportJob, ExportOptions, ExportRange, MatchCalibration, QUALITIES,
    RESOLUTIONS,
};
use reco_app::preview::playback::PlayState;
use reco_app::preview::worker::PreviewCommand;
use reco_app::telemetry::UsageEvent;
use reco_app::toasts::Severity;

use crate::export_text::{
    grouped, percent, progress_detail, size_label, starting_line, time_left, tracking_note,
};
use crate::project_view::Pick;
use crate::time_ruler::clock;
use crate::ui::preview::RecoPreview;
use crate::value_text::Reading;
use crate::{live, App};

/// An export, from the click on Export to its end.
pub(crate) struct Exporting {
    /// What it writes (the blend and colour arrive with the snapshot).
    options: ExportOptions,
    /// The job, once the worker sent the tuned calibration.
    job: Option<ExportJob>,
    /// Whether AI tracking started, once the job says.
    tracking: Option<Result<(), String>>,
}

/// The encoders, probed once off the UI thread.
pub(crate) struct CodecProbe(Receiver<Vec<String>>);

impl CodecProbe {
    pub(crate) fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("reco-codecs".into())
            .spawn(move || {
                if tx.send(export::available_codecs()).is_ok() {
                    SignalToUI::set_ui_signal();
                }
            });
        if let Err(e) = spawned {
            log!("export codecs: couldn't probe ({e}); offering H.264");
        }
        Self(rx)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Seconds for a range slider at `fraction` of a `length`-second match:
/// whole seconds, and all of it at the right end.
fn slider_seconds(fraction: f64, length: f64) -> f64 {
    if fraction >= 1.0 {
        length
    } else {
        (fraction * length).round().min(length)
    }
}

impl App {
    /// The codecs, once probed.
    pub(crate) fn collect_codecs(&mut self, cx: &mut Cx) {
        let Some(codecs) = self.codec_probe.as_ref().and_then(|p| p.0.try_recv().ok()) else {
            return;
        };
        self.codec_probe = None;
        log!("export codecs: {}", codecs.join(", "));
        let shown = self.shown_codec(cx);
        self.export_codecs = codecs;
        self.show_codecs(cx, shown);
    }

    /// The codec the sheet shows, once it has been filled in: it stays
    /// when the list changes.
    fn shown_codec(&mut self, cx: &mut Cx) -> Option<String> {
        if !self.export_sheet_filled {
            return None;
        }
        let row = self.ui.drop_down(cx, ids!(export_codec)).selected_item();
        self.codecs().get(row).cloned()
    }

    /// The codecs to offer: the probed ones (H.264 until they are known).
    pub(crate) fn codecs(&self) -> Vec<String> {
        if self.export_codecs.is_empty() {
            vec!["h264".into()]
        } else {
            self.export_codecs.clone()
        }
    }

    /// The codecs into the sheet, `shown` chosen (the saved one when
    /// `None`).
    fn show_codecs(&mut self, cx: &mut Cx, shown: Option<String>) {
        let codecs = self.codecs();
        let wanted = shown.unwrap_or_else(|| self.settings.export_codec.clone());
        let dropdown = self.ui.drop_down(cx, ids!(export_codec));
        dropdown.set_labels(cx, codecs.iter().map(|c| export::codec_label(c)).collect());
        let row = codecs.iter().position(|c| *c == wanted);
        dropdown.set_selected_item(cx, row.unwrap_or(0));
    }

    /// Whether an export is starting or running.
    pub(crate) fn exporting(&self) -> bool {
        self.export.is_some()
    }

    /// The range for the open match, `length` seconds long: the same match
    /// keeps its range (a new sync offset changes the length); a new one
    /// starts with all of it, or the command line's `--export-range`.
    pub(crate) fn fit_export_range(&mut self, length: f64) {
        let first_left = self
            .live
            .as_ref()
            .and_then(|l| l.files.left.first().cloned());
        let range = match self.export_range {
            Some(range) if self.export_range_for == first_left => range.refit(length),
            _ => match self.args.export_range {
                Some((start, end)) => ExportRange::new(start, end, length),
                None => ExportRange::whole(length),
            },
        };
        self.export_range = Some(range);
        self.export_range_for = first_left;
    }

    /// The open match's length: the lanes' (exact, after the sync offset)
    /// once measured, else the first open's.
    fn match_length(&self) -> Option<f64> {
        let live = self.live.as_ref()?;
        match live.lanes.as_ref() {
            Some(lanes) => Some(lanes.length),
            None => live.info.as_ref().map(live::length_secs),
        }
    }

    /// The range to tint on the ruler: only part of the match.
    pub(crate) fn ruler_export_range(&self) -> Option<(f64, f64)> {
        self.export_range
            .filter(|r| !r.is_whole())
            .map(|r| (r.start(), r.end()))
    }

    /// Open the sheet, filled from the settings and the open match.
    pub(crate) fn open_export_sheet(&mut self, cx: &mut Cx) {
        let Some(length) = self.match_length() else {
            return;
        };
        let Some(first_left) = self
            .live
            .as_ref()
            .and_then(|l| l.files.left.first().cloned())
        else {
            return;
        };
        self.fit_export_range(length);
        // A new match gets its own file name.
        if self.export_named_for.as_ref() != Some(&first_left) {
            let default = export::default_output(&first_left);
            self.ui
                .text_input(cx, ids!(export_output))
                .set_text(cx, &default.display().to_string());
            self.export_named_for = Some(first_left);
        }
        // The saved choices the first time; after that the sheet keeps what
        // was chosen, exported or not (DESIGN.md Rule 9).
        if !self.export_sheet_filled {
            self.fill_export_choices(cx);
            self.export_sheet_filled = true;
        }
        self.show_export_error(cx, None);
        self.refresh_ai_rows(cx);
        self.show_export_range(cx);
        self.ui.modal(cx, ids!(export_sheet)).open(cx);
    }

    /// The saved size, codec, quality, extras and AI choices into the sheet.
    fn fill_export_choices(&mut self, cx: &mut Cx) {
        let size = self.ui.drop_down(cx, ids!(export_size));
        size.set_labels(
            cx,
            RESOLUTIONS
                .iter()
                .map(|(name, w, h)| size_label(name, *w, *h))
                .collect(),
        );
        size.set_selected_item(cx, export::size_index(&self.settings.export_size));
        self.show_codecs(cx, None);
        let quality = QUALITIES
            .iter()
            .position(|q| *q == self.settings.export_quality)
            .unwrap_or(1);
        self.ui
            .drop_down(cx, ids!(export_quality))
            .set_selected_item(cx, quality);
        let (replay, events) = (self.settings.export_replay, self.settings.export_events);
        self.ui
            .check_box(cx, ids!(export_replay))
            .set_active(cx, replay, Animate::No);
        self.ui
            .check_box(cx, ids!(export_events))
            .set_active(cx, events, Animate::No);
        self.fill_ai_choices(cx);
    }

    /// The range into the sliders, the time fields and the length line,
    /// and onto the ruler; Export follows.
    fn show_export_range(&mut self, cx: &mut Cx) {
        let Some(range) = self.export_range else {
            return;
        };
        let (from, to) = range.fractions();
        self.ui.slider(cx, ids!(range_start)).set_value(cx, from);
        self.ui.slider(cx, ids!(range_end)).set_value(cx, to);
        self.set_label(cx, ids!(range_start_text), &clock(range.start()));
        self.set_label(cx, ids!(range_end_text), &clock(range.end()));
        let length = format!("{} of {}", clock(range.duration()), clock(range.length()));
        self.set_label(cx, ids!(range_length), &length);
        self.set_visible(cx, ids!(range_length), !range.is_empty());
        self.set_visible(cx, ids!(range_empty), range.is_empty());
        self.show_export_enabled(cx);
        self.update_ruler(cx);
    }

    /// Export needs a file named, something to export, and AI tracking
    /// that can run as chosen.
    pub(crate) fn show_export_enabled(&mut self, cx: &mut Cx) {
        let named = !self
            .ui
            .text_input(cx, ids!(export_output))
            .text()
            .trim()
            .is_empty();
        let something = self.export_range.is_some_and(|r| !r.is_empty());
        let tracks = !self.ai_blocks_export(cx);
        self.set_button_enabled(cx, ids!(sheet_export), named && something && tracks);
    }

    fn show_export_error(&mut self, cx: &mut Cx, error: Option<&str>) {
        self.set_label(cx, ids!(export_error_text), error.unwrap_or(""));
        self.set_visible(cx, ids!(export_error), error.is_some());
    }

    /// The Save dialog's answer.
    pub(crate) fn export_file_picked(&mut self, cx: &mut Cx, path: PathBuf) {
        self.ui
            .text_input(cx, ids!(export_output))
            .set_text(cx, &export::with_mp4(&path).display().to_string());
        self.show_export_error(cx, None);
        self.show_export_enabled(cx);
    }

    /// Export in the top bar, the sheet's controls, and the card's Cancel.
    pub(crate) fn export_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(export_button)).clicked(actions) && !self.exporting() {
            self.open_export_sheet(cx);
        }
        if self.ui.button(cx, ids!(export_browse)).clicked(actions) {
            self.pick(cx, Pick::Export);
        }
        if self
            .ui
            .text_input(cx, ids!(export_output))
            .changed(actions)
            .is_some()
        {
            self.show_export_error(cx, None);
            self.show_export_enabled(cx);
        }
        let length = self.export_range.map_or(0.0, |r| r.length());
        for (slider, start) in [(ids!(range_start), true), (ids!(range_end), false)] {
            let slider = self.ui.slider(cx, slider);
            if let Some(fraction) = slider.slided(actions).or(slider.end_slide(actions)) {
                if let Some(range) = self.export_range.as_mut() {
                    let seconds = slider_seconds(fraction, length);
                    if start {
                        range.set_start(seconds);
                    } else {
                        range.set_end(seconds);
                    }
                }
                self.show_export_range(cx);
            }
        }
        // A time applies on Return or when the field loses the keyboard (a
        // click anywhere else, Export included, takes it); ↑/↓ step seconds.
        for (field, start) in [
            (ids!(range_start_text), true),
            (ids!(range_end_text), false),
        ] {
            if let Some(input) = self.field_input(cx, actions, field) {
                if let Some(range) = self.export_range.as_mut() {
                    // The shown time is whole seconds: left as it was, it
                    // would cut a fraction off the end.
                    let shown = if start { range.start() } else { range.end() };
                    match input.value(Reading::Clock, shown) {
                        Some(seconds) if start => range.set_start(seconds),
                        Some(seconds) => range.set_end(seconds),
                        None => {}
                    }
                }
                // Also puts back a time that didn't read.
                self.show_export_range(cx);
            }
        }
        if self.ui.button(cx, ids!(sheet_cancel)).clicked(actions) {
            self.ui.modal(cx, ids!(export_sheet)).close(cx);
        }
        if self.ui.button(cx, ids!(sheet_export)).clicked(actions) {
            self.begin_export(cx);
        }
        if self.ui.button(cx, ids!(export_cancel)).clicked(actions) {
            self.cancel_export(cx);
        }
    }

    /// Check the sheet, remember its choices, pause the preview and ask
    /// the worker for the tuned calibration; the job starts with it.
    pub(crate) fn begin_export(&mut self, cx: &mut Cx) {
        let Some(range) = self.export_range.filter(|r| !r.is_empty()) else {
            return;
        };
        let Some(live) = self.live.as_ref().filter(|l| l.open) else {
            return;
        };
        let Some(fps) = live.info.as_ref().map(|i| i.fps) else {
            return;
        };
        let files = live.files.clone();
        let Some(first_left) = files.left.first() else {
            return;
        };
        let inputs: Vec<PathBuf> = files.left.iter().chain(&files.right).cloned().collect();
        let typed = self.ui.text_input(cx, ids!(export_output)).text();
        let output = match export::checked_output(&typed, first_left, &inputs) {
            Ok(path) => path,
            Err(why) => return self.show_export_error(cx, Some(&why)),
        };
        let size = self
            .ui
            .drop_down(cx, ids!(export_size))
            .selected_item()
            .min(RESOLUTIONS.len() - 1);
        let codec = self
            .codecs()
            .get(self.ui.drop_down(cx, ids!(export_codec)).selected_item())
            .cloned()
            .unwrap_or_else(|| "h264".into());
        let quality = QUALITIES[self
            .ui
            .drop_down(cx, ids!(export_quality))
            .selected_item()
            .min(QUALITIES.len() - 1)];
        let replay = self.ui.check_box(cx, ids!(export_replay)).active(cx);
        let events = self.ui.check_box(cx, ids!(export_events)).active(cx);
        let tracking = self.sheet_tracking(cx);
        if let Some(problem) = tracking.as_ref().and_then(Tracking::problem) {
            return self.show_export_error(cx, Some(&problem));
        }
        // The choices are the next export's.
        let (name, width, height) = RESOLUTIONS[size];
        self.settings.export_size = name.into();
        self.settings.export_codec = codec.clone();
        self.settings.export_quality = quality.into();
        self.settings.export_replay = replay;
        self.settings.export_events = events;
        self.remember_ai(tracking.as_ref());
        self.save_settings();
        self.ui.modal(cx, ids!(export_sheet)).close(cx);

        // The export needs the machine: the preview pauses where it is,
        // and a recording ends.
        if let Some(live) = self.live.as_ref() {
            if live.state == PlayState::Playing {
                live.worker.send(PreviewCommand::TogglePlay);
            }
            if live.recording.is_some() {
                live.worker.send(PreviewCommand::StopRecording);
            }
        }
        log!(
            "export: {} at {width}x{height}, {codec} {quality}, {:.1}-{:.1} s",
            output.display(),
            range.start(),
            range.end()
        );
        self.export = Some(Exporting {
            options: ExportOptions {
                output,
                size: (width, height),
                codec,
                quality: quality.into(),
                range: (range.start(), range.end()),
                fps,
                blend: 0.0,
                color_match: true,
                replay,
                events,
                tracking: tracking.clone(),
                whole_field: None,
            },
            job: None,
            tracking: None,
        });
        self.send_preview(PreviewCommand::Snapshot);
        self.set_button_enabled(cx, ids!(export_cancel), true);
        self.show_export_card(cx, &starting_line(range.start()), 0.0, "");
        let starting = tracking.is_some().then_some("AI tracking: starting…");
        self.show_tracking_line(cx, starting);
        self.lock_transport(cx, true);
        self.apply_shell(cx);
    }

    /// The tuned calibration arrived: start the job with it.
    pub(crate) fn export_snapshot(&mut self, calibration: MatchCalibration, color_match: bool) {
        let Some(files) = self.live.as_ref().map(|l| l.files.clone()) else {
            return;
        };
        let Some(export) = self.export.as_mut().filter(|e| e.job.is_none()) else {
            return;
        };
        export.options.blend = calibration.blend_width;
        export.options.color_match = color_match;
        export.job = Some(ExportJob::start(
            files.left_input(),
            files.right_input(),
            calibration,
            export.options.clone(),
            Arc::new(SignalToUI::set_ui_signal),
        ));
    }

    /// The preview failed before the export could start.
    pub(crate) fn export_lost_preview(&mut self, cx: &mut Cx) {
        if self.export.as_ref().is_some_and(|e| e.job.is_none()) {
            self.end_export(cx);
            self.toast(
                cx,
                Severity::Error,
                "Export didn't start",
                "The videos closed before it could begin.",
            );
        }
    }

    /// The job's progress and outcome.
    pub(crate) fn drain_export(&mut self, cx: &mut Cx) {
        while let Some(event) = self
            .export
            .as_ref()
            .and_then(|e| e.job.as_ref())
            .and_then(ExportJob::try_event)
        {
            let output = self
                .export
                .as_ref()
                .map(|e| export::with_mp4(&e.options.output))
                .unwrap_or_default();
            let (codec, fps) = self
                .export
                .as_ref()
                .map(|e| (e.options.codec.clone(), e.options.fps))
                .unwrap_or_default();
            let (asked, tracked) = self
                .export
                .as_ref()
                .map(|e| (e.options.tracking.is_some(), e.tracking.clone()))
                .unwrap_or_default();
            match event {
                ExportEvent::Progress { frames, total, fps } => {
                    let fraction = if total > 0 {
                        frames as f64 / total as f64
                    } else {
                        0.0
                    };
                    let detail = progress_detail(frames, total, fps);
                    self.show_export_card(cx, &detail, fraction, &time_left(frames, total, fps));
                    let status = format!("Exporting · {}%", percent(frames, total));
                    self.set_label(cx, ids!(status_text), &status);
                }
                ExportEvent::Finalizing => {
                    self.show_export_card(cx, "Finishing the file…", 1.0, "")
                }
                ExportEvent::Done {
                    path,
                    frames,
                    seconds,
                } => {
                    log!("export: done, {frames} frames to {}", path.display());
                    self.end_export(cx);
                    self.auto_export_ended(cx, "done");
                    let duration_secs = if fps > 0.0 { frames as f64 / fps } else { 0.0 };
                    self.send_usage(
                        cx,
                        UsageEvent::ExportComplete {
                            frames,
                            duration_secs,
                            codec,
                        },
                    );
                    // Show in folder reveals it, as after a recording.
                    if let Some(live) = self.live.as_mut() {
                        live.last_output = Some(path.clone());
                    }
                    self.set_visible(cx, ids!(show_in_folder), true);
                    let body = format!(
                        "{} · {} frames in {}{}",
                        file_name(&path),
                        grouped(frames),
                        clock(seconds),
                        tracking_note(asked, tracked.as_ref())
                    );
                    self.toast_for(
                        cx,
                        Severity::Info,
                        "Export complete",
                        &body,
                        Duration::from_secs(8),
                    );
                }
                ExportEvent::Failed(why) => {
                    log!("export: failed: {why}");
                    self.end_export(cx);
                    self.auto_export_ended(cx, "failed");
                    self.send_usage(
                        cx,
                        UsageEvent::ExportError {
                            error: why.clone(),
                            codec,
                        },
                    );
                    self.toast(cx, Severity::Error, "Export failed", &why);
                }
                ExportEvent::AiFigures(figures) => self.show_ai_figures(cx, &figures),
                ExportEvent::Figures(figures) => self.show_export_figures(cx, &figures),
                ExportEvent::Tracking(status) => {
                    match &status {
                        Ok(()) => log!("export: AI tracking active"),
                        Err(why) => log!("export: AI tracking not active: {why}"),
                    }
                    self.show_tracking_line(cx, Some(&ai::tracking_line(&status)));
                    if let Some(export) = self.export.as_mut() {
                        export.tracking = Some(status);
                    }
                }
                ExportEvent::Cancelled => {
                    log!("export: cancelled");
                    self.end_export(cx);
                    self.auto_export_ended(cx, "cancelled");
                    let body = if output.exists() {
                        format!("The part written so far is in {}.", file_name(&output))
                    } else {
                        "Nothing was written.".to_string()
                    };
                    self.toast(cx, Severity::Info, "Export cancelled", &body);
                }
            }
        }
    }

    /// Stop the export at its next frame (before the job starts, at once).
    fn cancel_export(&mut self, cx: &mut Cx) {
        let Some(export) = self.export.as_ref() else {
            return;
        };
        match export.job.as_ref() {
            Some(job) => {
                job.cancel();
                self.set_button_enabled(cx, ids!(export_cancel), false);
                self.set_label(cx, ids!(export_detail), "Cancelling…");
                self.set_label(cx, ids!(export_eta), "");
            }
            None => {
                log!("export: cancelled");
                self.end_export(cx);
                self.toast(
                    cx,
                    Severity::Info,
                    "Export cancelled",
                    "Nothing was written.",
                );
            }
        }
    }

    /// The card over the picture: what is written, how far, how long.
    fn show_export_card(&mut self, cx: &mut Cx, detail: &str, fraction: f64, eta: &str) {
        let name = self
            .export
            .as_ref()
            .map(|e| file_name(&e.options.output))
            .unwrap_or_default();
        self.set_visible(cx, ids!(export_card), true);
        self.set_label(cx, ids!(export_title), &format!("Exporting {name}"));
        self.set_label(cx, ids!(export_detail), detail);
        self.ui
            .progress_bar(cx, ids!(export_bar))
            .set_value(cx, fraction);
        self.set_label(cx, ids!(export_eta), eta);
    }

    /// The card's AI tracking line (hidden without tracking).
    fn show_tracking_line(&mut self, cx: &mut Cx, line: Option<&str>) {
        self.set_label(cx, ids!(export_tracking), line.unwrap_or(""));
        self.set_visible(cx, ids!(export_tracking), line.is_some());
    }

    /// The export is over: the card goes and the preview's controls come
    /// back.
    fn end_export(&mut self, cx: &mut Cx) {
        self.export = None;
        self.set_visible(cx, ids!(export_card), false);
        self.lock_transport(cx, false);
        self.refresh_status(cx);
        self.apply_shell(cx);
    }

    /// Keep playback still (keys, the ruler) while exporting.
    fn lock_transport(&mut self, cx: &mut Cx, locked: bool) {
        if let Some(mut preview) = self
            .ui
            .widget(cx, ids!(preview))
            .borrow_mut::<RecoPreview>()
        {
            preview.lock_transport(locked);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_sliders_give_whole_seconds_and_reach_the_end() {
        assert_eq!(slider_seconds(0.0, 60.4), 0.0);
        assert_eq!(slider_seconds(0.5, 60.4), 30.0);
        assert_eq!(slider_seconds(0.999, 60.4), 60.0);
        assert_eq!(
            slider_seconds(1.0, 60.4),
            60.4,
            "all of it at the right end"
        );
    }
}
