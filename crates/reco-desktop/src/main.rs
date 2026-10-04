//! Reco Desktop: the Makepad 2 desktop app for Reco.
//!
//! See `DESIGN.md` for the plan, rules and module order.

pub use makepad_widgets;
use makepad_widgets::*;

mod cli;
mod shell_state;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Reco"
                window.inner_size: vec2(1280, 820)
                body +: {
                    Label{text: "Reco"}
                }
            }
        }
    }
}

/// The application: the widget tree.
#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::theme_mod(vm);
        makepad_widgets::widgets_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
