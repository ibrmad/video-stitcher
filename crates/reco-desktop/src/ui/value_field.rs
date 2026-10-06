//! A slider's number, typed into (plans/2026-10-06-value-fields.md): a
//! quiet box with the digits on the right. A click selects the digits;
//! Return or a click away sends what was typed, Escape puts the value back,
//! ↑/↓ send steps. The App reads and applies them (`App::slider_input`), so
//! the field knows nothing of units or ranges.

use makepad_widgets::makepad_draw::text::selection::Selection;
use makepad_widgets::*;

use super::pointer_at;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoValueFieldBase = #(RecoValueField::register_widget(vm))

    // No border: a shade lighter under the pointer, a green ring while
    // typing. Makepad's flat field, defaults and all, on the wrapper.
    mod.widgets.RecoValueField = set_type_default() do mod.widgets.RecoValueFieldBase{
        ..mod.widgets.TextInputFlat
        // Fit, held to one width: the text is then one item the layout can
        // align (a fixed width lays it out from the left).
        width: Fit{min: theme.reco_value_field max: theme.reco_value_field}
        height: theme.reco_value_field_height margin: 0
        padding: Inset{left: theme.reco_gap_s right: theme.reco_gap_s}
        // The digits on the right, as numbers sit.
        align: Align{x: 1.0 y: 0.5}
        empty_text: ""
        draw_bg +: {
            border_size: uniform(theme.reco_focus_width)
            color: theme.reco_band
            color_hover: theme.reco_widget
            color_focus: theme.reco_band
            color_down: theme.reco_widget
            color_empty: theme.reco_band
            color_disabled: theme.reco_band
            border_color: theme.reco_transparent
            border_color_hover: theme.reco_transparent
            border_color_focus: theme.reco_accent
            border_color_down: theme.reco_transparent
            border_color_empty: theme.reco_transparent
            border_color_disabled: theme.reco_transparent
        }
        draw_text +: {
            color: theme.reco_text
            color_hover: theme.reco_text
            color_focus: theme.reco_text_strong
            color_down: theme.reco_text
            color_empty: theme.reco_text
            color_disabled: theme.reco_text_subdued
            text_style: theme.font_regular{font_size: theme.reco_font_body}
        }
    }
}

/// What a value field asks of the App.
#[derive(Clone, Debug, Default)]
pub enum RecoValueFieldAction {
    #[default]
    None,
    /// Return, or a click away from a field typed in: the text.
    Typed(String),
    /// ↑/↓ while typing: steps of the last digit shown (⇧: ten).
    Step(i32),
}

#[derive(Script, ScriptHook, Widget)]
pub struct RecoValueField {
    #[source]
    source: ScriptObjectRef,
    #[deref]
    input: TextInput,
    /// The value as the App last set it: what Escape puts back.
    #[rust]
    shown: String,
    /// From the click (or Tab) that gave the field the keyboard until
    /// Return, Escape or a click away.
    #[rust]
    typing: bool,
    /// Held while typing, so Escape is the field's: the export sheet would
    /// close on it.
    #[rust]
    cancel: Option<CancelScope>,
    /// Locked with its slider: no pointer, no typing.
    #[rust]
    locked: bool,
}

impl RecoValueField {
    /// Typing ends (Return, Escape, a click away), and the selection with
    /// it: Makepad keeps a selection that a press lands in and drops it as
    /// the press ends, so the next click would leave the caret where it
    /// landed instead of the digits selected.
    fn stop_typing(&mut self, cx: &mut Cx) {
        self.typing = false;
        self.cancel = None;
        self.input.set_selection(cx, Selection::default());
    }
}

impl Widget for RecoValueField {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        if let (true, Event::KeyDown(key)) = (self.typing, event) {
            let steps = match key.key_code {
                KeyCode::ArrowUp => 1,
                KeyCode::ArrowDown => -1,
                _ => 0,
            };
            if steps != 0 {
                let steps = if key.modifiers.shift {
                    steps * 10
                } else {
                    steps
                };
                // The step starts from the text in the field, and the App's
                // answer replaces it: what's typed is the value shown now.
                self.shown = Widget::text(&self.input);
                cx.widget_action(uid, RecoValueFieldAction::Step(steps));
                return;
            }
            if key.key_code == KeyCode::Escape
                && self
                    .cancel
                    .as_ref()
                    .is_some_and(|scope| cx.owns_cancel(scope))
            {
                self.stop_typing(cx);
                let shown = self.shown.clone();
                Widget::set_text(&mut self.input, cx, &shown);
                cx.set_key_focus(Area::Empty);
                return;
            }
        }
        if self.locked
            && pointer_at(event).is_some_and(|at| self.input.area().rect(cx).contains(at))
        {
            return;
        }
        let actions = cx.capture_actions(|cx| self.input.handle_event(cx, event, scope));
        for action in actions.iter() {
            match action.as_widget_action().cast() {
                TextInputAction::KeyFocus if !self.locked => {
                    self.typing = true;
                    self.shown = Widget::text(&self.input);
                    self.cancel = Some(self.begin_cancel_scope_for(cx, CancelScopeKind::Escape));
                    self.input.select_all(cx);
                }
                TextInputAction::Returned(text, _) if self.typing => {
                    self.stop_typing(cx);
                    cx.widget_action(uid, RecoValueFieldAction::Typed(text));
                }
                TextInputAction::KeyFocusLost if self.typing => {
                    self.stop_typing(cx);
                    let text = Widget::text(&self.input);
                    if text != self.shown {
                        cx.widget_action(uid, RecoValueFieldAction::Typed(text));
                    }
                }
                _ => {}
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.input.draw_walk(cx, scope, walk)
    }

    fn text(&self) -> String {
        Widget::text(&self.input)
    }

    /// The App's value. A field being typed in keeps what was typed; one
    /// focused but untouched (or just stepped) follows, its digits selected.
    fn set_text(&mut self, cx: &mut Cx, text: &str) {
        let untouched = Widget::text(&self.input) == self.shown;
        self.shown = text.to_string();
        if !self.typing || untouched {
            Widget::set_text(&mut self.input, cx, text);
            if self.typing {
                self.input.select_all(cx);
            }
        }
    }

    fn set_disabled(&mut self, cx: &mut Cx, disabled: bool) {
        self.locked = disabled;
        self.input.set_is_read_only(cx, disabled);
        Widget::set_disabled(&mut self.input, cx, disabled);
        if disabled && self.typing {
            self.stop_typing(cx);
            let shown = self.shown.clone();
            Widget::set_text(&mut self.input, cx, &shown);
            cx.set_key_focus(Area::Empty);
        }
    }

    fn disabled(&self, _cx: &Cx) -> bool {
        self.locked
    }
}
