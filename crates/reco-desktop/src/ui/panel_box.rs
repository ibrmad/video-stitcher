//! A panel's frame while it slides. Its content can be held at one width, so
//! nothing re-wraps as the room around it narrows: anchored to its end, Setup
//! slides out to the left; Adjust, anchored to its start, slides out to the
//! right. Its own height can be set (the lanes, cut from the bottom), and it
//! notes its natural height for the way back.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoPanelBoxBase = #(RecoPanelBox::register_widget(vm))

    // Makepad's View, defaults and all, on the wrapper.
    mod.widgets.RecoPanelBox = set_type_default() do mod.widgets.RecoPanelBoxBase{
        ..mod.widgets.View
        width: Fill height: Fill
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct RecoPanelBox {
    #[source]
    source: ScriptObjectRef,
    #[deref]
    view: View,
    /// Held at its end (right) edge while pinned: the content slides out
    /// to the left as the room narrows.
    #[live]
    anchor_end: bool,
    /// The content's width while it slides.
    #[rust]
    pinned_width: Option<f64>,
    /// The box's height while it slides.
    #[rust]
    height: Option<f64>,
    /// The height it last had on its own (0 until drawn so).
    #[rust]
    natural_height: f64,
}

impl RecoPanelBox {
    /// Hold the content at `width` (or let it fill again).
    pub fn set_pinned_width(&mut self, cx: &mut Cx, width: Option<f64>) {
        self.pinned_width = width;
        self.view.redraw(cx);
    }

    /// Set the box's own height (or let it fit again).
    pub fn set_height(&mut self, cx: &mut Cx, height: Option<f64>) {
        self.height = height;
        self.view.redraw(cx);
    }

    /// The height it last had on its own.
    pub fn natural_height(&self) -> f64 {
        self.natural_height
    }
}

impl Widget for RecoPanelBox {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let mut walk = walk;
        if let Some(width) = self.pinned_width {
            if self.anchor_end {
                // The room it has, less its own width: negative while the
                // room is the narrower, which keeps its end on the room's.
                walk.margin.left = cx.turtle().inner_rect().size.x - width;
            }
            walk.width = Size::Fixed(width);
        }
        if let Some(height) = self.height {
            walk.height = Size::Fixed(height);
        }
        let step = self.view.draw_walk(cx, scope, walk);
        if self.height.is_none() && step.is_done() {
            self.natural_height = self.view.area().rect(cx).size.y;
        }
        step
    }
}
