//! The project's side of the App: each camera's files and the calibration,
//! picked through the system's file dialogs, shown in the Setup panel, the
//! title and the next-step card, and turned into an open preview once both
//! cameras and a calibration are set (`sync_live`).

use std::path::{Path, PathBuf};

use makepad_widgets::makepad_platform::file_dialogs::{FileDialog, FileDialogAction};
use makepad_widgets::*;
use reco_app::calibrate::STEPS;
use reco_app::preview::lanes::camera_spans;
use reco_app::preview::worker::PreviewCommand;
use reco_app::project::{Camera, Stage, VIDEO_EXTENSIONS};
use reco_app::toasts::Severity;

use crate::calibrate_view::{failure_words, Calibrating};
use crate::live;
use crate::names::middle_ellipsis;
use crate::time_ruler;
use crate::ui::file_list::{FileListAction, RecoFileList};
use crate::ui::panorama::RecoPanorama;
use crate::ui::time_panel::RecoTimeRuler;
use crate::{cli, App, Step, PROJECT_NAME_CHARS};

/// Checks answer the file dialogs from this JSON file (`{"left": [paths],
/// "right": [...], "calibration": [...]}`) instead of opening them; the
/// answer then takes the same path as a real pick.
const DIALOG_ANSWERS: &str = "RECO_DESKTOP_DIALOG_ANSWERS";

/// What a file dialog is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pick {
    /// One camera's videos (several at once).
    Videos(Camera),
    /// A calibration file.
    Calibration,
    /// The file to export to (a Save dialog).
    Export,
    /// A lens profile file.
    LensFile,
    /// Where recordings go (a folder; Preferences).
    RecordingFolder,
    /// The AI tracking's model (the export sheet).
    ExportModel,
}

impl Pick {
    fn id(self) -> LiveId {
        match self {
            Pick::Videos(Camera::Left) => live_id!(pick_left),
            Pick::Videos(Camera::Right) => live_id!(pick_right),
            Pick::Calibration => live_id!(pick_calibration),
            Pick::Export => live_id!(pick_export),
            Pick::LensFile => live_id!(pick_lens),
            Pick::RecordingFolder => live_id!(pick_recording_folder),
            Pick::ExportModel => live_id!(pick_export_model),
        }
    }

    fn from_id(id: LiveId) -> Option<Self> {
        [
            Pick::Videos(Camera::Left),
            Pick::Videos(Camera::Right),
            Pick::Calibration,
            Pick::Export,
            Pick::LensFile,
            Pick::RecordingFolder,
            Pick::ExportModel,
        ]
        .into_iter()
        .find(|p| p.id() == id)
    }

    /// The pick's key in the checks' answers file.
    fn key(self) -> &'static str {
        match self {
            Pick::Videos(Camera::Left) => "left",
            Pick::Videos(Camera::Right) => "right",
            Pick::Calibration => "calibration",
            Pick::Export => "export",
            Pick::LensFile => "lens",
            Pick::RecordingFolder => "recording_folder",
            Pick::ExportModel => "model",
        }
    }
}

/// The checks' answer for `pick`, when they set one (`None` when the
/// variable is unset: a real dialog opens).
fn checks_answer(pick: Pick) -> Option<Vec<PathBuf>> {
    let file = std::env::var_os(DIALOG_ANSWERS)?;
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let answers: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    Some(
        answers[pick.key()]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(PathBuf::from))
            .collect(),
    )
}

