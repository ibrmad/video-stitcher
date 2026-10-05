//! Reco Desktop: the Makepad 2 desktop app for Reco.
//!
//! Module 0 (see `DESIGN.md`): the window shell and the look, after the
//! Rerun viewer. `--look-preview[=STATE]` shows each state of the job (one
//! camera, both cameras, calibrating, calibration failed, ready, exporting)
//! with sample content for design review.
//!
//! Module 1: `--left/--right/--calibration` open two cameras' videos into
//! the live stitched preview, rendered by `reco-app`'s worker thread.

pub use makepad_widgets;
use makepad_widgets::*;

mod cli;
mod keys;
mod live;
mod names;
mod session_view;
mod shell_state;
mod theme;
mod time_ruler;
mod ui;

use cli::{Args, LookPreview};
use live::Live;
use names::middle_ellipsis;
use reco_app::settings::{self, DesktopSettings};
use shell_state::{Panel, ShellState};
use ui::panorama::RecoPanorama;
use ui::time_panel::RecoTimeRuler;

/// Longest project name shown in the title bar before its middle is cut.
const PROJECT_NAME_CHARS: usize = 44;

/// The sample match the look preview shows: 1:45:00, the playhead at 12:34.
const SAMPLE_LENGTH: f64 = 6300.0;
const SAMPLE_PLAYHEAD: f64 = 754.0;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    use mod.draw.KeyCode

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Reco"
                window.inner_size: vec2(1280, 820)
                caption_bar +: {
                    caption_label +: {
                        flow: Right spacing: 0 align: Align{y: 0.5}
                        caption_icon +: {width: 0 height: 0 margin: 0}
                        label +: {visible: false}
                        top_bar := RecoTopBar{}
                    }
                }
                window_menu +: {
                    main := MenuItem.Main{items: [@app_menu_item, @view_menu]}
                    app_menu_item := MenuItem.Sub{name: "Reco" items: [@quit]}
                    quit := MenuItem.Item{name: "Quit Reco" key: KeyCode.KeyQ enabled: true}
                    view_menu := MenuItem.Sub{name: "View" items: [@toggle_media_menu, @toggle_inspector_menu, @toggle_timeline_menu]}
                    toggle_media_menu := MenuItem.Item{name: "Setup Panel" key: KeyCode.Key1 enabled: true}
                    toggle_inspector_menu := MenuItem.Item{name: "Adjust Panel" key: KeyCode.Key2 enabled: true}
                    toggle_timeline_menu := MenuItem.Item{name: "Time Panel" key: KeyCode.Key3 enabled: true}
                }
                body +: {
                    flow: Overlay
                    shell := RecoShell{}
                    tip_layer := TipLayer{}
                    // Menus as Rerun's: a dark floating panel, a grey row
                    // under the pointer, Inter at the app's one size.
                    menus := MenuLayer{
                        draw_bg +: {
                            color: theme.reco_band
                            border_color: theme.reco_widget
                            radius: theme.container_corner_radius
                        }
                        draw_row +: {
                            color_hover: theme.reco_widget
                            color_down: theme.reco_hover
                        }
                        draw_sep +: {color: theme.reco_separator}
                        draw_label +: {
                            color: theme.reco_text
                            text_style: theme.font_regular{font_size: theme.reco_font_body}
                        }
                        draw_section +: {
                            color: theme.reco_text_subdued
                            text_style: theme.font_regular{font_size: theme.reco_font_small}
                        }
                        draw_shortcut +: {
                            color: theme.reco_text_subdued
                            text_style: theme.font_regular{font_size: theme.reco_font_body}
                        }
                    }
                }
            }
        }
    }
}

/// Where a step of the job stands, for the viewer's stepper.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Todo,
    Current,
    Done,
}

/// The app menu: what Rerun keeps under its logo.
fn app_menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::new(live_id!(shortcuts), "Keyboard shortcuts"),
        MenuRow::new(live_id!(preferences), "Preferences…"),
        MenuRow::separator(),
        MenuRow::new(live_id!(report_bug), "Report a bug…"),
        MenuRow::separator(),
        MenuRow::section(concat!("Reco ", env!("CARGO_PKG_VERSION"))),
    ]
}

