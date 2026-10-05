//! The live session's side of the App: it starts the render worker, routes
//! its events to the preview widget and the shell, and turns transport
//! actions into commands. Split from main.rs, which keeps the shell, the
//! look preview and startup.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::preview::lanes::Lanes;
use reco_app::preview::playback::PlayState;
use reco_app::preview::recorder::Recording;
use reco_app::preview::view::PreviewAspect;
use reco_app::preview::worker::{
    PreviewCommand, PreviewConfig, PreviewEvent, PreviewInfo, PreviewWorker,
};
use reco_app::recording::{
    recording_file_name, recording_folder, recording_size, RecordingQuality,
};
use reco_app::toasts::Severity;

use crate::live::{self, Live};
use crate::time_ruler;
use crate::ui::preview::{PreviewAction, RecoPreview};
use crate::ui::time_panel::{RecoTimeRuler, RulerAction};
use crate::{cli, display_device, App, Step};

impl App {
    /// Start the render worker on the files from the command line.
    pub(crate) fn start_live(&mut self, cx: &mut Cx, files: cli::FileArgs) {
        let config = PreviewConfig {
            display_device: display_device(cx),
            force_readback: self.args.preview_readback,
        };
        let worker = PreviewWorker::spawn(config, Arc::new(SignalToUI::set_ui_signal));
        worker.send(PreviewCommand::Open {
            left: files.left_input(),
            right: files.right_input(),
            calibration: files.calibration.clone(),
        });
        if let Some(mut preview) = self
            .ui
            .widget(cx, ids!(preview))
            .borrow_mut::<RecoPreview>()
        {
            preview.attach(cx, worker.sender());
        }
        self.live = Some(Live::new(worker, files));
        self.show_opening(cx);
    }

