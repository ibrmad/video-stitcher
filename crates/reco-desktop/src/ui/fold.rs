//! A fold (a section, an Advanced tier, a camera's files) that keeps a
//! closed body out of reach and out of sight.
//!
//! - Pointer events: Makepad's FoldHeader draws a closed body scrolled out
//!   of sight inside a zero-height clip but still hands it every event, so
//!   its hidden rows took presses and drags over the rows above: Seam blend
//!   and Field of view didn't take a drag while the Advanced tier under
//!   them was closed (FRICTION.md). Closed, this fold passes pointer events
//!   only inside what it shows (its header).
//! - First draw: FoldHeader draws a body it has never measured whole, to
//!   measure it, whatever its state, and a closed fold first drawn in view
//!   showed open until the next redraw (the owner saw the export sheet's
//!   Advanced tier open until a scroll closed it). A closed fold's first
//!   draw is clipped to its header, and it draws again, measured and
//!   closed, on the next frame.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoFoldBase = #(RecoFold::register_widget(vm))

    // Makepad's FoldHeader, defaults and all, on the wrapper.
    mod.widgets.RecoFold = set_type_default() do mod.widgets.RecoFoldBase{
        ..mod.widgets.FoldHeader
        header_height: theme.reco_row
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
    /// The header's height: a closed fold's first draw shows only this.
    #[live]
    header_height: f64,
    /// FoldHeader has drawn the body once, and so knows its height.
    #[rust]
    drawn: bool,
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
        let closed = !self.fold.is_open(cx) && self.fold.opened() <= 0.0;
        if self.drawn || !closed {
            self.drawn = true;
            return self.fold.draw_walk(cx, scope, walk);
        }
        cx.begin_turtle(
            Walk {
                height: Size::Fixed(self.header_height),
                ..walk
            },
            Layout {
                clip_y: true,
                ..Layout::default()
            },
        );
        let inner = Walk {
            height: Size::fit(),
            margin: Inset::default(),
            ..walk
        };
        let step = self.fold.draw_walk(cx, scope, inner);
        cx.end_turtle();
        self.drawn = true;
        self.fold.redraw(cx);
        step
    }
}
