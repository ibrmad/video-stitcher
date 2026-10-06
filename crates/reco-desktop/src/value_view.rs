//! The sliders' value fields in the App (plans/2026-10-06-value-fields.md):
//! a typed or stepped value goes through its slider, which keeps it in its
//! range, then on the path a drag takes.

use makepad_widgets::*;

use crate::ui::value_field::RecoValueFieldAction;
use crate::value_text::Reading;
use crate::App;

/// What a value field asked for this time, with its text then.
pub(crate) enum FieldInput {
    Typed(String),
    Step(i32, String),
}

impl FieldInput {
    /// The value asked for, read as `reading` from a field showing
    /// `current`; `None` asks for nothing (the text shown, or text that
    /// doesn't read). A step starts from the number in the field, typed or
    /// not.
    pub(crate) fn value(&self, reading: Reading, current: f64) -> Option<f64> {
        match self {
            Self::Typed(text) => reading.typed(text, current),
            Self::Step(steps, text) => {
                let from = reading.read(text).unwrap_or(current);
                Some(reading.stepped(from, *steps))
            }
        }
    }
}

impl App {
    /// What a value field asked for, if anything.
    pub(crate) fn field_input(
        &self,
        cx: &mut Cx,
        actions: &Actions,
        field: &[LiveId],
    ) -> Option<FieldInput> {
        let widget = self.ui.widget(cx, field);
        match actions.find_widget_action(widget.widget_uid())?.cast() {
            RecoValueFieldAction::Typed(text) => Some(FieldInput::Typed(text)),
            RecoValueFieldAction::Step(steps) => Some(FieldInput::Step(steps, widget.text())),
            RecoValueFieldAction::None => None,
        }
    }

    /// A slider's new value, from a drag or from its field (typed or
    /// stepped, then through the slider, which keeps it in range). The field
    /// shows it, or puts back the value it had when it asked for nothing.
    pub(crate) fn slider_input(
        &mut self,
        cx: &mut Cx,
        actions: &Actions,
        slider: &[LiveId],
        field: &[LiveId],
        reading: Reading,
    ) -> Option<f64> {
        let knob = self.ui.slider(cx, slider);
        if let Some(value) = knob.slided(actions).or(knob.end_slide(actions)) {
            self.set_label(cx, field, &reading.text(value));
            return Some(value);
        }
        let input = self.field_input(cx, actions, field)?;
        let current = knob.value()?;
        let value = input.value(reading, current).map(|wanted| {
            knob.set_value(cx, wanted);
            knob.value().unwrap_or(wanted)
        });
        self.set_label(cx, field, &reading.text(value.unwrap_or(current)));
        value
    }
}