/// Recently used camera pairs (Module 3 fills this from settings).
fn recent_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::new(live_id!(recent_1), "GX010120 + GX010092"),
        MenuRow::new(live_id!(recent_2), "GX010087 + GX010061"),
        MenuRow::separator(),
        MenuRow::new(live_id!(clear_recent), "Clear recent files"),
    ]
}

/// `count` equal files covering `length` seconds, as (start, end) spans.
fn sample_files(count: usize, length: f64) -> Vec<(f64, f64)> {
    let each = length / count as f64;
    (0..count)
        .map(|i| (i as f64 * each, (i + 1) as f64 * each))
        .collect()
}

/// The UI's `MTLDevice`, for the zero-copy check (`None` off Apple
/// platforms: Makepad has no Metal device there and the preview reads back).
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
fn display_device(cx: &Cx) -> Option<usize> {
    cx.metal_device().map(|d| d as usize)
}

/// See the Apple version.
#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "tvos")))]
fn display_device(_cx: &Cx) -> Option<usize> {
    None
}

/// The application: widget tree, shell state and startup options.
#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    shell: ShellState,
    #[rust]
    args: Args,
    /// The state shown by `--look-preview` (Module 0 only; the engine drives
    /// these states from Module 1 on).
    #[rust]
    preview: Option<LookPreview>,
    /// The time panel shows only its control row.
    #[rust]
    timeline_folded: bool,
    /// The live preview, when files were given on the command line.
    #[rust]
    live: Option<Live>,
    /// The last input came from the pointer, not the keyboard.
    #[rust]
    pointer_input: bool,
    /// What the app remembers between runs (desktop.json).
    #[rust]
    settings: DesktopSettings,
}

impl App {
    /// Push the shell state into the widgets: panel folds, enabled
    /// controls, the narrow-window hint.
    fn apply_shell(&mut self, cx: &mut Cx) {
        let media = if self.shell.is_open(Panel::Media) {
            SplitterCollapse::None
        } else {
            SplitterCollapse::A
        };
        self.ui
            .splitter(cx, ids!(main_split))
            .set_collapse(cx, media);
        let inspector = if self.shell.is_open(Panel::Inspector) {
            SplitterCollapse::None
        } else {
            SplitterCollapse::B
        };
        self.ui
            .splitter(cx, ids!(inner_split))
            .set_collapse(cx, inspector);
        // Lanes once a camera has video, unless folded away.
        let has_video =
            self.preview.is_some() || self.live.as_ref().is_some_and(|l| l.info.is_some());
        self.set_visible(cx, ids!(lanes), has_video && !self.timeline_folded);

        let loaded = self.shell.files_loaded();
        let exporting = self.preview == Some(LookPreview::Exporting);
        self.set_button_enabled(cx, ids!(export_button), loaded && !exporting);
        let inspector_toggle = self.shell.can_toggle(Panel::Inspector);
        self.set_button_enabled(cx, ids!(toggle_inspector), inspector_toggle);
        for id in [ids!(step_back), ids!(play_pause), ids!(step_forward)] {
            self.set_button_enabled(cx, id, loaded);
        }
        self.set_button_enabled(cx, ids!(record_button), loaded && !exporting);
        self.ui.widget(cx, ids!(timeline)).set_disabled(cx, !loaded);
        self.ui.widget(cx, ids!(aspect)).set_disabled(cx, !loaded);
        let fold_hint = loaded && self.shell.auto_folded(Panel::Inspector);
        self.set_visible(cx, ids!(fold_hint), fold_hint);
        self.ui.redraw(cx);
    }