/// "Left" or "right".
fn side(camera: Camera) -> &'static str {
    match camera {
        Camera::Left => "left",
        Camera::Right => "right",
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl App {
    /// Ask for files: the system's dialog (its answer arrives as an
    /// action), or the checks' answer straight away.
    pub(crate) fn pick(&mut self, cx: &mut Cx, pick: Pick) {
        if let Some(paths) = checks_answer(pick) {
            if !paths.is_empty() {
                self.picked(cx, pick, paths);
            }
            return;
        }
        let mut dialog = match pick {
            Pick::Videos(camera) => FileDialog::new()
                .set_title(format!("Add the {} camera's videos", side(camera)))
                .add_filter(
                    "Videos".into(),
                    VIDEO_EXTENSIONS.iter().map(|e| e.to_string()).collect(),
                ),
            Pick::Calibration => FileDialog::new()
                .set_title("Load a calibration".into())
                .add_filter("Calibration".into(), vec!["json".into()]),
            Pick::Export => return self.pick_export_file(cx),
            Pick::LensFile => FileDialog::new()
                .set_title("Load a lens profile".into())
                .add_filter("Lens profile".into(), vec!["json".into()]),
            Pick::RecordingFolder => return self.pick_recording_folder(cx),
            Pick::ExportModel => FileDialog::new()
                .set_title("Choose the AI tracking's model".into())
                .add_filter("ONNX model".into(), vec!["onnx".into()]),
        };
        // Start beside the videos already chosen.
        let near = self
            .project
            .files(Camera::Left)
            .first()
            .or_else(|| self.project.files(Camera::Right).first())
            .and_then(|p| p.parent());
        if let Some(folder) = near {
            dialog = dialog.set_location(folder.to_path_buf());
        }
        dialog.multiple = matches!(pick, Pick::Videos(_));
        dialog.id = pick.id();
        cx.open_select_file_dialog(dialog);
    }

    /// A dialog's answer: add the videos, or use the calibration.
    pub(crate) fn picked(&mut self, cx: &mut Cx, pick: Pick, paths: Vec<PathBuf>) {
        match pick {
            Pick::Videos(camera) => {
                if self.project.add(camera, paths) == 0 {
                    self.toast(
                        cx,
                        Severity::Warn,
                        "No videos added",
                        "Choose MP4, MOV, AVI or MKV files the camera doesn't have yet.",
                    );
                }
            }
            Pick::Calibration => {
                if let Some(path) = paths.into_iter().next() {
                    self.project.calibration = Some(path);
                    self.calibration_failure = None;
                }
            }
            Pick::Export => {
                if let Some(path) = paths.into_iter().next() {
                    self.export_file_picked(cx, path);
                }
                return;
            }
            Pick::LensFile => {
                if let Some(path) = paths.into_iter().next() {
                    self.lens_file_picked(cx, path);
                }
                return;
            }
            Pick::RecordingFolder => {
                if let Some(path) = paths.first() {
                    self.prefs_folder_picked(cx, path);
                }
                return;
            }
            Pick::ExportModel => {
                if let Some(path) = paths.first() {
                    self.export_model_picked(cx, path);
                }
                return;
            }
        }
        self.project_changed(cx);
    }

    /// The system's Save dialog, at the file the export sheet names.
    fn pick_export_file(&mut self, cx: &mut Cx) {
        let typed = self.ui.text_input(cx, ids!(export_output)).text();
        let current = PathBuf::from(typed.trim());
        let mut dialog = FileDialog::new().set_title("Export to".into()).add_filter(
            "Video".into(),
            vec!["mp4".into(), "mov".into(), "mkv".into()],
        );
        if let Some(name) = current.file_name() {
            dialog = dialog.set_filename(name.to_string_lossy().into_owned());
        }
        if let Some(folder) = current.parent().filter(|f| f.is_dir()) {
            dialog = dialog.set_location(folder.to_path_buf());
        }
        dialog.id = Pick::Export.id();
        cx.open_save_file_dialog(dialog);
    }

    /// The system's folder dialog, at the folder Preferences names (Makepad
    /// answers a folder dialog without an id: this is the app's only one).
    fn pick_recording_folder(&mut self, cx: &mut Cx) {
        let typed = self.ui.text_input(cx, ids!(prefs_folder)).text();
        let mut dialog = FileDialog::new().set_title("Record to".into());
        let current = PathBuf::from(typed.trim());
        if current.is_dir() {
            dialog = dialog.set_location(current);
        }
        cx.open_select_folder_dialog(dialog);
    }

    /// File dialog answers.
    pub(crate) fn file_dialog_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            match action.downcast_ref::<FileDialogAction>() {
                Some(FileDialogAction::FileSelected { id, paths }) => {
                    if let Some(pick) = Pick::from_id(*id) {
                        self.picked(cx, pick, paths.clone());
                    }
                }
                Some(FileDialogAction::SaveFileSelected { id, path }) => {
                    if let Some(pick) = Pick::from_id(*id) {
                        self.picked(cx, pick, vec![path.clone()]);
                    }
                }
                Some(FileDialogAction::FolderSelected(path)) => {
                    self.picked(cx, Pick::RecordingFolder, vec![path.clone()]);
                }
                _ => {}
            }
        }
    }

    /// The Setup panel's Add and More buttons, and the next-step card.
    pub(crate) fn project_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        for (ids, camera) in [
            (ids!(add_left), Camera::Left),
            (ids!(more_left), Camera::Left),
            (ids!(add_right), Camera::Right),
            (ids!(more_right), Camera::Right),
        ] {
            if self.ui.button(cx, ids).clicked(actions) {
                self.pick(cx, Pick::Videos(camera));
            }
        }
        for (list, clear, camera) in [
            (ids!(left_list), ids!(clear_left), Camera::Left),
            (ids!(right_list), ids!(clear_right), Camera::Right),
        ] {
            let uid = self.ui.widget(cx, list).widget_uid();
            let mut changed = false;
            for action in actions.filter_widget_actions_cast::<FileListAction>(uid) {
                changed |= match action {
                    FileListAction::Remove(index) => self.project.remove(camera, index),
                    FileListAction::Move { from, to } => self.project.move_file(camera, from, to),
                    FileListAction::None => false,
                };
            }
            if self.ui.button(cx, clear).clicked(actions) {
                self.project.clear(camera);
                changed = true;
            }
            if changed {
                self.project_changed(cx);
            }
        }
        if self.ui.button(cx, ids!(load_calibration)).clicked(actions) {
            self.pick(cx, Pick::Calibration);
        }
        if self.ui.button(cx, ids!(next_primary)).clicked(actions) {
            match self.project.stage() {
                Stage::NoVideos | Stage::OneCamera(Camera::Right) => {
                    self.pick(cx, Pick::Videos(Camera::Left))
                }
                Stage::OneCamera(Camera::Left) => self.pick(cx, Pick::Videos(Camera::Right)),
                Stage::Cameras => self.start_calibration(cx),
                Stage::Ready => {}
            }
        }
        if self.ui.button(cx, ids!(next_secondary)).clicked(actions)
            && self.project.stage() == Stage::Cameras
        {
            self.pick(cx, Pick::Calibration);
        }
    }

    /// The project changed: use a saved calibration, measure new files,
    /// open or close the preview, and show it all.
    pub(crate) fn project_changed(&mut self, cx: &mut Cx) {
        self.refresh_project(cx, false);
    }

    /// As `project_changed`; `reload` reopens the preview even when its
    /// files are the same (a new calibration saved over the old file).
    pub(crate) fn refresh_project(&mut self, cx: &mut Cx, reload: bool) {
        self.cancel_stale_calibration(cx);
        self.use_saved_calibration(cx);
        if let Some(durations) = self.durations.as_mut() {
            durations.request(self.project.files(Camera::Left));
            durations.request(self.project.files(Camera::Right));
        }
        self.sync_live(cx, reload);
        self.show_project(cx);
        self.apply_shell(cx);
    }

    /// A calibration auto-calibrate saved beside the left video loads by
    /// itself, once per file (clearing it doesn't bring it back).
    fn use_saved_calibration(&mut self, cx: &mut Cx) {
        if self.project.stage() != Stage::Cameras {
            return;
        }
        let Some(saved) = self.project.sibling_calibration() else {
            return;
        };
        if self.offered_calibration.as_ref() == Some(&saved) || !saved.is_file() {
            return;
        }
        self.offered_calibration = Some(saved.clone());
        let name = file_name(&saved);
        self.project.calibration = Some(saved);
        self.toast(cx, Severity::Info, "Found a saved calibration", &name);
    }

    /// Open the preview once both cameras and a calibration are set, reopen
    /// it when they change, and close it when they no longer are.
    fn sync_live(&mut self, cx: &mut Cx, reload: bool) {
        let wanted = (self.project.stage() == Stage::Ready).then(|| cli::FileArgs {
            left: self.project.files(Camera::Left).to_vec(),
            right: self.project.files(Camera::Right).to_vec(),
            calibration: self.project.calibration.clone().unwrap_or_default(),
        });
        match (wanted, self.live.as_mut()) {
            (Some(files), None) => self.start_live(cx, files),
            (Some(files), Some(live)) => {
                if live.open && live.files == files && !reload {
                    return;
                }
                live.worker.send(PreviewCommand::Open {
                    left: files.left_input(),
                    right: files.right_input(),
                    calibration: files.calibration.clone(),
                });
                live.reopen(files);
                self.shell.set_files_loaded(false);
                self.show_opening(cx);
                self.apply_shell(cx);
            }
            (None, Some(live)) if live.open => {
                live.worker.send(PreviewCommand::Close);
                live.close();
                self.set_visible(cx, ids!(preview), false);
                self.shell.set_files_loaded(false);
                self.apply_shell(cx);
            }
            (None, _) => {}
        }
    }

    /// Whether the preview is open (or opening).
    fn previewing(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.open)
    }

    /// "3 files · 31:24" (the length once measured); empty for no files.
    fn camera_summary(&self, camera: Camera) -> String {
        let files = self.project.files(camera);
        if files.is_empty() {
            return String::new();
        }
        let count = live::file_count(files.len());
        match self.durations.as_ref().and_then(|d| d.total(files)) {
            Some(secs) => format!("{count} · {}", time_ruler::clock(secs)),
            None => count,
        }
    }

    /// Each of `camera`'s files: its name and, once measured, its length.
    fn file_rows(&self, camera: Camera) -> Vec<(String, String)> {
        self.project
            .files(camera)
            .iter()
            .map(|f| {
                let length = self
                    .durations
                    .as_ref()
                    .and_then(|d| d.duration(f))
                    .map(time_ruler::clock)
                    .unwrap_or_default();
                (file_name(f), length)
            })
            .collect()
    }

    /// The Setup panel, the title, the calibration section, and (while no
    /// preview is open) the next-step card and the camera lanes.
    pub(crate) fn show_project(&mut self, cx: &mut Cx) {
        let stage = self.project.stage();
        let left = !self.project.files(Camera::Left).is_empty();
        let right = !self.project.files(Camera::Right).is_empty();

        // The title: each camera's first file.
        let first = |camera| self.project.files(camera).first().map(|p| file_name(p));
        let title = match (first(Camera::Left), first(Camera::Right)) {
            (Some(l), Some(r)) => format!("{l} + {r}"),
            (Some(one), None) | (None, Some(one)) => one,
            (None, None) => "No videos yet".to_string(),
        };
        self.set_label(
            cx,
            ids!(project_name),
            &middle_ellipsis(&title, PROJECT_NAME_CHARS),
        );

        // The camera pair.
        for (on, idle, lit) in [
            (ids!(left_badge_on), ids!(left_badge), left),
            (ids!(lane_left_badge_on), ids!(lane_left_badge), left),
            (ids!(right_badge_on), ids!(right_badge), right),
            (ids!(lane_right_badge_on), ids!(lane_right_badge), right),
        ] {
            self.set_visible(cx, on, lit);
            self.set_visible(cx, idle, !lit);
        }
        let both = left && right;
        self.set_visible(cx, ids!(link_idle), !both);
        self.set_visible(cx, ids!(link_on), both);
        self.set_visible(cx, ids!(add_left), !left);
        self.set_visible(cx, ids!(more_left), left);
        self.set_visible(cx, ids!(add_right), !right);
        self.set_visible(cx, ids!(more_right), right);
        let (left_text, right_text) = (
            self.camera_summary(Camera::Left),
            self.camera_summary(Camera::Right),
        );
        self.set_label(cx, ids!(left_files), &left_text);
        self.set_label(cx, ids!(right_files), &right_text);
        self.set_visible(cx, ids!(left_fold), left);
        self.set_visible(cx, ids!(right_fold), right);
        for (list, camera) in [
            (ids!(left_list), Camera::Left),
            (ids!(right_list), Camera::Right),
        ] {
            let rows = self.file_rows(camera);
            if let Some(mut list) = self.ui.widget(cx, list).borrow_mut::<RecoFileList>() {
                list.set_rows(cx, rows);
            }
        }

        // Calibration: running, failed, done, or not yet.
        let calibration = self.project.calibration.as_deref().map(file_name);
        let calibrating = self.calibrating.clone();
        let failure = self
            .calibration_failure
            .as_deref()
            .filter(|_| calibration.is_none())
            .map(failure_words);
        let (status, detail) = match (&calibrating, &failure, &calibration) {
            (Some(run), _, _) if run.cancelling => ("Calibrating…", "Stopping…".to_string()),
            (Some(run), _, _) => (
                "Calibrating…",
                run.step
                    .as_ref()
                    .map_or("Starting…".to_string(), |(step, of, words)| {
                        format!("Step {step} of {of} · {words}")
                    }),
            ),
            (None, Some(why), _) => ("Calibration failed", why.clone()),
            (None, None, Some(name)) => ("Calibrated", name.clone()),
            (None, None, None) if both => ("Not calibrated", "Ready to calibrate".to_string()),
            (None, None, None) => ("Not calibrated", "Add both cameras first".to_string()),
        };
        self.set_label(cx, ids!(calibration_status), status);
        self.set_label(cx, ids!(calibration_detail), &detail);
        let running = calibrating.is_some();
        let calibrated = calibration.is_some();
        let failed = failure.is_some();
        for (dot, on) in [
            (ids!(cal_dot_idle), !running && !calibrated && !failed),
            (ids!(cal_dot_busy), running),
            (ids!(cal_dot_ok), !running && calibrated),
            (ids!(cal_dot_error), !running && failed),
        ] {
            self.set_visible(cx, dot, on);
        }
        self.set_visible(cx, ids!(auto_calibrate), !calibrated && !running);
        self.set_button_enabled(cx, ids!(auto_calibrate), both);
        self.set_visible(cx, ids!(recalibrate), calibrated && !running);
        self.set_visible(cx, ids!(cancel_calibration), running);
        self.set_button_enabled(
            cx,
            ids!(cancel_calibration),
            !calibrating.as_ref().is_some_and(|c| c.cancelling),
        );
        self.set_button_enabled(cx, ids!(load_calibration), both && !running);
        self.set_visible(cx, ids!(clear_calibration), calibrated && !running);
        // The Advanced options are locked while a calibration runs.
        for id in [
            ids!(cal_frames),
            ids!(cal_imu),
            ids!(cal_akaze),
            ids!(cal_y_min),
            ids!(cal_y_max),
            ids!(cal_skip_end),
        ] {
            self.ui.widget(cx, id).set_disabled(cx, running);
        }
        if let Some(text) = self.calibration_status() {
            self.set_label(cx, ids!(status_text), &text);
        }

        // The steps: cameras, calibration, export.
        self.set_step(
            cx,
            [ids!(step1_todo), ids!(step1_current), ids!(step1_done)],
            if both { Step::Done } else { Step::Current },
        );
        let step2 = match stage {
            Stage::Ready => Step::Done,
            Stage::Cameras => Step::Current,
            _ => Step::Todo,
        };
        self.set_step(
            cx,
            [ids!(step2_todo), ids!(step2_current), ids!(step2_done)],
            step2,
        );
        self.set_step(
            cx,
            [ids!(step3_todo), ids!(step3_current), ids!(step3_done)],
            if stage == Stage::Ready {
                Step::Current
            } else {
                Step::Todo
            },
        );
        if let Some(mut panorama) = self
            .ui
            .widget(cx, ids!(panorama))
            .borrow_mut::<RecoPanorama>()
        {
            panorama.set_lit(cx, left, right, false);
        }

        if !self.previewing() {
            self.show_next_step(cx, stage, calibrating.as_ref(), failure.as_deref());
            self.show_file_lanes(cx);
        }
        self.ui.redraw(cx);
    }

    /// The next-step card while no preview is open.
    fn show_next_step(
        &mut self,
        cx: &mut Cx,
        stage: Stage,
        calibrating: Option<&Calibrating>,
        failure: Option<&str>,
    ) {
        if let Some(run) = calibrating {
            let (step, of, words) = run
                .step
                .clone()
                .unwrap_or((0, STEPS, "Starting…".to_string()));
            self.set_label(cx, ids!(next_title), "Calibrating…");
            self.set_label(cx, ids!(next_body), &words);
            self.set_visible(cx, ids!(next_actions), false);
            self.set_visible(cx, ids!(calibrate_progress), true);
            self.ui
                .progress_bar(cx, ids!(calibrate_bar))
                .set_value(cx, step as f64 / of.max(1) as f64);
            let eta = if step > 0 {
                format!("Step {step} of {of}")
            } else {
                String::new()
            };
            self.set_label(cx, ids!(calibrate_eta), &eta);
            return;
        }
        // Earlier sessions are a click away while cameras are missing.
        let recent = if self.settings.recent.is_empty() {
            ""
        } else {
            "Recent files…"
        };
        let failed_body;
        let (title, body, primary, secondary) = match (stage, failure) {
            (Stage::Cameras, Some(why)) => {
                failed_body = why.to_string();
                (
                    "Calibration didn't work",
                    failed_body.as_str(),
                    "Try again",
                    "Load calibration file…",
                )
            }
            _ => match stage {
            Stage::NoVideos => (
                "Add your two camera videos",
                "Start with the left camera. All the GoPro files from one camera go in together.",
                "Add left camera…",
                recent,
            ),
            Stage::OneCamera(Camera::Left) => (
                "Now add the right camera",
                "Choose all the GoPro files from the right-hand camera.",
                "Add right camera…",
                recent,
            ),
            Stage::OneCamera(Camera::Right) => (
                "Now add the left camera",
                "Choose all the GoPro files from the left-hand camera.",
                "Add left camera…",
                recent,
            ),
            Stage::Cameras => (
                "Line up the two cameras",
                "Auto-calibrate matches the pitch markings both cameras can see. It takes a few minutes.",
                "Auto-calibrate",
                "Load calibration file…",
            ),
            Stage::Ready => ("", "", "", ""),
            },
        };
        self.set_label(cx, ids!(next_title), title);
        self.set_label(cx, ids!(next_body), body);
        self.set_visible(cx, ids!(next_actions), !primary.is_empty());
        self.ui.button(cx, ids!(next_primary)).set_text(cx, primary);
        self.ui
            .button(cx, ids!(next_secondary))
            .set_text(cx, secondary);
        self.set_visible(cx, ids!(next_secondary), !secondary.is_empty());
        self.set_visible(cx, ids!(calibrate_progress), false);
        self.set_visible(cx, ids!(empty_state), true);
        self.set_visible(cx, ids!(sample_frame), false);
        self.set_visible(cx, ids!(preview), false);
        self.set_visible(cx, ids!(time_display), false);
        self.set_label(cx, ids!(status_text), "");
    }

    /// Each camera's files on the lanes, from the start, before a preview
    /// lines them up.
    fn show_file_lanes(&mut self, cx: &mut Cx) {
        let lane = |camera| -> Vec<(f64, f64)> {
            let files = self.project.files(camera);
            match self.durations.as_ref().and_then(|d| {
                files
                    .iter()
                    .map(|f| d.duration(f))
                    .collect::<Option<Vec<f64>>>()
            }) {
                Some(lengths) => camera_spans(&lengths, 0.0),
                None => Vec::new(),
            }
        };
        let lanes = vec![lane(Camera::Left), lane(Camera::Right)];
        let length = lanes
            .iter()
            .filter_map(|l| l.last().map(|s| s.1))
            .fold(0.0, f64::max);
        if let Some(mut ruler) = self
            .ui
            .widget(cx, ids!(timeline))
            .borrow_mut::<RecoTimeRuler>()
        {
            ruler.set_timeline(cx, length, 0.0, lanes);
        }
    }

    /// Measurements arrived: show the new lengths.
    pub(crate) fn collect_durations(&mut self, cx: &mut Cx) {
        if self.durations.as_mut().is_some_and(|d| d.collect()) {
            self.show_project(cx);
        }
    }
}
