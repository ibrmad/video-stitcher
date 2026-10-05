//! A fold (a section, an Advanced tier, a camera's files) that gives a
//! closed body no pointer events. Makepad's FoldHeader draws a closed body
//! scrolled out of sight inside a zero-height clip but still hands it every
//! event, so its hidden rows take presses and drags over the rows above:
//! Seam blend and Field of view didn't take a drag while the Advanced tier
//! under them was closed (FRICTION.md). Closed, this fold passes pointer
//! events only inside what it shows (its header).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoFoldBase = #(RecoFold::register_widget(vm))

    // Makepad's FoldHeader, defaults and all, on the wrapper.
    mod.widgets.RecoFold = set_type_default() do mod.widgets.RecoFoldBase{
        ..mod.widgets.FoldHeader
    }
}

/// Where a pointer event happened, if it is one.
fn pointer_at(event: &Event) -> Option<DVec2> {
    match event {
        Event::MouseDown(e) => Some(e.abs),
        Event::MouseMove(e) => Some(e.abs),
        Event::MouseUp(e) => Some(e.abs),
        Event::Scroll(e) => Some(e.abs),
        Event::TouchUpdate(e) => e.touches.first().map(|t| t.abs),
        _ => None,
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RecoFold {
    #[source]
    source: ScriptObjectRef,
    #[deref]
    fold: FoldHeader,
}

impl Widget for RecoFold {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let closed = !self.fold.is_open(cx) && self.fold.opened() <= 0.0;
        if closed {
            if let Some(at) = pointer_at(event) {
                if !self.fold.area().rect(cx).contains(at) {
                    return;
                }
            }
        }
        self.fold.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.fold.draw_walk(cx, scope, walk)
    }
}