    /// Show one state of the job (`None` is a fresh start).
    fn show_state(&mut self, cx: &mut Cx, state: Option<LookPreview>) {
        use LookPreview::*;
        self.preview = state;
        let left = state.is_some();
        let right = !matches!(state, None | Some(OneCamera));
        let calibrating = state == Some(Calibrating);
        let failed = state == Some(CalibrationFailed);
        let calibrated = matches!(state, Some(Ready | Exporting));
        let exporting = state == Some(Exporting);
        self.shell.set_files_loaded(calibrated);

        let project = match (left, right) {
            (false, _) => "No videos yet".to_string(),
            (true, false) => "GX010120.MP4".to_string(),
            (true, true) => middle_ellipsis("GX010120.MP4 + GX010092.MP4", PROJECT_NAME_CHARS),
        };
        self.set_label(cx, ids!(project_name), &project);

        // Setup panel: the camera pair. The title bar names the first
        // files; the rows say how much each camera has.
        for (on, idle, lit) in [
            (ids!(left_badge_on), ids!(left_badge), left),
            (ids!(lane_left_badge_on), ids!(lane_left_badge), left),
            (ids!(right_badge_on), ids!(right_badge), right),
            (ids!(lane_right_badge_on), ids!(lane_right_badge), right),
        ] {
            self.set_visible(cx, on, lit);
            self.set_visible(cx, idle, !lit);
        }
        self.set_visible(cx, ids!(link_idle), !right);
        self.set_visible(cx, ids!(link_on), right);
        self.set_visible(cx, ids!(add_left), !left);
        self.set_visible(cx, ids!(change_left), left);
        self.set_visible(cx, ids!(add_right), !right);
        self.set_visible(cx, ids!(change_right), right);
        // An empty camera says nothing: its Add button is the message.
        let files = |has: bool, text| if has { text } else { "" };
        self.set_label(cx, ids!(left_files), files(left, "21 files · 1:45:00"));
        self.set_label(cx, ids!(right_files), files(right, "22 files · 1:45:00"));

        // Setup panel: calibration.
        let (status, detail) = if calibrated {
            ("Calibrated", "match.json · 412 matched points")
        } else if calibrating {
            ("Calibrating…", "Step 3 of 6 · matching pitch markings")
        } else if failed {
            (
                "Calibration failed",
                "Too few pitch markings in view of both cameras.",
            )
        } else if right {
            ("Not calibrated", "Ready to calibrate")
        } else {
            ("Not calibrated", "Add both cameras first")
        };
        self.set_label(cx, ids!(calibration_status), status);
        self.set_label(cx, ids!(calibration_detail), detail);
        let idle = !calibrating && !calibrated && !failed;
        self.set_visible(cx, ids!(cal_dot_idle), idle);
        self.set_visible(cx, ids!(cal_dot_busy), calibrating);
        self.set_visible(cx, ids!(cal_dot_ok), calibrated);
        self.set_visible(cx, ids!(cal_dot_error), failed);
        self.set_visible(cx, ids!(auto_calibrate), !calibrated);
        self.set_button_enabled(cx, ids!(auto_calibrate), right && !calibrating);
        self.set_visible(cx, ids!(recalibrate), calibrated);

        // Viewer: the next step, or the preview.
        let (title, body, primary, secondary) = match state {
            None => (
                "Add your two camera videos",
                "Start with the left camera. All the GoPro files from one camera go in together.",
                "Add left camera…",
                "Recent files…",
            ),
            Some(OneCamera) => (
                "Now add the right camera",
                "Choose all the GoPro files from the right-hand camera.",
                "Add right camera…",
                "Recent files…",
            ),
            Some(Cameras) => (
                "Line up the two cameras",
                "Auto-calibrate matches the pitch markings both cameras can see. It takes about a minute.",
                "Auto-calibrate",
                "Load calibration file…",
            ),
            Some(Calibrating) => (
                "Calibrating…",
                "Matching the pitch markings both cameras can see.",
                "",
                "",
            ),
            Some(CalibrationFailed) => (
                "Calibration didn't work",
                "Too few pitch markings were in view of both cameras. Try again at a moment when more of the pitch is visible, or load a calibration file.",
                "Try again",
                "Load calibration file…",
            ),
            Some(Ready | Exporting) => ("", "", "", ""),
        };
        self.set_label(cx, ids!(next_title), title);
        self.set_label(cx, ids!(next_body), body);
        self.set_visible(cx, ids!(next_actions), !primary.is_empty());
        self.ui.button(cx, ids!(next_primary)).set_text(cx, primary);
        self.ui
            .button(cx, ids!(next_secondary))
            .set_text(cx, secondary);
        self.set_visible(cx, ids!(empty_state), !calibrated);
        self.set_visible(cx, ids!(sample_frame), calibrated);
        if let Some(mut panorama) = self
            .ui
            .widget(cx, ids!(panorama))
            .borrow_mut::<RecoPanorama>()
        {
            panorama.set_lit(cx, left, right, calibrating);
        }
        let step2 = if calibrated {
            Step::Done
        } else if right {
            Step::Current
        } else {
            Step::Todo
        };
        let step3 = if calibrated {
            Step::Current
        } else {
            Step::Todo
        };
        self.set_step(
            cx,
            [ids!(step1_todo), ids!(step1_current), ids!(step1_done)],
            if right { Step::Done } else { Step::Current },
        );
        self.set_step(
            cx,
            [ids!(step2_todo), ids!(step2_current), ids!(step2_done)],
            step2,
        );
        self.set_step(
            cx,
            [ids!(step3_todo), ids!(step3_current), ids!(step3_done)],
            step3,
        );

        // Long tasks: calibration shows its progress in the next-step
        // column, an export on a card over the picture.
        self.set_visible(cx, ids!(calibrate_progress), calibrating);
        self.ui
            .progress_bar(cx, ids!(calibrate_bar))
            .set_value(cx, 0.45);
        self.set_label(cx, ids!(calibrate_eta), "About 1 min left");
        self.set_visible(cx, ids!(export_card), exporting);
        self.set_label(cx, ids!(export_title), "Exporting match_stitched.mp4");
        self.set_label(cx, ids!(export_detail), "Frame 48,210 of 141,000 · 62 fps");
        self.ui
            .progress_bar(cx, ids!(export_bar))
            .set_value(cx, 0.34);
        self.set_label(cx, ids!(export_eta), "About 25 min left");

        // Time panel: the time, a status line, and each camera's files.
        let length = if left { SAMPLE_LENGTH } else { 0.0 };
        let playhead = if calibrated { SAMPLE_PLAYHEAD } else { 0.0 };
        self.set_visible(cx, ids!(time_display), calibrated);
        self.set_label(cx, ids!(time_current), &time_ruler::clock(playhead));
        self.set_label(cx, ids!(time_total), &time_ruler::clock(length));
        let status = match state {
            Some(Calibrating) => "Calibrating · step 3 of 6",
            Some(CalibrationFailed) => "Calibration failed",
            Some(Ready) => "59.9 fps",
            Some(Exporting) => "Exporting · 34%",
            _ => "",
        };
        self.set_label(cx, ids!(status_text), status);
        let lane = |has: bool, count| {
            if has {
                sample_files(count, SAMPLE_LENGTH)
            } else {
                Vec::new()
            }
        };
        if let Some(mut ruler) = self
            .ui
            .widget(cx, ids!(timeline))
            .borrow_mut::<RecoTimeRuler>()
        {
            ruler.set_timeline(cx, length, playhead, vec![lane(left, 21), lane(right, 22)]);
        }
    }

