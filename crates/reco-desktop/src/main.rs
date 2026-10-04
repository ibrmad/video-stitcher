//! Reco Desktop: the Makepad 2 desktop app for Reco.
//!
//! Module 0 (see `DESIGN.md`): the window shell and the look. Nothing is
//! wired to the engine yet; `--look-preview` shows the loaded-state layout
//! with sample content for design review.

pub use makepad_widgets;
use makepad_widgets::*;

mod cli;
mod shell_state;
mod theme;
mod ui;

use cli::Args;
use shell_state::{Panel, ShellState};

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
                    main := MenuItem.Main{items: [@app_menu, @view_menu]}
                    app_menu := MenuItem.Sub{name: "Reco" items: [@quit]}
                    quit := MenuItem.Item{name: "Quit Reco" key: KeyCode.KeyQ enabled: true}
                    view_menu := MenuItem.Sub{name: "View" items: [@toggle_media_menu, @toggle_inspector_menu]}
                    toggle_media_menu := MenuItem.Item{name: "Media Panel" key: KeyCode.Key1 enabled: true}
                    toggle_inspector_menu := MenuItem.Item{name: "Inspector" key: KeyCode.Key2 enabled: true}
                }
                body +: {
                    flow: Overlay
                    shell := RecoShell{}
                    tip_layer := TipLayer{}
                }
            }
        }
    }
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
}

impl App {
    /// Push the shell state into the widgets: panel folds, enabled
    /// controls, empty state versus sample frame.
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

        let loaded = self.shell.files_loaded();
        self.set_button_enabled(cx, ids!(export_button), loaded);
        let inspector_toggle = self.shell.can_toggle(Panel::Inspector);
        self.set_button_enabled(cx, ids!(toggle_inspector), inspector_toggle);
        for id in [
            ids!(step_back),
            ids!(play_pause),
            ids!(step_forward),
            ids!(record_button),
        ] {
            self.set_button_enabled(cx, id, loaded);
        }
        self.ui.widget(cx, ids!(timeline)).set_disabled(cx, !loaded);
        self.ui.widget(cx, ids!(aspect)).set_disabled(cx, !loaded);
        self.ui.view(cx, ids!(empty_state)).set_visible(cx, !loaded);
        self.ui.view(cx, ids!(sample_frame)).set_visible(cx, loaded);
        self.ui.redraw(cx);
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

    /// `--look-preview`: the loaded-state layout with sample texts.
    fn show_look_preview(&mut self, cx: &mut Cx) {
        self.shell.set_files_loaded(true);
        self.ui
            .label(cx, ids!(project_name))
            .set_text(cx, "GX010120.MP4 + GX010092.MP4");
        self.ui.label(cx, ids!(time_current)).set_text(cx, "12:34");
        self.ui.label(cx, ids!(time_total)).set_text(cx, "1:45:00");
        self.ui
            .label(cx, ids!(status_text))
            .set_text(cx, "Ready - 59.9 fps");
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        match Args::parse(std::env::args().skip(1)) {
            Ok(args) => self.args = args,
            Err(err) => log!("ignoring command line: {err}"),
        }
        self.ui
            .label(cx, ids!(version_text))
            .set_text(cx, concat!("v", env!("CARGO_PKG_VERSION")));
        if let Some((w, h)) = self.args.window_size {
            self.ui
                .window(cx, ids!(main_window))
                .resize(cx, dvec2(w, h));
        }
        if self.args.look_preview {
            self.show_look_preview(cx);
        }
        self.apply_shell(cx);
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(toggle_media)).clicked(actions) {
            self.toggle(cx, Panel::Media);
        }
        if self.ui.button(cx, ids!(toggle_inspector)).clicked(actions) {
            self.toggle(cx, Panel::Inspector);
        }
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
            Event::WindowGeomChange(ge) => {
                self.shell.fit_width(ge.new_geom.inner_size.x);
                self.apply_shell(cx);
            }
            Event::MacosMenuCommand(item) if *item == live_id!(toggle_media_menu) => {
                self.toggle(cx, Panel::Media);
            }
            Event::MacosMenuCommand(item) if *item == live_id!(toggle_inspector_menu) => {
                self.toggle(cx, Panel::Inspector);
            }
            _ => {}
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
