//! The live session's side of the App: it starts the render worker, routes
//! its events to the preview widget and the shell, and turns transport
//! actions into commands. Split from main.rs, which keeps the shell, the
//! look preview and startup.

use std::sync::Arc;
use std::time::Instant;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::preview::view::PreviewAspect;
use reco_app::preview::worker::{
    PreviewCommand, PreviewConfig, PreviewEvent, PreviewInfo, PreviewWorker,
};

use crate::live::{self, FpsMeter, Live};
use crate::names::middle_ellipsis;
use crate::time_ruler;
use crate::ui::preview::{PreviewAction, RecoPreview};
use crate::ui::time_panel::RecoTimeRuler;
use crate::{cli, display_device, App, Step, PROJECT_NAME_CHARS};

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
        self.live = Some(Live {
            worker,
            files,
            info: None,
            frame: 0,
            playing: false,
            fps: FpsMeter::default(),
        });
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
                Some(PreviewEvent::Time { frame, playing }) => self.show_time(cx, frame, playing),
                // The widget takes frames; nothing else is left.
                Some(_) | None => {}
            }
        }
    }

    /// "Opening the videos…" while the worker opens them.
    fn show_opening(&mut self, cx: &mut Cx) {
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
        let (left, right) = live::names(&live.files);
        let counts = (live.files.left.len(), live.files.right.len());
        let calibration = live
            .files
            .calibration
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.shell.set_files_loaded(true);
        self.set_visible(cx, ids!(empty_state), false);
        self.set_visible(cx, ids!(sample_frame), false);
        self.set_visible(cx, ids!(preview), true);
        for (on, idle) in [
            (ids!(left_badge_on), ids!(left_badge)),
            (ids!(right_badge_on), ids!(right_badge)),
            (ids!(lane_left_badge_on), ids!(lane_left_badge)),
            (ids!(lane_right_badge_on), ids!(lane_right_badge)),
            (ids!(link_on), ids!(link_idle)),
            (ids!(change_left), ids!(add_left)),
            (ids!(change_right), ids!(add_right)),
            (ids!(recalibrate), ids!(auto_calibrate)),
        ] {
            self.set_visible(cx, on, true);
            self.set_visible(cx, idle, false);
        }
        for (dot, on) in [
            (ids!(cal_dot_idle), false),
            (ids!(cal_dot_busy), false),
            (ids!(cal_dot_ok), true),
            (ids!(cal_dot_error), false),
        ] {
            self.set_visible(cx, dot, on);
        }
        self.set_label(cx, ids!(left_files), &live::file_count(counts.0));
        self.set_label(cx, ids!(right_files), &live::file_count(counts.1));
        self.set_label(cx, ids!(calibration_status), "Calibrated");
        self.set_label(cx, ids!(calibration_detail), &calibration);
        self.set_label(
            cx,
            ids!(project_name),
            &middle_ellipsis(&format!("{left} + {right}"), PROJECT_NAME_CHARS),
        );
        self.set_visible(cx, ids!(time_display), true);
        self.set_label(
            cx,
            ids!(time_total),
            &time_ruler::clock(live::length_secs(&info)),
        );
        self.set_label(cx, ids!(status_text), "Ready");
        self.update_ruler(cx);
        self.apply_shell(cx);
    }

    /// Opening failed: say so in the viewer, the worker's words split into
    /// a title and its detail, back at the first step.
    fn show_failed(&mut self, cx: &mut Cx, message: &str) {
        let (title, detail) = live::failure_text(message);
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
        self.ui.redraw(cx);
    }

    /// Something failed while open (a render, a seek): playback paused, the
    /// picture and the panels stay, the status line says what happened.
    fn show_stopped(&mut self, cx: &mut Cx, message: &str) {
        error!("preview: {message}");
        let (title, _) = live::failure_text(message);
        self.set_label(cx, ids!(status_text), &title);
    }

    /// The playhead moved, or play started or stopped.
    fn show_time(&mut self, cx: &mut Cx, frame: u64, playing: bool) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        live.frame = frame;
        live.playing = playing;
        let reading = if playing {
            live.fps.tick(Instant::now())
        } else {
            None
        };
        let fps = live.info.as_ref().map_or(0.0, |i| i.fps);
        let now = if fps > 0.0 {
            frame.saturating_sub(1) as f64 / fps
        } else {
            0.0
        };
        self.set_label(cx, ids!(time_current), &time_ruler::clock(now));
        if let Some(rate) = reading {
            self.set_label(cx, ids!(status_text), &format!("{rate:.1} fps"));
        }
        self.update_ruler(cx);
    }

    /// One block per camera over the whole length (file boundaries arrive
    /// with Module 2), and the playhead.
    fn update_ruler(&mut self, cx: &mut Cx) {
        let Some((length, playhead)) = self.live.as_ref().and_then(|l| {
            let info = l.info.as_ref()?;
            let playhead = if info.fps > 0.0 {
                l.frame.saturating_sub(1) as f64 / info.fps
            } else {
                0.0
            };
            Some((live::length_secs(info), playhead))
        }) else {
            return;
        };
        if let Some(mut ruler) = self
            .ui
            .widget(cx, ids!(timeline))
            .borrow_mut::<RecoTimeRuler>()
        {
            ruler.set_timeline(
                cx,
                length,
                playhead,
                vec![vec![(0.0, length)], vec![(0.0, length)]],
            );
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
            if let Some(mut preview) = self
                .ui
                .widget(cx, ids!(preview))
                .borrow_mut::<RecoPreview>()
            {
                preview.set_aspect(cx, PreviewAspect::from_index(index));
            }
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