    /// Show one of a step's three badges.
    fn set_step(&self, cx: &mut Cx, badges: [&[LiveId]; 3], step: Step) {
        let [todo, current, done] = badges;
        self.set_visible(cx, todo, step == Step::Todo);
        self.set_visible(cx, current, step == Step::Current);
        self.set_visible(cx, done, step == Step::Done);
    }

    fn set_label(&self, cx: &mut Cx, id: &[LiveId], text: &str) {
        self.ui.label(cx, id).set_text(cx, text);
    }

    fn set_visible(&self, cx: &mut Cx, id: &[LiveId], visible: bool) {
        self.ui.widget(cx, id).set_visible(cx, visible);
    }

    /// Enable or disable a button. Makepad keeps these apart: `set_enabled`
    /// gates input, `set_disabled` drives the dimmed look.
    fn set_button_enabled(&self, cx: &mut Cx, id: &[LiveId], enabled: bool) {
        self.ui.button(cx, id).set_enabled(cx, enabled);
        self.ui.widget(cx, id).set_disabled(cx, !enabled);
    }

    fn toggle(&mut self, cx: &mut Cx, panel: Panel) {
        if self.shell.toggle(panel) {
            self.apply_shell(cx);
        }
    }

    /// Show the saved choices in their widgets.
    fn apply_settings(&mut self, cx: &mut Cx) {
        let aspect = self.settings.aspect();
        self.ui
            .drop_down(cx, ids!(aspect))
            .set_selected_item(cx, aspect.index());
        if let Some(mut preview) = self
            .ui
            .widget(cx, ids!(preview))
            .borrow_mut::<ui::preview::RecoPreview>()
        {
            preview.set_aspect(cx, aspect);
        }
    }