    /// Hand the worker's events to the preview widget, then the rest here.
    pub(crate) fn drain_preview(&mut self, cx: &mut Cx) {
        while let Some(event) = self.live.as_ref().and_then(|l| l.worker.try_event()) {
            let rest = match self
                .ui
                .widget(cx, ids!(preview))
                .borrow_mut::<RecoPreview>()
            {
                Some(mut preview) => preview.on_event(cx, event),
                None => Some(event),
            };
            match rest {
                Some(PreviewEvent::Opening) => self.show_opening(cx),
                Some(PreviewEvent::Ready(info)) => self.show_live(cx, info),
                Some(PreviewEvent::Failed(message)) => self.show_failed(cx, &message),
                Some(PreviewEvent::Stopped(message)) => self.show_stopped(cx, &message),
                Some(PreviewEvent::Time { frame, state }) => self.show_time(cx, frame, state),
                Some(PreviewEvent::Lanes(lanes)) => self.show_lanes(cx, lanes),
                Some(PreviewEvent::Calibration(values)) => self.show_calibration_values(cx, values),
                Some(PreviewEvent::CalibrationSaved(path)) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    self.toast(cx, Severity::Info, "Calibration saved", &name);
                }
                Some(PreviewEvent::CalibrationSaveFailed(why)) => {
                    self.toast(cx, Severity::Error, "Couldn't save the calibration", &why)
                }
                Some(PreviewEvent::RecordingStarted { path }) => self.recording_started(cx, path),
                Some(PreviewEvent::Recorded { frames }) => self.recorded(cx, frames),
                Some(PreviewEvent::RecordingSaved(recording)) => {
                    self.recording_saved(cx, recording)
                }
                Some(PreviewEvent::RecordingFailed(reason)) => self.recording_failed(cx, &reason),
                // The widget takes frames; nothing else is left.
                Some(_) | None => {}
            }
        }
    }

    /// "Opening the videos…" while the worker opens them.
    pub(crate) fn show_opening(&mut self, cx: &mut Cx) {
        let (left, right) = self
            .live
            .as_ref()
            .map(|l| live::names(&l.files))
            .unwrap_or_default();
        self.set_label(cx, ids!(next_title), "Opening the videos…");
        self.set_label(cx, ids!(next_body), &format!("Reading {left} and {right}."));
        self.set_visible(cx, ids!(empty_state), true);
        self.set_visible(cx, ids!(next_actions), false);
        self.set_visible(cx, ids!(calibrate_progress), false);
        self.set_visible(cx, ids!(preview), false);
        self.set_visible(cx, ids!(sample_frame), false);
        self.set_step(
            cx,
            [ids!(step1_todo), ids!(step1_current), ids!(step1_done)],
            Step::Done,
        );
        self.set_step(
            cx,
            [ids!(step2_todo), ids!(step2_current), ids!(step2_done)],
            Step::Done,
        );
        self.set_step(
            cx,
            [ids!(step3_todo), ids!(step3_current), ids!(step3_done)],
            Step::Current,
        );
        self.set_label(cx, ids!(status_text), "Opening…");
        self.ui.redraw(cx);
    }

    /// The preview is open: show it and fill the panels from the files.
    fn show_live(&mut self, cx: &mut Cx, info: PreviewInfo) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        // Makepad's log: the remote log ring and the checks read this line.
        log!(
            "preview: {}x{} input, {} on {}",
            info.width,
            info.height,
            if info.zero_copy {
                "zero-copy"
            } else {
                "readback"
            },
            info.gpu
        );
        live.info = Some(info.clone());
        let status = live.status();
        self.shell.set_files_loaded(true);
        self.set_visible(cx, ids!(empty_state), false);
        self.set_visible(cx, ids!(sample_frame), false);
        self.set_visible(cx, ids!(preview), true);
        self.show_project(cx);
        self.remember_session(cx);
        self.set_visible(cx, ids!(time_display), true);
        self.set_label(
            cx,
            ids!(time_total),
            &time_ruler::clock(live::length_secs(&info)),
        );
        self.set_label(cx, ids!(status_text), &status);
        self.update_ruler(cx);
        self.apply_shell(cx);
        if self.args.toast_demo {
            self.toast_demo(cx);
        }
    }

    /// Opening failed: say so in the viewer, the worker's words split into
    /// a title and its detail, back at the first step.
    fn show_failed(&mut self, cx: &mut Cx, message: &str) {
        let (title, detail) = live::failure_text(message);
        self.show_project(cx);
        self.set_visible(cx, ids!(preview), false);
        self.set_visible(cx, ids!(empty_state), true);
        self.set_visible(cx, ids!(next_actions), false);
        self.set_label(cx, ids!(next_title), &title);
        self.set_label(cx, ids!(next_body), &detail);
        self.set_step(
            cx,
            [ids!(step1_todo), ids!(step1_current), ids!(step1_done)],
            Step::Current,
        );
        self.set_step(
            cx,
            [ids!(step2_todo), ids!(step2_current), ids!(step2_done)],
            Step::Todo,
        );
        self.set_step(
            cx,
            [ids!(step3_todo), ids!(step3_current), ids!(step3_done)],
            Step::Todo,
        );
        self.set_label(cx, ids!(status_text), &title);
        self.toast(cx, Severity::Error, &title, &detail);
        self.ui.redraw(cx);
    }

    /// Something failed while open (a render, a seek): playback paused, the
    /// picture and the panels stay, the status line says what happened.
    fn show_stopped(&mut self, cx: &mut Cx, message: &str) {
        error!("preview: {message}");
        let (title, detail) = live::failure_text(message);
        if let Some(live) = self.live.as_mut() {
            live.problem = Some(title.clone());
        }
        self.set_label(cx, ids!(status_text), &title);
        self.toast(cx, Severity::Error, &title, &detail);
    }

    /// Show pause while playing, play otherwise. The SVG handle is swapped
    /// in place; Makepad reloads the icon when the handle changes.
    pub(crate) fn set_play_icon(&mut self, cx: &mut Cx, playing: bool) {
        let want = if playing {
            self.pause_icon.clone()
        } else {
            self.play_icon.clone()
        };
        self.swap_icon(cx, ids!(play_pause), want);
    }

    /// Swap a button's icon in place (Makepad reloads it when the handle
    /// changes).
    pub(crate) fn swap_icon(
        &mut self,
        cx: &mut Cx,
        button: &[LiveId],
        icon: Option<ScriptHandleRef>,
    ) {
        let button = self.ui.button(cx, button);
        if let Some(mut inner) = button.borrow_mut() {
            let same = inner.draw_icon.svg.as_ref().map(|h| h.as_handle())
                == icon.as_ref().map(|h| h.as_handle());
            if !same {
                inner.draw_icon.svg = icon;
            }
        }
        button.redraw(cx);
    }

    /// Record or stop. A recording is 1080 rows at the preview aspect, in
    /// the saved folder or beside the left video, named as the Slint app
    /// named them.
    fn toggle_recording(&mut self) {
        let Some(live) = self.live.as_ref() else {
            return;
        };
        if live.recording.is_some() {
            live.worker.send(PreviewCommand::StopRecording);
            return;
        }
        let first_left = live.files.left.first().cloned().unwrap_or_default();
        let folder = recording_folder(self.settings.recording_folder.as_deref(), &first_left);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        live.worker.send(PreviewCommand::StartRecording {
            path: folder.join(recording_file_name(now)),
            size: recording_size(self.settings.aspect()),
            quality: self.settings.quality(),
        });
    }

    /// The view bar while recording: the badge and Stop; otherwise the
    /// quality and Record.
    fn show_recording(&mut self, cx: &mut Cx, recording: bool) {
        self.set_visible(cx, ids!(recording_badge), recording);
        self.set_visible(cx, ids!(quality_tip), !recording);
        if recording {
            self.set_visible(cx, ids!(show_in_folder), false);
            self.set_label(cx, ids!(recording_time), "0:00");
        }
        let icon = if recording {
            self.stop_icon.clone()
        } else {
            self.record_icon.clone()
        };
        self.swap_icon(cx, ids!(record_button), icon);
    }

    fn recording_started(&mut self, cx: &mut Cx, path: PathBuf) {
        if let Some(live) = self.live.as_mut() {
            live.recording = Some(0);
        }
        self.show_recording(cx, true);
        self.toast(
            cx,
            Severity::Info,
            "Recording started",
            &path.display().to_string(),
        );
    }

    fn recorded(&mut self, cx: &mut Cx, frames: u64) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        live.recording = Some(frames);
        let fps = live.info.as_ref().map_or(30.0, |i| i.fps.max(1.0));
        let status = live.status();
        self.set_label(
            cx,
            ids!(recording_time),
            &time_ruler::clock(frames as f64 / fps),
        );
        self.set_label(cx, ids!(status_text), &status);
    }

    fn recording_saved(&mut self, cx: &mut Cx, recording: Recording) {
        let file = recording
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(live) = self.live.as_mut() {
            live.recording = None;
            live.last_output = Some(recording.path.clone());
        }
        self.show_recording(cx, false);
        self.set_visible(cx, ids!(show_in_folder), true);
        self.toast_for(
            cx,
            Severity::Info,
            "Recording saved",
            &format!("{} frames · {file}", recording.frames),
            Duration::from_secs(8),
        );
        self.refresh_status(cx);
    }

    fn recording_failed(&mut self, cx: &mut Cx, reason: &str) {
        if let Some(live) = self.live.as_mut() {
            live.recording = None;
        }
        self.show_recording(cx, false);
        self.toast(cx, Severity::Error, "Recording failed", reason);
        self.refresh_status(cx);
    }

    fn refresh_status(&mut self, cx: &mut Cx) {
        if let Some(status) = self.live.as_ref().map(Live::status) {
            self.set_label(cx, ids!(status_text), &status);
        }
    }

    /// Record, the quality, and Show in folder.
    pub(crate) fn record_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(record_button)).clicked(actions) {
            self.toggle_recording();
        }
        if let Some(index) = self
            .ui
            .drop_down(cx, ids!(record_quality))
            .selected(actions)
        {
            self.settings
                .set_quality(RecordingQuality::from_index(index));
            self.save_settings();
            if self.pointer_input {
                cx.set_key_focus(Area::Empty);
            }
        }
        if self.ui.button(cx, ids!(show_in_folder)).clicked(actions) {
            if let Some(path) = self.live.as_ref().and_then(|l| l.last_output.clone()) {
                if let Err(e) = reco_app::reveal::reveal(&path) {
                    error!("couldn't show {}: {e}", path.display());
                }
            }
        }
    }

    /// Quitting while recording: stop and wait up to 3 s for the file to
    /// close, so it plays. The one time the UI thread waits (DESIGN Rule 4).
    pub(crate) fn finish_recording_on_quit(&mut self) {
        let Some(live) = self.live.as_ref() else {
            return;
        };
        if live.recording.is_none() {
            return;
        }
        live.worker.send(PreviewCommand::StopRecording);
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            match live.worker.try_event() {
                Some(PreviewEvent::RecordingSaved(_) | PreviewEvent::RecordingFailed(_)) => return,
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }

    /// The playhead moved, or the play state changed.
    fn show_time(&mut self, cx: &mut Cx, frame: u64, state: PlayState) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        let playing = state == PlayState::Playing;
        if playing && live.state != PlayState::Playing {
            live.fps.reset();
            live.fps_reading = None;
        }
        if playing || frame != live.frame {
            live.problem = None;
        }
        live.played |= playing;
        live.frame = frame;
        live.state = state;
        live.fps_reading = if playing {
            live.fps.tick(Instant::now()).or(live.fps_reading)
        } else {
            None
        };
        let fps = live.info.as_ref().map_or(0.0, |i| i.fps);
        let now = if fps > 0.0 {
            frame.saturating_sub(1) as f64 / fps
        } else {
            0.0
        };
        let status = live.status();
        let status = self.calibration_status().unwrap_or(status);
        let scrubbing = self
            .ui
            .widget(cx, ids!(timeline))
            .borrow::<RecoTimeRuler>()
            .is_some_and(|r| r.is_dragging());
        if !scrubbing {
            self.set_label(cx, ids!(time_current), &time_ruler::clock(now));
        }
        self.set_label(cx, ids!(status_text), &status);
        self.set_play_icon(cx, playing);
        self.update_ruler(cx);
    }

    /// Each camera's files (one block each until they are probed), the
    /// playhead, and the export range.
    fn update_ruler(&mut self, cx: &mut Cx) {
        let range = self.args.export_range;
        let Some((length, playhead, lanes)) = self.live.as_ref().and_then(|l| {
            let info = l.info.as_ref()?;
            let playhead = if info.fps > 0.0 {
                l.frame.saturating_sub(1) as f64 / info.fps
            } else {
                0.0
            };
            Some(match l.lanes.as_ref() {
                Some(lanes) => (
                    lanes.length,
                    playhead,
                    vec![lanes.left.clone(), lanes.right.clone()],
                ),
                None => {
                    let length = live::length_secs(info);
                    (
                        length,
                        playhead,
                        vec![vec![(0.0, length)], vec![(0.0, length)]],
                    )
                }
            })
        }) else {
            return;
        };
        if let Some(mut ruler) = self
            .ui
            .widget(cx, ids!(timeline))
            .borrow_mut::<RecoTimeRuler>()
        {
            ruler.set_timeline(cx, length, playhead, lanes);
            ruler.set_export_range(cx, range);
        }
    }

    /// The files' lanes and the exact length arrived.
    fn show_lanes(&mut self, cx: &mut Cx, lanes: Lanes) {
        self.set_label(cx, ids!(time_total), &time_ruler::clock(lanes.length));
        if let Some(live) = self.live.as_mut() {
            live.lanes = Some(lanes);
        }
        self.update_ruler(cx);
    }

    /// Scrubbing shows the time; releasing seeks.
    pub(crate) fn ruler_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let uid = self.ui.widget(cx, ids!(timeline)).widget_uid();
        for action in actions.filter_widget_actions_cast::<RulerAction>(uid) {
            match action {
                RulerAction::Scrub(secs) => {
                    self.set_label(cx, ids!(time_current), &time_ruler::clock(secs))
                }
                RulerAction::Seek(secs) => {
                    if let Some(live) = self.live.as_ref() {
                        let fps = live.info.as_ref().map_or(0.0, |i| i.fps);
                        if fps > 0.0 {
                            live.worker.send(PreviewCommand::SeekTo {
                                frame: (secs * fps).floor() as u64,
                            });
                        }
                    }
                }
                RulerAction::None => {}
            }
        }
    }

    /// Transport buttons, the preview aspect and fullscreen.
    pub(crate) fn preview_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let Some(live) = self.live.as_ref() else {
            return;
        };
        for (id, command) in [
            (ids!(play_pause), PreviewCommand::TogglePlay),
            (ids!(step_back), PreviewCommand::Step { forward: false }),
            (ids!(step_forward), PreviewCommand::Step { forward: true }),
        ] {
            if self.ui.button(cx, id).clicked(actions) {
                live.worker.send(command);
            }
        }
        if let Some(index) = self.ui.drop_down(cx, ids!(aspect)).selected(actions) {
            let aspect = PreviewAspect::from_index(index);
            if let Some(mut preview) = self
                .ui
                .widget(cx, ids!(preview))
                .borrow_mut::<RecoPreview>()
            {
                preview.set_aspect(cx, aspect);
            }
            self.settings.set_aspect(aspect);
            self.save_settings();
            // A dropdown takes the keyboard on a click. A pick made with the
            // mouse gives it back to the preview's shortcuts; arrow keys on
            // a focused dropdown keep stepping through the choices.
            if self.pointer_input {
                cx.set_key_focus(Area::Empty);
            }
        }
        let uid = self.ui.widget(cx, ids!(preview)).widget_uid();
        if actions
            .find_widget_action(uid)
            .is_some_and(|a| a.cast::<PreviewAction>() == PreviewAction::ToggleFullscreen)
        {
            // On macOS `maximize` toggles fullscreen; `fullscreen()` is a
            // no-op there.
            self.ui.window(cx, ids!(main_window)).maximize(cx);
        }
    }
}