    /// Save the settings; a failure is logged, not shown (nothing is lost
    /// but the choice for the next run).
    fn save_settings(&self) {
        if let Err(e) = settings::save(&self.settings) {
            error!("couldn't save the settings: {e}");
        }
    }

    fn toggle_timeline(&mut self, cx: &mut Cx) {
        self.timeline_folded = !self.timeline_folded;
        self.apply_shell(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        match Args::parse(std::env::args().skip(1)) {
            Ok(args) => self.args = args,
            Err(err) => log!("ignoring command line: {err}"),
        }
        self.settings = settings::load();
        self.apply_settings(cx);
        self.ui
            .menu_button(cx, ids!(app_menu))
            .set_rows(app_menu_rows());
        self.ui
            .menu_button(cx, ids!(recent_menu))
            .set_rows(recent_rows());
        if let Some((w, h)) = self.args.window_size {
            self.ui
                .window(cx, ids!(main_window))
                .resize(cx, dvec2(w, h));
        }
        self.show_state(cx, self.args.look_preview);
        if let (Some(files), None) = (self.args.files.clone(), self.args.look_preview) {
            self.start_live(cx, files);
        }
        self.apply_shell(cx);
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(toggle_media)).clicked(actions) {
            self.toggle(cx, Panel::Media);
        }
        if self.ui.button(cx, ids!(toggle_inspector)).clicked(actions)
            || self.ui.button(cx, ids!(fold_hint_button)).clicked(actions)
        {
            self.toggle(cx, Panel::Inspector);
        }
        if self.ui.button(cx, ids!(toggle_timeline)).clicked(actions) {
            self.toggle_timeline(cx);
        }
        // The menus' commands arrive with Modules 3 and 7.
        let app_menu = self.ui.menu_button(cx, ids!(app_menu)).menu_owner();
        if let Some(picked) = menu_picked(actions, app_menu) {
            log!("app menu: {picked} (not wired in Module 0)");
        }
        self.preview_actions(cx, actions);
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::theme_mod(vm);
        crate::theme::script_mod(vm);
        makepad_widgets::widgets_mod(vm);
        crate::ui::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        match event {
            Event::KeyDown(_) => self.pointer_input = false,
            Event::MouseDown(_) | Event::MouseUp(_) => self.pointer_input = true,
            _ => {}
        }
        match event {
            // Only a width change can fold or unfold a panel; moves and
            // height changes skip the work.
            Event::WindowGeomChange(ge) if ge.old_geom.inner_size.x != ge.new_geom.inner_size.x => {
                self.shell.fit_width(ge.new_geom.inner_size.x);
                self.apply_shell(cx);
            }
            Event::MacosMenuCommand(item) if *item == live_id!(toggle_media_menu) => {
                self.toggle(cx, Panel::Media);
            }
            Event::MacosMenuCommand(item) if *item == live_id!(toggle_inspector_menu) => {
                self.toggle(cx, Panel::Inspector);
            }
            Event::MacosMenuCommand(item) if *item == live_id!(toggle_timeline_menu) => {
                self.toggle_timeline(cx);
            }
            Event::Signal => self.drain_preview(cx),
            _ => {}
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
